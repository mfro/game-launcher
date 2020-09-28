#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate bitflags;
extern crate cef;
extern crate lazy_static;
extern crate percent_encoding;
extern crate winapi;

use std::{
    cell::RefCell, fs::DirEntry, fs::File, fs::ReadDir, io::prelude::*, path::Path, path::PathBuf,
    process::Command,
};

use percent_encoding::percent_decode_str;

use cef::*;

use winapi::{
    shared::{minwindef::DWORD, windef::HWND},
    um::{
        dwmapi::DwmExtendFrameIntoClientArea,
        libloaderapi::GetModuleHandleA,
        shellapi::ShellExecuteW,
        uxtheme::MARGINS,
        winuser::{
            keybd_event, SetFocus, SetForegroundWindow, SetWindowLongPtrA, GWL_EXSTYLE, GWL_STYLE,
            VK_LWIN, VK_RETURN, WM_KEYDOWN, WM_KEYUP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
            WS_EX_TRANSPARENT, WS_POPUP, WS_VISIBLE,
        },
    },
};

#[macro_use]
pub mod flat_data;
mod hook;
pub mod lnk;

use lnk::ShellLink;

thread_local! {
    static HOOK_CALLBACKS: RefCell<Vec<CefV8Value>> = Default::default();
    static CONFIG_CALLBACKS: RefCell<Vec<CefV8Value>> = Default::default();
}

struct MySchemeHandlerFactory;
impl SchemeHandlerFactory for MySchemeHandlerFactory {
    fn create(
        &mut self,
        _browser: Option<CefBrowser>,
        _frame: Option<CefFrame>,
        _scheme_name: &CefString,
        request: CefRequest,
    ) -> Option<CefResourceHandler> {
        let url = request.get_url().to_string();
        let path = percent_decode_str(&url[4..]).decode_utf8().unwrap();
        let path = path.trim_start_matches('/');

        if path.starts_with("link/") {
            let lnk_path = &path[5..];

            let mut raw = vec![];
            File::open(lnk_path).unwrap().read_to_end(&mut raw).unwrap();
            let lnk = ShellLink::load(&raw);
            let data = match lnk::extract_ico(&lnk) {
                Some(vec) => vec,
                None => vec![],
            };

            Some(CefResourceHandler::new(InMemoryResourceHandler {
                mime_type: Some("image/x-icon".into()),
                headers: vec![],
                data,
                index: 0,
            }))
        } else {
            let mut file = match File::open(&path) {
                Ok(file) => file,
                Err(_) => return Some(NotFoundResourceHandler.into()),
            };

            let mut data = vec![];
            file.read_to_end(&mut data).unwrap();

            let mime_type = match path.rfind('.') {
                None => "application/octet-stream".into(),
                Some(i) => match &path[i + 1..] {
                    "html" => "text/html",
                    "css" => "text/css",
                    "js" => "text/javascript",
                    "ttf" => "font/ttf",
                    "png" => "image/png",
                    "jpg" => "image/jpeg",
                    "jpeg" => "image/jpeg",
                    _ => panic!("unknown mime type: {}", path),
                },
            };

            let headers = vec![];

            Some(CefResourceHandler::new(InMemoryResourceHandler {
                mime_type: Some(mime_type.into()),
                headers,
                data,
                index: 0,
            }))
        }
    }
}

struct NotFoundResourceHandler;
impl ResourceHandler for NotFoundResourceHandler {
    fn open(
        &mut self,
        _request: CefRequest,
        handle_request: &mut bool,
        _callback: CefCallback,
    ) -> bool {
        *handle_request = true;
        true
    }

    fn get_response_headers(
        &mut self,
        response: CefResponse,
        response_length: &mut i64,
        _redirect_url: &mut CefString,
    ) -> () {
        response.set_status(404);
        *response_length = 0;
    }
}

struct InMemoryResourceHandler {
    mime_type: Option<String>,
    headers: Vec<(String, String)>,
    data: Vec<u8>,
    index: usize,
}
impl ResourceHandler for InMemoryResourceHandler {
    fn open(
        &mut self,
        _request: CefRequest,
        handle_request: &mut bool,
        _callback: CefCallback,
    ) -> bool {
        *handle_request = true;
        true
    }

    fn get_response_headers(
        &mut self,
        response: CefResponse,
        response_length: &mut i64,
        _redirect_url: &mut CefString,
    ) -> () {
        response.set_status(200);

        if let Some(txt) = &self.mime_type {
            response.set_mime_type(Some(&txt.into()));
        }

        for (name, value) in &self.headers {
            response.set_header_by_name(&name.into(), Some(&value.into()), true);
        }

        *response_length = self.data.len() as _;
    }

    fn skip(
        &mut self,
        _bytes_to_skip: i64,
        _bytes_skipped: &mut i64,
        _callback: CefResourceSkipCallback,
    ) -> bool {
        panic!();
    }

    fn read(
        &mut self,
        data_out: &mut [u8],
        bytes_read: &mut std::os::raw::c_int,
        _callback: CefResourceReadCallback,
    ) -> bool {
        if self.index < self.data.len() {
            let len = data_out.len().min(self.data.len() - self.index);
            let src = &self.data[self.index..self.index + len];

            &mut data_out[0..len].copy_from_slice(src);
            *bytes_read = len as _;
            self.index += len;

            true
        } else {
            false
        }
    }

    fn cancel(&mut self) -> () {
        Default::default()
    }
}

struct MyApp;
impl App for MyApp {
    fn on_register_custom_schemes(&mut self, registrar: CefSchemeRegistrar) -> () {
        let options = CefSchemeOptions::SECURE
            | CefSchemeOptions::STANDARD
            | CefSchemeOptions::CORS_ENABLED
            | CefSchemeOptions::FETCH_ENABLED;

        let scheme_name = "app".into();

        registrar.add_custom_scheme(&scheme_name, options.into());
    }

    fn on_before_command_line_processing(
        &mut self,
        _process_type: Option<&CefString>,
        command_line: CefCommandLine,
    ) -> () {
        command_line.append_switch(&"disable-extensions".into());
    }

    fn get_render_process_handler(&mut self) -> Option<CefRenderProcessHandler> {
        Some(CefRenderProcessHandler::new(MyRenderProcessHandler))
    }
}

struct MyClient {
    life_span_handler: CefLifeSpanHandler,
}
impl Client for MyClient {
    fn get_life_span_handler(&mut self) -> Option<CefLifeSpanHandler> {
        Some(self.life_span_handler.clone())
    }

    fn on_process_message_received(
        &mut self,
        browser: CefBrowser,
        _frame: CefFrame,
        _source_process: CefProcessId,
        message: CefProcessMessage,
    ) -> bool {
        let name = message.get_name().to_string();
        let args = message.get_argument_list().unwrap();

        if name == "toggle_window" {
            let visible = args.get_bool(0);

            let host = browser.get_host().unwrap();
            let hwnd = host.get_window_handle() as HWND;

            toggle(hwnd, visible);

            // host.set_focus(true);

            // struct SEND<T>(T);
            // unsafe impl<T> Send for SEND<T> {}

            // let host = SEND(host);
            // std::thread::spawn(move || {
            //     host.0.set_focus(true);
            //     host.0.set_focus(true);
            //     host.0.set_focus(true);
            //     host.0.set_focus(true);
            //     host.0.set_focus(true);
            //     host.0.set_focus(true);
            // });
        }

        true
    }
}

struct MyLifeSpanHandler;
impl LifeSpanHandler for MyLifeSpanHandler {
    fn on_before_close(&mut self, _browser: CefBrowser) {
        cef_quit_message_loop();
    }
}

struct MyRenderProcessHandler;
impl RenderProcessHandler for MyRenderProcessHandler {
    fn on_context_created(
        &mut self,
        browser: CefBrowser,
        _frame: CefFrame,
        context: CefV8Context,
    ) -> () {
        let globals = context.get_global().unwrap();

        let fn_attach_name = "config_attach".into();
        let fn_attach = CefV8Value::create_function(&fn_attach_name, AttachFunction).unwrap();

        let fn_launch_name = "config_launch".into();
        let fn_launch = CefV8Value::create_function(&fn_launch_name, LaunchFunction).unwrap();

        let fn_toggle_name = "config_toggle".into();
        let fn_toggle = CefV8Value::create_function(
            &fn_toggle_name,
            ToggleFunction(browser.get_main_frame().unwrap()),
        )
        .unwrap();

        globals.set_value_bykey(
            Some(&fn_attach_name),
            fn_attach,
            CefV8Propertyattribute::NONE,
        );

        globals.set_value_bykey(
            Some(&fn_launch_name),
            fn_launch,
            CefV8Propertyattribute::NONE,
        );

        globals.set_value_bykey(
            Some(&fn_toggle_name),
            fn_toggle,
            CefV8Propertyattribute::NONE,
        );
    }

    fn on_process_message_received(
        &mut self,
        _browser: CefBrowser,
        frame: CefFrame,
        _source_process: CefProcessId,
        message: CefProcessMessage,
    ) -> bool {
        let name = message.get_name().to_string();
        // let args = message.get_argument_list().unwrap();

        if name == "hook" {
            let context = frame.get_v8context().unwrap();
            if context.enter() {
                HOOK_CALLBACKS.with(|x| {
                    for callback in x.borrow().iter() {
                        callback.execute_function(None, &[]);
                    }
                });
                context.exit();
            }
        }

        true
    }
}

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

            let mut content = vec![];
            match File::open("config.yaml") {
                Ok(mut f) => {
                    f.read_to_end(&mut content).unwrap();
                }
                Err(_) => {}
            }

            let content = String::from_utf8(content).unwrap();
            let content = content.as_str().into();
            let content = CefV8Value::create_string(Some(&content)).unwrap();

            let index = make_index();
            let array = CefV8Value::create_array(index.len() as i32).unwrap();
            for (i, path) in index.iter().enumerate() {
                let name1 = path.file_stem().and_then(|os| os.to_str()).unwrap();
                let name2 = lnk::get_display_name(&path);

                let name_array = if name2 == name1 {
                    let l = CefV8Value::create_array(1).unwrap();
                    l.set_value_byindex(0, CefV8Value::create_string(Some(&name1.into())).unwrap());
                    l
                } else {
                    let l = CefV8Value::create_array(2).unwrap();
                    l.set_value_byindex(0, CefV8Value::create_string(Some(&name1.into())).unwrap());
                    l.set_value_byindex(
                        1,
                        CefV8Value::create_string(Some(&name2.as_str().into())).unwrap(),
                    );
                    l
                };
                let path = CefV8Value::create_string(Some(&path.to_str().unwrap().into())).unwrap();

                let object = CefV8Value::create_object(None, None).unwrap();
                object.set_value_bykey(
                    Some(&"names".into()),
                    name_array,
                    CefV8Propertyattribute::NONE,
                );

                object.set_value_bykey(Some(&"path".into()), path, CefV8Propertyattribute::NONE);
                array.set_value_byindex(i as i32, object);
            }

            arguments[0].execute_function(None, &[content, array]);
        }

        true
    }
}

struct LaunchFunction;
impl V8Handler for LaunchFunction {
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
        } else if arguments[0].is_array() {
            let len = arguments[0].get_array_length();
            let parts: Vec<String> = (0..len)
                .map(|i| arguments[0].get_value_byindex(i).unwrap())
                .map(|v8| {
                    if v8.is_string() {
                        v8.get_string_value().to_string()
                    } else if v8.is_int() {
                        format!("{}", v8.get_int_value())
                    } else if v8.is_uint() {
                        format!("{}", v8.get_uint_value())
                    } else if v8.is_bool() {
                        format!("{}", v8.get_bool_value())
                    } else if v8.is_double() {
                        format!("{}", v8.get_double_value())
                    } else {
                        "".to_string()
                    }
                })
                .collect();

            Command::new(&parts[0])
                .args(&parts[1..]) //
                .spawn()
                .unwrap();
        } else if arguments[0].is_string() {
            let path = arguments[0].get_string_value().to_string();

            let op: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
            let raw: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

            unsafe {
                ShellExecuteW(
                    std::ptr::null_mut(),
                    op.as_ptr(),
                    raw.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    1,
                );
            }
        } else {
            *exception = "invalid arguments".into();
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
        } else if !arguments[0].is_bool() {
            *exception = "invalid arguments".into();
        } else {
            let msg = CefProcessMessage::create(&"toggle_window".into()).unwrap();
            let args = msg.get_argument_list().unwrap();

            args.set_bool(0, arguments[0].get_bool_value());

            self.0.send_process_message(CefProcessId::BROWSER, msg);
        }

        true
    }
}

fn toggle(hwnd: HWND, visible: bool) {
    let base_style = WS_POPUP;
    let base_ex_style = WS_EX_TOOLWINDOW;

    if visible {
        unsafe {
            let x = hwnd as usize;

            SetWindowLongPtrA(hwnd, GWL_STYLE, (base_style | WS_VISIBLE) as _);
            SetWindowLongPtrA(hwnd, GWL_EXSTYLE, base_ex_style as _);

            std::thread::spawn(move || {
                keybd_event(0x12, 0, 1, 0);

                SetForegroundWindow(x as _);
                SetFocus(x as _);

                keybd_event(0x12, 0, 3, 0);
            });
        }
    } else {
        unsafe {
            SetWindowLongPtrA(hwnd, GWL_STYLE, base_style as _);
            SetWindowLongPtrA(hwnd, GWL_EXSTYLE, (base_ex_style | WS_EX_TRANSPARENT) as _);
        }
    }
}

struct RecursiveSearch {
    stack: Vec<ReadDir>,
}

impl RecursiveSearch {
    pub fn new<P: AsRef<Path>>(path: &P) -> RecursiveSearch {
        let stack = match std::fs::read_dir(path) {
            Ok(iter) => vec![iter],
            Err(_) => vec![],
        };

        RecursiveSearch { stack }
    }
}

impl Iterator for RecursiveSearch {
    type Item = DirEntry;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let iter = match self.stack.last_mut() {
                Some(iter) => iter,
                None => break None,
            };

            let entry = match iter.next() {
                Some(result) => match result {
                    Err(_) => continue,
                    Ok(entry) => entry,
                },
                None => {
                    self.stack.pop();
                    continue;
                }
            };

            let ty = match entry.file_type() {
                Ok(ty) => ty,
                Err(_) => continue,
            };

            if ty.is_file() {
                break Some(entry);
            }

            if ty.is_dir() {
                match std::fs::read_dir(&entry.path()) {
                    Ok(iter) => {
                        self.stack.push(iter);
                        continue;
                    }
                    Err(_) => continue,
                }
            };
        }
    }
}

fn make_index() -> Vec<PathBuf> {
    let appdata = std::env::var("APPDATA").unwrap();
    let roots = [
        PathBuf::from(r"C:\ProgramData\Microsoft\Windows\Start Menu"),
        PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu"),
    ];

    roots
        .iter()
        .map(|root| RecursiveSearch::new(root).into_iter())
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let ext = match path.extension() {
                Some(ext) => ext,
                None => return None,
            };

            if ext == "lnk" {
                Some(path)
            } else {
                None
            }
        })
        .collect()
}

fn main() {
    let hinstance = unsafe { GetModuleHandleA(std::ptr::null()) };
    let main_args = CefMainArgs::new(hinstance as _);

    let app = CefApp::new(MyApp);

    if cef_execute_process(&main_args, Some(app.clone()), None) >= 0 {
        return;
    }

    let settings = CefSettings::default()
        .set_log_severity(CefLogSeverity::FATAL)
        .set_remote_debugging_port(8081)
        .build();

    cef_initialize(&main_args, &settings, Some(app.clone()), None);

    let scheme_name = "app".into();
    let factory = MySchemeHandlerFactory.into();
    cef_register_scheme_handler_factory(&scheme_name, None, Some(factory));

    let client = CefClient::new(MyClient {
        life_span_handler: MyLifeSpanHandler.into(),
    });

    let browser_settings = CefBrowserSettings::default()
        .set_size(std::mem::size_of::<CefBrowserSettings>() as _)
        .set_allow_transparency(1)
        .set_background_color(CefColor::new(0x00, 0x00, 0, 0))
        .build();

    let size = (480 + 200, 480 + 200);

    let main_window_info = CefWindowInfo::default()
        .set_style(WS_POPUP)
        .set_ex_style(WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_TOPMOST)
        .set_x((1920 - size.0) / 2)
        .set_y((1080 - size.1) / 2)
        .set_width(size.0)
        .set_height(size.1)
        .set_window_name("games")
        .build();

    let url = match std::env::var("DEBUG_RENDER").is_ok() {
        true => "http://localhost:8080/index.html",
        false => "app://app/index.html",
    };

    let browser = CefBrowserHost::create_browser_sync(
        &main_window_info,
        Some(client),
        Some(&url.into()),
        &browser_settings,
        None,
        None,
    )
    .unwrap();

    let host = browser.get_host().unwrap();

    // let devtools_window_info = CefWindowInfo::default()
    //     .set_style(WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN | WS_CLIPSIBLINGS | WS_VISIBLE)
    //     .set_x(1920)
    //     .set_y(0)
    //     .set_width(2560 / 2)
    //     .set_height(1080)
    //     .set_window_name("dev tools")
    //     .build();

    // host.show_dev_tools(
    //     Some(&devtools_window_info),
    //     Some(client.clone()),
    //     None,
    //     None,
    // );

    let hwnd = host.get_window_handle() as HWND;

    let margins = MARGINS {
        cxLeftWidth: -1,
        cyTopHeight: -1,
        cxRightWidth: -1,
        cyBottomHeight: -1,
    };

    unsafe { DwmExtendFrameIntoClientArea(hwnd, &margins) };

    let mut held = std::collections::HashSet::<DWORD>::new();
    let _hook = hook::set_hook(move |ty, info| {
        if ty == WM_KEYUP {
            held.remove(&info.vkCode);
        } else if ty == WM_KEYDOWN {
            held.insert(info.vkCode);

            if info.vkCode == VK_RETURN as u32 && held.contains(&(VK_LWIN as u32)) {
                let frame = browser.get_main_frame().unwrap();
                let msg = CefProcessMessage::create(&"hook".into()).unwrap();

                frame.send_process_message(CefProcessId::RENDERER, msg);
            }
        }
    });

    cef_run_message_loop();

    cef_shutdown();
}
