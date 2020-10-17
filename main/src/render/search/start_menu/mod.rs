use std::{fs::File, io::Cursor, io::Error, io::ErrorKind, io::prelude::*, path::PathBuf};

use image::{DynamicImage, ico::IcoDecoder};
use winapi::um::shellapi::ShellExecuteW;

mod lnk;
use lnk::ShellLink;

use crate::common::{RecursiveSearch, ToWide};

use super::SearchProvider;

pub struct StartMenuProvider {}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
pub struct StartMenuTarget {
    name: String,
    relative: String,
    lnk_path: PathBuf,
}

impl StartMenuProvider {
    pub fn new() -> StartMenuProvider {
        StartMenuProvider {}
    }

    pub fn index(&self) -> Vec<StartMenuTarget> {
        let appdata = std::env::var("APPDATA").unwrap();
        let roots = [
            PathBuf::from(r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs"),
            PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs"),
        ];

        let mut vec: Vec<_> = roots
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
                let ext = path.extension().and_then(|x| x.to_str())?;
                match ext {
                    "lnk" => Some((path, relative)),
                    "ini" | "url" => None,
                    _ => {
                        println!("unknown start menu entry: {:?}", relative);
                        None
                    }
                }
            })
            // open and parse the .lnk files
            .filter_map(|(path, relative)| {
                let target = crate::attempt!(("resolve lnk {:?}", path), {
                    let mut raw = vec![];
                    File::open(&path)?.read_to_end(&mut raw)?;
                    let lnk = ShellLink::load(&raw);

                    let target = lnk::resolve(&lnk)?;
                    PathBuf::from(target)
                });

                Some((path, relative, target))
            })
            // select only .lnk files that point to 'exe', 'msc', 'url' files
            .filter(|(path, _, target)| match target {
                None => true, // allow links that couldn't be resolved
                Some(target) => match target.extension().and_then(|x| x.to_str()) {
                    None => false,
                    Some(ext) => match ext {
                        "exe" | "msc" => true,
                        "url" | "chm" | "txt" | "rtf" | "pdf" | "html" | "ini" => false,
                        ext => {
                            println!("Unknown lnk target extension: {} {:?}", ext, path);
                            true
                        }
                    },
                },
            })
            // get display names and add to the tuple
            .map(|(path, relative, target)| {
                let name = lnk::get_display_name(&path);
                (path, relative, target, name)
            })
            .collect();

        for i in (0..vec.len()).rev() {
            // deduplicate .lnk files that either:
            //   - are in the same relative path within the start menu
            //   - have the same target and name

            let a = &vec[i];
            let other = vec[..i]
                .iter()
                .find(|b| a.1 == b.1 || (a.2 == b.2 && a.3 == b.3))
                .is_some();

            if other {
                vec.remove(i);
            }
        }

        // declare new variable for deduplication
        vec.into_iter()
            // construct index entries
            .map(|(lnk_path, relative, _, name)| StartMenuTarget {
                name,
                lnk_path,
                relative: relative.to_str().unwrap().to_owned(),
            })
            .collect()
    }
}

impl SearchProvider<StartMenuTarget> for StartMenuProvider {
    fn keys(&self, entry: &StartMenuTarget) -> Vec<String> {
        let relative = match entry.relative.rfind('.') {
            Some(i) => entry.relative[..i].to_owned(),
            None => entry.relative.clone(),
        };

        vec![
            entry
                .lnk_path
                .file_stem()
                .and_then(|os| os.to_str())
                .unwrap()
                .to_owned(),
            entry.name.clone(),
            relative,
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
        crate::attempt!(("get lnk icon {:?}", entry.lnk_path), {
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

            let r = Cursor::new(&data);
            let decoder = IcoDecoder::new_unchecked(r)?;
            DynamicImage::from_decoder(decoder)?
            // image::load_from_memory_with_format(&data, ImageFormat::Ico)?
        })
    }
}
