//! The built-in picks: addons checked by hand to have Forever builds on GitHub.

use gryphon_shared::Source;

pub struct Entry {
    pub id: &'static str,
    pub name: &'static str,
    pub blurb: &'static str,
    /// What it does, a point at a time, for its own page. From each addon's README.
    pub about: &'static [&'static str],
    pub repo: &'static str,
    pub tag_prefix: Option<&'static str>,
    /// Catalog ids to install first.
    pub needs: &'static [&'static str],
    pub size_hint_mb: u32,
}

impl Entry {
    pub fn source(&self) -> Source {
        Source { repo: self.repo.to_string(), tag_prefix: self.tag_prefix.map(String::from), asset: None }
    }
}

pub const CATALOG: &[Entry] = &[
    Entry {
        id: "openmap",
        name: "OpenMap",
        blurb: "One seamless world map. Zoom from the whole world down to street level, no clicking into zones.",
        about: &[
            "Stitches every zone into one map. Scroll out of Elwynn into Westfall, then the whole continent, then the world.",
            "Dungeon and raid entrances with their level ranges. Click one to open that dungeon's own map.",
            "Flight paths, boats and zeppelins, with their routes.",
            "Search for a zone, city, dungeon or flight master, and drop pins with notes.",
            "With Questie, it shows vendors, trainers, spirit healers and rares too.",
            "Still in beta, so expect a few rough edges.",
        ],
        repo: "openmap-world/openmap",
        tag_prefix: None,
        needs: &[],
        size_hint_mb: 90,
    },
    Entry {
        id: "questie",
        name: "Questie",
        blurb: "Quest helper. Quest givers, objectives and turn-ins on your map.",
        about: &[
            "Marks where to pick up quests, where their objectives are and where to hand them in, on the world map and the minimap.",
            "Mob and item tooltips say which of your quests they count for.",
            "A tracker lists your objectives as you go.",
            "OpenMap uses it for its vendor and rare icons.",
        ],
        repo: "Questie/Questie",
        tag_prefix: None,
        needs: &[],
        size_hint_mb: 93,
    },
    Entry {
        id: "dialogueui",
        name: "DialogueUI",
        blurb: "Turns the quest window into a full-screen conversation.",
        about: &[
            "Swaps the quest and gossip windows for a full-screen conversation, with the camera turned to the NPC.",
            "Works with a keyboard and mouse, or an Xbox, PlayStation or Switch controller.",
            "Can read the dialogue out loud with your computer's text-to-speech voices.",
            "Books, letters and notes get a new reading window too.",
            "Spoken has a narrator style made to match it.",
        ],
        repo: "Peterodox/YUI-Dialogue",
        tag_prefix: None,
        needs: &[],
        size_hint_mb: 3,
    },
    Entry {
        id: "spoken",
        name: "Spoken",
        blurb: "AI-voiced quest dialogue, zone lore and books, with subtitles. Needs a voice pack.",
        about: &[
            "Plays voiced quest dialogue, zone lore, boss stories, and the books and letters you find in the world.",
            "Shows the words as subtitles or in a small or large window, or just plays the voice.",
            "Comes with its modules for quests, places, books and bosses. The recordings are separate downloads.",
            "For quests, get Spoken quest voices too.",
        ],
        repo: "rusty-key/spoken-wow",
        tag_prefix: Some("spoken/v"),
        needs: &[],
        size_hint_mb: 72,
    },
    Entry {
        id: "spoken-quest-voices",
        name: "Spoken quest voices",
        blurb: "English voices for every quest and NPC chat. A big download.",
        about: &[
            "English recordings for quest dialogue and NPC gossip, Alliance and Horde.",
            "Spoken plays them, so Gryphon installs Spoken first.",
        ],
        repo: "rusty-key/spoken-wow",
        tag_prefix: Some("quests-audio/v"),
        needs: &["spoken"],
        size_hint_mb: 1479,
    },
];

pub fn find(id: &str) -> Option<&'static Entry> {
    CATALOG.iter().find(|e| e.id == id)
}

/// The pick a GitHub repo is, so pasting a pick's repo installs the pick rather than a second
/// copy under another id. A repo with several picks gives the first unless a tag prefix says.
pub fn for_source(source: &Source) -> Option<&'static Entry> {
    CATALOG.iter().find(|e| {
        e.repo.eq_ignore_ascii_case(&source.repo) && source.tag_prefix.as_deref().is_none_or(|p| e.tag_prefix == Some(p))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picks_repo_is_the_pick() {
        assert_eq!(for_source(&Source::github("peterodox/yui-dialogue")).map(|e| e.id), Some("dialogueui"));
        assert_eq!(for_source(&Source::github("rusty-key/spoken-wow")).map(|e| e.id), Some("spoken"));
        let voices = Source { tag_prefix: Some("quests-audio/v".into()), ..Source::github("rusty-key/spoken-wow") };
        assert_eq!(for_source(&voices).map(|e| e.id), Some("spoken-quest-voices"));
        assert!(for_source(&Source::github("leatrix/leatrix-plus")).is_none());
    }
}
