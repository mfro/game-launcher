use std::{fs::File, io::prelude::*, process::Command};

use super::{icon_from_file, IndexEntry, LaunchTarget};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConfigEntry {
    names: Vec<String>,
    target: Vec<String>,
    icon: Option<String>,
}

pub fn index() -> impl Iterator<Item = (IndexEntry, LaunchTarget)> {
    let raw: Vec<ConfigEntry> = crate::nonfatal(|| {
        let mut content = vec![];
        File::open("config.yaml")?.read_to_end(&mut content)?;
        Ok(serde_yaml::from_slice(&content)?)
    })
    .unwrap_or_default();

    let index = raw.into_iter().map(|src| {
        let keys = src.names.iter();

        let path = match &src.icon {
            Some(name) => name,
            None => &src.names[0],
        };
        let display_icon = icon_from_file(path);

        let target = src.target;
        let launch = Box::new(move || {
            Command::new(&target[0])
                .args(target[1..].iter()) //
                .spawn()
                .expect("spawn process");
        });

        let index = IndexEntry::new(keys);

        let target = LaunchTarget {
            display_icon,
            launch,
        };

        (index, target)
    });

    let index: Vec<_> = index.collect();

    index.into_iter()
}
