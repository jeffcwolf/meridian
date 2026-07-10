//! Meridian — a cross-border ESEF filing explorer.
//!
//! This crate is the Leptos + Axum web application. It renders the UI (SSR with
//! client-side hydration) and, in the `ssr` build, reads pre-cached filing data
//! from the SQLite database produced by the Python data pipeline (`pipeline/`).
//! It does not call external APIs at request time.
//!
//! The module tree splits into UI ([`app`], [`components`], [`pages`]), the
//! serialisable types that cross the server-function boundary ([`model`]), and
//! the server-only data layer (`data`, [`export`]).

pub mod app;
pub mod components;
pub mod model;
pub mod pages;
pub(crate) mod query;

#[cfg(feature = "ssr")]
pub(crate) mod data;

#[cfg(feature = "ssr")]
pub mod export;

/// Client-side entry point: hydrate the server-rendered DOM. Called by the
/// generated WASM bindings when the `hydrate` build loads in the browser.
#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::App;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
