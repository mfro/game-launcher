use mime_guess::Mime;

use cef::{CefV8Context, CefV8Value, V8ArrayBufferReleaseCallback};

pub fn create_asset(ctx: &CefV8Context, mime: &Mime, data: &mut [u8]) -> CefV8Value {
    let key = "_asset_factory".into();

    let mut factory = ctx
        .get_global()
        .unwrap()
        .get_value_bykey(Some(&key))
        .unwrap();

    if factory.is_undefined() {
        println!("create asset factory");
        // CEF/Chromium crashes when using an external ArrayBuffer in a blob for some reason
        // so slice() to copy it to a V8/JS managed ArrayBuffer
        let js = "window._asset_factory = (type, data) => {
            let url = URL.createObjectURL(new Blob([data.slice()], { type }));
            let img = new Image();
            img.src = url
            img.onload = () => URL.revokeObjectURL(url);
            return img;
        }";

        let mut retval = None;
        let mut error = None;
        ctx.eval(&js.into(), None, 0, &mut retval, &mut error);
        if let Some(e) = error {
            println!("{}", e.get_message());
        }

        factory = retval.unwrap();
    }

    let mime = mime.to_string().into();
    let data = CefV8Value::create_array_buffer(data, ReleaseCallback).unwrap();

    factory.execute_function(None, &[mime, data]).unwrap()
}

struct ReleaseCallback;
impl V8ArrayBufferReleaseCallback for ReleaseCallback {
    fn release_buffer(&mut self, _ptr: &mut std::ffi::c_void) {
        // oops! this memory is probably already freed...
    }
}
