use wasm_bindgen::JsValue;

pub fn now_ms() -> u64 {
    js_sys::Date::now() as u64
}

pub fn parse_github_time_ms(value: &str) -> Option<u64> {
    Some(js_sys::Date::new(&JsValue::from_str(value)).get_time() as u64)
        .filter(|millis| *millis > 0)
}
