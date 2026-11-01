//! Updates to Gryphon itself, the way Toolbox does them.
//!
//! Nothing here runs by itself. A click on the sidebar's update row calls [`check_app_update`],
//! and only then does the app ask anything: one GET of `plugins.updater.endpoints` in
//! `tauri.conf.json`, the `latest.json` on the newest release of the public tasyusef/gryphon
//! repo. A newer version is kept in [`Pending`]. The next click calls [`install_app_update`],
//! which downloads that version's package, checks its signature against
//! `plugins.updater.pubkey` (Toolbox's update key, and that it was signed for the version the
//! feed names, `requireSignedVersion`), and installs it:
//!
//! - macOS: the new `Gryphon.app` from a `.app.tar.gz` is swapped in for the running one.
//! - Windows: the installer runs passive, and the app exits for it; the installer reopens it.
//! - Linux: an AppImage replaces its own file; a deb goes through `pkexec dpkg -i`.
//!
//! [`restart_app`] then opens the new version. The release side, signing the packages and
//! writing `latest.json`, is `scripts/update-feed.sh`.

use std::sync::Mutex;

use gryphon_core::shared::{AppProgress, AppUpdate};
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};

type Answer<T> = Result<T, String>;

/// The update the last check found, if it found one.
#[derive(Default)]
pub struct Pending(Mutex<Option<Update>>);

/// Asks the feed, and keeps what it offers for the install. `None` when this is the newest
/// version: a feed naming this version or an older one is not an update.
#[tauri::command]
pub async fn check_app_update(app: AppHandle, pending: State<'_, Pending>) -> Answer<Option<AppUpdate>> {
    let update = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    let info = update.as_ref().map(|u| AppUpdate { version: u.version.clone(), current: u.current_version.clone() });
    *pending.0.lock().unwrap_or_else(|p| p.into_inner()) = update;
    Ok(info)
}

/// Downloads the pending update, sending each chunk's total to `on_progress`, checks it and
/// installs it. On Windows the installer takes over and the app exits.
#[tauri::command]
pub async fn install_app_update(pending: State<'_, Pending>, on_progress: Channel<AppProgress>) -> Answer<()> {
    let update = pending
        .0
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
        .ok_or("No update to install. Check for updates first.")?;
    let mut downloaded = 0u64;
    let bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                let _ = on_progress.send(AppProgress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;
    // Moving the app and running installers blocks, and on macOS may wait for an administrator
    // prompt, so it runs off the async runtime.
    tauri::async_runtime::spawn_blocking(move || update.install(bytes))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Quits and opens the version just installed.
#[tauri::command]
pub fn restart_app(app: AppHandle) {
    app.restart()
}

#[cfg(test)]
mod tests {
    /// The feed is the newest public release's `latest.json`, a package must be signed for the
    /// version the feed names, and the key is a minisign public key (Toolbox's).
    #[test]
    fn the_config_reads_the_release_feed_and_trusts_the_update_key() {
        let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let updater = &config["plugins"]["updater"];
        assert_eq!(updater["endpoints"], serde_json::json!(["https://github.com/tasyusef/gryphon/releases/latest/download/latest.json"]));
        assert_eq!(updater["requireSignedVersion"], true);
        assert_eq!(updater["dangerousInsecureTransportProtocol"], serde_json::Value::Null);
        let key = updater["pubkey"].as_str().unwrap();
        assert!(key.starts_with("dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6"), "plugins.updater.pubkey is not a minisign public key");
    }
}
