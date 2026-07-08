pub mod app;
pub mod components;
pub mod model;
pub mod pages;

#[cfg(feature = "ssr")]
pub mod data;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::App;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
