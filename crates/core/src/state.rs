//! What Gryphon installed, kept beside AddOns (`Interface/gryphon.json`) so it travels with
//! the game folder and the CLI and the app read the same record.

use std::collections::BTreeMap;
use std::path::PathBuf;

use gryphon_shared::Source;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::wow::Game;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub addons: BTreeMap<String, Installed>,
    /// The game's own value for each setting Gryphon changed in Config.wtf, from before it did,
    /// keyed by the CVar in lower case. `None` when Config.wtf had no line for it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub settings: BTreeMap<String, Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Installed {
    pub name: String,
    pub source: Source,
    pub tag: String,
    pub asset: String,
    /// The folders in AddOns this addon put there.
    pub folders: Vec<String>,
    pub installed_at: u64,
}

impl State {
    pub fn path(game: &Game) -> PathBuf {
        game.interface_dir().join("gryphon.json")
    }

    pub fn load(game: &Game) -> Result<State> {
        match std::fs::read(State::path(game)) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(State::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Writes beside the old record, then swaps it in, so a crash never leaves half a file.
    pub fn save(&self, game: &Game) -> Result<()> {
        let path = State::path(game);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(temp, path)?;
        Ok(())
    }

    pub fn owner_of(&self, folder: &str) -> Option<&str> {
        self.addons.iter().find(|(_, a)| a.folders.iter().any(|f| f.eq_ignore_ascii_case(folder))).map(|(id, _)| id.as_str())
    }
}
