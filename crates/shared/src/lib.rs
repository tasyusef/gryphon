//! The types the core, the Tauri shell and the WASM UI share. Plain data, no IO, so the UI
//! can depend on it without pulling in the network or file code.

use serde::{Deserialize, Serialize};

/// Where an addon comes from: a GitHub repo, and which of its releases and files are the addon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    /// `owner/name`.
    pub repo: String,
    /// Only releases whose tag starts with this. One repo can ship several addons this way
    /// (Spoken: `spoken/v3.6.0`, `quests-audio/v2.4.0`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag_prefix: Option<String>,
    /// Only zip files whose name contains this, when a release has more than one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset: Option<String>,
}

impl Source {
    pub fn github(repo: &str) -> Source {
        Source { repo: repo.to_string(), tag_prefix: None, asset: None }
    }

    /// Reads `owner/name`, `gh:owner/name`, `github:owner/name` or any github.com URL into
    /// the repo.
    pub fn parse(input: &str) -> Option<Source> {
        let s = input.trim().trim_end_matches('/');
        let s = s.strip_suffix(".git").unwrap_or(s);
        let rest = ["https://github.com/", "http://github.com/", "https://www.github.com/", "github.com/", "github:", "gh:"]
            .iter()
            .find_map(|p| s.strip_prefix(p))
            .unwrap_or(s);
        let mut parts = rest.split('/');
        let (owner, name) = (parts.next()?, parts.next()?);
        let ok = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
        (ok(owner) && ok(name)).then(|| Source::github(&format!("{owner}/{name}")))
    }

    pub fn url(&self) -> String {
        format!("https://github.com/{}", self.repo)
    }
}

/// Whether an addon folder loads on WoW Forever, read from the TOC the game would use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fit {
    /// Its `## Interface:` lists a Forever number (16xxx).
    Forever,
    /// It has a TOC, but for another client.
    OtherClient,
    /// Its `## AllowLoadGameType:` leaves Forever out.
    Blocked,
    /// No TOC the game would read: not an addon folder.
    NoToc,
}

/// One folder in `Interface/AddOns`, or in a download.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub name: String,
    pub title: Option<String>,
    pub version: Option<String>,
    pub interface: Vec<u32>,
    pub fit: Fit,
}

/// The game install the app is working on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInfo {
    /// The World of Warcraft folder.
    pub root: String,
    pub addons_dir: String,
    /// The client build, `1.60.1.70707`, from `.build.info`.
    pub version: Option<String>,
    /// The interface number that build expects, `16001`.
    pub interface: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledInfo {
    pub tag: String,
    pub folders: Vec<String>,
    /// Seconds since the Unix epoch.
    pub installed_at: u64,
}

/// The newest release that fits, from the last check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestInfo {
    pub tag: String,
    pub published_at: Option<String>,
    pub size: u64,
}

/// One row in the library: a catalog pick, an addon added from GitHub, or both.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Addon {
    pub id: String,
    pub name: String,
    pub blurb: Option<String>,
    pub source: Source,
    /// In the built-in catalog.
    pub featured: bool,
    /// Catalog ids this one needs first.
    pub needs: Vec<String>,
    pub size_hint_mb: Option<u32>,
    pub installed: Option<InstalledInfo>,
    pub latest: Option<LatestInfo>,
    /// The last check's error, when it failed.
    pub problem: Option<String>,
}

impl Addon {
    pub fn update_available(&self) -> bool {
        matches!((&self.installed, &self.latest), (Some(i), Some(l)) if i.tag != l.tag)
    }
}

/// Everything the main screen shows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub game: Option<GameInfo>,
    pub addons: Vec<Addon>,
    /// Folders in AddOns that no addon here installed.
    pub other: Vec<Folder>,
}

/// A step of an install, sent to the UI as it happens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum Step {
    Checking,
    Downloading { done: u64, total: Option<u64> },
    Unpacking,
    Done,
    Failed { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Progress {
    pub id: String,
    #[serde(flatten)]
    pub step: Step,
}

/// The app's settings as the UI sees them. The token itself never goes back to the page.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SettingsInfo {
    /// The game folder picked by hand; `None` finds it in the usual places.
    pub wow_path: Option<String>,
    pub has_token: bool,
    pub light: bool,
}

/// A shareable addon list: what a friend imports to get the same setup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pack {
    pub name: String,
    pub addons: Vec<PackAddon>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackAddon {
    pub id: String,
    pub name: String,
    pub source: Source,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_repo_forms() {
        for input in [
            "Questie/Questie",
            "gh:Questie/Questie",
            "github:Questie/Questie",
            "https://github.com/Questie/Questie",
            "https://github.com/Questie/Questie/releases/latest",
            "github.com/Questie/Questie.git",
        ] {
            assert_eq!(Source::parse(input).map(|s| s.repo), Some("Questie/Questie".into()), "{input}");
        }
        assert!(Source::parse("Questie").is_none());
        assert!(Source::parse("https://github.com/a b/c").is_none());
    }
}
