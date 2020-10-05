#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate base64;
extern crate bitflags;
extern crate cef;
extern crate lazy_static;
extern crate percent_encoding;
extern crate serde;
extern crate serde_yaml;
extern crate winapi;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate flat;

use cef::{
    cef_execute_process, App, CefApp, CefCommandLine, CefMainArgs, CefRenderProcessHandler,
    CefSchemeOptions, CefSchemeRegistrar, CefString,
};
use winapi::um::libloaderapi::GetModuleHandleA;

mod browser;
mod render;

struct MyApp;
impl App for MyApp {
    fn on_before_command_line_processing(
        &mut self,
        _process_type: Option<&CefString>,
        command_line: CefCommandLine,
    ) -> () {
        command_line.append_switch(&"disable-extensions".into());

        // println!("{:?}", _process_type);
    }

    fn on_register_custom_schemes(&mut self, registrar: CefSchemeRegistrar) -> () {
        let options = CefSchemeOptions::SECURE
            | CefSchemeOptions::STANDARD
            | CefSchemeOptions::CORS_ENABLED
            | CefSchemeOptions::FETCH_ENABLED;

        let scheme_name = "app".into();
        registrar.add_custom_scheme(&scheme_name, options.into());
    }

    fn get_render_process_handler(&mut self) -> Option<CefRenderProcessHandler> {
        Some(CefRenderProcessHandler::new(
            render::MyRenderProcessHandler::new(),
        ))
    }
}

fn main() {
    // let x = render::search::Search::new();
    // x.search("valor".to_owned());
    // ntfs::test();
    // return;

    let hinstance = unsafe { GetModuleHandleA(std::ptr::null()) };
    let main_args = CefMainArgs::new(hinstance as _);

    let app = CefApp::new(MyApp);

    if cef_execute_process(&main_args, Some(app.clone()), None) >= 0 {
        return;
    }

    browser::main(main_args, app);
}

pub fn to_wstr<I: Iterator<Item = u16>>(src: I) -> Vec<u16> {
    src.chain(std::iter::once(0)).collect()
}
