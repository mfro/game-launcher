use std::io::{ErrorKind, Result};
use std::{
    fs::File,
    path::{Path, PathBuf},
};
use std::{
    io::{prelude::*, Error},
    process::Command,
};

use winapi::um::shellapi::ShellExecuteW;

use crate::lnk::{self, ShellLink};

#[derive(Serialize, Deserialize, Debug)]
pub struct Custom {
    names: Vec<String>,
    target: Vec<String>,
    icon: Option<String>,
}

pub enum Lunchable {
    Custom(Custom),
    ShellLink(PathBuf),
}

fn to_wstr<I: Iterator<Item = u16>>(src: I) -> Vec<u16> {
    src.chain(std::iter::once(0)).collect()
}

impl Lunchable {
    pub fn display_name(&self) -> String {
        match self {
            Lunchable::Custom(c) => c.names[0].to_string(),
            Lunchable::ShellLink(path) => lnk::get_display_name(path),
        }
    }

    pub fn keys(&self) -> Vec<String> {
        match self {
            Lunchable::Custom(c) => c.names.clone(),
            Lunchable::ShellLink(path) => vec![
                path.file_stem()
                    .and_then(|os| os.to_str())
                    .unwrap()
                    .to_owned(),
                lnk::get_display_name(&path),
            ],
        }
    }

    pub fn icon<P: AsRef<Path>>(&self, context: P) -> Result<Vec<u8>> {
        match self {
            Lunchable::Custom(c) => {
                let path = match &c.icon {
                    Some(name) => context.as_ref().join(name),
                    None => context.as_ref().join(&c.names[0]),
                };
                let mut data = vec![];
                File::open(path)?.read_to_end(&mut data)?;
                Ok(data)
            }
            Lunchable::ShellLink(path) => {
                let mut content = vec![];
                File::open(&path)?.read_to_end(&mut content)?;
                let lnk = ShellLink::load(&content);

                lnk::extract_ico(&lnk).map(|x| Ok(x)).unwrap_or_else(|| {
                    Err(Error::new(
                        ErrorKind::NotFound,
                        "unable to extract icon for lnk",
                    ))
                })
            }
        }
    }

    pub fn launch(&self) {
        match self {
            Lunchable::Custom(c) => {
                Command::new(&c.target[0])
                    .args(&c.target[1..]) //
                    .spawn()
                    .unwrap();
            }
            Lunchable::ShellLink(path) => {
                use std::os::windows::ffi::OsStrExt;

                let op = to_wstr("open".encode_utf16());
                let raw = to_wstr(path.as_os_str().encode_wide());

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
            }
        }
    }
}
