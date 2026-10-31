# WoW Forever addon manager — research notes

_Researched 2026-10-30. Launch is **Nov 4, 2026, 3pm PT**._

## TL;DR
- **Building our own is very doable.** It's mostly: read `.toc` files, check for updates, download a zip, unzip it.
- **CurseForge is the one hard wall.** It's owned by Overwolf, needs an API key, and authors can block outside apps.
- **GitHub + WoWInterface are free and open.** Most big addons publish there too.
- **4 open-source managers already support Forever.** Good backup for launch day, and good code to learn from.

---

## Your addons (checked 2026-10-30)
**All of them are on GitHub Releases with a real Forever build.** None of them need CurseForge.

| You wanted | Addon | GitHub repo | Forever build? | Latest | Download |
|---|---|---|---|---|---|
| Smooth-scroll world map | **OpenMap** | [openmap-world/openmap](https://github.com/openmap-world/openmap) | ✅ `## Interface: 16001` | v0.7.5-beta (Oct 29) | 90 MB |
| AI-voiced quests | **Spoken** | [rusty-key/spoken-wow](https://github.com/rusty-key/spoken-wow) | ✅ `16001` | v3.6.0 (Oct 28) | 72 MB + voice pack |
| Nicer quest window | **DialogueUI** | [Peterodox/YUI-Dialogue](https://github.com/Peterodox/YUI-Dialogue) | ✅ `16001` + `AllowLoadGameType camelot` | v1.0.6 (Oct 4, same as CurseForge) | small |
| Quest helper | **Questie** | [Questie/Questie](https://github.com/Questie/Questie) | ✅ `release.json` says `forever` / `16001` | v12.3.1 (Oct 26) | 93 MB |

Notes:
- **OpenMap** is still beta. It moved repos recently (the old `jiforever` repo is gone). It uses Questie for vendor and rare icons.
- **Spoken** works with DialogueUI and has a "DialogueUI" narrator style. That's likely the combo you saw.
- **Spoken voice packs are big.** English quests "All" pack = **~1.5 GB** (`quests-audio/v2.4.0`). Zone and book packs are separate.
- **Questie on Forever:** one guide says new Forever-only quests aren't in it yet (secondhand).
- **WoWInterface had none of these** (its Questie is from 2021).

Alternatives if these aren't the ones: Forever Simple World Map (CurseForge only, still click-into-zone), VoiceOver Forever, SpeakStone, WoW F Quest Dialogue.

### What this means for our tool
- **GitHub-first covers everything you want.** WoWInterface can come later.
- **Check `release.json` first.** It lists flavor + interface, so we know it's a Forever build without downloading. If it's missing, read the `.toc` inside the zip.
- **One repo can hold many addons.** Spoken uses tag prefixes (`spoken/v…`, `quests-audio/v…`), so each addon needs a "which tag prefix" setting.
- **Big downloads need care:** progress bar, resume, and skip if unchanged.
- **Repos move.** Show a clear "source not found" error instead of failing silently.
- **GitHub limit:** 60 requests/hour without a token. Fine for a handful of addons; a token is optional.

## Your install (checked on this Mac)
- Folder: `/Applications/World of Warcraft/_forever_`
- Product code: `wowf` · Build: `1.60.1.70707`
- Addons will go in `_forever_/Interface/AddOns` (folder appears after first launch).

## How Forever addons work
- Uses the **modern (Midnight) addon API**, not the old Classic one. Same combat-data limits as Midnight.
- Old Classic addons **need a Forever build** from their author. Don't copy old addon folders over.
- **Interface number:** `16001` (beta/test). Live number not published yet.
- **TOC file names** the game looks for:
  - `AddonName_Forever.toc` (also `_Camelot.toc`, the old code name)
  - `AddonName_Mainline.toc` (shared with retail Midnight)
- Tip: an addon "supports Forever" if its zip has one of those TOC files, or a Mainline TOC with `16001` in `## Interface:`.

## Where addons can come from

| Source | Key needed? | Notes |
|---|---|---|
| **GitHub Releases** | No (optional token = faster) | Free. BigWigs packager added Forever support Sept 17, so many authors auto-publish here. |
| **WoWInterface** | No | Public file list API works (checked: 2.7 MB list, HTTP 200). |
| **Tukui** | No | Only ElvUI / Tukui. |
| **Wago Addons** | Yes, **paid** (Patreon) | Per CurseBreaker docs. |
| **CurseForge** | Yes | See below. |

### CurseForge details
- Two kinds of key:
  - **App key:** apply through a form, reviewed by Overwolf. Can't share or redistribute it.
  - **Personal key:** free from the CurseForge dev console. Each friend would need their own. Gray area.
- Authors can **turn off third-party downloads** → those addons just won't download through the API.
- instawow's author notes API downloads may not count toward author earnings (2022 info, may be outdated).

## Existing open-source managers

| Tool | Type | Forever support | CurseForge | Notes |
|---|---|---|---|---|
| **instawow** | CLI + GUI (Python) | Yes (Sept 17) | Your own key | GitHub, WoWI, Tukui, Wago. GPL-3. |
| **CurseBreaker** | Terminal app | Yes (Oct 26) | **None** | GitHub, WoWI, Tukui, Wago (paid). Runs on macOS 15+. GPL-3. |
| **strongbox** | GUI (Clojure) | Yes (Oct 3) | Not checked | macOS + Linux. AGPL-3. |
| **WowUp** | GUI (Electron) | Yes (Oct 18) | Yes | ⚠️ Now built on **Overwolf's Electron** (`@overwolf/ow-electron`). Overwolf owns CurseForge. |
| Ajour | — | — | — | Dead (archived 2024). |

## What our own tool needs to do
1. **Find the game:** `/Applications/World of Warcraft/_forever_/Interface/AddOns` (Windows path differs, for friends).
2. **Scan installed addons:** read each `.toc` for version and source IDs (`X-Curse-Project-ID`, `X-WoWI-ID`, `X-Wago-ID` — common packager convention).
3. **Check for updates:** GitHub Releases API + WoWInterface file list. CurseForge optional, with the user's own key.
4. **Install:** download zip → confirm it has a Forever TOC → back up `WTF` folder → unzip into `AddOns`.
5. **Share with friends:** a shared addon list file ("our guild pack") they can import.

## Sources
- [World of Warcraft: Forever — Warcraft Wiki](https://warcraft.wiki.gg/wiki/World_of_Warcraft:_Forever)
- [TOC format — Warcraft Wiki](https://warcraft.wiki.gg/wiki/TOC_format)
- [WoW Forever release times — Insider Gaming](https://insider-gaming.com/wow-forever-release-date-times/)
- [WoW Forever overview — Zockify](https://www.zockify.com/forever/)
- [WoW Forever addons — ExitLag](https://www.exitlag.com/blog/wow-forever-addons/)
- [CurseForge API docs](https://docs.curseforge.com/rest-api/)
- [CurseForge 3rd-party API application — support](https://support.curseforge.com/support/solutions/articles/9000208346)
- [CurseForge API key types — CurseForge.ApiProxy](https://github.com/CurseForgeCommunity/CurseForge.ApiProxy)
- [Wago API docs](https://docs.wago.io/)
- [OpenMap on CurseForge](https://www.curseforge.com/wow/addons/openmap) · [DialogueUI on CurseForge](https://www.curseforge.com/wow/addons/dialogueui) · [VoiceOver Forever](https://www.curseforge.com/wow/addons/voiceover-forever)
- [Dialogue UI, Spoken & Lore Forever — Teamoverpowered](https://teamoverpowered.com/forever/news/dialogue-ui-spoken-lore-forever-immersion/)
- [Leveling addons for Forever — kami-labs](https://kami-labs.fr/en/wow-forever/addons-leveling-wow-forever/)
- [instawow](https://github.com/layday/instawow) · [CurseBreaker](https://github.com/AcidWeb/CurseBreaker) · [strongbox](https://github.com/ogri-la/strongbox) · [WowUp](https://github.com/WowUp/WowUp) · [BigWigs packager](https://github.com/BigWigsMods/packager)
