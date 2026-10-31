//! Calls into the app (`crates/app/src/commands.rs`) through `window.__TAURI__`
//! (`withGlobalTauri`). Arguments go in camelCase; an install's steps come back over a Tauri
//! channel. In a plain browser there is no app, and [`crate::demo`] answers instead.

use gryphon_shared::Progress;
use js_sys::{Array, Function, JSON, Promise, Reflect};
use serde::de::DeserializeOwned;
use serde_json::Value;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

fn tauri() -> Option<JsValue> {
    let window: JsValue = web_sys::window()?.into();
    let global = Reflect::get(&window, &"__TAURI__".into()).ok().filter(|v| v.is_object())?;
    Reflect::get(&global, &"core".into()).ok().filter(|v| v.is_object())
}

/// Running inside the app, not the browser preview.
pub fn in_app() -> bool {
    tauri().is_some()
}

/// Calls `command`. `on_progress` gets each step an install sends while it runs.
pub async fn call<T: DeserializeOwned>(command: &str, args: Value, on_progress: Option<Box<dyn Fn(Progress)>>) -> Result<T, String> {
    let Some(core) = tauri() else {
        return crate::demo::answer(command).and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()));
    };
    let js_args = JSON::parse(&args.to_string()).map_err(text)?;
    if let Some(on_progress) = on_progress {
        let constructor: Function = Reflect::get(&core, &"Channel".into()).map_err(text)?.dyn_into().map_err(text)?;
        let channel = Reflect::construct(&constructor, &Array::new()).map_err(text)?;
        let handler = Closure::<dyn Fn(JsValue)>::new(move |message: JsValue| {
            if let Some(progress) = from_js::<Progress>(&message) {
                on_progress(progress);
            }
        });
        Reflect::set(&channel, &"onmessage".into(), handler.as_ref()).map_err(text)?;
        handler.forget(); // a step can arrive after the call has answered
        Reflect::set(&js_args, &"onProgress".into(), &channel).map_err(text)?;
    }
    let invoke: Function = Reflect::get(&core, &"invoke".into()).map_err(text)?.dyn_into().map_err(text)?;
    let promise: Promise = invoke.call2(&core, &command.into(), &js_args).map_err(text)?.dyn_into().map_err(text)?;
    let answer = JsFuture::from(promise).await.map_err(text)?;
    from_js(&answer).ok_or_else(|| format!("The app answered {command} in a shape this page does not read."))
}

fn from_js<T: DeserializeOwned>(value: &JsValue) -> Option<T> {
    let json = if value.is_undefined() { "null".to_string() } else { JSON::stringify(value).ok()?.as_string()? };
    serde_json::from_str(&json).ok()
}

/// A command's error is its message; anything else, whatever it prints as.
fn text(value: JsValue) -> String {
    value.as_string().unwrap_or_else(|| format!("{value:?}"))
}
