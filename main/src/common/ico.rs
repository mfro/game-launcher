use std::{fs::File, io::prelude::*, path::Path};

use winapi::{
    shared::minwindef::HMODULE,
    um::libloaderapi::{
        EnumResourceNamesW, FindResourceW, FreeLibrary, LoadLibraryExW, LoadResource, LockResource,
        SizeofResource, LOAD_LIBRARY_AS_DATAFILE, LOAD_LIBRARY_AS_IMAGE_RESOURCE,
    },
};

use flat::prelude::*;

flat_data!(IconHeader);
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct IconHeader {
    pub _reserved: u16le,
    pub image_type: u16le,
    pub image_count: u16le,
}

flat_data!(IconImageHeader);
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct IconImageHeader {
    pub width: u8,
    pub height: u8,
    pub colors: u8,
    pub _reserved: u8,
    pub color_planes: u16le,
    pub bits_per_pixel: u16le,
    pub size: u32le,
}

pub fn extract_icons(path: &Path) -> std::io::Result<Vec<Vec<u8>>> {
    let mut magic = [0; 4];
    File::open(&path)?.read_exact(&mut magic)?;

    if magic == [0, 0, 1, 0] {
        let mut data = vec![];
        File::open(path)?.read_to_end(&mut data)?;
        Ok(vec![data])
    } else {
        let libpath: Vec<_> = path
            .to_str()
            .unwrap()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let libmodule = unsafe {
            LoadLibraryExW(
                libpath.as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE,
            )
        };

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

        let mut indexes: Vec<isize> = vec![];
        unsafe {
            unsafe extern "system" fn iter_fn(
                _module: HMODULE,
                _ty: *const u16,
                name: *mut u16,
                lparam: isize,
            ) -> i32 {
                let out = lparam as *mut Vec<isize>;
                (*out).push(name as isize);
                1
            }

            let lparam = &mut indexes as *mut _ as isize;
            EnumResourceNamesW(libmodule, 14 as _, Some(iter_fn), lparam);
        }

        let mut icons = vec![];
        for idx in indexes {
            let mut cursor = load_resource(libmodule, idx as _, 14).unwrap();

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

                let image = load_resource(libmodule, image_id as _, 3).unwrap();
                file_blob.extend_from_slice(image);
            }

            file_header.append(&mut file_blob);
            icons.push(file_header);
        }

        unsafe { FreeLibrary(libmodule) };

        Ok(icons)
    }
}
