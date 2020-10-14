use std::ffi::OsStr;
use std::path::Path;

use winapi::{
    shared::windef::HWND, um::winuser::keybd_event, um::winuser::SetFocus,
    um::winuser::SetForegroundWindow,
};

mod read_dir_recursive;
pub use read_dir_recursive::*;

mod dllimport;
pub use dllimport::*;

mod ico;
pub use ico::*;

pub fn focus_window(hwnd: HWND) {
    unsafe {
        keybd_event(0x12, 0, 1, 0);

        SetForegroundWindow(hwnd);
        SetFocus(hwnd);

        keybd_event(0x12, 0, 3, 0);
    }
}

pub trait ToWide {
    fn to_wide(self) -> Vec<u16>;
}

impl<'a, T: ToUtf16<'a>> ToWide for T {
    fn to_wide(self) -> Vec<u16> {
        self.iter_chars().chain(std::iter::once(0)).collect()
    }
}

pub trait ToUtf16<'a> {
    type Iter: 'a + Iterator<Item = u16>;

    fn iter_chars(self) -> Self::Iter;
}

impl<'a> ToUtf16<'a> for &'a str {
    type Iter = std::str::EncodeUtf16<'a>;

    fn iter_chars(self) -> Self::Iter {
        self.encode_utf16()
    }
}

impl<'a> ToUtf16<'a> for &'a OsStr {
    type Iter = std::os::windows::ffi::EncodeWide<'a>;

    fn iter_chars(self) -> Self::Iter {
        use std::os::windows::ffi::OsStrExt;
        self.encode_wide()
    }
}

impl<'a> ToUtf16<'a> for &'a Path {
    type Iter = std::os::windows::ffi::EncodeWide<'a>;

    fn iter_chars(self) -> Self::Iter {
        self.as_os_str().iter_chars()
    }
}
