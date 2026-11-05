//! The commands the page may call. build.rs gives each an `allow-<command>` permission and
//! `capabilities/main.json` grants them to the window; the test below checks the two agree.

#[allow(dead_code)] // the app reads these through build.rs
pub const COMMANDS: &[&str] = &[
    "library",
    "check_updates",
    "install",
    "add_github",
    "update_all",
    "remove",
    "choose_game_folder",
    "auto_game_folder",
    "open_folder",
    "open_github",
    "graphics",
    "set_graphics",
    "settings",
    "set_light",
    "set_token",
    "check_app_update",
    "install_app_update",
    "restart_app",
];

#[cfg(test)]
mod tests {
    #[test]
    fn the_window_may_call_every_command() {
        let capability: serde_json::Value = serde_json::from_str(include_str!("../capabilities/main.json")).unwrap();
        let granted: Vec<&str> = capability["permissions"].as_array().unwrap().iter().filter_map(|p| p.as_str()).collect();
        for name in super::COMMANDS {
            let permission = format!("allow-{}", name.replace('_', "-"));
            assert!(granted.contains(&permission.as_str()), "capabilities/main.json lacks {permission}");
        }
        assert_eq!(granted.len(), super::COMMANDS.len(), "capabilities/main.json grants something else too");
    }
}
