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

use cef::{
    cef_execute_process, App, CefApp, CefCommandLine, CefMainArgs, CefRenderProcessHandler,
    CefSchemeOptions, CefSchemeRegistrar, CefString,
};
use winapi::um::libloaderapi::GetModuleHandleA;

#[macro_use]
pub mod flat_data;
mod hook;
mod index;
mod lnk;

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

/*

#[derive(Default)]
struct AttachFunction;
impl V8Handler for AttachFunction {
    fn execute(
        &mut self,
        _name: &CefString,
        _object: CefV8Value,
        arguments: &[CefV8Value],
        _retval: &mut Option<CefV8Value>,
        exception: &mut CefString,
    ) -> bool {
        if arguments.len() != 2 {
            *exception = "invalid arguments".into();
        } else if !arguments[0].is_function() || !arguments[1].is_function() {
            *exception = "invalid arguments".into();
        } else {
            HOOK_CALLBACKS.with(|c| {
                c.borrow_mut().push(arguments[1].clone());
            });
            CONFIG_CALLBACKS.with(|c| {
                c.borrow_mut().push(arguments[0].clone());
            });

            if INDEX.lock().unwrap().borrow().is_empty() {
                build_index();
            }

            let start = std::time::Instant::now();

            let v8: Vec<_> = INDEX
                .lock()
                .unwrap()
                .borrow()
                .iter()
                .enumerate()
                .map(|(i, target)| V8Lunchable::new(i, target).into())
                .collect();

            let end = std::time::Instant::now();

            println!("{:?}", end - start);

            let array = make_v8_array(v8.into_iter());
            // let content = String::from_utf8(content).unwrap();
            // let content = content.as_str().into();
            // let content = CefV8Value::create_string(Some(&content)).unwrap();

            // let index = make_index();
            // let array = CefV8Value::create_array(index.len() as i32).unwrap();
            // for (i, (path, relative)) in index.iter().enumerate() {
            //     let name1 = path.file_stem().and_then(|os| os.to_str()).unwrap();
            //     let name2 = lnk::get_display_name(&path);

            //     let relative = match relative.parent() {
            //         Some(p) => p.join(relative.file_stem().unwrap()),
            //         None => relative.file_stem().unwrap().into(),
            //     };
            //     let relative = relative
            //         .iter()
            //         .map(|os| os.to_str().unwrap())
            //         .collect::<Vec<&str>>()
            //         .join("/");

            //     let mut names: Vec<CefString> = vec![];
            //     // names.push(relative.as_str().into());
            //     names.push(name1.into());

            //     if name1 != name2 {
            //         names.push(name2.as_str().into());
            //     }

            //     let names: Vec<CefV8Value> = names.iter().map(|x| x.into()).collect();
            //     let names = make_v8_array(&names);
            //     let path = CefV8Value::create_string(Some(&path.to_str().unwrap().into())).unwrap();

            //     let object = CefV8Value::create_object(None, None).unwrap();
            //     object.set_value_bykey(Some(&"names".into()), names, CefV8Propertyattribute::NONE);

            //     object.set_value_bykey(Some(&"path".into()), path, CefV8Propertyattribute::NONE);
            //     array.set_value_byindex(i as i32, object);
            // }

            arguments[0].execute_function(None, &[array]);
        }

        true
    }
}

struct ToggleFunction(CefFrame);
impl V8Handler for ToggleFunction {
    fn execute(
        &mut self,
        _name: &CefString,
        _object: CefV8Value,
        arguments: &[CefV8Value],
        _retval: &mut Option<CefV8Value>,
        exception: &mut CefString,
    ) -> bool {
        if arguments.len() != 1 {
            *exception = "invalid arguments".into();
        } else if !arguments[0].is_uint() {
            *exception = "invalid arguments".into();
        } else {
            let msg = CefProcessMessage::create(&"toggle_window".into()).unwrap();
            let args = msg.get_argument_list().unwrap();

            args.set_int(0, arguments[0].get_uint_value() as _);

            self.0.send_process_message(CefProcessId::BROWSER, msg);
        }

        true
    }
}

*/

fn main() {
    let hinstance = unsafe { GetModuleHandleA(std::ptr::null()) };
    let main_args = CefMainArgs::new(hinstance as _);

    let app = CefApp::new(MyApp);

    if cef_execute_process(&main_args, Some(app.clone()), None) >= 0 {
        return;
    }

    browser::main(main_args, app);
}
