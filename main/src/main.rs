#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate bitflags;
extern crate cef;
extern crate image;
extern crate lazy_static;
extern crate percent_encoding;
extern crate quick_xml;
extern crate serde;
extern crate serde_yaml;
extern crate winapi;
extern crate winrt;

#[macro_use]
extern crate com;

#[macro_use]
extern crate flat;

#[macro_use]
extern crate serde_derive;

use std::{
    fmt::Debug, fs::OpenOptions, io::prelude::*, io::Error, io::ErrorKind, path::Path,
    path::PathBuf,
};

use backtrace::Backtrace;
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

    let x = log_path().unwrap();
    if x.exists() {
        std::fs::remove_file(x).unwrap();
    }

    let hinstance = unsafe { GetModuleHandleA(std::ptr::null()) };
    let main_args = CefMainArgs::new(hinstance as _);

    let app = CefApp::new(MyApp);

    if cef_execute_process(&main_args, Some(app.clone()), None) >= 0 {
        return;
    }

    browser::main(main_args, app);
}

fn log_path() -> std::io::Result<PathBuf> {
    let log_dir = if Path::new("config.yaml").exists() {
        std::env::current_dir()?
    } else {
        match std::env::current_exe()?.parent() {
            Some(p) => p.to_owned(),
            None => return Err(Error::new(ErrorKind::NotFound, "?")),
        }
    };

    Ok(log_dir.join("error.log"))
}

#[macro_export]
macro_rules! log {
    ( $($arg:tt)* ) => {
        crate::log(&format!( $( $arg )* ))
    };
}

pub(crate) fn log(a: &str) {
    fn log(e: &str) -> std::io::Result<()> {
        let log_path = log_path()?;

        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&log_path)?;

        writeln!(file, "{}", e)?;

        Ok(())
    }

    let _ = log(a);
}

#[macro_export]
macro_rules! attempt {
    // ( $action:expr ) => {
    //     crate::attempt(None, || Ok($action))
    // };

    ( ( $($arg:tt)* ), $action:expr ) => {
        crate::attempt(Some(|| format!( $( $arg )* )), || Ok($action))
    };
}

pub(crate) fn attempt<T, F, F2>(describe: Option<F2>, f: F) -> Option<T>
where
    F: FnOnce() -> Result<T, MyError>,
    F2: FnOnce() -> String,
{
    match f() {
        Ok(v) => Some(v),
        Err(e) => {
            if let Some(describe) = describe {
                log(&format!("{}\n{:?}\n{:?}", describe(), e.inner, e.trace));
            }

            None
        }
    }
}

pub(crate) struct MyError {
    inner: Box<dyn Debug>,
    trace: Backtrace,
}

impl<T: 'static + Debug> From<T> for MyError {
    fn from(src: T) -> Self {
        let inner = Box::new(src);
        let trace = Backtrace::new();
        MyError { inner, trace }
    }
}

#[macro_export]
macro_rules! mark {
    ( $format:tt, $name:ident ) => {
        crate::mark!($format, $name,);
    };

    ( $format:tt, $name:ident, $($arg:tt)* ) => {
        let $name = {
            let tmp = ::std::time::Instant::now();
            println!($format, tmp - $name, $( $arg )* );
            tmp
        };
    };
}
