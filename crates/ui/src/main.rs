//! Gryphon's page: the WASM binary Trunk builds (`index.html`, `boot.js`).

mod app;
mod bridge;
mod demo;
mod marks;
mod screens;
mod text;
mod updates;

fn main() {
    leptos::mount::mount_to_body(app::App);
}
