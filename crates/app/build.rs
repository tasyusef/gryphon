//! Tauri's build step, with an app manifest so each command in `src/names.rs` gets an
//! `allow-<command>` permission. `capabilities/main.json` grants exactly those to the window.

#[path = "src/names.rs"]
mod names;

fn main() {
    // tauri-build bakes icon.ico into the Windows .exe but doesn't watch it, so a new icon would
    // otherwise ship inside the old cached resource (0.3.0's Windows app kept the yellow G)
    println!("cargo:rerun-if-changed=icons");
    let attributes = tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(names::COMMANDS));
    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}
