//! The built-in picks: addons checked by hand to have Forever builds on GitHub
//! (RESEARCH.md, "Your addons").

use gryphon_shared::Source;

pub struct Entry {
    pub id: &'static str,
    pub name: &'static str,
    pub blurb: &'static str,
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
        repo: "openmap-world/openmap",
        tag_prefix: None,
        needs: &[],
        size_hint_mb: 90,
    },
    Entry {
        id: "questie",
        name: "Questie",
        blurb: "Quest helper. Quest givers, objectives and turn-ins on your map.",
        repo: "Questie/Questie",
        tag_prefix: None,
        needs: &[],
        size_hint_mb: 93,
    },
    Entry {
        id: "dialogueui",
        name: "DialogueUI",
        blurb: "Turns the quest window into a full-screen conversation.",
        repo: "Peterodox/YUI-Dialogue",
        tag_prefix: None,
        needs: &[],
        size_hint_mb: 3,
    },
    Entry {
        id: "spoken",
        name: "Spoken",
        blurb: "AI-voiced quest dialogue, zone lore and books, with subtitles. Needs a voice pack.",
        repo: "rusty-key/spoken-wow",
        tag_prefix: Some("spoken/v"),
        needs: &[],
        size_hint_mb: 72,
    },
    Entry {
        id: "spoken-quest-voices",
        name: "Spoken quest voices",
        blurb: "English voices for every quest and NPC chat. A big download.",
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
