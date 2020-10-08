use std::{cmp::Ordering, fs::File, io::prelude::*, path::Path};

use cef::{v8, CefV8Context, CefV8Propertyattribute, CefV8Value};
use mime_guess::Mime;

mod assets;

pub mod appx;
mod config;
mod start_menu;

pub type MatchScore = [usize; 2];

/// Contains information required to find a value in the index.
/// That means a list of strings
pub struct IndexEntry {
    keys: Vec<String>,
}

impl IndexEntry {
    pub fn new<A: AsRef<str>, I: Iterator<Item = A>>(keys: I) -> IndexEntry {
        let keys = keys.map(|x| x.as_ref().to_owned()).collect();
        IndexEntry { keys }
    }

    pub fn do_match(&self, query: &str) -> Option<(&str, usize, MatchScore)> {
        for key in &self.keys {
            if let Some(index) = key.to_lowercase().find(&query) {
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
pub struct LaunchTarget {
    display_icon: Option<(Mime, Vec<u8>)>,
    launch: Box<dyn Fn()>,
}

pub struct Search {
    index: Vec<(IndexEntry, CefV8Value)>,
}

struct Match<'a> {
    key: &'a str,
    index: usize,
    score: MatchScore,
    object: &'a CefV8Value,
}

struct ReleaseCallback;
impl cef::V8ArrayBufferReleaseCallback for ReleaseCallback {
    fn release_buffer(&mut self, buffer: &mut std::ffi::c_void) {
        println!("{:?}", buffer as *mut _);
    }
}

impl Search {
    pub fn new(ctx: &CefV8Context) -> Search {
        let start = std::time::Instant::now();

        let mut index = vec![];

        for (entry, info) in build_index() {
            let object = CefV8Value::create_object(None, None).unwrap();

            let key = "display_icon";
            let value: CefV8Value = match info.display_icon {
                Some((mime, mut data)) => assets::create_asset(ctx, &mime, &mut data),
                None => "app://notfound".into(),
            };
            object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

            let key = "launch";
            let launch = info.launch;
            let value = v8::v8_function0(key, move || launch());
            object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

            index.push((entry, object));
        }

        let end = std::time::Instant::now();
        println!("index built: {:?}", end - start);

        Search { index }
    }

    pub fn search(&self, query: String) -> CefV8Value {
        let query = query.to_lowercase();

        let mut matches: Vec<_> = self
            .index
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

            match Ord::cmp(&a.key.len(), &b.key.len()) {
                Ordering::Equal => {}
                o => return o,
            }

            Ord::cmp(a.key, b.key)
        });

        let limit = 7.min(matches.len());
        let display = &matches[0..limit];

        v8::v8_array(display.iter().map(|m| {
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

pub fn icon_from_file<P: AsRef<Path>>(path: P) -> Option<(Mime, Vec<u8>)> {
    crate::nonfatal(|| {
        let mut data = vec![];
        File::open(path.as_ref())?.read_to_end(&mut data)?;
        let mime = mime_guess::from_path(path);
        let mime = mime.first().unwrap();
        Ok((mime, data))
    })
}

fn build_index() -> impl Iterator<Item = (IndexEntry, LaunchTarget)> {
    let config = config::load();

    let mut index: Vec<_> = config::index(&config).collect();

    if config.index_appx {
        index.extend(appx::index());
    }

    if config.index_appx {
        index.extend(start_menu::index());
    }

    index.into_iter()
}
