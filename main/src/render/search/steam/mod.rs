use std::{
    collections::HashMap, fs::File, io::prelude::*, path::Path, path::PathBuf, process::Command,
};

use flat::prelude::*;
use image::ImageFormat;
use serde::Deserialize;

mod vdf_text;
use vdf_text::{key_value, AnyValue};

mod vdf_binary;
use vdf_binary::ValveDeserializer;

use crate::common::{extract_icons};

use super::{IndexEntry, LaunchTarget};

flat_data!(AppInfoHeader);
#[repr(C, packed)]
#[derive(Copy, Clone)]
struct AppInfoHeader {
    magic: u32,
    universe: u32,
}

flat_data!(AppInfoEntryHeader);
#[repr(C, packed)]
#[derive(Copy, Clone)]
struct AppInfoEntryHeader {
    app_id: u32,
    size: u32,
    state: u32,
    last_updated: u32,
    pics_token: u64,
    sha1: [u8; 20],
    change_number: u32,
}

#[derive(Deserialize, Debug)]
struct AppInfoEntry {
    appinfo: AppInfo,
}

#[derive(Deserialize, Debug)]
struct AppInfo {
    appid: i32,
    common: Option<AppInfoCommon>,
    config: Option<AppInfoConfig>,
}

#[derive(Deserialize, Debug)]
struct AppInfoCommon {
    name: String,
    #[serde(rename = "type")]
    ty: String,
}

#[derive(Deserialize, Debug)]
struct AppInfoConfig {
    installdir: Option<String>,
    launch: Option<Vec<AppLaunch>>,
}

#[derive(Deserialize, Debug)]
struct AppLaunch {
    executable: String,
    arguments: Option<Scalar>,
    config: Option<AppLaunchConfig>,
}

#[derive(Deserialize, Debug)]
struct AppLaunchConfig {
    oslist: Option<String>,
}

#[serde(untagged)]
#[derive(Deserialize, Debug)]
enum Scalar {
    I32(i32),
    I64(i64),
    U64(u64),
    F32(f32),
    String(String),
}

impl From<Scalar> for String {
    fn from(src: Scalar) -> Self {
        match src {
            Scalar::I32(v) => format!("{}", v),
            Scalar::I64(v) => format!("{}", v),
            Scalar::U64(v) => format!("{}", v),
            Scalar::F32(v) => format!("{}", v),
            Scalar::String(v) => format!("{}", v),
        }
    }
}

fn get_library_paths() -> Vec<PathBuf> {
    let mut content = String::new();
    File::open(r"C:\Program Files (x86)\Steam\steamapps\libraryfolders.vdf")
        .unwrap()
        .read_to_string(&mut content)
        .unwrap();

    let (_, (_, value)) = key_value(content.trim()).unwrap();
    let map = match value {
        AnyValue::Map(a) => a,
        _ => panic!(),
    };

    let mut out = vec![];
    let mut index = 1;
    while map.contains_key(&index.to_string()) {
        match &map[&index.to_string()] {
            AnyValue::String(p) => out.push(PathBuf::from(p)),
            _ => (),
        };
        index += 1;
    }
    out
}

struct SteamApp {
    app_id: u32,
    name: String,
    install_dir: PathBuf,
}

fn get_apps(library_path: &Path) -> Vec<SteamApp> {
    let steam_apps = library_path.join("steamapps");
    let common = steam_apps.join("common");

    let mut out = vec![];

    for entry in steam_apps.read_dir().unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if let Some("acf") = path.extension().and_then(|e| e.to_str()) {
            let mut content = String::new();
            File::open(&path)
                .unwrap()
                .read_to_string(&mut content)
                .unwrap();

            let (_, (_, value)) = key_value(content.trim()).unwrap();
            let map = match value {
                AnyValue::Map(a) => a,
                _ => panic!(),
            };

            let app_id = match &map["appid"] {
                AnyValue::String(s) => s.parse().unwrap(),
                _ => continue,
            };

            let name = match &map["name"] {
                AnyValue::String(s) => s.clone(),
                _ => continue,
            };

            let install_dir = match &map["installdir"] {
                AnyValue::String(s) => s,
                _ => continue,
            };

            let install_dir = common.join(install_dir);

            out.push(SteamApp {
                app_id,
                name,
                install_dir,
            });
        }
    }

    out
}

pub fn index(dir: &str) -> impl Iterator<Item = (IndexEntry, LaunchTarget)> {
    let steam_root = Path::new(dir);

    let content = crate::nonfatal(|| {
        let appinfo = steam_root.join("appcache/appinfo.vdf");
        let mut content = vec![];
        File::open(appinfo)?.read_to_end(&mut content)?;
        Ok(content)
    });

    let content = match content {
        Some(v) => v,
        None => return vec![].into_iter(),
    };

    let mut content: &[u8] = content.as_ref();
    let _header: &AppInfoHeader = content.load();

    let mut collect = HashMap::new();
    while content.len() > 4 {
        let entry: &AppInfoEntryHeader = content.load();

        let mut deserializer = ValveDeserializer::new(&mut content);

        let x: AppInfoEntry = Deserialize::deserialize(&mut deserializer).unwrap();
        collect.insert(entry.app_id, x.appinfo);
    }

    let library_paths = get_library_paths();
    let apps = library_paths.into_iter().map(|p| get_apps(&p)).flatten();

    let vec: Vec<_> = apps
        .filter_map(|app| {
            println!("{}: {} {:?}", app.name, app.app_id, app.install_dir);

            let info = collect.get(&app.app_id)?;

            let mut all_icons = vec![];
            if let Some(config) = &info.config {
                if let Some(launch) = &config.launch {
                    for launch in launch {
                        let exe_path = app.install_dir.join(&launch.executable);
                        if exe_path.exists() {
                            if let Some("exe") = exe_path.extension().and_then(|s| s.to_str()) {
                                match extract_icons(&exe_path) {
                                    Err(e) => println!("{:?}", e),
                                    Ok(mut v) => {
                                        println!("  {}: {:?}", v.len(), exe_path);
                                        all_icons.append(&mut v);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // if all_icons.is_empty() {
            //     for entry in RecursiveSearch::new(&app.install_dir) {
            //         let path = entry.path();
            //         if let Some("exe") | Some("ico") = path.extension().and_then(|s| s.to_str()) {
            //             match extract_icons(&path) {
            //                 Err(e) => println!("{:?}", e),
            //                 Ok(mut v) => {
            //                     println!("  {}: {:?}", v.len(), path);
            //                     all_icons.append(&mut v);
            //                 }
            //             }
            //         }
            //     }
            // }

            let keys = vec![app.name.clone()];

            let details = format!("Steam Game");

            let display_icon = all_icons.get(0).and_then(|data| {
                crate::nonfatal(|| {
                    Ok(image::load_from_memory_with_format(
                        &data,
                        ImageFormat::Ico,
                    )?)
                })
            });

            let steam_exe = steam_root.join("steam.exe");
            let launch = Box::new(move || {
                Command::new(&steam_exe)
                    .arg("-applaunch") //
                    .arg(&format!("{}", app.app_id)) //
                    .spawn()
                    .expect("spawn process");
            });

            let index = IndexEntry::new(keys.into_iter());

            let target = LaunchTarget {
                details,
                display_icon,
                launch,
            };

            Some((index, target))
        })
        .collect();

    vec.into_iter()
}
