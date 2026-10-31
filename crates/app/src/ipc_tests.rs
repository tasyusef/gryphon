//! The commands called the way the window calls them: through Tauri's IPC, by name, with
//! camelCase arguments and a frontend channel. They change a real game folder and reach GitHub,
//! so they only run when `GRYPHON_TEST_GAME` names a scratch copy of one.

use gryphon_core::shared::{Library, SettingsInfo};
use serde_json::{json, Value};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{WebviewWindow, WebviewWindowBuilder};

use crate::commands::{self, Kit};
use crate::settings::Settings;

fn window(game: &str) -> WebviewWindow<MockRuntime> {
    let settings = Settings { wow_path: Some(game.into()), ..Settings::default() };
    let app = mock_builder()
        .manage(Kit::new(settings))
        .invoke_handler(tauri::generate_handler![
            commands::library,
            commands::check_updates,
            commands::install,
            commands::remove,
            commands::settings,
        ])
        .build(mock_context(noop_assets()))
        .unwrap();
    WebviewWindowBuilder::new(&app, "main", Default::default()).build().unwrap()
}

fn call(window: &WebviewWindow<MockRuntime>, cmd: &str, args: Value) -> Result<Value, Value> {
    let request = InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: "tauri://localhost".parse().unwrap(),
        body: InvokeBody::Json(args),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.into(),
    };
    get_ipc_response(window, request).map(|body| body.deserialize().unwrap())
}

fn library(value: Value) -> Library {
    serde_json::from_value(value).unwrap()
}

fn summary(library: &Library) -> Vec<String> {
    library
        .addons
        .iter()
        .map(|a| {
            let installed = a.installed.as_ref().map_or("-", |i| i.tag.as_str());
            let latest = a.latest.as_ref().map_or("-", |l| l.tag.as_str());
            format!("{} installed={installed} latest={latest} problem={:?}", a.id, a.problem)
        })
        .collect()
}

#[test]
fn the_window_can_drive_every_step() {
    let Ok(game) = std::env::var("GRYPHON_TEST_GAME") else {
        eprintln!("skipped: set GRYPHON_TEST_GAME to a scratch game folder");
        return;
    };
    let window = window(&game);

    let settings: SettingsInfo = serde_json::from_value(call(&window, "settings", json!({})).unwrap()).unwrap();
    assert_eq!(settings.wow_path.as_deref(), Some(game.as_str()));

    let before = library(call(&window, "library", json!({})).unwrap());
    assert_eq!(before.game.as_ref().map(|g| g.root.as_str()), Some(game.as_str()));
    eprintln!("library:\n  {}", summary(&before).join("\n  "));

    let checked = library(call(&window, "check_updates", json!({})).unwrap());
    eprintln!("checked:\n  {}", summary(&checked).join("\n  "));
    assert!(checked.addons.iter().filter(|a| a.featured).all(|a| a.latest.is_some()), "every pick has a release");

    let removed = library(call(&window, "remove", json!({ "id": "questie" })).unwrap());
    assert!(removed.addons.iter().any(|a| a.id == "questie" && a.installed.is_none()));

    let installed = call(&window, "install", json!({ "ids": ["questie"], "onProgress": "__CHANNEL__:7" }));
    let installed = library(installed.expect("install answers"));
    let questie = installed.addons.iter().find(|a| a.id == "questie").unwrap();
    assert!(questie.installed.is_some(), "questie is back: {:?}", questie.problem);
    eprintln!("after reinstall:\n  {}", summary(&installed).join("\n  "));
}
