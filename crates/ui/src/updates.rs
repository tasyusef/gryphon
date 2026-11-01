//! Updates to Gryphon itself: the sidebar row under Dark mode, the way Toolbox has it.
//!
//! What one click on the row does, by state: check, install what the check found, reopen into
//! it, or try the failed step again. Nothing happens until the row is clicked; the app makes no
//! request of its own (`crates/app/src/update.rs`). The toolbar's Check for updates is the
//! addons'; this row only ever means the app.

use gryphon_shared::{AppProgress, AppUpdate};
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde_json::json;

use crate::bridge;

/// The version running: the workspace's one version, which the app and its bundle carry too.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateState {
    /// Not asked yet this run.
    Idle,
    Checking,
    /// This is the newest version.
    Latest,
    /// A newer version is out.
    Available(AppUpdate),
    /// Downloading the newer version: whole percent, when the size is known.
    Downloading { version: String, percent: Option<u8> },
    /// Installed; reopening starts it.
    Installed(String),
    /// The last step failed: its message, and what a click tries again.
    Failed { message: String, retry: Retry },
}

/// The step a click after a failure tries again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Retry {
    Check,
    Install(AppUpdate),
    Restart(String),
}

impl UpdateState {
    /// A step is under way, so a click does nothing.
    pub fn busy(&self) -> bool {
        matches!(self, UpdateState::Checking | UpdateState::Downloading { .. })
    }

    /// The row's words, which say what a click does or what is under way, and the tooltip that
    /// says the rest. The sidebar fits about thirteen letters in a row.
    pub fn text(&self) -> (String, String) {
        match self {
            UpdateState::Idle => ("App updates".into(), format!("Gryphon {VERSION}. Check for a newer version of the app.")),
            UpdateState::Checking => ("Checking…".into(), "Checking for a newer version of Gryphon…".into()),
            UpdateState::Latest => ("Up to date".into(), format!("Gryphon {VERSION} is the newest version. Click to check again.")),
            UpdateState::Available(update) => {
                (format!("Install {}", update.version), format!("Gryphon {} is out. Download and install it.", update.version))
            }
            UpdateState::Downloading { version, percent } => {
                let words = match percent {
                    Some(percent) if *percent < 100 => format!("Updating {percent}%"),
                    // Done downloading (installing), or a download of unknown size.
                    _ => "Updating…".to_string(),
                };
                (words, format!("Downloading Gryphon {version}"))
            }
            UpdateState::Installed(version) => ("Reopen app".into(), format!("Gryphon {version} is installed. Reopen the app to start it.")),
            UpdateState::Failed { message, retry } => {
                let (words, what) = match retry {
                    Retry::Check => ("Check failed", "Couldn't check for updates"),
                    Retry::Install(_) => ("Update failed", "Couldn't install the update"),
                    Retry::Restart(_) => ("Reopen failed", "Couldn't reopen the app"),
                };
                (words.into(), format!("{what}: {}. Click to try again.", message.trim_end_matches('.')))
            }
        }
    }
}

/// The row's state, one per window, in the [`Kit`](crate::app::Kit).
#[derive(Clone, Copy)]
pub struct Updates {
    pub state: RwSignal<UpdateState>,
}

impl Updates {
    pub fn new() -> Updates {
        Updates { state: RwSignal::new(UpdateState::Idle) }
    }

    /// The row's click: the next step for the state it is in.
    pub fn click(self) {
        match self.state.get_untracked() {
            UpdateState::Idle | UpdateState::Latest | UpdateState::Failed { retry: Retry::Check, .. } => self.check(),
            UpdateState::Available(update) | UpdateState::Failed { retry: Retry::Install(update), .. } => self.install(update),
            UpdateState::Installed(version) | UpdateState::Failed { retry: Retry::Restart(version), .. } => self.restart(version),
            UpdateState::Checking | UpdateState::Downloading { .. } => {}
        }
    }

    fn check(self) {
        self.state.set(UpdateState::Checking);
        spawn_local(async move {
            let next = match bridge::call::<Option<AppUpdate>>("check_app_update", json!({}), None).await {
                Ok(None) => UpdateState::Latest,
                Ok(Some(update)) => UpdateState::Available(update),
                Err(message) => UpdateState::Failed { message, retry: Retry::Check },
            };
            self.state.set(next);
        });
    }

    fn install(self, update: AppUpdate) {
        let version = update.version.clone();
        self.state.set(UpdateState::Downloading { version: version.clone(), percent: Some(0) });
        let state = self.state;
        let shown = version.clone();
        // A message that lands after the answer finds the row moved on, and changes nothing.
        let on_progress = Box::new(move |progress: AppProgress| {
            if state.with_untracked(|now| matches!(now, UpdateState::Downloading { .. })) {
                let next = UpdateState::Downloading { version: shown.clone(), percent: progress.percent() };
                if state.with_untracked(|now| *now != next) {
                    state.set(next);
                }
            }
        });
        spawn_local(async move {
            let next = match bridge::stream::<(), AppProgress>("install_app_update", json!({}), Some(on_progress)).await {
                Ok(()) => UpdateState::Installed(version),
                Err(message) => UpdateState::Failed { message, retry: Retry::Install(update) },
            };
            state.set(next);
        });
    }

    fn restart(self, version: String) {
        spawn_local(async move {
            if let Err(message) = bridge::call::<()>("restart_app", json!({}), None).await {
                self.state.set(UpdateState::Failed { message, retry: Retry::Restart(version) });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn update(version: &str) -> AppUpdate {
        AppUpdate { version: version.into(), current: VERSION.into() }
    }

    /// Every state's words fit the sidebar, about thirteen letters.
    #[test]
    fn the_row_words_fit_the_sidebar() {
        let failed = |retry| UpdateState::Failed { message: "offline".into(), retry };
        for state in [
            UpdateState::Idle,
            UpdateState::Checking,
            UpdateState::Latest,
            UpdateState::Available(update("0.12.0")),
            UpdateState::Downloading { version: "0.12.0".into(), percent: Some(99) },
            UpdateState::Downloading { version: "0.12.0".into(), percent: None },
            UpdateState::Installed("0.12.0".into()),
            failed(Retry::Check),
            failed(Retry::Install(update("0.12.0"))),
            failed(Retry::Restart("0.12.0".into())),
        ] {
            let (words, title) = state.text();
            assert!(words.chars().count() <= 14, "{words:?} is too long for the sidebar");
            assert!(!title.is_empty());
        }
    }

    #[test]
    fn a_finished_download_says_updating_not_100() {
        let state = UpdateState::Downloading { version: "0.2.0".into(), percent: Some(100) };
        assert_eq!(state.text().0, "Updating…");
    }
}
