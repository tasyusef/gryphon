//! Gryphon's page: the WASM binary Trunk builds (`index.html`, `boot.js`).

mod app;
mod bridge;
mod demo;
mod screens;
mod text;

fn main() {
    leptos::mount::mount_to_body(app::App);
}
