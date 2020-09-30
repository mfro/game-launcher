use std::{fs::File, io::Read, path::Path};

use bitflags::bitflags;
use winapi::{
    shared::minwindef::HMODULE,
    shared::minwindef::MAX_PATH,
    um::{
        libloaderapi::EnumResourceNamesW, libloaderapi::FindResourceW, libloaderapi::FreeLibrary,
        libloaderapi::LoadLibraryExW, libloaderapi::LoadResource, libloaderapi::LockResource,
        libloaderapi::SizeofResource, libloaderapi::LOAD_LIBRARY_AS_DATAFILE,
        libloaderapi::LOAD_LIBRARY_AS_IMAGE_RESOURCE, processenv::ExpandEnvironmentStringsW,
        shellapi::SHGetFileInfoW, shellapi::SHFILEINFOW, shellapi::SHGFI_DISPLAYNAME,
        shlobj::SHGetPathFromIDListW,
    },
};

use crate::flat_data::{num::*, FlatDataImpl, LoadExt, StoreExt};

flat_data!(IconHeader);
#[repr(packed)]
#[derive(Copy, Clone)]
pub struct IconHeader {
    pub _reserved: u16le,
    pub image_type: u16le,
    pub image_count: u16le,
}

flat_data!(IconImageHeader);
#[repr(packed)]
#[derive(Copy, Clone)]
pub struct IconImageHeader {
    pub width: u8,
    pub height: u8,
    pub colors: u8,
    pub _reserved: u8,
    pub color_planes: u16,
    pub bits_per_pixel: u16,
    pub size: u32,
}

pub struct ShellLink {
    pub link_flags: LinkFlags,
    pub file_attributes: u32,
    pub creation_time: u64,
    pub access_time: u64,
    pub write_time: u64,
    pub file_size: u32,
    pub icon_index: i32,
    pub show_command: u32,
    pub hotkey: u16,

    pub link_target_id_list: Option<Vec<u8>>,
    pub link_info: Option<LinkInfo>,

    pub name: Option<String>,
    pub relative_path: Option<String>,
    pub working_dir: Option<String>,
    pub command_line_arguments: Option<String>,
    pub icon_location: Option<String>,

    pub environment_variable_data: Option<String>,
    pub icon_environment_data: Option<String>,

    pub extra_data: Vec<(u32, Vec<u8>)>,
}

pub fn expand_environment_data(value: &str) -> String {
    let utf16: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    let mut value = [0; 1024];

    let len = unsafe {
        ExpandEnvironmentStringsW(utf16.as_ptr(), value.as_mut_ptr(), value.len() as u32) - 1
    };

    String::from_utf16(&value[0..len as usize]).unwrap()
}

pub fn resolve(lnk: &ShellLink) -> Option<String> {
    if let Some(env_data) = &lnk.environment_variable_data {
        Some(expand_environment_data(env_data))
    } else if let Some(target) = &lnk.link_target_id_list {
        let pidlist = target.as_ptr() as _;
        let mut value = [0; 1024];
        let result = unsafe { SHGetPathFromIDListW(pidlist, value.as_mut_ptr()) };
        if result == 1 {
            let len = value.iter().position(|x| *x == 0).unwrap();
            Some(String::from_utf16(&value[0..len]).unwrap())
        } else {
            None
        }
    } else {
        todo!("shell link without EnvironmentVariableDataBlock or LinkTargetIDList");
    }
}

pub fn extract_ico(lnk: &ShellLink) -> Option<Vec<u8>> {
    let icon_path = if let Some(icon_path) = &lnk.icon_location {
        // println!("icon A");
        expand_environment_data(icon_path)
    } else if let Some(env_data) = &lnk.icon_environment_data {
        // println!("icon B");
        expand_environment_data(env_data)
    } else {
        // println!("icon C");
        resolve(lnk)?
    };

    if icon_path.ends_with(".ico") {
        let mut data = vec![];
        File::open(icon_path)
            .unwrap()
            .read_to_end(&mut data)
            .unwrap();

        Some(data)
    } else {
        let libpath: Vec<_> = icon_path.encode_utf16().chain(std::iter::once(0)).collect();
        let libmodule = unsafe {
            LoadLibraryExW(
                libpath.as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE,
            )
        };

        unsafe extern "system" fn iter_fn(
            _module: HMODULE,
            _ty: *const u16,
            name: *mut u16,
            arg: isize,
        ) -> i32 {
            let counter = arg as *mut isize;
            // println!("enum {} {:?}", *counter, name);

            if *counter == 0 {
                *counter = name as isize;
                0
            } else {
                *counter -= 1;
                1
            }
        }

        fn load_resource(module: HMODULE, name: *const u16, ty: u16) -> Option<&'static [u8]> {
            unsafe {
                let resource = FindResourceW(module, name, ty as _);
                if resource.is_null() {
                    return None;
                }

                let size = SizeofResource(module, resource);
                let handle = LoadResource(module, resource);
                let resource_raw = LockResource(handle);

                Some(std::slice::from_raw_parts(
                    resource_raw as *mut u8,
                    size as usize,
                ))
            }
        }

        let mut icon_id = lnk.icon_index as isize;
        icon_id *= icon_id.signum();

        // println!("{} {:?} {}", icon_path, libmodule, icon_id);
        let mut cursor = match load_resource(libmodule, icon_id as _, 14) {
            Some(slice) => slice,
            None => {
                unsafe {
                    let lparam = &mut icon_id as *mut _ as _;
                    EnumResourceNamesW(libmodule, 14 as _, Some(iter_fn), lparam);
                }

                load_resource(libmodule, icon_id as _, 14)?
            }
        };

        let header: &IconHeader = cursor.load();
        let mut file_header = vec![];
        let mut file_blob = vec![];
        file_header.store(header);

        let image_count = header.image_count.get();
        for _ in 0..image_count {
            let image_header: &IconImageHeader = cursor.load();
            let image_id = cursor.load::<u16le>().get();
            file_header.store(image_header);
            file_header.store(6 + 16 * image_count as u32 + file_blob.len() as u32);

            let image = load_resource(libmodule, image_id as _, 3)?;
            file_blob.extend_from_slice(image);
        }

        unsafe { FreeLibrary(libmodule) };

        file_header.append(&mut file_blob);
        Some(file_header)
    }
}

pub fn get_display_name<P: AsRef<Path>>(path: P) -> String {
    let mut file_info = SHFILEINFOW {
        hIcon: std::ptr::null_mut(),
        iIcon: 0,
        dwAttributes: 0,
        szDisplayName: [0; MAX_PATH],
        szTypeName: [0; 80],
    };

    use std::os::windows::ffi::OsStrExt;
    let os_path: Vec<u16> = path
        .as_ref()
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        SHGetFileInfoW(
            os_path.as_ptr(),
            0,
            &mut file_info,
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_DISPLAYNAME,
        );
    }

    let strlen = file_info
        .szDisplayName
        .iter()
        .position(|x| *x == 0)
        .unwrap();

    String::from_utf16(&file_info.szDisplayName[..strlen]).unwrap()
}

impl ShellLink {
    fn load_string_data(cursor: &mut &[u8]) -> String {
        let len = cursor.load::<u16le>().get();
        let chars: Vec<u16> = (0..len).map(|_| cursor.load()).collect();
        String::from_utf16(&chars).unwrap()
    }

    pub fn load(src: &[u8]) -> ShellLink {
        let mut cursor = src;
        let cursor = &mut cursor;
        let header: &ShellLinkHeader = cursor.load();

        let link_flags = match LinkFlags::from_bits(header.link_flags.get()) {
            Some(flags) => flags,
            None => panic!("invalid flags: {:x}", header.link_flags.get()),
        };

        let file_attributes = header.file_attributes.get();
        let creation_time = header.creation_time.get();
        let access_time = header.access_time.get();
        let write_time = header.write_time.get();
        let file_size = header.file_size.get();
        let icon_index = header.icon_index.get();
        let show_command = header.show_command.get();
        let hotkey = header.hotkey.get();

        let link_target_id_list = if link_flags.contains(LinkFlags::HAS_LINK_TARGET_ID_LIST) {
            let size = cursor.load::<u16le>().get() as usize;
            let data = &cursor[0..size];
            *cursor = &cursor[size..];
            Some(data.to_vec())
        } else {
            None
        };

        let link_info = if link_flags.contains(LinkFlags::HAS_LINK_INFO) {
            let size = u32le::load(cursor).get() as usize;
            // let data = &cursor[0..size];
            *cursor = &cursor[size..];

            None
        // let content = &cursor[..size];
        // *cursor = &cursor[4..];

        // let header_size = cursor.load::<u32le>().get();
        // let flags = cursor.load::<u32le>().get();

        // let volume_id = if (flags & 1) == 1 {
        //     let offset = cursor.load::<u32le>().get();
        //     let data =
        // } else {
        //     *cursor = &cursor[4..];
        //     None
        // }

        // Some(data.to_vec())
        } else {
            None
        };

        let name = match link_flags.contains(LinkFlags::HAS_NAME) {
            true => Some(Self::load_string_data(cursor)),
            false => None,
        };

        let relative_path = match link_flags.contains(LinkFlags::HAS_RELATIVE_PATH) {
            true => Some(Self::load_string_data(cursor)),
            false => None,
        };

        let working_dir = match link_flags.contains(LinkFlags::HAS_WORKING_DIR) {
            true => Some(Self::load_string_data(cursor)),
            false => None,
        };

        let command_line_arguments = match link_flags.contains(LinkFlags::HAS_ARGUMENTS) {
            true => Some(Self::load_string_data(cursor)),
            false => None,
        };

        let icon_location = match link_flags.contains(LinkFlags::HAS_ICON_LOCATION) {
            true => Some(Self::load_string_data(cursor)),
            false => None,
        };

        let mut environment_variable_data = None;
        let mut icon_environment_data = None;

        let mut extra_data = vec![];
        loop {
            let size = cursor.load::<u32le>().get() as usize;
            if size < 4 {
                break;
            }

            let sig = cursor.load::<u32le>().get();
            match sig {
                0xa000_0001 => {
                    assert_eq!(size, 0x314);
                    *cursor = &cursor[260..];
                    let utf16: Vec<u16> = (0..260).map(|_| cursor.load()).collect();
                    let strlen = utf16.iter().position(|x| *x == 0).unwrap();
                    let value = String::from_utf16(&utf16[..strlen]).unwrap();
                    environment_variable_data = Some(value);
                }
                0xa000_0007 => {
                    assert_eq!(size, 0x314);
                    *cursor = &cursor[260..];
                    let utf16: Vec<u16> = (0..260).map(|_| cursor.load()).collect();
                    let strlen = utf16.iter().position(|x| *x == 0).unwrap();
                    let value = String::from_utf16(&utf16[..strlen]).unwrap();
                    icon_environment_data = Some(value);
                }
                _ => {
                    let data = &cursor[..size - 8];
                    *cursor = &cursor[size - 8..];
                    extra_data.push((sig, data.to_vec()))
                }
            }
        }

        ShellLink {
            link_flags,
            file_attributes,
            creation_time,
            access_time,
            write_time,
            file_size,
            icon_index,
            show_command,
            hotkey,
            link_target_id_list,
            link_info,
            name,
            relative_path,
            working_dir,
            command_line_arguments,
            icon_location,
            environment_variable_data,
            icon_environment_data,
            extra_data,
        }
    }
}

flat_data!(ShellLinkHeader);
#[repr(packed)]
#[derive(Copy, Clone)]
pub struct ShellLinkHeader {
    pub header_size: u32le,
    pub lnk_clsid: [u32le; 4],
    pub link_flags: u32le,
    pub file_attributes: u32le,
    pub creation_time: u64le,
    pub access_time: u64le,
    pub write_time: u64le,
    pub file_size: u32le,
    pub icon_index: i32le,
    pub show_command: u32le,
    pub hotkey: u16le,
    pub _reserved1: u16le,
    pub _reserved2: u32le,
    pub _reserved3: u32le,
}

bitflags! {
    pub struct LinkFlags: u32 {
        const HAS_LINK_TARGET_ID_LIST = 1 << 0;
        const HAS_LINK_INFO = 1 << 1;
        const HAS_NAME = 1 << 2;
        const HAS_RELATIVE_PATH = 1 << 3;
        const HAS_WORKING_DIR = 1 << 4;
        const HAS_ARGUMENTS = 1 << 5;
        const HAS_ICON_LOCATION = 1 << 6;
        const IS_UNICODE = 1 << 7;
        const FORCE_NO_LINK_INFO = 1 << 8;
        const HAS_EXP_STRING = 1 << 9;
        const RUN_IN_SEPARATE_PROCESS = 1 << 10;
        // const UNUSED1 = 1 << 11;
        const HAS_DARWIN_ID = 1 << 12;
        const RUN_AS_USER = 1 << 13;
        const HAS_EXP_ICON = 1 << 14;
        const NO_PIDL_ALIAS = 1 << 15;
        // const UNUSED2 = 1 << 16;
        const RUN_WITH_SHIM_LAYER = 1 << 17;
        const FORCE_NO_LINK_TRACK = 1 << 18;
        const ENABLE_TARGET_METADATA = 1 << 19;
        const DISABLE_LINK_PATH_TRACKING = 1 << 20;
        const DISABLE_KNOWN_FOLDER_TRACKING = 1 << 21;
        const DISABLE_KNOWN_FOLDER_ALIAS = 1 << 22;
        const ALLOW_LINK_TO_LINK = 1 << 23;
        const UNALIAS_ON_SAVE = 1 << 24;
        const PREFER_ENVIRONMENT_PATH = 1 << 25;
        const KEEP_LOCAL_ID_LIST_FOR_UNC_TARGET = 1 << 26;
    }
}

pub struct LinkInfo {
    pub flags: u32,
    pub volume_id: Option<String>,
    pub local_base_path: Option<String>,
    pub common_network_relative_link: Option<String>,
    pub common_path_suffix: String,
    pub local_base_path_unicode: Option<String>,
    pub common_path_suffix_unicode: Option<String>,
}
