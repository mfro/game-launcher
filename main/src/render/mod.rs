use std::{
    cell::RefCell, fs::DirEntry, fs::File, fs::ReadDir, io::prelude::*, path::Path, path::PathBuf,
    sync::Mutex,
};

use cef::{
    v8, CefBrowser, CefFrame, CefProcessId, CefProcessMessage, CefString, CefV8Context,
    CefV8Handler, CefV8Propertyattribute, CefV8Value, RenderProcessHandler, V8Handler,
};

use crate::lnk::ShellLink;

lazy_static::lazy_static! {
    static ref INDEX: Mutex<RefCell<Vec<crate::index::Lunchable>>> = Default::default();
}

thread_local! {
    static HOOK_CALLBACKS: RefCell<Vec<v8::V8Function>> = Default::default();
    static CONFIG_CALLBACKS: RefCell<Vec<v8::V8Function>> = Default::default();
}

pub struct MyRenderProcessHandler;
impl MyRenderProcessHandler {
    pub fn new() -> MyRenderProcessHandler {
        MyRenderProcessHandler
    }
}

impl RenderProcessHandler for MyRenderProcessHandler {
    fn on_context_created(
        &mut self,
        browser: CefBrowser,
        _frame: CefFrame,
        context: CefV8Context,
    ) -> () {
        let globals = context.get_global().unwrap();

        let main_frame = browser.get_main_frame().unwrap();

        let fn_attach_name = "config_attach";
        let fn_attach = v8::v8_function2(fn_attach_name.clone(), attach);

        let toggle = move |state: i32| toggle(&main_frame, state);
        let fn_toggle_name = "config_toggle";
        let fn_toggle = v8::v8_function1(fn_toggle_name.clone(), toggle);

        globals.set_value_bykey(
            Some(&fn_attach_name.into()),
            fn_attach,
            CefV8Propertyattribute::NONE,
        );

        globals.set_value_bykey(
            Some(&fn_toggle_name.into()),
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
                        callback.apply(None, &[]);
                    }
                });
                context.exit();
            }
        }

        true
    }
}

fn attach(config_cb: v8::V8Function, hook_cb: v8::V8Function) {
    HOOK_CALLBACKS.with(|c| {
        c.borrow_mut().push(hook_cb.clone());
    });
    CONFIG_CALLBACKS.with(|c| {
        c.borrow_mut().push(config_cb.clone());
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

    let array = v8::v8_array(v8.into_iter());

    config_cb.apply(None, &[array]);
}

fn toggle(main_frame: &CefFrame, state: i32) {
    let msg = CefProcessMessage::create(&"toggle_window".into()).unwrap();
    let args = msg.get_argument_list().unwrap();

    args.set_int(0, state);

    main_frame.send_process_message(CefProcessId::BROWSER, msg);
}

struct V8Lunchable {
    id: usize,
}

impl V8Lunchable {
    pub fn new(id: usize, target: &crate::index::Lunchable) -> CefV8Value {
        let icon = match target.icon("") {
            Ok(data) => format!("data:image/x-icon;base64,{}", base64::encode(&data)),
            Err(_) => format!("app://404"),
        };

        let icon = icon.as_str().into();
        let display_name = target.display_name().as_str().into();
        let keys = target
            .keys()
            .into_iter()
            .map(|x| x.to_lowercase())
            .map(|x| CefV8Value::create_string(Some(&x.as_str().into())).unwrap());

        let handler: CefV8Handler = V8Lunchable { id }.into();

        let icon = CefV8Value::create_string(Some(&icon)).unwrap();
        let display_name = CefV8Value::create_string(Some(&display_name)).unwrap();
        let keys = v8::v8_array(keys);
        let launch = CefV8Value::create_function(&"launch".into(), handler.clone()).unwrap();

        let attrs = CefV8Propertyattribute::NONE;

        let object = CefV8Value::create_object(None, None).unwrap();
        object.set_value_bykey(Some(&"icon".into()), icon, attrs);
        object.set_value_bykey(Some(&"display_name".into()), display_name, attrs);
        object.set_value_bykey(Some(&"keys".into()), keys, attrs);
        object.set_value_bykey(Some(&"launch".into()), launch, attrs);

        object
    }
}

macro_rules! v8_require {
    ( $pred:expr, $out:expr, $msg:expr $(, $arg:expr )* ) => {
        if !$pred { *$out = format!($msg, $( $arg ),* ).as_str().into(); return true; }
    };
}

impl V8Handler for V8Lunchable {
    fn execute(
        &mut self,
        name: &CefString,
        _this: CefV8Value,
        args: &[CefV8Value],
        _ret: &mut Option<CefV8Value>,
        err: &mut CefString,
    ) -> bool {
        let index = INDEX.lock().unwrap();
        let index = index.borrow();
        let target = &index[self.id];

        match name.to_string().as_str() {
            "launch" => {
                v8_require!(args.len() == 0, err, "0 args");
                target.launch();
                true
            }
            _ => false,
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

fn find_lnks() -> Vec<(PathBuf, PathBuf)> {
    let appdata = std::env::var("APPDATA").unwrap();
    let roots = [
        PathBuf::from(r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs"),
        PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs"),
    ];

    roots
        .iter()
        .map(|root| {
            let iter = RecursiveSearch::new(&root).into_iter();
            iter.map(move |entry| {
                let relative = entry.path().strip_prefix(&root).unwrap().to_owned();
                (entry, relative)
            })
        })
        .flatten()
        .filter_map(|(entry, relative)| {
            let path = entry.path();
            match path.extension() {
                None => None,
                Some(ext) => match ext.to_str() {
                    Some("lnk") => Some((path, relative)),
                    Some("ini") | Some("url") => None,
                    _ => {
                        println!("unknown start menu entry: {:?}", relative);
                        None
                    }
                },
            }
        })
        .filter(|(path, _)| {
            let mut raw = vec![];
            File::open(path).unwrap().read_to_end(&mut raw).unwrap();
            let lnk = ShellLink::load(&raw);
            match crate::lnk::resolve(&lnk) {
                None => true,
                Some(target) => match target.rfind('.') {
                    None => panic!(),
                    Some(i) => match &target[i + 1..] {
                        "exe" | "msc" | "url" => true,
                        "chm" | "txt" | "rtf" | "pdf" | "html" => false,
                        other => {
                            println!("Unknown lnk target extension: {} {:?}", other, path);
                            false
                        }
                    },
                },
            }
        })
        .collect()
}

fn build_index() {
    let yaml = match File::open("config.yaml") {
        Ok(mut f) => {
            let mut content = vec![];
            f.read_to_end(&mut content).unwrap();
            serde_yaml::from_slice(&content).unwrap()
        }
        Err(_) => vec![],
    };

    let from_yaml = yaml.into_iter().map(|x| crate::index::Lunchable::Custom(x));
    let from_lnks = find_lnks()
        .into_iter()
        .filter_map(|(path, _)| Some(crate::index::Lunchable::ShellLink(path)));

    let config: Vec<_> = from_yaml.chain(from_lnks).collect();

    let index = INDEX.lock().unwrap();
    index.replace(config);
}
