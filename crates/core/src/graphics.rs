//! The game's own settings that its graphics menu stops short of. They're CVars, kept in
//! `_forever_/WTF/Config.wtf` as `SET name "value"` lines. The game reads that file when it
//! starts and writes all of it back when it closes, so Gryphon only changes it while the game is
//! shut. It keeps what each line said before (in `gryphon.json`) so it can put it back.

use std::path::PathBuf;

use gryphon_shared::{Graphics, Tweak};

use crate::error::{Result, msg};
use crate::install::BACKUP_DIR;
use crate::state::State;
use crate::wow::Game;

pub struct Setting {
    /// The CVar.
    pub id: &'static str,
    pub name: &'static str,
    pub about: &'static str,
    pub tip: &'static str,
    pub min: i64,
    pub max: i64,
    /// What the game uses when Config.wtf has no line for it.
    pub default: i64,
    pub toggle: bool,
    /// The screen shows the CVar times this, so a fraction like 1.3 is a whole number (130).
    pub scale: i64,
}

impl Setting {
    /// A Config.wtf value as the screen shows it: `"7000.0"` is 7000, and `"1.300000"` is 130
    /// when the scale is 100.
    fn shown(&self, value: &str) -> Option<i64> {
        value.trim().parse::<f64>().ok().filter(|n| n.is_finite()).map(|n| (n * self.scale as f64).round() as i64)
    }

    /// The other way: what goes in Config.wtf for a number on the screen.
    fn written(&self, value: i64) -> String {
        if self.scale == 1 { value.to_string() } else { (value as f64 / self.scale as f64).to_string() }
    }
}

pub const SETTINGS: &[Setting] = &[
    Setting {
        id: "farclip",
        name: "Draw distance",
        about: "How far away the world is drawn.",
        tip: "Around 4000 is enough to see Nordrassil on Mount Hyjal from the Barrens. Higher costs frames, and moving the game's own View Distance slider sets it again.",
        min: 500,
        max: 10000,
        default: 7000,
        toggle: false,
        scale: 1,
    },
    Setting {
        id: "groundEffectDensity",
        name: "Grass and clutter",
        about: "How thick the grass and small plants are.",
        tip: "48 is the game's usual. 256 is the most it takes, and that can cost frames in busy places. Moving the game's own Ground Clutter slider sets it again.",
        min: 16,
        max: 256,
        default: 48,
        toggle: false,
        scale: 1,
    },
    Setting {
        id: "groundEffectDist",
        name: "Grass distance",
        about: "How far out grass and small plants are drawn.",
        tip: "The game's own Ground Clutter slider goes up to 200. 500 fills in hillsides further out, which costs some frames. Moving that slider sets it again.",
        min: 50,
        max: 500,
        default: 70,
        toggle: false,
        scale: 1,
    },
    Setting {
        id: "groundEffectFade",
        name: "Grass fade",
        about: "Where grass starts to thin out, so it fades away instead of ending in a line.",
        tip: "70 is the game's usual, even with Ground Clutter at its highest. Keep it a little under Grass distance: 370 goes well with 500.",
        min: 50,
        max: 500,
        default: 70,
        toggle: false,
        scale: 1,
    },
    Setting {
        id: "lodObjectCullSize",
        name: "Small objects",
        about: "Keeps small things like rocks and fences in view further out. Lower keeps more.",
        tip: "Anything smaller than this stops being drawn once it's far away. Try 8 to keep most of it.",
        min: 0,
        max: 40,
        default: 18,
        toggle: false,
        scale: 1,
    },
    Setting {
        id: "lodObjectFadeScale",
        name: "Detail distance",
        about: "How far away buildings and trees keep their full detail.",
        tip: "100 is the game's usual. 200 keeps the detailed models twice as far out, which costs some frames.",
        min: 50,
        max: 300,
        default: 100,
        toggle: false,
        scale: 1,
    },
    Setting {
        id: "weatherDensity",
        name: "Weather",
        about: "How heavy rain and snow look.",
        tip: "2 is the game's usual and 3 is the heaviest. 0 is the lightest, though weather never goes away completely.",
        min: 0,
        max: 3,
        default: 2,
        toggle: false,
        scale: 1,
    },
    Setting {
        id: "ResampleAlwaysSharpen",
        name: "Sharpening",
        about: "Sharpens the picture all the time, not only when the game renders below full size.",
        tip: "The game sharpens on its own when Render Scale is under 100%. This keeps it on at 100% too. It costs next to nothing.",
        min: 0,
        max: 1,
        default: 0,
        toggle: true,
        scale: 1,
    },
    Setting {
        id: "HDROutput",
        name: "HDR",
        about: "Turns on HDR for screens that have it, like OLED and mini-LED.",
        tip: "Turn HDR on for the screen in your computer's display settings first. If the screen can't show HDR, the game stays as it was.",
        min: 0,
        max: 1,
        default: 0,
        toggle: true,
        scale: 1,
    },
    Setting {
        id: "HDRFactor",
        name: "HDR highlights",
        about: "How much brighter than normal the brightest spots can get with HDR on.",
        tip: "200 is the game's usual, twice as bright as without HDR. Try 130 if it looks harsh. Your screen's own limit still caps it.",
        min: 100,
        max: 200,
        default: 200,
        toggle: false,
        scale: 100,
    },
];

/// The game keeps a second copy of some settings for raids and battlegrounds, under this prefix.
/// Where Config.wtf has one, Gryphon changes it too, so a setting holds everywhere.
const RAID: &str = "RAID";

/// A change to one setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Set(i64),
    /// Back to what it was before Gryphon changed it.
    PutBack,
}

pub fn path(game: &Game) -> PathBuf {
    game.client.join("WTF").join("Config.wtf")
}

pub fn find(id: &str) -> Option<&'static Setting> {
    SETTINGS.iter().find(|s| s.id.eq_ignore_ascii_case(id))
}

pub fn read(game: &Game) -> Result<Graphics> {
    let config = load(game)?;
    let state = State::load(game)?;
    let tweaks = SETTINGS
        .iter()
        .map(|s| {
            let value = config.as_ref().and_then(|c| c.get(s.id)).and_then(|v| s.shown(v)).unwrap_or(s.default);
            let before = state.settings.get(&s.id.to_ascii_lowercase()).map(|v| v.as_deref().and_then(|v| s.shown(v)).unwrap_or(s.default));
            Tweak {
                id: s.id.to_string(),
                name: s.name.to_string(),
                about: s.about.to_string(),
                tip: s.tip.to_string(),
                min: s.min,
                max: s.max,
                default: s.default,
                toggle: s.toggle,
                value,
                before,
            }
        })
        .collect();
    Ok(Graphics { tweaks, running: game.running(), missing: config.is_none() })
}

/// Makes the changes, after copying Config.wtf into the backup folder. Nothing is written
/// unless every change is one Gryphon knows and in range.
pub fn change(game: &Game, changes: &[(String, Change)]) -> Result<Graphics> {
    if game.running() {
        return Err(msg("Close WoW Forever first. It saves its own settings when it closes, over the top of anything changed while it's open."));
    }
    let file = path(game);
    let text = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(msg("WoW Forever hasn't saved its settings yet. Start it once, close it, then try again."));
        }
        Err(e) => return Err(e.into()),
    };
    let mut config = Config::parse(&text);
    let mut state = State::load(game)?;
    for (id, change) in changes {
        let setting = find(id).ok_or_else(|| msg(format!("Gryphon doesn't know a setting called {id}.")))?;
        let raid = format!("{RAID}{}", setting.id);
        match *change {
            Change::Set(value) => {
                if !(setting.min..=setting.max).contains(&value) {
                    return Err(msg(format!("{} goes from {} to {}.", setting.name, setting.min, setting.max)));
                }
                let names: Vec<String> = [setting.id.to_string(), raid].into_iter().filter(|n| n == setting.id || config.get(n).is_some()).collect();
                for name in names {
                    let key = name.to_ascii_lowercase();
                    let before = state.settings.entry(key.clone()).or_insert_with(|| config.get(&name).map(String::from)).clone();
                    config.set(&name, Some(&setting.written(value)));
                    // set back to the game's own value by hand: nothing left to put back
                    if before.as_deref().and_then(|v| setting.shown(v)) == Some(value) {
                        state.settings.remove(&key);
                    }
                }
            }
            Change::PutBack => {
                for name in [setting.id.to_string(), raid] {
                    if let Some(before) = state.settings.remove(&name.to_ascii_lowercase()) {
                        config.set(&name, before.as_deref());
                    }
                }
            }
        }
    }

    let backup = game.interface_dir().join(BACKUP_DIR);
    std::fs::create_dir_all(&backup)?;
    std::fs::write(backup.join("Config.wtf"), &text)?;
    // the record first: if writing Config.wtf then fails, putting back only rewrites what's there
    state.save(game)?;
    let temp = file.with_extension("wtf.tmp");
    std::fs::write(&temp, config.text())?;
    std::fs::rename(temp, &file)?;
    read(game)
}

fn load(game: &Game) -> Result<Option<Config>> {
    match std::fs::read_to_string(path(game)) {
        Ok(text) => Ok(Some(Config::parse(&text))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Config.wtf a line at a time, so a change leaves every other line as the game wrote it.
struct Config {
    lines: Vec<String>,
    newline: &'static str,
}

impl Config {
    fn parse(text: &str) -> Config {
        let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
        Config { lines: text.lines().map(String::from).collect(), newline }
    }

    fn text(&self) -> String {
        let mut text = self.lines.join(self.newline);
        text.push_str(self.newline);
        text
    }

    /// What the file sets `name` to. CVar names ignore case, and the game reads the file top to
    /// bottom, so the last line wins.
    fn get(&self, name: &str) -> Option<&str> {
        self.lines.iter().rev().filter_map(|l| set_line(l)).find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| v)
    }

    /// Sets `name` on its own line, where the game had it, or takes the line out with `None`.
    fn set(&mut self, name: &str, value: Option<&str>) {
        let mut done = false;
        self.lines.retain_mut(|line| {
            let Some(found) = set_line(line).map(|(n, _)| n.to_string()).filter(|n| n.eq_ignore_ascii_case(name)) else {
                return true;
            };
            match value {
                Some(value) if !done => {
                    *line = format!("SET {found} \"{value}\"");
                    done = true;
                    true
                }
                _ => false,
            }
        });
        if let (Some(value), false) = (value, done) {
            self.lines.push(format!("SET {name} \"{value}\""));
        }
    }
}

/// `SET farclip "7000"` as `("farclip", "7000")`.
fn set_line(line: &str) -> Option<(&str, &str)> {
    let (word, rest) = line.trim().split_once(char::is_whitespace)?;
    if !word.eq_ignore_ascii_case("SET") {
        return None;
    }
    let rest = rest.trim_start();
    let (name, value) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let value = value.trim();
    Some((name, value.strip_prefix('"').and_then(|v| v.strip_suffix('"')).unwrap_or(value)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = "SET locale \"enUS\"\r\nSET farclip \"7000\"\r\nSET RAIDfarclip \"7000\"\r\nSET groundEffectDensity \"48\"\r\nSET gxApi \"MTL\"\r\n";

    fn game_with(config: Option<&str>) -> (tempfile::TempDir, Game) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("_forever_/Interface/AddOns")).unwrap();
        std::fs::create_dir_all(dir.path().join("_forever_/WTF")).unwrap();
        if let Some(config) = config {
            std::fs::write(dir.path().join("_forever_/WTF/Config.wtf"), config).unwrap();
        }
        let game = Game::open(dir.path()).unwrap();
        (dir, game)
    }

    fn value(graphics: &Graphics, id: &str) -> (i64, Option<i64>) {
        let tweak = graphics.tweaks.iter().find(|t| t.id == id).unwrap();
        (tweak.value, tweak.before)
    }

    #[test]
    fn reads_what_the_file_says_and_the_defaults_for_the_rest() {
        let (_dir, game) = game_with(Some(CONFIG));
        let graphics = read(&game).unwrap();
        assert!(!graphics.missing);
        assert_eq!(value(&graphics, "farclip"), (7000, None));
        assert_eq!(value(&graphics, "lodObjectFadeScale"), (100, None));
        assert_eq!(value(&graphics, "ResampleAlwaysSharpen"), (0, None));
    }

    #[test]
    fn changes_lines_in_place_and_puts_them_back() {
        let (_dir, game) = game_with(Some(CONFIG));
        let changed = change(
            &game,
            &[("farclip".into(), Change::Set(5000)), ("ResampleAlwaysSharpen".into(), Change::Set(1)), ("groundeffectdensity".into(), Change::Set(48))],
        )
        .unwrap();
        assert_eq!(value(&changed, "farclip"), (5000, Some(7000)));
        assert_eq!(value(&changed, "ResampleAlwaysSharpen"), (1, Some(0)));
        // set to what it already was: nothing to put back
        assert_eq!(value(&changed, "groundEffectDensity"), (48, None));
        let text = std::fs::read_to_string(path(&game)).unwrap();
        assert_eq!(
            text,
            "SET locale \"enUS\"\r\nSET farclip \"5000\"\r\nSET RAIDfarclip \"5000\"\r\nSET groundEffectDensity \"48\"\r\nSET gxApi \"MTL\"\r\nSET ResampleAlwaysSharpen \"1\"\r\n"
        );
        let backup = std::fs::read_to_string(game.interface_dir().join(BACKUP_DIR).join("Config.wtf")).unwrap();
        assert_eq!(backup, CONFIG);

        let back = change(&game, &[("farclip".into(), Change::PutBack), ("ResampleAlwaysSharpen".into(), Change::PutBack)]).unwrap();
        assert_eq!(value(&back, "farclip"), (7000, None));
        assert_eq!(value(&back, "ResampleAlwaysSharpen"), (0, None));
        assert_eq!(std::fs::read_to_string(path(&game)).unwrap(), CONFIG);
        assert!(State::load(&game).unwrap().settings.is_empty());
    }

    #[test]
    fn keeps_the_first_change_as_the_one_to_put_back() {
        let (_dir, game) = game_with(Some(CONFIG));
        change(&game, &[("farclip".into(), Change::Set(5000))]).unwrap();
        let again = change(&game, &[("farclip".into(), Change::Set(9000))]).unwrap();
        assert_eq!(value(&again, "farclip"), (9000, Some(7000)));
    }

    #[test]
    fn writes_fractions_the_way_the_game_reads_them() {
        let (_dir, game) = game_with(Some(CONFIG));
        assert_eq!(value(&read(&game).unwrap(), "HDRFactor"), (200, None));
        let changed = change(&game, &[("HDRFactor".into(), Change::Set(130))]).unwrap();
        assert_eq!(value(&changed, "HDRFactor"), (130, Some(200)));
        assert!(std::fs::read_to_string(path(&game)).unwrap().ends_with("SET HDRFactor \"1.3\"\r\n"));
        change(&game, &[("HDRFactor".into(), Change::PutBack)]).unwrap();
        assert_eq!(std::fs::read_to_string(path(&game)).unwrap(), CONFIG);
    }

    #[test]
    fn refuses_what_it_does_not_know_or_is_out_of_range() {
        let (_dir, game) = game_with(Some(CONFIG));
        assert!(change(&game, &[("gxApi".into(), Change::Set(1))]).is_err());
        let err = change(&game, &[("farclip".into(), Change::Set(5000)), ("groundEffectDensity".into(), Change::Set(9999))]).unwrap_err();
        assert_eq!(err.to_string(), "Grass and clutter goes from 16 to 256.");
        // nothing was written
        assert_eq!(std::fs::read_to_string(path(&game)).unwrap(), CONFIG);
    }

    #[test]
    fn waits_for_the_game_to_write_its_settings_first() {
        let (_dir, game) = game_with(None);
        assert!(read(&game).unwrap().missing);
        assert!(change(&game, &[("farclip".into(), Change::Set(5000))]).is_err());
    }

    #[test]
    fn reads_set_lines() {
        assert_eq!(set_line("SET farclip \"7000\""), Some(("farclip", "7000")));
        assert_eq!(set_line("  set gxWindow 1"), Some(("gxWindow", "1")));
        assert_eq!(set_line("SETTINGS"), None);
        let farclip = find("farclip").unwrap();
        assert_eq!(farclip.shown("7000.4"), Some(7000));
        assert_eq!(farclip.shown("MTL"), None);
        assert_eq!(farclip.written(5000), "5000");
        let hdr = find("hdrfactor").unwrap();
        assert_eq!(hdr.shown("1.300000"), Some(130));
        assert_eq!(hdr.shown("2"), Some(200));
        assert_eq!(hdr.written(130), "1.3");
        assert_eq!(hdr.written(200), "2");
    }
}
