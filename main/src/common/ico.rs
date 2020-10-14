use std::{fs::File, io::prelude::*, marker::PhantomData, path::Path};

use winapi::{
    shared::minwindef::HMODULE,
    um::libloaderapi::{
        EnumResourceNamesW, FindResourceW, FreeLibrary, LoadLibraryExW, LoadResource, LockResource,
        SizeofResource, LOAD_LIBRARY_AS_DATAFILE, LOAD_LIBRARY_AS_IMAGE_RESOURCE,
    },
};

use flat::prelude::*;

use super::ToWide;

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

struct ResourceLibrary<'a> {
    handle: HMODULE,
    _lifetime: PhantomData<&'a ()>,
}

impl Drop for ResourceLibrary<'_> {
    fn drop(&mut self) {
        unsafe {
            FreeLibrary(self.handle);
        }
    }
}

impl<'a> ResourceLibrary<'a> {
    pub fn load<P: ToWide>(path: P) -> ResourceLibrary<'a> {
        let path = path.to_wide();

        let handle = unsafe {
            LoadLibraryExW(
                path.as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_AS_DATAFILE | LOAD_LIBRARY_AS_IMAGE_RESOURCE,
            )
        };

        let _lifetime = Default::default();

        ResourceLibrary { handle, _lifetime }
    }

    pub fn load_resource(&self, name: *const u16, ty: u16) -> Option<&'a [u8]> {
        if self.handle.is_null() {
            return None;
        }

        let slice = unsafe {
            let resource = FindResourceW(self.handle, name, ty as _);
            if resource.is_null() {
                return None;
            }

            let len = SizeofResource(self.handle, resource);
            let rsrc = LoadResource(self.handle, resource);
            let pointer = LockResource(rsrc);

            std::slice::from_raw_parts(pointer as *mut u8, len as usize)
        };

        Some(slice)
    }
}

pub fn extract_icons(path: &Path) -> std::io::Result<Vec<Vec<u8>>> {
    let mut magic = [0; 4];
    File::open(&path)?.read_exact(&mut magic)?;

    if magic == [0, 0, 1, 0] {
        let mut data = vec![];
        File::open(path)?.read_to_end(&mut data)?;
        Ok(vec![data])
    } else {
        struct LParam<'a> {
            module: ResourceLibrary<'a>,
            icons: Vec<Vec<u8>>,
        }

        unsafe extern "system" fn iter_fn(
            libmodule: HMODULE,
            _ty: *const u16,
            name: *mut u16,
            lparam: isize,
        ) -> i32 {
            let state = &mut *(lparam as *mut LParam);

            let mut cursor = match state.module.load_resource(name, 14) {
                Some(x) => x,
                None => panic!("{:?} {:?}", libmodule, name),
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

                let image = state.module.load_resource(image_id as _, 3).unwrap();
                file_blob.extend_from_slice(image);
            }

            file_header.append(&mut file_blob);
            state.icons.push(file_header);

            1
        }

        let mut state = LParam {
            module: ResourceLibrary::load(path),
            icons: vec![],
        };

        unsafe {
            let lparam = &mut state as *mut _ as isize;
            EnumResourceNamesW(state.module.handle, 14 as _, Some(iter_fn), lparam);
        }

        Ok(state.icons)
    }
}
