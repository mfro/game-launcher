use std::{fs::File, io::prelude::*, process::Command};

use super::SearchProvider;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearchConfig {
    pub index_steam: Option<String>,
    pub index_appx: bool,
    pub index_start_menu: bool,
    pub index_manual: Vec<ManualTarget>,
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

impl SearchConfig {
    pub fn load() -> SearchConfig {
        let raw = crate::attempt!(("load config"), {
            let mut content = vec![];
            File::open("config.yaml")?.read_to_end(&mut content)?;
            serde_yaml::from_slice(&content)?
        });

        raw.unwrap_or_default()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
pub struct ManualTarget {
    names: Vec<String>,
    target: Vec<String>,
    icon: Option<String>,
}

impl SearchConfig {
    pub fn index(&self) -> Vec<ManualTarget> {
        self.index_manual.clone()
    }
}

impl SearchProvider<ManualTarget> for SearchConfig {
    fn keys(&self, entry: &ManualTarget) -> Vec<String> {
        entry.names.clone()
    }

    fn launch(&self, entry: &ManualTarget) -> Box<dyn Fn()> {
        let target = entry.target.clone();

        Box::new(move || {
            Command::new(&target[0])
                .args(target[1..].iter())
                .spawn()
                .expect("spawn process");
        })
    }

    fn details(&self, entry: &ManualTarget) -> String {
        entry.target[0].clone()
    }

    fn display_icon(&self, entry: &ManualTarget) -> Option<image::DynamicImage> {
        let icon_path = match &entry.icon {
            Some(name) => name,
            None => &entry.names[0],
        };

        crate::attempt!(("open manual icon {}", icon_path), {
            image::open(icon_path)?
        })
    }
}
