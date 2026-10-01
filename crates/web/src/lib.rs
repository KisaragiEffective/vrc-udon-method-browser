#![deny(clippy::all)]
#![warn(clippy::nursery)]
#![allow(clippy::future_not_send)]

mod app;
mod files;
mod remote;

pub use app::App;

#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}
