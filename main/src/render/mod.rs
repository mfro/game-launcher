use std::cell::RefCell;

use cef::{
    v8, CefBrowser, CefFrame, CefProcessId, CefProcessMessage, CefV8Context,
    CefV8Propertyattribute, CefV8Value, RenderProcessHandler,
};

pub mod search;

thread_local! {
    static HOOK_CALLBACKS: RefCell<Vec<v8::V8Function>> = Default::default();
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
        let root_object = CefV8Value::create_object(None, None).unwrap();

        let key = "hook";
        let value = v8::v8_function1(key.clone(), hook);
        root_object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

        let key = "toggle";
        let main_frame = browser.get_main_frame().unwrap();
        let value = move |state| toggle(&main_frame, state);
        let value = v8::v8_function1(key.clone(), value);
        root_object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

        let key = "search";
        let search = search::Search::new();
        let search = move |query| search.search(query);
        let value = v8::v8_function1(key.clone(), search);
        root_object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

        let globals = context.get_global().unwrap();
        globals.set_value_bykey(
            Some(&"search".into()),
            root_object,
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

fn hook(callback: v8::V8Function) {
    HOOK_CALLBACKS.with(|c| {
        c.borrow_mut().push(callback.clone());
    });
}

fn toggle(main_frame: &CefFrame, state: i32) {
    let msg = CefProcessMessage::create(&"toggle_window".into()).unwrap();
    let args = msg.get_argument_list().unwrap();

    args.set_int(0, state);

    main_frame.send_process_message(CefProcessId::BROWSER, msg);
}
