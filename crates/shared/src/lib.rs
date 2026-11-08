//! The types the core, the Tauri shell and the WASM UI share. Plain data, no IO, so the UI
//! can depend on it without pulling in the network or file code.

use serde::{Deserialize, Serialize};

/// Where an addon comes from: a GitHub repo, and which of its releases and files are the addon,
/// or a CurseForge project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    /// `owner/name` on GitHub. Empty for a CurseForge project.
    #[serde(default)]
    pub repo: String,
    /// Only releases whose tag starts with this. One repo can ship several addons this way
    /// (Spoken: `spoken/v3.6.0`, `quests-audio/v2.4.0`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag_prefix: Option<String>,
    /// Only zip files whose name contains this, when a release has more than one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset: Option<String>,
    /// A CurseForge project rather than a GitHub repo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curseforge: Option<CurseProject>,
}

/// A project on CurseForge: its number, which never changes, and the slug in its page's address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurseProject {
    pub id: u64,
    pub slug: String,
}

impl Source {
    pub fn github(repo: &str) -> Source {
        Source { repo: repo.to_string(), tag_prefix: None, asset: None, curseforge: None }
    }

    pub fn curseforge(id: u64, slug: &str) -> Source {
        Source { repo: String::new(), tag_prefix: None, asset: None, curseforge: Some(CurseProject { id, slug: slug.to_string() }) }
    }

    /// Where it comes from, in a few words: `owner/name`, or `curseforge.com/wow/addons/slug`.
    pub fn label(&self) -> String {
        match &self.curseforge {
            Some(project) => format!("curseforge.com/wow/addons/{}", project.slug),
            None => self.repo.clone(),
        }
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
        match &self.curseforge {
            Some(project) => curseforge_url(&project.slug),
            None => format!("https://github.com/{}", self.repo),
        }
    }
}

/// A CurseForge project's page.
pub fn curseforge_url(slug: &str) -> String {
    format!("https://www.curseforge.com/wow/addons/{slug}")
}

/// A slug as CurseForge makes them: lowercase letters, digits and dashes. Anything else is
/// refused before it goes into an address.
pub fn is_slug(slug: &str) -> bool {
    !slug.is_empty() && slug.len() <= 128 && slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
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
    /// What it does, a point at a time, for its own page. Picks only.
    #[serde(default)]
    pub about: Vec<String>,
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

/// A newer Gryphon than the one running, from the release feed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppUpdate {
    /// The version on offer, `0.2.0`.
    pub version: String,
    /// The version running now.
    pub current: String,
}

/// How much of a Gryphon update has downloaded, and the whole when the server says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

impl AppProgress {
    /// Whole percent done, `None` while the size is unknown.
    pub fn percent(&self) -> Option<u8> {
        let total = self.total.filter(|t| *t > 0)?;
        Some((self.downloaded.min(total) * 100 / total) as u8)
    }
}

/// The app's settings as the UI sees them. The token and key never go back to the page.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SettingsInfo {
    /// The game folder picked by hand; `None` finds it in the usual places.
    pub wow_path: Option<String>,
    pub has_token: bool,
    /// A CurseForge API key is saved, so CurseForge can be searched.
    #[serde(default)]
    pub has_curseforge_key: bool,
    pub light: bool,
}

/// One addon from a CurseForge search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Found {
    pub id: u64,
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub authors: Vec<String>,
    pub downloads: u64,
    /// The newest file's date, `2026-11-02T18:00:00Z`.
    pub updated: Option<String>,
    /// It has a file built for WoW Forever.
    pub forever: bool,
    /// The author lets apps other than CurseForge's own download it.
    pub distributable: bool,
    /// The folders its newest Forever file puts in AddOns, as CurseForge lists them.
    pub folders: Vec<String>,
    /// The addon in the library it already is: installed from CurseForge, or one whose folders
    /// it would replace. Its id and name.
    pub installed_as: Option<(String, String)>,
}

/// A page of CurseForge search results.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Search {
    pub results: Vec<Found>,
    /// How many CurseForge says match, when it says.
    pub total: Option<u64>,
}

/// One of the game's own settings (a CVar) Gryphon can change, and where it stands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tweak {
    /// The CVar, `farclip`.
    pub id: String,
    pub name: String,
    /// What it does, in a line.
    pub about: String,
    /// The longer story: what the numbers mean and what they cost.
    pub tip: String,
    pub min: i64,
    pub max: i64,
    /// What the game uses with nothing set.
    pub default: i64,
    /// On or off (1 or 0) rather than a number.
    pub toggle: bool,
    /// What Config.wtf says now, or the default when it says nothing.
    pub value: i64,
    /// What it was before Gryphon changed it. `None` when Gryphon never did.
    pub before: Option<i64>,
}

/// The settings Gryphon can change in Config.wtf.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Graphics {
    pub tweaks: Vec<Tweak>,
    /// The game is open. It writes its settings back when it closes, over any change made now.
    pub running: bool,
    /// The game hasn't written Config.wtf yet, so there is nothing to change.
    pub missing: bool,
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
