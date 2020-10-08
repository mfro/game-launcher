use std::io::{Error, Result};
use winapi::{
    shared::minwindef::HMODULE, um::errhandlingapi::GetLastError, um::libloaderapi::GetProcAddress,
    um::libloaderapi::LoadLibraryW,
};

unsafe impl Sync for Dll {}

#[derive(Copy, Clone)]
pub struct Dll {
    handle: HMODULE,
}

impl Dll {
    pub fn load<S: AsRef<str>>(name: S) -> Result<Dll> {
        let raw = crate::common::to_wstr(name.as_ref().encode_utf16());
        let handle = unsafe { LoadLibraryW(raw.as_ptr()) };
        if handle.is_null() {
            let error = unsafe { GetLastError() };
            Err(Error::from_raw_os_error(error as i32))
        } else {
            Ok(Dll { handle })
        }
    }

    pub fn get_function<S: AsRef<str>>(&self, name: S) -> Result<*const ()> {
        let raw: Vec<_> = name
            .as_ref()
            .as_bytes()
            .iter()
            .cloned()
            .chain(std::iter::once(0))
            .collect();
        let addr = unsafe { GetProcAddress(self.handle, raw.as_ptr() as *const i8) };
        if addr.is_null() {
            let error = unsafe { GetLastError() };
            Err(Error::from_raw_os_error(error as i32))
        } else {
            Ok(addr as *const ())
        }
    }
}
