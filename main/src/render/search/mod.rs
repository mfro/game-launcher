use std::{
    cell::RefCell, cmp::Ordering, fmt::Debug, fs::File, io::BufReader, path::PathBuf, rc::Rc,
};

use cef::{v8, CefV8Context, CefV8Propertyattribute, CefV8Value};
use image::{imageops::FilterType, DynamicImage, ImageOutputFormat};

mod assets;

mod config;
use config::{ManualTarget, SearchConfig};

mod appx;
use appx::{AppxProvider, AppxTarget};

mod start_menu;
use start_menu::{StartMenuProvider, StartMenuTarget};

mod steam;
use steam::{SteamProvider, SteamTarget};

pub type MatchScore = (usize, usize, u64);

pub trait SearchProvider<K> {
    fn keys(&self, target: &K) -> Vec<String>;
    fn launch(&self, target: &K) -> Box<dyn Fn()>;
    fn details(&self, target: &K) -> String;
    fn display_icon(&self, target: &K) -> Option<DynamicImage>;
}

pub struct Match<'a, T> {
    pub key: &'a str,
    pub index: usize,
    pub score: MatchScore,
    pub value: T,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct IndexEntryMeta {
    icon: String,
    counter: u64,
}

pub struct IndexEntry<T, D> {
    pub data: D,
    pub target: T,
    keys: Vec<(String, String)>,
    meta: IndexEntryMeta,
}

impl<T, D> IndexEntry<T, D> {
    pub fn new<P: SearchProvider<T>>(
        provider: &P,
        meta: IndexEntryMeta,
        target: T,
        data: D,
    ) -> IndexEntry<T, D> {
        let keys = provider
            .keys(&target)
            .into_iter()
            .map(|x| {
                let lower = x.to_lowercase();
                (x, lower)
            })
            .collect();

        IndexEntry {
            keys,
            meta,
            target,
            data,
        }
    }

    pub fn with_data<D2>(self, data: D2) -> IndexEntry<T, D2> {
        IndexEntry {
            keys: self.keys,
            meta: self.meta,
            target: self.target,
            data,
        }
    }

    pub fn do_match(&self, query: &str) -> Option<(&str, usize, MatchScore)> {
        for (key, lower) in &self.keys {
            if let Some(byte_index) = lower.find(query) {
                let char_index = lower
                    .char_indices()
                    .position(|(idx, _)| idx == byte_index)
                    .unwrap();

                let chars: Vec<_> = lower.chars().take(char_index).collect();
                let word_index = chars.iter().filter(|&&c| c == ' ').count();
                let within_word_index = chars
                    .iter()
                    .rev()
                    .position(|&x| x == ' ')
                    .unwrap_or(char_index);

                let score = (within_word_index, word_index, u64::MAX - self.meta.counter);

                return Some((key, char_index, score));
            }
        }

        None
    }
}

pub fn make_v8object<D1, D2: 'static>(
    rc: Rc<RefCell<Search<D2>>>,
    provider: &Provider,
    ctx: &CefV8Context,
    index: usize,
    entry: &IndexEntry<AnyTarget, D1>,
) -> CefV8Value {
    let object = CefV8Value::create_object(None, None).unwrap();

    let key = "details";
    let value = provider.details(&entry.target);
    object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

    let key = "display_icon";
    let display_icon = crate::attempt!(
        ("open cached icon {} {:?}", entry.meta.icon, entry.target),
        image::open(&entry.meta.icon)?
    );
    let value: CefV8Value = match display_icon {
        None => ().into(),
        Some(image) => {
            let mut data = vec![];
            image.write_to(&mut data, ImageOutputFormat::Png).unwrap();
            assets::create_asset(ctx, "image/png", &mut data)
        }
    };
    object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

    let key = "launch";
    let launch = provider.launch(&entry.target);
    let launch = Box::new(move || {
        launch();
        let mut search = rc.borrow_mut();
        let mut entry = &mut search.index[index];
        entry.meta.counter += 1;
        search.save();
    });
    let value = v8::v8_function0(key, move || launch());
    object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

    object
}

#[derive(Serialize, Deserialize, Clone, Default)]
struct SearchMeta {
    next_icon: u64,
}

pub struct Search<D> {
    meta: SearchMeta,
    index: Vec<IndexEntry<AnyTarget, D>>,
    save_path: PathBuf,
}

impl Search<()> {
    pub fn load(provider: &Provider, save_path: PathBuf) -> Search<()> {
        let save = crate::attempt!(("open search save"), {
            let src = BufReader::new(File::open(&save_path)?);
            serde_json::from_reader(src)?
        });

        let save: SearchSave = save.unwrap_or_default();

        let meta = save.meta;
        let index = save
            .index
            .into_iter()
            .map(|src| IndexEntry::new(provider, src.meta, src.target, ()))
            .collect();

        Search {
            meta,
            index,
            save_path,
        }
    }

    pub fn include(
        &mut self,
        provider: &Provider,
        targets: impl IntoIterator<Item = impl Into<AnyTarget>>,
    ) {
        let mut values = targets
            .into_iter()
            .filter_map(|target| {
                let target = target.into();

                let existing = self.index.iter().any(|a| a.target == target);
                if existing {
                    return None;
                }

                let display_icon = provider.display_icon(&target).map(|icon| {
                    let icon = icon.to_rgba();

                    let filter = if icon.dimensions().0 <= 32 {
                        FilterType::Nearest
                    } else {
                        FilterType::CatmullRom
                    };

                    let scaled = image::imageops::resize(&icon, 64, 64, filter);

                    let mut out = image::RgbaImage::from_pixel(64, 64, [0; 4].into());
                    image::imageops::overlay(&mut out, &scaled, 0, 0);

                    DynamicImage::ImageRgba8(out)
                });

                let icon = format!("icons/{}.png", self.meta.next_icon);
                self.meta.next_icon += 1;

                display_icon.as_ref().and_then(|image| {
                    crate::attempt!(("save cached icon {:?}", provider.keys(&target)), {
                        image.save(&icon)?
                    })
                });

                let meta = IndexEntryMeta { icon, counter: 0 };

                Some(IndexEntry::new(provider, meta, target, ()))
            })
            .collect();

        self.index.append(&mut values);
    }
}

impl<D> Search<D> {
    pub fn save(&self) {
        let meta = self.meta.clone();
        let index = self
            .index
            .iter()
            .map(|e| IndexEntrySave {
                meta: e.meta.clone(),
                target: e.target.clone(),
            })
            .collect();

        let save = SearchSave { meta, index };

        crate::attempt!(("search save"), {
            let src = File::create(&self.save_path)?;
            serde_json::to_writer(src, &save)?;
        });
    }

    pub fn into_cef(
        self,
        ctx: &CefV8Context,
        provider: &Provider,
    ) -> Rc<RefCell<Search<CefV8Value>>> {
        let rc = Rc::new(RefCell::new(Search {
            meta: self.meta,
            index: vec![],
            save_path: self.save_path,
        }));

        let index = self
            .index
            .into_iter()
            .enumerate()
            .map(|(i, entry)| {
                let v8 = make_v8object(rc.clone(), provider, ctx, i, &entry);
                entry.with_data(v8)
            })
            .collect();

        rc.borrow_mut().index = index;

        rc
    }

    pub fn search(&self, query: &str) -> Vec<Match<&IndexEntry<AnyTarget, D>>> {
        let mut matches: Vec<_> = self
            .index
            .iter()
            .filter_map(|entry| {
                entry.do_match(query).map(|(key, index, score)| Match {
                    key,
                    index,
                    score,
                    value: entry,
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

#[derive(Serialize, Deserialize, Default)]
pub struct SearchSave {
    meta: SearchMeta,
    index: Vec<IndexEntrySave>,
}

#[derive(Serialize, Deserialize)]
struct IndexEntrySave {
    #[serde(flatten)]
    meta: IndexEntryMeta,
    #[serde(flatten)]
    target: AnyTarget,
}

macro_rules! any_search {
    ( $( ( $variant:ident, $name:ident, $target:ty, $provider:ty ), )* ) => {
        #[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
        pub enum AnyTarget {
            $( $variant ( $target ) ),*
        }

        $(
            impl From<$target> for AnyTarget {
                fn from(src: $target) -> AnyTarget {
                    AnyTarget::$variant(src)
                }
            }
        )*

        pub struct Provider {
            $( pub $name: $provider ),*
        }

        impl SearchProvider<AnyTarget> for Provider {
            fn keys(&self, target: &AnyTarget) -> Vec<String> {
                match target {
                    $( AnyTarget::$variant(t) => self.$name.keys(t), )*
                }
            }

            fn launch(&self, target: &AnyTarget) -> Box<dyn Fn()> {
                match target {
                    $( AnyTarget::$variant(t) => self.$name.launch(t), )*
                }
            }

            fn details(&self, target: &AnyTarget) -> String {
                match target {
                    $( AnyTarget::$variant(t) => self.$name.details(t), )*
                }
            }

            fn display_icon(&self, target: &AnyTarget) -> Option<DynamicImage> {
                match target {
                    $( AnyTarget::$variant(t) => self.$name.display_icon(t), )*
                }
            }
        }
    };
}

any_search!(
    (Config, config, ManualTarget, SearchConfig),
    (Appx, appx, AppxTarget, AppxProvider),
    (Steam, steam, SteamTarget, SteamProvider),
    (StartMenu, start_menu, StartMenuTarget, StartMenuProvider),
);
// }

impl Provider {
    pub fn new() -> Provider {
        let config = SearchConfig::load();

        if config.index_appx {
            let appx = AppxProvider::new();

            if let Some(root) = &config.index_steam {
                let steam = SteamProvider::new(root);

                if config.index_start_menu {
                    let start_menu = StartMenuProvider::new();

                    return Provider {
                        config,
                        appx,
                        steam,
                        start_menu,
                    };
                }
            }
        }

        panic!()
    }
}
