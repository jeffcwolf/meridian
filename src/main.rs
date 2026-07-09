#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use axum::routing::get;
    use axum::Router;
    use leptos::logging::log;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use meridian::app::{shell, App};
    use meridian::export;

    let conf = leptos::config::get_configuration(None)
        .expect("Leptos configuration is valid (from Cargo.toml metadata / env)");
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;
    let routes = generate_route_list(App);

    let app = Router::new()
        .route("/export/company/{id}/{format}", get(export::company_export))
        .route("/export/compare/{format}", get(export::compare_export))
        .leptos_routes(&leptos_options, routes, {
            let leptos_options = leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        .with_state(leptos_options);

    log!("Meridian listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("cannot bind {addr}: {e}"));
    axum::serve(listener, app.into_make_service())
        .await
        .expect("axum server runs until shutdown");
}

// The `hydrate` (wasm) build has no server binary; cargo-leptos still compiles
// this crate as a bin, so provide a no-op entry point for that configuration.
#[cfg(not(feature = "ssr"))]
fn main() {}
