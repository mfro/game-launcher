use std::{fs::File, io::prelude::*, io::Error, io::ErrorKind, path::PathBuf};

use image::ImageFormat;
use winapi::um::shellapi::ShellExecuteW;

mod lnk;
use lnk::ShellLink;

use crate::common::{RecursiveSearch, ToWide};

use super::Index;

pub struct StartMenuIndex {}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
pub struct StartMenuTarget {
    name: String,
    relative: String,
    lnk_path: PathBuf,
}

impl StartMenuIndex {
    pub fn new() -> StartMenuIndex {
        StartMenuIndex {}
    }

    pub fn index(&self) -> Vec<StartMenuTarget> {
        let appdata = std::env::var("APPDATA").unwrap();
        let roots = [
            PathBuf::from(r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs"),
            PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs"),
        ];

        let vec: Vec<_> = roots
            .iter()
            .map(|root| {
                let iter = RecursiveSearch::new(&root).into_iter();
                iter.map(move |entry| {
                    let relative = entry.path().strip_prefix(&root).unwrap().to_owned();
                    (entry, relative)
                })
            })
            .flatten()
            // select only .lnk files
            .filter_map(|(entry, relative)| {
                let path = entry.path();
                match path.extension() {
                    None => None,
                    Some(ext) => match ext.to_str() {
                        Some("lnk") => Some((path, relative)),
                        Some("ini") | Some("url") => None,
                        _ => {
                            println!("unknown start menu entry: {:?}", relative);
                            None
                        }
                    },
                }
            })
            // open and parse the .lnk files
            .filter_map(|(path, relative)| {
                let lnk = crate::nonfatal(|| {
                    let mut raw = vec![];
                    File::open(&path)?.read_to_end(&mut raw)?;
                    Ok(ShellLink::load(&raw))
                })?;

                let target = lnk::resolve(&lnk)?;
                Some((path, relative, target))
            })
            // select only .lnk files that point to 'exe', 'msc', 'url' files
            .filter(|(path, _, target)| match target.rfind('.') {
                None => panic!(),
                Some(i) => match &target[i + 1..] {
                    "exe" | "msc" => true,
                    "url" | "chm" | "txt" | "rtf" | "pdf" | "html" | "ini" => false,
                    other => {
                        println!("Unknown lnk target extension: {} {:?}", other, path);
                        false
                    }
                },
            })
            // get display names and add to the tuple
            .map(|(path, relative, target)| {
                let name = lnk::get_display_name(&path);
                (path, relative, target, name)
            })
            .collect();

        // declare new variable for deduplication
        vec.iter()
            // deduplicate .lnk files that are in the same relative path within the start menu
            .filter(|(path1, relative, _, name1)| {
                let (path2, _, _, name2) = vec
                    .iter()
                    .rfind(|(_, relative2, _, _)| relative2 == relative)
                    .unwrap();

                // if path == path2, then its the same lnk
                // if don't have the same name, then they are distinct
                path1 == path2 || name1 != name2
            })
            // construct index entries
            .map(|(path, relative, _, name)| StartMenuTarget {
                name: name.clone(),
                relative: relative.to_str().unwrap().to_owned(),
                lnk_path: path.clone(),
            })
            .collect()
    }
}

impl Index<StartMenuTarget> for StartMenuIndex {
    fn keys(&self, entry: &StartMenuTarget) -> Vec<String> {
        vec![
            entry
                .lnk_path
                .file_stem()
                .and_then(|os| os.to_str())
                .unwrap()
                .to_owned(),
            entry.name.clone(),
        ]
    }

    fn launch(&self, entry: &StartMenuTarget) -> Box<dyn Fn()> {
        let lnk_path = entry.lnk_path.clone();

        Box::new(move || {
            let op = "open".to_wide();
            let raw = lnk_path.to_wide();

            unsafe {
                ShellExecuteW(
                    std::ptr::null_mut(),
                    op.as_ptr(),
                    raw.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    1,
                );
            }
        })
    }

    fn details(&self, entry: &StartMenuTarget) -> String {
        entry.relative.clone()
    }

    fn display_icon(&self, entry: &StartMenuTarget) -> Option<image::DynamicImage> {
        crate::nonfatal(|| {
            let mut raw = vec![];
            File::open(&entry.lnk_path)?.read_to_end(&mut raw)?;
            let lnk = ShellLink::load(&raw);

            let data = match lnk::extract_ico(&lnk) {
                Some(data) => data,
                None => {
                    return Err(Error::new(
                        ErrorKind::NotFound,
                        format!("unable to extract icon for lnk {:?}", entry.lnk_path),
                    ))?
                }
            };

            Ok(image::load_from_memory_with_format(
                &data,
                ImageFormat::Ico,
            )?)
        })
    }
}
