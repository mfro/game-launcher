use std::cmp::Ordering;

use cef::{v8, CefV8Context, CefV8Propertyattribute, CefV8Value};
use image::{imageops::FilterType, png::PngEncoder, ColorType, DynamicImage};

mod assets;

pub mod appx;
mod config;
mod start_menu;
mod steam;

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
pub struct LaunchTarget {
    details: String,
    display_icon: Option<DynamicImage>,
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

            let key = "details";
            let value = &info.details;
            object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

            let key = "display_icon";
            let value: CefV8Value = match info.display_icon {
                None => ().into(),
                Some(image) => {
                    let image = image.to_rgba();

                    let scaled = if image.dimensions().0 <= 32 {
                        println!("{}: {}", entry.keys[0], image.dimensions().0);
                        image::imageops::resize(&image, 64, 64, FilterType::Nearest)
                    } else {
                        if image.dimensions().0 < 64 {
                            println!("{}: {}", entry.keys[0], image.dimensions().0)
                        }

                        image::imageops::resize(&image, 64, 64, FilterType::CatmullRom)
                    };

                    let mut out = image::RgbaImage::from_pixel(64, 64, [0; 4].into());
                    image::imageops::overlay(&mut out, &scaled, 0, 0);

                    let mut data = vec![];
                    PngEncoder::new(&mut data)
                        .encode(out.as_raw(), out.width(), out.height(), ColorType::Rgba8)
                        .unwrap();

                    assets::create_asset(ctx, "image/png", &mut data)
                }
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

            // match Ord::cmp(&a.key.len(), &b.key.len()) {
            //     Ordering::Equal => {}
            //     o => return o,
            // }

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

fn build_index() -> impl Iterator<Item = (IndexEntry, LaunchTarget)> {
    let config = config::load();

    let mut index: Vec<_> = config::index(&config).collect();

    if config.index_appx {
        index.extend(appx::index());
    }

    if config.index_start_menu {
        index.extend(start_menu::index());
    }

    if let Some(steam_dir) = config.index_steam {
        index.extend(steam::index(&steam_dir));
    }

    index.into_iter()
}
