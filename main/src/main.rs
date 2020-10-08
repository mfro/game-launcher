#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate base64;
extern crate bitflags;
extern crate cef;
extern crate image;
extern crate lazy_static;
extern crate mime_guess;
extern crate percent_encoding;
extern crate quick_xml;
extern crate serde;
extern crate serde_yaml;
extern crate winapi;
extern crate winrt;

#[macro_use]
extern crate com;

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate flat;

use cef::{
    cef_execute_process, App, CefApp, CefCommandLine, CefMainArgs, CefRenderProcessHandler,
    CefSchemeOptions, CefSchemeRegistrar, CefString,
};
use winapi::um::libloaderapi::GetModuleHandleA;

pub mod common;

mod browser;
mod render;

mod bindings {
    include!(concat!(env!("OUT_DIR"), "/winrt.rs"));
}

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
    unsafe { winapi::um::objbase::CoInitialize(std::ptr::null_mut()) };

    // render::search::appx::index();
    // let x = render::search::Search::new();
    // x.search("valor".to_owned());
    // ntfs::test();

    let hinstance = unsafe { GetModuleHandleA(std::ptr::null()) };
    let main_args = CefMainArgs::new(hinstance as _);

    let app = CefApp::new(MyApp);

    if cef_execute_process(&main_args, Some(app.clone()), None) >= 0 {
        return;
    }

    browser::main(main_args, app);
}

pub fn nonfatal<T, F>(f: F) -> Option<T>
where
    F: FnOnce() -> Result<T, MyError>,
{
    match f() {
        Ok(v) => Some(v),
        Err(e) => {
            println!("{:?}", e.inner);
            None
        }
    }
}

pub struct MyError {
    inner: Box<dyn std::fmt::Debug>,
}

impl From<std::io::Error> for MyError {
    fn from(src: std::io::Error) -> Self {
        let inner = Box::new(src);
        MyError { inner }
    }
}

impl From<winrt::Error> for MyError {
    fn from(src: winrt::Error) -> Self {
        let inner = Box::new(src);
        MyError { inner }
    }
}

impl From<serde_yaml::Error> for MyError {
    fn from(src: serde_yaml::Error) -> Self {
        let inner = Box::new(src);
        MyError { inner }
    }
}

impl From<quick_xml::DeError> for MyError {
    fn from(src: quick_xml::DeError) -> Self {
        let inner = Box::new(src);
        MyError { inner }
    }
}

impl From<image::ImageError> for MyError {
    fn from(src: image::ImageError) -> Self {
        let inner = Box::new(src);
        MyError { inner }
    }
}
