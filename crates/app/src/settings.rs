//! What the app remembers between runs, in its config folder (`settings.json`). The addon
//! record itself stays beside the game (`gryphon_core::state`), shared with the CLI.

use std::path::PathBuf;

use gryphon_core::shared::SettingsInfo;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Settings {
    /// The World of Warcraft folder, when picked by hand.
    #[serde(default)]
    pub wow_path: Option<PathBuf>,
    /// Raises GitHub's hourly limit. Kept in plain text: it only needs to read public repos.
    #[serde(default)]
    pub github_token: Option<String>,
    #[serde(default)]
    pub light: bool,
}

impl Settings {
    fn path(app: &AppHandle) -> Option<PathBuf> {
        app.path().app_config_dir().ok().map(|dir| dir.join("settings.json"))
    }

    /// The saved settings, or the defaults when there are none or they do not read.
    pub fn load(app: &AppHandle) -> Settings {
        Settings::path(app).and_then(|path| std::fs::read(path).ok()).and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
    }

    pub fn save(&self, app: &AppHandle) -> Result<(), String> {
        let path = Settings::path(app).ok_or("This system has no folder for app settings.")?;
        let failed = |e: std::io::Error| format!("Could not save settings to {}: {e}", path.display());
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(failed)?;
        }
        let text = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(failed)
    }

    pub fn info(&self) -> SettingsInfo {
        SettingsInfo {
            wow_path: self.wow_path.as_ref().map(|p| p.display().to_string()),
            has_token: self.github_token.is_some(),
            light: self.light,
        }
    }
}
