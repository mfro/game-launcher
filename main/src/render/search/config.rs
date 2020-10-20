use std::{fs::File, io::prelude::*, process::Command};

use super::{appx::AppxConfig, start_menu::StartMenuConfig, steam::SteamConfig, SearchProvider};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SearchConfig {
    pub appx: Option<AppxConfig>,
    pub steam: Option<SteamConfig>,
    pub start_menu: Option<StartMenuConfig>,
    pub custom: Vec<ManualTarget>,
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            appx: None,
            steam: None,
            start_menu: None,
            custom: vec![],
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

impl SearchProvider<ManualTarget> for SearchConfig {
    fn index(&self) -> Vec<ManualTarget> {
        self.custom.clone()
    }

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
