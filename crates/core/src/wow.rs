//! Finding the game. A WoW install is one folder with a subfolder per client; Forever's is
//! `_forever_`, and `.build.info` at the top names each client's build.

use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use gryphon_shared::GameInfo;

/// Forever's client folder inside the World of Warcraft folder.
pub const FOREVER_DIR: &str = "_forever_";
/// Forever's product code in `.build.info`.
pub const FOREVER_PRODUCT: &str = "wowf";
/// Interface numbers Forever builds use: build 1.60.x is 160xx. A range, so a patch does not
/// make every addon look out of date.
pub const FOREVER_INTERFACES: RangeInclusive<u32> = 16000..=16999;

#[derive(Debug, Clone)]
pub struct Game {
    /// The World of Warcraft folder.
    pub root: PathBuf,
    /// `root/_forever_`.
    pub client: PathBuf,
    /// `1.60.1.70707`.
    pub version: Option<String>,
    /// `16001`.
    pub interface: Option<u32>,
}

impl Game {
    /// Opens the install that holds `path`: the World of Warcraft folder, its `_forever_`
    /// folder, or anything inside that, such as AddOns.
    pub fn open(path: &Path) -> Option<Game> {
        for dir in path.ancestors() {
            if dir.file_name().is_some_and(|n| n == FOREVER_DIR) {
                return dir.parent().map(Game::at);
            }
            if dir.join(FOREVER_DIR).is_dir() {
                return Some(Game::at(dir));
            }
        }
        None
    }

    /// The first install in the usual places for this system.
    pub fn find() -> Option<Game> {
        default_roots().iter().find_map(|p| Game::open(p))
    }

    fn at(root: &Path) -> Game {
        let version = read_build_version(root);
        let interface = version.as_deref().and_then(interface_for);
        Game { root: root.to_path_buf(), client: root.join(FOREVER_DIR), version, interface }
    }

    pub fn interface_dir(&self) -> PathBuf {
        self.client.join("Interface")
    }

    pub fn addons_dir(&self) -> PathBuf {
        self.interface_dir().join("AddOns")
    }

    pub fn info(&self) -> GameInfo {
        GameInfo {
            root: self.root.display().to_string(),
            addons_dir: self.addons_dir().display().to_string(),
            version: self.version.clone(),
            interface: self.interface,
        }
    }
}

/// Where Battle.net puts the game by default on each system.
pub fn default_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if cfg!(target_os = "macos") {
        roots.push(PathBuf::from("/Applications/World of Warcraft"));
        if let Some(home) = std::env::var_os("HOME") {
            roots.push(Path::new(&home).join("Applications/World of Warcraft"));
        }
    } else if cfg!(windows) {
        for var in ["ProgramFiles(x86)", "ProgramFiles"] {
            if let Some(dir) = std::env::var_os(var) {
                roots.push(Path::new(&dir).join("World of Warcraft"));
            }
        }
        for drive in 'C'..='H' {
            for sub in ["World of Warcraft", "Games\\World of Warcraft", "Program Files (x86)\\World of Warcraft"] {
                roots.push(PathBuf::from(format!("{drive}:\\{sub}")));
            }
        }
    } else if let Some(home) = std::env::var_os("HOME") {
        // Linux runs Battle.net through Wine; these are the prefixes Lutris and Wine make.
        let home = Path::new(&home);
        for prefix in ["Games/battlenet", "Games/world-of-warcraft", "Games/battle-net", ".wine"] {
            roots.push(home.join(prefix).join("drive_c/Program Files (x86)/World of Warcraft"));
        }
    }
    roots
}

/// The Forever row's `Version` in `.build.info`, a `|`-separated table whose header names
/// each column as `Name!TYPE:size`.
fn read_build_version(root: &Path) -> Option<String> {
    let text = std::fs::read_to_string(root.join(".build.info")).ok()?;
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next()?.split('|').map(|h| h.split('!').next().unwrap_or(h)).collect();
    let product = header.iter().position(|h| *h == "Product")?;
    let version = header.iter().position(|h| *h == "Version")?;
    lines
        .map(|line| line.split('|').collect::<Vec<_>>())
        .find(|row| row.get(product) == Some(&FOREVER_PRODUCT))
        .and_then(|row| row.get(version).map(|v| v.to_string()))
        .filter(|v| !v.is_empty())
}

/// The interface number a build expects: `1.60.1.70707` is 16001.
pub fn interface_for(version: &str) -> Option<u32> {
    let mut parts = version.split('.').map(|p| p.parse::<u32>().ok());
    let (major, minor, patch) = (parts.next()??, parts.next()??, parts.next()??);
    Some(major * 10000 + minor * 100 + patch)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUILD_INFO: &str = "Branch!STRING:0|Active!DEC:1|Build Key!HEX:16|Version!STRING:0|KeyRing!HEX:16|Product!STRING:0\nus|1|7feb|1.60.1.70707||wowf\n";

    fn fake_install() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("_forever_/Interface/AddOns")).unwrap();
        std::fs::write(dir.path().join(".build.info"), BUILD_INFO).unwrap();
        dir
    }

    #[test]
    fn reads_the_forever_build() {
        let dir = fake_install();
        let game = Game::open(dir.path()).unwrap();
        assert_eq!(game.version.as_deref(), Some("1.60.1.70707"));
        assert_eq!(game.interface, Some(16001));
    }

    #[test]
    fn opens_from_any_folder_inside() {
        let dir = fake_install();
        for inner in ["_forever_", "_forever_/Interface/AddOns"] {
            assert_eq!(Game::open(&dir.path().join(inner)).unwrap().root, dir.path());
        }
    }

    #[test]
    fn interface_numbers() {
        assert_eq!(interface_for("1.60.1.70707"), Some(16001));
        assert_eq!(interface_for("12.1.5.1"), Some(120105));
        assert_eq!(interface_for("nope"), None);
    }
}
