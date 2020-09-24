use lazy_static::lazy_static;

use std::collections::HashMap;
use std::os::raw::c_int;
use std::ptr;
use std::sync::Mutex;

use winapi::shared::minwindef::{LPARAM, LRESULT, WPARAM};
use winapi::shared::windef::HHOOK;
use winapi::um::libloaderapi::GetModuleHandleA;
use winapi::um::winuser::{
    CallNextHookEx, SetWindowsHookExA, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, WH_KEYBOARD_LL,
};

lazy_static! {
    static ref GLOBALS: Mutex<HookGlobals> = Mutex::new(HookGlobals {
        id: 0,
        hook_id: ptr::null_mut(),
        hook_list: HashMap::new(),
    });
}

unsafe impl Send for HookGlobals {}
struct HookGlobals {
    id: usize,
    hook_id: HHOOK,
    hook_list: HashMap<usize, HookData>,
}

struct HookData {
    callback: Box<dyn FnMut(u32, &KBDLLHOOKSTRUCT)>,
}

pub struct HookHandle {
    id: usize,
}

impl Drop for HookHandle {
    fn drop(&mut self) {
        let mut globals = GLOBALS.lock().unwrap();
        globals.hook_list.remove(&self.id);

        if globals.hook_list.len() == 0 {
            unsafe { UnhookWindowsHookEx(globals.hook_id) };
        }
    }
}

unsafe extern "system" fn hook_callback(code: c_int, w: WPARAM, l: LPARAM) -> LRESULT {
    let mut globals = GLOBALS.lock().unwrap();

    for value in globals.hook_list.values_mut() {
        (value.callback)(w as _, &*(l as *const _));
    }

    CallNextHookEx(globals.hook_id, code, w, l)
}

pub fn set_hook<F>(callback: F) -> HookHandle
where
    F: 'static + FnMut(u32, &KBDLLHOOKSTRUCT),
{
    let mut globals = GLOBALS.lock().unwrap();

    if globals.hook_list.len() == 0 {
        unsafe {
            let hmod = GetModuleHandleA(ptr::null());
            globals.hook_id = SetWindowsHookExA(WH_KEYBOARD_LL, Some(hook_callback), hmod, 0);
        }
    }

    let id = globals.id;
    globals.id += 1;

    let callback = Box::new(callback);
    globals.hook_list.insert(id, HookData { callback });

    HookHandle { id }
}
