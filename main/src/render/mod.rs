use std::{cell::RefCell, rc::Rc, time::Instant};

use assets::AssetFactory;
use cef::{
    v8, CefBrowser, CefFrame, CefProcessId, CefProcessMessage, CefV8Context,
    CefV8Propertyattribute, CefV8Value, RenderProcessHandler,
};

pub mod search;
use image::ImageOutputFormat;
use search::{AnyTarget, Index, Match, Provider, SearchProvider};

mod assets;

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
        let mark = Instant::now();

        let provider = Provider::new();

        let mut index = Index::open(provider, "index.json".into());
        crate::mark!("open index: {:?}", mark);

        index.include(index.provider.config.index());
        crate::mark!("index config: {:?}", mark);

        index.include(index.provider.appx.index());
        crate::mark!("index appx: {:?}", mark);

        index.include(index.provider.steam.index());
        crate::mark!("index steam: {:?}", mark);

        index.include(index.provider.start_menu.index());
        crate::mark!("index start menu: {:?}", mark);

        index.save();

        let b = Instant::now();
        println!("created search index: {:?}", b - mark);

        let rc = Rc::new(RefCell::new(index));
        let assets = AssetFactory::new(&context);
        let objects: Vec<_> = rc
            .borrow()
            .iter()
            .map(|index| make_cef_target(rc.clone(), &assets, index))
            .collect();

        let c = Instant::now();
        println!("created object cache: {:?}", c - b);

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
        let search_fn = move |query: String| {
            let search = rc.borrow();
            let matches = search.search(&query.to_lowercase());

            let limit = 7.min(matches.len());
            let display = matches
                .into_iter()
                .take(limit)
                .map(|m| make_cef_match(&objects, &query, m));

            v8::v8_array(display)
        };
        let value = v8::v8_function1(key.clone(), search_fn);
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
            let context = frame.get_v8context().expect("get v8 context");
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

pub fn make_cef_target(
    rc: Rc<RefCell<Index<AnyTarget, Provider>>>,
    assets: &AssetFactory,
    index: usize,
) -> CefV8Value {
    let object = CefV8Value::create_object(None, None).unwrap();

    let key = "details";
    let value = rc.details(&index);
    object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

    let key = "display_icon";
    let display_icon = rc.display_icon(&index);
    let value: CefV8Value = match display_icon {
        None => ().into(),
        Some(image) => {
            let mut data = vec![];
            image.write_to(&mut data, ImageOutputFormat::Png).unwrap();
            assets.create_asset("image/png", &mut data)
        }
    };
    object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

    let key = "launch";
    let launch = rc.launch(&index);
    let value = v8::v8_function0(key, move || launch());
    object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

    object
}

pub fn make_cef_match(targets: &[CefV8Value], query: &str, m: Match<usize>) -> CefV8Value {
    let object = CefV8Value::create_object(None, None).unwrap();

    let key = "key";
    let value = m.key;
    object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

    let key = "start";
    let value = m.index;
    object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

    let key = "end";
    let value = m.index + query.len();
    object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

    let key = "target";
    let value = targets[m.value].clone();
    object.set_value_bykey(Some(&key.into()), value, CefV8Propertyattribute::NONE);

    object
}
