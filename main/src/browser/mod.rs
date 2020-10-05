use std::{cell::Cell, fs::File, io::prelude::*, sync::Mutex};

use cef::{
    cef_initialize, cef_quit_message_loop, cef_register_scheme_handler_factory,
    cef_run_message_loop, cef_shutdown, CefApp, CefBrowser, CefBrowserHost, CefBrowserSettings,
    CefCallback, CefClient, CefColor, CefFrame, CefLifeSpanHandler, CefLogSeverity, CefMainArgs,
    CefProcessId, CefProcessMessage, CefRequest, CefResourceHandler, CefResourceReadCallback,
    CefResourceSkipCallback, CefResponse, CefSettings, CefString, CefWindowInfo, Client,
    LifeSpanHandler, ResourceHandler, SchemeHandlerFactory,
};

use percent_encoding::percent_decode_str;
use winapi::{
    shared::minwindef::DWORD,
    shared::windef::HWND,
    um::{
        dwmapi::DwmExtendFrameIntoClientArea,
        uxtheme::MARGINS,
        winuser::{
            keybd_event, GetForegroundWindow, SetFocus, SetForegroundWindow, SetWindowLongPtrA,
            GWL_EXSTYLE, GWL_STYLE, VK_LWIN, VK_RETURN, WM_KEYDOWN, WM_KEYUP, WS_EX_TOOLWINDOW,
            WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, WS_VISIBLE,
        },
    },
};

mod hook;

lazy_static::lazy_static! {
    static ref PREVIOUS_FOCUS: Mutex<Cell<Option<usize>>> = Default::default();
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

        if path.starts_with("app/") {
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
        } else {
            Some(NotFoundResourceHandler.into())
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

pub struct MyClient {
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
            let visible = args.get_int(0);

            let host = browser.get_host().unwrap();
            let hwnd = host.get_window_handle() as HWND;

            toggle(hwnd, visible);
        }

        true
    }
}

pub struct MyLifeSpanHandler;
impl LifeSpanHandler for MyLifeSpanHandler {
    fn on_before_close(&mut self, _browser: CefBrowser) {
        cef_quit_message_loop();
    }
}

fn toggle(hwnd: HWND, state: i32) {
    let base_style = WS_POPUP;
    let base_ex_style = WS_EX_TOOLWINDOW;

    if state == 1 {
        unsafe {
            SetWindowLongPtrA(hwnd, GWL_STYLE, (base_style | WS_VISIBLE) as _);
            SetWindowLongPtrA(hwnd, GWL_EXSTYLE, base_ex_style as _);

            let hwnd = hwnd as usize;
            let prev = GetForegroundWindow() as usize;

            if prev != hwnd {
                PREVIOUS_FOCUS.lock().unwrap().set(Some(prev));

                std::thread::spawn(move || {
                    keybd_event(0x12, 0, 1, 0);

                    SetForegroundWindow(hwnd as _);
                    SetFocus(hwnd as _);

                    keybd_event(0x12, 0, 3, 0);
                });
            }
        }
    } else {
        unsafe {
            SetWindowLongPtrA(hwnd, GWL_STYLE, base_style as _);
            SetWindowLongPtrA(hwnd, GWL_EXSTYLE, (base_ex_style | WS_EX_TRANSPARENT) as _);

            if state == 2 {
                if let Some(prev) = PREVIOUS_FOCUS.lock().unwrap().take() {
                    keybd_event(0x12, 0, 1, 0);

                    SetForegroundWindow(prev as _);
                    SetFocus(prev as _);

                    keybd_event(0x12, 0, 3, 0);
                }
            }
        }
    }
}

pub fn main(main_args: CefMainArgs, app: CefApp) {
    let debug_render = std::env::var("DEBUG_RENDER").is_ok();

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

    let size = (480 + 200, 1000);

    let main_window_info = CefWindowInfo::default()
        .set_style(WS_POPUP)
        .set_ex_style(WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_TOPMOST)
        .set_x((1920 - size.0) / 2)
        .set_y(0)
        .set_width(size.0)
        .set_height(size.1)
        .set_window_name("games")
        .build();

    let url = match debug_render {
        true => "http://localhost:8080/index.html",
        false => "app://app/index.html",
    };

    let browser = CefBrowserHost::create_browser_sync(
        &main_window_info,
        Some(client.clone()),
        Some(&url.into()),
        &browser_settings,
        None,
        None,
    )
    .unwrap();

    let host = browser.get_host().unwrap();

    // if std::env::var("DEBUG_RENDER").is_ok() {
    //     let devtools_window_info = CefWindowInfo::default()
    //         .set_style(WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN | WS_CLIPSIBLINGS | WS_VISIBLE)
    //         .set_x(CW_USEDEFAULT)
    //         .set_y(CW_USEDEFAULT)
    //         .set_width(CW_USEDEFAULT)
    //         .set_height(CW_USEDEFAULT)
    //         .set_window_name("dev tools")
    //         .build();

    //     host.show_dev_tools(
    //         Some(&devtools_window_info),
    //         Some(client.clone()),
    //         None,
    //         None,
    //     );
    // }

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
