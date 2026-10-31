//! Tauri's build step, with an app manifest so each command in `src/names.rs` gets an
//! `allow-<command>` permission. `capabilities/main.json` grants exactly those to the window.

#[path = "src/names.rs"]
mod names;

fn main() {
    let attributes = tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(names::COMMANDS));
    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}
