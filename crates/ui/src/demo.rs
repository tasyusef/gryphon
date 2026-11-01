//! The browser preview's answers (`trunk serve` in a plain browser): a fixed library, so the
//! screens can be looked at without the app. Anything that would change files says so.

use serde_json::{Value, json};

pub fn answer(command: &str) -> Result<Value, String> {
    match command {
        "library" | "check_updates" => Ok(library()),
        "settings" => Ok(json!({ "wow_path": null, "has_token": false, "light": false })),
        // The preview is always the newest version.
        "check_app_update" => Ok(Value::Null),
        _ => Err("This is the browser preview. Open the Gryphon app to change addons.".into()),
    }
}

fn library() -> Value {
    let pick = |id: &str, name: &str, blurb: &str, repo: &str, prefix: Option<&str>, needs: &[&str], size: u32| {
        json!({
            "id": id, "name": name, "blurb": blurb,
            "source": { "repo": repo, "tag_prefix": prefix },
            "featured": true, "needs": needs, "size_hint_mb": size,
            "installed": null, "latest": null, "problem": null,
        })
    };
    let installed = |tag: &str| json!({ "tag": tag, "folders": [], "installed_at": 1_793_000_000u64 });
    let latest = |tag: &str, mb: u64| json!({ "tag": tag, "published_at": "2026-10-28T12:00:00Z", "size": mb << 20 });

    let mut openmap = pick("openmap", "OpenMap", "One seamless world map. Zoom from the whole world down to street level, no clicking into zones.", "openmap-world/openmap", None, &[], 90);
    openmap["installed"] = installed("v0.7.4-beta");
    openmap["latest"] = latest("v0.7.5-beta", 86);
    let mut questie = pick("questie", "Questie", "Quest helper. Quest givers, objectives and turn-ins on your map.", "Questie/Questie", None, &[], 93);
    questie["installed"] = installed("bundle/v12.3.1+v1.0.9");
    questie["latest"] = latest("bundle/v12.3.1+v1.0.9", 88);
    let mut dialogue = pick("dialogueui", "DialogueUI", "Turns the quest window into a full-screen conversation.", "Peterodox/YUI-Dialogue", None, &[], 3);
    dialogue["latest"] = latest("v1.0.6", 9);
    let mut spoken = pick("spoken", "Spoken", "AI-voiced quest dialogue, zone lore and books, with subtitles. Needs a voice pack.", "rusty-key/spoken-wow", Some("spoken/v"), &[], 72);
    spoken["latest"] = latest("spoken/v3.6.0", 68);
    let mut voices = pick("spoken-quest-voices", "Spoken quest voices", "English voices for every quest and NPC chat. A big download.", "rusty-key/spoken-wow", Some("quests-audio/v"), &["spoken"], 1479);
    voices["latest"] = latest("quests-audio/v2.4.0", 1410);
    let mut leatrix = json!({
        "id": "leatrix/leatrix-plus", "name": "leatrix-plus", "blurb": null,
        "source": { "repo": "leatrix/leatrix-plus" }, "featured": false, "needs": [], "size_hint_mb": null,
        "installed": installed("v4.1.2"), "latest": null,
        "problem": "leatrix/leatrix-plus has no release with a WoW Forever zip.",
    });
    leatrix["installed"]["folders"] = json!(["Leatrix_Plus"]);

    json!({
        "game": {
            "root": "/Applications/World of Warcraft",
            "addons_dir": "/Applications/World of Warcraft/_forever_/Interface/AddOns",
            "version": "1.60.1.70707",
            "interface": 16001,
        },
        "addons": [openmap, questie, dialogue, spoken, voices, leatrix],
        "other": [
            { "name": "WeakAuras", "title": "WeakAuras", "version": "5.19.2", "interface": [11507, 50500], "fit": "other_client" },
            { "name": "Details", "title": "Details! Damage Meter", "version": "#Details.20260901", "interface": [16001], "fit": "forever" },
            { "name": "MyNotes", "title": null, "version": null, "interface": [], "fit": "no_toc" },
        ],
    })
}
