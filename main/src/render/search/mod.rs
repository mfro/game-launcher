use std::{
    cell::RefCell, cmp::Ordering, fmt::Debug, fs::File, io::BufReader, path::PathBuf, rc::Rc,
};

use image::{imageops::FilterType, DynamicImage};

mod config;
use config::{ManualTarget, SearchConfig};

mod appx;
use appx::{AppxProvider, AppxTarget};

mod start_menu;
use serde::{de::DeserializeOwned, Serialize};
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
struct IndexEntryMeta {
    icon: String,
    counter: u64,
}

struct IndexEntry<T> {
    target: T,
    keys: Vec<(String, String)>,
    meta: IndexEntryMeta,
}

impl<T> IndexEntry<T> {
    pub fn new<P>(provider: &P, meta: IndexEntryMeta, target: T) -> IndexEntry<T>
    where
        P: SearchProvider<T>,
    {
        let keys = provider
            .keys(&target)
            .into_iter()
            .map(|x| {
                let lower = x.to_lowercase();
                (x, lower)
            })
            .collect();

        IndexEntry { keys, meta, target }
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

#[derive(Serialize, Deserialize, Clone, Default)]
struct IndexMeta {
    next_icon: u64,
}

pub struct Index<T, P> {
    pub provider: P,
    meta: IndexMeta,
    entries: Vec<IndexEntry<T>>,
    save_path: PathBuf,
}

impl<T, P> Index<T, P>
where
    T: Serialize + DeserializeOwned + Clone + Eq,
    P: SearchProvider<T>,
{
    pub fn open(provider: P, save_path: PathBuf) -> Index<T, P> {
        let save = crate::attempt!(("open search save"), {
            let src = BufReader::new(File::open(&save_path)?);
            serde_json::from_reader(src)?
        });

        let save: IndexSave<T> = save.unwrap_or_default();

        let meta = save.meta;
        let entries = save
            .entries
            .into_iter()
            .map(|src| IndexEntry::new(&provider, src.meta, src.target))
            .collect();

        Index {
            meta,
            entries,
            provider,
            save_path,
        }
    }

    pub fn save(&self) {
        let meta = self.meta.clone();
        let entries = self
            .entries
            .iter()
            .map(|e| IndexEntrySave {
                meta: e.meta.clone(),
                target: e.target.clone(),
            })
            .collect();

        let save = IndexSave {
            meta,
            entries: entries,
        };

        crate::attempt!(("search save"), {
            let src = File::create(&self.save_path)?;
            serde_json::to_writer(src, &save)?;
        });
    }

    pub fn include(&mut self, targets: impl IntoIterator<Item = impl Into<T>>) {
        let mut values = targets
            .into_iter()
            .filter_map(|target| {
                let target = target.into();

                let existing = self.entries.iter().any(|a| a.target == target);
                if existing {
                    return None;
                }

                let display_icon = self.provider.display_icon(&target).map(|icon| {
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
                    crate::attempt!(("save cached icon {:?}", self.provider.keys(&target)), {
                        image.save(&icon)?
                    })
                });

                let meta = IndexEntryMeta { icon, counter: 0 };

                Some(IndexEntry::new(&self.provider, meta, target))
            })
            .collect();

        self.entries.append(&mut values);
    }

    pub fn search(&self, query: &str) -> Vec<Match<usize>> {
        let mut matches: Vec<_> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, entry)| {
                entry.do_match(query).map(|(key, index, score)| Match {
                    key,
                    index,
                    score,
                    value: i,
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

    pub fn iter(&self) -> impl Iterator<Item = usize> {
        0..self.entries.len()
    }
}

impl<T, P> SearchProvider<usize> for Rc<RefCell<Index<T, P>>>
where
    T: 'static + Serialize + DeserializeOwned + Clone + Eq + Debug,
    P: 'static + SearchProvider<T>,
{
    fn keys(&self, &target: &usize) -> Vec<String> {
        let this = self.borrow();
        let entry = &this.entries[target];
        this.provider.keys(&entry.target)
    }

    fn details(&self, &target: &usize) -> String {
        let this = self.borrow();
        let entry = &this.entries[target];
        this.provider.details(&entry.target)
    }

    fn launch(&self, &target: &usize) -> Box<dyn Fn()> {
        let this = self.borrow();
        let entry = &this.entries[target];
        let launch = this.provider.launch(&entry.target);

        let rc = self.clone();
        Box::new(move || {
            launch();
            let mut this = rc.borrow_mut();
            this.entries[target].meta.counter += 1;
            this.save();
        })
    }

    fn display_icon(&self, &target: &usize) -> Option<DynamicImage> {
        let this = self.borrow();
        let entry = &this.entries[target];

        crate::attempt!(
            ("open cached icon {} {:?}", entry.meta.icon, entry.target),
            image::open(&entry.meta.icon)?
        )
    }
}

#[derive(Serialize, Deserialize)]
struct IndexSave<T = AnyTarget> {
    meta: IndexMeta,
    entries: Vec<IndexEntrySave<T>>,
}

#[derive(Serialize, Deserialize)]
struct IndexEntrySave<T = AnyTarget> {
    #[serde(flatten)]
    meta: IndexEntryMeta,
    #[serde(flatten)]
    target: T,
}

impl<T> Default for IndexSave<T> {
    fn default() -> Self {
        IndexSave {
            meta: Default::default(),
            entries: Default::default(),
        }
    }
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
