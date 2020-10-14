use std::{fs::File, io::prelude::*, process::Command};

use super::{IndexEntry, LaunchTarget};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearchConfig {
    pub index_steam: Option<String>,
    pub index_appx: bool,
    pub index_start_menu: bool,
    pub index_manual: Vec<ConfigIndexEntry>,
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            index_steam: None,
            index_appx: true,
            index_start_menu: true,
            index_manual: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConfigIndexEntry {
    names: Vec<String>,
    target: Vec<String>,
    icon: Option<String>,
}

pub fn load() -> SearchConfig {
    let raw: SearchConfig = crate::nonfatal(|| {
        let mut content = vec![];
        File::open("config.yaml")?.read_to_end(&mut content)?;
        Ok(serde_yaml::from_slice(&content)?)
    })
    .unwrap_or_default();

    raw
}

pub fn index(config: &SearchConfig) -> impl Iterator<Item = (IndexEntry, LaunchTarget)> {
    let index = config.index_manual.iter().map(|src| {
        let keys = src.names.iter();

        let details = src.target[0].clone();

        let icon_path = match &src.icon {
            Some(name) => name,
            None => &src.names[0],
        };
        let display_icon = crate::nonfatal(|| Ok(image::open(icon_path)?));

        let target = src.target.clone();
        let launch = Box::new(move || {
            Command::new(&target[0])
                .args(target[1..].iter()) //
                .spawn()
                .expect("spawn process");
        });

        let index = IndexEntry::new(keys);

        let target = LaunchTarget {
            details,
            display_icon,
            launch,
        };

        (index, target)
    });

    let index: Vec<_> = index.collect();

    index.into_iter()
}
