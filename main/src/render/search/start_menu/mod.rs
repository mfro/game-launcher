use std::{
    fs::DirEntry, fs::File, fs::ReadDir, io::prelude::*, io::Error, io::ErrorKind, path::Path,
    path::PathBuf,
};

use winapi::um::shellapi::ShellExecuteW;

mod lnk;
use lnk::ShellLink;

use super::{icon_helper, IndexEntry, LaunchTarget};

struct RecursiveSearch {
    stack: Vec<ReadDir>,
}

impl RecursiveSearch {
    pub fn new<P: AsRef<Path>>(path: &P) -> RecursiveSearch {
        let stack = match std::fs::read_dir(path) {
            Ok(iter) => vec![iter],
            Err(_) => vec![],
        };

        RecursiveSearch { stack }
    }
}

impl Iterator for RecursiveSearch {
    type Item = DirEntry;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let iter = match self.stack.last_mut() {
                Some(iter) => iter,
                None => break None,
            };

            let entry = match iter.next() {
                Some(result) => match result {
                    Err(_) => continue,
                    Ok(entry) => entry,
                },
                None => {
                    self.stack.pop();
                    continue;
                }
            };

            let ty = match entry.file_type() {
                Ok(ty) => ty,
                Err(_) => continue,
            };

            if ty.is_file() {
                break Some(entry);
            }

            if ty.is_dir() {
                match std::fs::read_dir(&entry.path()) {
                    Ok(iter) => {
                        self.stack.push(iter);
                        continue;
                    }
                    Err(_) => continue,
                }
            };
        }
    }
}

pub fn index() -> impl Iterator<Item = (IndexEntry, LaunchTarget)> {
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
                    Some("lnk") => Some(path),
                    Some("ini") | Some("url") => None,
                    _ => {
                        println!("unknown start menu entry: {:?}", relative);
                        None
                    }
                },
            }
        })
        // open and parse the .lnk files
        .filter_map(|path| {
            let mut raw = vec![];
            File::open(&path).unwrap().read_to_end(&mut raw).unwrap();
            let lnk = ShellLink::load(&raw);
            let target = lnk::resolve(&lnk)?;
            Some((path, target))
        })
        // select only .lnk files that point to 'exe', 'msc', 'url' files
        .filter(|(path, target)| match target.rfind('.') {
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
        .map(|(path, target)| {
            let name = lnk::get_display_name(&path);
            (path, target, name)
        })
        .collect();

    // declare new variable for deduplication
    let vec: Vec<_> = vec
        .iter()
        // deduplicate .lnk files that point to the same target and have the same name
        .filter(|(path1, target1, name1)| {
            let (path2, _, name2) = vec
                .iter()
                .rfind(|(_, target2, _)| target2 == target1)
                .unwrap();

            // if path == path2, then its the same lnk
            // if don't have the same name, then they are distinct
            path1 == path2 || name1 != name2
        })
        // construct index entries
        .map(|(path, _, name)| {
            let keys = vec![
                path.file_stem()
                    .and_then(|os| os.to_str())
                    .unwrap()
                    .to_owned(),
                name.clone(),
            ];

            let display_name = name.clone();
            let display_icon = icon_helper(|| {
                let mut raw = vec![];
                File::open(&path)?.read_to_end(&mut raw)?;
                let lnk = ShellLink::load(&raw);

                let data = match lnk::extract_ico(&lnk) {
                    Some(data) => data,
                    None => {
                        return Err(Error::new(
                            ErrorKind::NotFound,
                            "unable to extract icon for lnk",
                        ))
                    }
                };

                Ok(("image/x-icon".parse().unwrap(), data))
            });

            let path = path.clone();
            let launch = Box::new(move || {
                use std::os::windows::ffi::OsStrExt;

                let op = crate::to_wstr("open".encode_utf16());
                let raw = crate::to_wstr(path.as_os_str().encode_wide());

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
            });

            let index = IndexEntry::new(keys.into_iter());

            let target = LaunchTarget {
                display_name,
                display_icon,
                launch,
            };

            (index, target)
        })
        .collect();

    vec.into_iter()
}
