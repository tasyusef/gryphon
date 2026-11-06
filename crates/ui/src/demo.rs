//! The browser preview's answers (`trunk serve` in a plain browser): a fixed library, so the
//! screens can be looked at without the app. Anything that would change files says so.

use serde_json::{Value, json};

pub fn answer(command: &str) -> Result<Value, String> {
    match command {
        "library" | "check_updates" => Ok(library()),
        "settings" => Ok(json!({ "wow_path": null, "has_token": false, "light": false })),
        // The preview is always the newest version.
        "check_app_update" => Ok(Value::Null),
        "graphics" => Ok(graphics()),
        _ => Err("This is the browser preview. Open the Gryphon app to change anything.".into()),
    }
}

/// The settings as core's `graphics::SETTINGS` has them, with the draw distance changed once.
fn graphics() -> Value {
    let tweak = |id: &str, name: &str, about: &str, tip: &str, (min, max, default): (i64, i64, i64), value: i64, before: Option<i64>| {
        json!({
            "id": id, "name": name, "about": about, "tip": tip,
            "min": min, "max": max, "default": default, "toggle": max == 1,
            "value": value, "before": before,
        })
    };
    json!({
        "running": false,
        "missing": false,
        "tweaks": [
            tweak("farclip", "Draw distance", "How far away the world is drawn.", "Around 4000 is enough to see Nordrassil on Mount Hyjal from the Barrens. Higher costs frames, and moving the game's own View Distance slider sets it again.", (500, 10000, 7000), 5000, Some(7000)),
            tweak("groundEffectDensity", "Grass and clutter", "How thick the grass and small plants are.", "48 is the game's usual. 256 is the most it takes, and that can cost frames in busy places. Moving the game's own Ground Clutter slider sets it again.", (16, 256, 48), 48, None),
            tweak("groundEffectDist", "Grass distance", "How far out grass and small plants are drawn.", "The game's own Ground Clutter slider goes up to 200. 500 fills in hillsides further out, which costs some frames. Moving that slider sets it again.", (50, 500, 70), 200, None),
            tweak("groundEffectFade", "Grass fade", "Where grass starts to thin out, so it fades away instead of ending in a line.", "70 is the game's usual, even with Ground Clutter at its highest. Keep it a little under Grass distance: 370 goes well with 500.", (50, 500, 70), 70, None),
            tweak("lodObjectCullSize", "Small objects", "Keeps small things like rocks and fences in view further out. Lower keeps more.", "Anything smaller than this stops being drawn once it's far away. Try 8 to keep most of it.", (0, 40, 18), 18, None),
            tweak("lodObjectFadeScale", "Detail distance", "How far away buildings and trees keep their full detail.", "100 is the game's usual. 200 keeps the detailed models twice as far out, which costs some frames.", (50, 300, 100), 100, None),
            tweak("weatherDensity", "Weather", "How heavy rain and snow look.", "2 is the game's usual and 3 is the heaviest. 0 is the lightest, though weather never goes away completely.", (0, 3, 2), 2, None),
            tweak("ResampleAlwaysSharpen", "Sharpening", "Sharpens the picture all the time, not only when the game renders below full size.", "The game sharpens on its own when Render Scale is under 100%. This keeps it on at 100% too. It costs next to nothing.", (0, 1, 0), 0, None),
            tweak("HDROutput", "HDR", "Turns on HDR for screens that have it, like OLED and mini-LED.", "Turn HDR on for the screen in your computer's display settings first. If the screen can't show HDR, the game stays as it was.", (0, 1, 0), 0, None),
            tweak("HDRFactor", "HDR highlights", "How much brighter than normal the brightest spots can get with HDR on.", "200 is the game's usual, twice as bright as without HDR. Try 130 if it looks harsh. Your screen's own limit still caps it.", (100, 200, 200), 200, None),
        ],
    })
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
    let mut tome = pick("tome", "Tome", "One row of tabs on top of the windows you open most, so each is a click from the others.", "tasyusef/tome", None, &[], 1);
    tome["latest"] = latest("v0.1.0", 1);
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
        "addons": [openmap, questie, dialogue, spoken, voices, tome, leatrix],
        "other": [
            { "name": "WeakAuras", "title": "WeakAuras", "version": "5.19.2", "interface": [11507, 50500], "fit": "other_client" },
            { "name": "Details", "title": "Details! Damage Meter", "version": "#Details.20260901", "interface": [16001], "fit": "forever" },
            { "name": "MyNotes", "title": null, "version": null, "interface": [], "fit": "no_toc" },
        ],
    })
}
