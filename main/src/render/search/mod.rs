use std::{
    cell::RefCell, cmp::Ordering, fs::File, io::BufReader, path::Path, path::PathBuf, rc::Rc,
};

use cef::{v8, CefV8Context, CefV8Propertyattribute, CefV8Value};
use image::{imageops::FilterType, DynamicImage, ImageOutputFormat};

mod assets;

mod config;
use config::{ManualTarget, SearchConfig};

mod appx;
use appx::{AppxIndex, AppxTarget};

mod start_menu;
use start_menu::{StartMenuIndex, StartMenuTarget};

mod steam;
use steam::{SteamIndex, SteamTarget};

pub type MatchScore = (usize, usize, u64);

pub trait SearchProvider<K> {
    fn keys(&self, target: &K) -> Vec<String>;
    fn launch(&self, target: &K) -> Box<dyn Fn()>;
    fn details(&self, target: &K) -> String;
    fn display_icon(&self, target: &K) -> Option<DynamicImage>;
}

/// Contains information required to find a value in the index.
/// That means a list of strings
pub struct Key {
    count: u64,
    keys: Vec<String>,
}

impl Key {
    pub fn do_match(&self, query: &str) -> Option<(&str, usize, MatchScore)> {
        for key in &self.keys {
            let lower = key.to_lowercase();

            if let Some(byte) = lower.find(&query) {
                let index = lower
                    .char_indices()
                    .position(|(idx, _)| idx == byte)
                    .unwrap();

                let chars: Vec<_> = key.chars().take(index).collect();
                let word_index = chars.iter().filter(|&&c| c == ' ').count();
                let char_index = chars.iter().rev().position(|&x| x == ' ').unwrap_or(index);

                let score = (char_index, word_index, u64::MAX - self.count);

                return Some((key, index, score));
            }
        }

        None
    }
}

/// Contains information about a value in the index.
/// That means a display name & icon for rendering, and a function to launch the target
pub struct Target {
    details: String,
    display_icon: Option<DynamicImage>,
    launch: Box<dyn Fn()>,
}

impl Target {
    fn to_cef(self, ctx: &CefV8Context) -> CefV8Value {
        let object = CefV8Value::create_object(None, None).unwrap();

        let key = "details";
        let value = &self.details;
        object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

        let key = "display_icon";
        let value: CefV8Value = match self.display_icon {
            None => ().into(),
            Some(image) => {
                let mut data = vec![];
                image.write_to(&mut data, ImageOutputFormat::Png).unwrap();
                assets::create_asset(ctx, "image/png", &mut data)
            }
        };
        object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

        let key = "launch";
        let launch = self.launch;
        let value = v8::v8_function0(key, move || launch());
        object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

        object
    }
}

pub struct Search<T = Target> {
    entries: Vec<(Key, T)>,
}

pub struct Match<'a, T> {
    key: &'a str,
    index: usize,
    score: MatchScore,
    object: &'a T,
}

impl Search {
    pub fn load<P: AsRef<Path>>(state_path: P) -> Search {
        let index = IndexFile::open(state_path.as_ref().to_owned());
        let index = Rc::new(RefCell::new(index));

        let mut idx = Search { entries: vec![] };

        let config = SearchConfig::load();
        idx.include_helper(&index, &config, config.index());

        if config.index_appx {
            let appx = AppxIndex::new();
            idx.include_helper(&index, &appx, appx.index());
        }

        if let Some(root) = &config.index_steam {
            let steam = SteamIndex::new(root);
            idx.include_helper(&index, &steam, steam.index());
        }

        if config.index_start_menu {
            let start_menu = StartMenuIndex::new();
            idx.include_helper(&index, &start_menu, start_menu.index());
        }

        index.borrow().save();

        idx
    }

    fn include_helper<K, P>(&mut self, index: &Rc<RefCell<IndexFile>>, provider: &P, new: Vec<K>)
    where
        K: 'static + Clone + Eq,
        P: SearchProvider<K>,
        Index: IndexType<K>,
    {
        let mut index_mut = index.borrow_mut();
        let index_mut = &mut index_mut.index;

        for target in new {
            if index_mut.get_save().iter().any(|x| x.target == target) {
                continue;
            }

            index_mut.add_entry(target, provider);
        }

        let entries = index_mut
            .get_save()
            .iter()
            .map(|e| e.prepare(index.clone(), provider));

        self.entries.extend(entries);
    }

    pub fn into_cef(self, ctx: &CefV8Context) -> Search<CefV8Value> {
        let entries: Vec<_> = self
            .entries
            .into_iter()
            .map(|(entry, target)| (entry, target.to_cef(ctx)))
            .collect();

        Search { entries }
    }
}

impl<T> Search<T> {
    pub fn search(&self, query: &str) -> Vec<Match<T>> {
        let query = query.to_lowercase();

        let mut matches: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(entry, object)| {
                entry.do_match(&query).map(|(key, index, score)| Match {
                    key,
                    index,
                    score,
                    object,
                })
            })
            .collect();

        matches.sort_unstable_by(|a, b| {
            match Ord::cmp(&a.score, &b.score) {
                Ordering::Equal => {}
                o => return o,
            };

            Ord::cmp(a.key, b.key)
        });

        matches
    }
}

impl Search<CefV8Value> {
    pub fn search_cef(&self, query: &str) -> CefV8Value {
        let matches = self.search(query);

        let limit = 7.min(matches.len());
        let display = matches.into_iter().take(limit);

        v8::v8_array(display.map(|m| {
            let object = CefV8Value::create_object(None, None).unwrap();

            let key = "key";
            let value = m.key;
            object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

            let key = "start";
            let value = m.index;
            object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

            let key = "end";
            let value = m.index + query.len();
            object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

            let key = "target";
            let value = m.object.clone();
            object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

            object
        }))
    }
}

#[derive(Serialize, Deserialize, Clone)]
struct IndexEntry<K> {
    target: K,
    icon: Option<String>,
    counter: u64,
}

impl<K> IndexEntry<K>
where
    K: 'static + Clone + Eq,
{
    fn prepare<P>(&self, index: Rc<RefCell<IndexFile>>, provider: &P) -> (Key, Target)
    where
        P: SearchProvider<K>,
        Index: IndexType<K>,
    {
        let keys = provider.keys(&self.target);

        let key = Key {
            count: self.counter,
            keys,
        };

        let details = provider.details(&self.target);

        let launch = provider.launch(&self.target);

        let target = self.target.clone();
        let launch = Box::new(move || {
            launch();
            let mut index = index.borrow_mut();
            let mut entry = index.index.get_entry(&target);
            entry.counter += 1;
            index.save();
        });

        let display_icon = self.icon.as_ref().and_then(|id| {
            crate::attempt(
                || format!("open cached icon {}", id),
                || Ok(image::open(id)?),
            )
        });

        let target = Target {
            details,
            display_icon,
            launch,
        };

        (key, target)
    }
}

#[derive(Serialize, Deserialize, Default)]
struct Index {
    next_icon: u64,
    config: Vec<IndexEntry<ManualTarget>>,
    appx: Vec<IndexEntry<AppxTarget>>,
    start_menu: Vec<IndexEntry<StartMenuTarget>>,
    steam: Vec<IndexEntry<SteamTarget>>,
}

impl Index {
    fn add_entry<K, P>(&mut self, target: K, provider: &P)
    where
        K: Clone + Eq,
        P: SearchProvider<K>,
        Index: IndexType<K>,
    {
        let display_icon = provider.display_icon(&target).map(|icon| {
            let icon = icon.to_rgba();

            let filter = if icon.dimensions().0 <= 32 {
                // println!("{}: {}", index.keys(key)[0], icon.dimensions().0);
                FilterType::Nearest
            } else {
                // if icon.dimensions().0 < 64 {
                //     println!("{}: {}", index.keys(key)[0], icon.dimensions().0)
                // }

                FilterType::CatmullRom
            };

            let scaled = image::imageops::resize(&icon, 64, 64, filter);

            let mut out = image::RgbaImage::from_pixel(64, 64, [0; 4].into());
            image::imageops::overlay(&mut out, &scaled, 0, 0);

            DynamicImage::ImageRgba8(out)
        });

        let icon = display_icon.as_ref().and_then(|image| {
            crate::attempt(
                || format!("save cached icon {:?}", provider.keys(&target)),
                || {
                    let icon_name = format!("icons/{}.png", self.next_icon);
                    image.save(&icon_name)?;
                    self.next_icon += 1;
                    Ok(icon_name)
                },
            )
        });

        let entry = IndexEntry {
            target,
            icon,
            counter: 0,
        };

        self.get_save_mut().push(entry);
    }

    fn get_entry<K>(&mut self, key: &K) -> &mut IndexEntry<K>
    where
        K: Clone + Eq,
        Index: IndexType<K>,
    {
        let list = self.get_save_mut();
        let existing = list.iter_mut().find(|x| x.target == *key);

        existing.unwrap()
    }
}

struct IndexFile {
    path: PathBuf,
    index: Index,
}

impl IndexFile {
    pub fn open(path: PathBuf) -> IndexFile {
        let index = crate::attempt(
            || format!("load index"),
            || {
                let src = BufReader::new(File::open(&path)?);
                let index = serde_json::from_reader(src)?;
                Ok(index)
            },
        );

        let index = index.unwrap_or_default();

        IndexFile { index, path }
    }

    pub fn save(&self) {
        crate::attempt(
            || format!("save index"),
            || {
                let dst = File::create(&self.path)?;
                serde_json::to_writer(dst, &self.index)?;
                Ok(())
            },
        );
    }
}

trait IndexType<K: Clone + Eq> {
    fn get_save(&self) -> &[IndexEntry<K>];
    fn get_save_mut(&mut self) -> &mut Vec<IndexEntry<K>>;
}

macro_rules! save_state {
    ( $name:ident, $key:ty ) => {
        impl IndexType<$key> for Index {
            fn get_save(&self) -> &[IndexEntry<$key>] {
                &self.$name
            }

            fn get_save_mut(&mut self) -> &mut Vec<IndexEntry<$key>> {
                &mut self.$name
            }
        }
    };
}

save_state!(config, ManualTarget);
save_state!(appx, AppxTarget);
save_state!(steam, SteamTarget);
save_state!(start_menu, StartMenuTarget);
