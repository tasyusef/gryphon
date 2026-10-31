//! Gryphon's window: the Leptos page (`crates/ui`) and the commands it calls.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
#[cfg(test)]
mod ipc_tests;
mod names;
mod settings;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            app.manage(commands::Kit::new(settings::Settings::load(app.handle())));
            Ok(())
        })
        // keep in step with names::COMMANDS
        .invoke_handler(tauri::generate_handler![
            commands::library,
            commands::check_updates,
            commands::install,
            commands::add_github,
            commands::update_all,
            commands::remove,
            commands::choose_game_folder,
            commands::auto_game_folder,
            commands::open_folder,
            commands::settings,
            commands::set_light,
            commands::set_token,
        ])
        .run(tauri::generate_context!())
        .expect("Gryphon could not start");
}
