use std::{cmp::Ordering, fs::File, io::BufReader, path::Path};

use cef::{v8, CefV8Context, CefV8Propertyattribute, CefV8Value};
use image::{imageops::FilterType, DynamicImage, ImageOutputFormat};
use serde::{de::DeserializeOwned, Serialize};

mod assets;

mod config;
use config::SearchConfig;

mod appx;
use appx::AppxIndex;

mod start_menu;
use start_menu::StartMenuIndex;

mod steam;
use steam::SteamIndex;

pub type MatchScore = [usize; 2];

pub trait Index<K> {
    fn keys(&self, entry: &K) -> Vec<String>;
    fn launch(&self, entry: &K) -> Box<dyn Fn()>;
    fn details(&self, entry: &K) -> String;
    fn display_icon(&self, entry: &K) -> Option<DynamicImage>;
}

/// Contains information required to find a value in the index.
/// That means a list of strings
pub struct Key {
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

                return Some((key, index, [char_index, word_index]));
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
    pub fn from_index<K, Idx: Index<K>>(index: &Idx, key: &K) -> Target {
        let details = index.details(key);
        let display_icon = index.display_icon(key);
        let launch = index.launch(key);

        let display_icon = display_icon.map(|icon| {
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

        Target {
            details,
            display_icon,
            launch,
        }
    }

    pub fn to_cef(self, ctx: &CefV8Context) -> CefV8Value {
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

pub struct SearchIndex<T = Target> {
    entries: Vec<(Key, T)>,
}

pub struct Match<'a, T> {
    key: &'a str,
    index: usize,
    score: MatchScore,
    object: &'a T,
}

impl SearchIndex {
    pub fn new() -> SearchIndex {
        let mut idx = SearchIndex { entries: vec![] };

        let config = SearchConfig::load();
        idx.include(&config, &config.index());

        if config.index_appx {
            let appx = AppxIndex::new();
            idx.include(&appx, &appx.index());
        }

        if config.index_start_menu {
            let start_menu = StartMenuIndex::new();
            idx.include(&start_menu, &start_menu.index());
        }

        if let Some(root) = &config.index_steam {
            let steam = SteamIndex::new(root);
            idx.include(&steam, &steam.index());
        }

        idx
    }

    pub fn include_with_cache<K, Idx>(&mut self, key: &str, index: &Idx)
    where
        K: Serialize + DeserializeOwned + Eq,
        Idx: Index<K>,
    {
        crate::nonfatal(|| {
            let path = Path::new("index").join(key).with_extension("json");
            let file = BufReader::new(File::open(path)?);
            let parsed: Vec<K> = serde_json::from_reader(file)?;

            for entry in &parsed {
                let keys = index.keys(entry);

                let key = Key { keys };
                let target = Target::from_index(index, entry);

                self.entries.push((key, target));
            }

            Ok(())
        });
    }

    pub fn include<'a, K, Idx, Iter>(&mut self, index: &Idx, entries: Iter)
    where
        K: 'a,
        Idx: Index<K>,
        Iter: IntoIterator<Item = &'a K>,
    {
        for entry in entries {
            let keys = index.keys(entry);

            let key = Key { keys };
            let target = Target::from_index(index, entry);

            self.entries.push((key, target));
        }
    }

    pub fn into_cef(self, ctx: &CefV8Context) -> SearchIndex<CefV8Value> {
        let entries = self
            .entries
            .into_iter()
            .map(|(entry, target)| (entry, target.to_cef(ctx)))
            .collect();

        SearchIndex { entries }
    }
}

impl<T> SearchIndex<T> {
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

impl SearchIndex<CefV8Value> {
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
