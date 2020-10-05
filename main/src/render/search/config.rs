use std::{fs::File, io::prelude::*, process::Command};

use super::{icon_helper, IndexEntry, LaunchTarget};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConfigEntry {
    names: Vec<String>,
    target: Vec<String>,
    icon: Option<String>,
}

pub fn index() -> impl Iterator<Item = (IndexEntry, LaunchTarget)> {
    let raw: Vec<ConfigEntry> = match File::open("config.yaml") {
        Ok(mut f) => {
            let mut content = vec![];
            f.read_to_end(&mut content).unwrap();
            serde_yaml::from_slice(&content).unwrap()
        }
        Err(_) => vec![],
    };

    let index = raw.into_iter().map(|src| {
        let keys = src.names.iter();

        let display_name = src.names[0].clone();

        let display_icon = icon_helper(|| {
            let path = match &src.icon {
                Some(name) => name,
                None => &src.names[0],
            };

            let mime = mime_guess::from_path(path);

            let mut data = vec![];
            File::open(path)?.read_to_end(&mut data)?;

            Ok((mime.first().unwrap(), data))
        });

        let target = src.target;
        let launch = Box::new(move || {
            Command::new(&target[0])
                .args(target[1..].iter()) //
                .spawn()
                .unwrap();
        });

        let index = IndexEntry::new(keys);

        let target = LaunchTarget {
            display_name,
            display_icon,
            launch,
        };

        (index, target)
    });

    let index: Vec<_> = index.collect();

    index.into_iter()
}
