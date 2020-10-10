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

pub fn to_wstr<I: Iterator<Item = u16>>(src: I) -> Vec<u16> {
    src.chain(std::iter::once(0)).collect()
}
