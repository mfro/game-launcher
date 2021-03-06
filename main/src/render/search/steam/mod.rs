use std::{
    collections::HashMap, fs::File, io::prelude::*, io::Cursor, path::Path, path::PathBuf,
    process::Command,
};

use flat::prelude::*;
use image::{ico::IcoDecoder, DynamicImage};
use serde::Deserialize;

mod vdf_text;
use vdf_text::{key_value, AnyValue};

mod vdf_binary;
use vdf_binary::ValveDeserializer;

use crate::common::extract_icons;

use super::SearchProvider;

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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SteamConfig {
    path: String,
}

pub struct SteamProvider {
    steam_dir: PathBuf,
    app_info: HashMap<u32, AppInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
pub struct SteamTarget {
    app_id: u32,
    name: String,
    install_dir: PathBuf,
}

impl SteamProvider {
    pub fn new(config: &SteamConfig) -> SteamProvider {
        let steam_dir = Path::new(&config.path);

        let app_info = crate::attempt!(("read appinfo.vdf {:?}", steam_dir), {
            let appinfo = steam_dir.join("appcache/appinfo.vdf");
            let mut content = vec![];
            File::open(appinfo)?.read_to_end(&mut content)?;

            let mut content: &[u8] = content.as_ref();
            let _header: &AppInfoHeader = content.load();

            let mut app_info = HashMap::new();
            while content.len() > 4 {
                let entry: &AppInfoEntryHeader = content.load();

                let mut deserializer = ValveDeserializer::new(&mut content);

                let x: AppInfoEntry = Deserialize::deserialize(&mut deserializer)?;
                app_info.insert(entry.app_id, x.appinfo);
            }

            app_info
        })
        .unwrap_or_default();

        SteamProvider {
            steam_dir: steam_dir.to_owned(),
            app_info,
        }
    }

    fn get_library_paths(&self) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
        let libraryfolders = self.steam_dir.join("steamapps/libraryfolders.vdf");

        let mut content = String::new();
        File::open(libraryfolders)?.read_to_string(&mut content)?;

        let (_, (_, value)) = key_value(content.trim())
            .map_err(|x| format!("failed to parse libraryfolders: {:?}", x))?;

        let map = match value {
            AnyValue::Map(a) => a,
            _ => panic!(),
        };

        let mut collect = vec![];
        let mut index = 1;
        while map.contains_key(&index.to_string()) {
            match &map[&index.to_string()] {
                AnyValue::String(p) => collect.push(PathBuf::from(p)),
                _ => (),
            };
            index += 1;
        }

        Ok(collect)
    }

    fn get_apps(
        &self,
        library_path: &Path,
    ) -> Result<Vec<SteamTarget>, Box<dyn std::error::Error>> {
        let steam_apps = library_path.join("steamapps");
        let common = steam_apps.join("common");

        let mut out = vec![];

        for entry in steam_apps.read_dir()? {
            let entry = entry?;
            let path = entry.path();

            if let Some("acf") = path.extension().and_then(|e| e.to_str()) {
                let mut content = String::new();
                File::open(&path)?.read_to_string(&mut content)?;

                let (_, (_, value)) = key_value(content.trim())
                    .map_err(|x| format!("failed to parse libraryfolders: {:?}", x))?;

                let map = match value {
                    AnyValue::Map(a) => a,
                    _ => panic!(),
                };

                let app_id = match &map["appid"] {
                    AnyValue::String(s) => s.parse()?,
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

                out.push(SteamTarget {
                    app_id,
                    name,
                    install_dir,
                });
            }
        }

        Ok(out)
    }
}

impl SearchProvider<SteamTarget> for SteamProvider {
    fn index(&self) -> Vec<SteamTarget> {
        let apps = self
            .get_library_paths()
            .into_iter()
            .flatten()
            .map(|p| self.get_apps(&p))
            .flatten()
            .flatten();

        apps.collect()
    }

    fn keys(&self, entry: &SteamTarget) -> Vec<String> {
        vec![entry.name.clone()]
    }

    fn launch(&self, entry: &SteamTarget) -> Box<dyn Fn()> {
        let steam_exe = self.steam_dir.join("steam.exe");
        let app_id = entry.app_id;

        Box::new(move || {
            Command::new(&steam_exe)
                .arg("-applaunch") //
                .arg(&format!("{}", app_id)) //
                .spawn()
                .expect("spawn process");
        })
    }

    fn details(&self, entry: &SteamTarget) -> String {
        format!(r"Steam: {}", entry.app_id)
    }

    fn display_icon(&self, entry: &SteamTarget) -> Option<image::DynamicImage> {
        self.app_info
            .get(&entry.app_id)
            .and_then(|x| Some(x.config.as_ref()?.launch.as_ref()?.iter()))
            .into_iter()
            .flatten()
            .filter_map(|launch| {
                let exe_path = entry.install_dir.join(&launch.executable);
                if !exe_path.exists() {
                    return None;
                }

                let ext = exe_path.extension()?.to_str()?;
                if ext != "exe" {
                    return None;
                }

                crate::attempt!(("load steam icon {:?}", exe_path), {
                    let data = extract_icons(&exe_path)?;
                    let data = data.get(0)?;

                    let r = Cursor::new(&data);
                    let decoder = IcoDecoder::new_unchecked(r)?;
                    DynamicImage::from_decoder(decoder)?
                })
            })
            .next()
    }
}

#[test]
fn test() -> Result<(), Box<dyn std::error::Error>> {
    use self::vdf_binary::{ValveReader, ValveToken};

    let steam_dir = Path::new(r"C:\Program Files (x86)\Steam");
    let appinfo = steam_dir.join("appcache/appinfo.vdf");
    let mut content = vec![];
    File::open(appinfo)?.read_to_end(&mut content)?;

    let mut content: &[u8] = content.as_ref();
    let _header: &AppInfoHeader = content.load();

    while content.len() > 4 {
        let _entry: &AppInfoEntryHeader = content.load();

        let reader = ValveReader::new(&mut content);
        let mut indent = String::new();

        for node in reader {
            match node {
                None => indent.truncate(indent.len() - 2),
                Some((key, ValveToken::Object)) => {
                    println!("{}{}:", indent, key);
                    indent += "  ";
                }
                Some((key, value)) => {
                    println!("{}{}: {:?}", indent, key, value);
                }
            }
        }
    }

    Ok(())
}
