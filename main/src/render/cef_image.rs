use cef::{CefV8Context, CefV8Value, V8ArrayBufferReleaseCallback};

pub struct CefImageFactory {
    factory: CefV8Value,
}

impl CefImageFactory {
    pub fn new(ctx: &CefV8Context) -> CefImageFactory {
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
            crate::log!("{}", e.get_message());
            panic!("{}", e.get_message());
        }

        let factory = retval.unwrap();

        CefImageFactory { factory }
    }

    pub fn create_asset(&self, mime: &str, data: &mut [u8]) -> CefV8Value {
        let mime = mime.into();
        let data = CefV8Value::create_array_buffer(data, ReleaseCallback).unwrap();

        self.factory.execute_function(None, &[mime, data]).unwrap()
    }
}

struct ReleaseCallback;
impl V8ArrayBufferReleaseCallback for ReleaseCallback {
    fn release_buffer(&mut self, _ptr: &mut std::ffi::c_void) {
        // oops! this memory is probably already freed...
    }
}
