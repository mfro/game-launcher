use std::cmp::Ordering;

use cef::{v8, CefV8Propertyattribute, CefV8Value};
use mime_guess::Mime;

mod config;
mod start_menu;


/// Contains information required to find a value in the index.
/// That means a list of lower-case strings
pub struct IndexEntry {
    keys: Vec<String>,
}

impl IndexEntry {
    pub fn new<A: AsRef<str>, I: Iterator<Item = A>>(keys: I) -> IndexEntry {
        let keys = keys.map(|s| s.as_ref().to_lowercase()).collect();
        IndexEntry { keys }
    }

    pub fn do_match(&self, query: &str) -> Option<(&str, usize)> {
        for key in &self.keys {
            if let Some(index) = key.to_lowercase().find(&query) {
                return Some((key, index));
            }
        }

        None
    }
}

/// Contains information about a value in the index.
/// That means a display name & icon for rendering, and a function to launch the target
pub struct LaunchTarget {
    display_name: String,
    display_icon: Option<String>,
    launch: Box<dyn Fn()>,
}

impl From<LaunchTarget> for CefV8Value {
    fn from(info: LaunchTarget) -> Self {
        let object = CefV8Value::create_object(None, None).unwrap();

        let key = "display_name";
        let value = &info.display_name;
        object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

        let key = "display_icon";
        let value = match &info.display_icon {
            Some(url) => url.as_ref(),
            None => "app://404",
        };
        object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

        let key = "launch";
        let launch = info.launch;
        let value = v8::v8_function0(key, move || launch());
        object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

        object
    }
}

pub struct Search {
    index: Vec<(IndexEntry, CefV8Value)>,
}

struct Match<'a> {
    key: &'a str,
    index: usize,
    object: &'a CefV8Value,
}

impl Search {
    pub fn new() -> Search {
        let index = build_index()
            .map(|(index, target)| (index, target.into()))
            .collect();

        Search { index }
    }

    pub fn search(&self, query: String) -> CefV8Value {
        let mut matches: Vec<_> = self
            .index
            .iter()
            .filter_map(|(entry, object)| {
                entry
                    .do_match(&query)
                    .map(|(key, index)| Match { key, index, object })
            })
            .collect();

        matches.sort_unstable_by(|a, b| {
            if a.index != b.index {
                return a.index.cmp(&b.index);
            } else if a.key.len() != b.key.len() {
                return a.key.len().cmp(&b.key.len());
            } else {
                return Ordering::Equal;
            }
        });

        let limit = 7.min(matches.len());
        let display = &matches[0..limit];

        let result = v8::v8_array(display.iter().map(|m| {
            let object = CefV8Value::create_object(None, None).unwrap();

            let key = "key";
            let value = m.key;
            object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

            let key = "index";
            let value = m.index;
            object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

            let key = "length";
            let value = query.len();
            object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

            let key = "target";
            let value = m.object.clone();
            object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

            object
        }));

        result
    }
}

pub fn icon_helper<F: FnOnce() -> std::io::Result<(Mime, Vec<u8>)>>(f: F) -> Option<String> {
    match f() {
        Ok((mime, data)) => Some(format!("data:{};base64,{}", mime, base64::encode(data))),
        Err(e) => {
            println!("{:?}", e);
            None
        }
    }
}

fn build_index() -> impl Iterator<Item = (IndexEntry, LaunchTarget)> {
    config::index().chain(start_menu::index())
}
