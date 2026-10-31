//! Addon TOC files: the `## Key: value` header that tells the game what a folder is and which
//! clients it was built for.

use std::path::Path;

use gryphon_shared::{Fit, Folder};

use crate::wow::FOREVER_INTERFACES;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Toc {
    pub title: Option<String>,
    pub version: Option<String>,
    pub interface: Vec<u32>,
    /// `## AllowLoadGameType:`, lowercased. Empty means every game type.
    pub game_types: Vec<String>,
}

pub fn parse(text: &str) -> Toc {
    let mut toc = Toc::default();
    for line in text.trim_start_matches('\u{feff}').lines() {
        let Some((key, value)) = line.strip_prefix("##").and_then(|rest| rest.split_once(':')) else { continue };
        // A key can carry a condition, `## Title [AllowLoadGameType tbc]`; the bare name counts.
        let key = key.split('[').next().unwrap_or(key).trim().to_ascii_lowercase();
        let value = value.trim();
        match key.as_str() {
            "interface" => toc.interface = value.split(',').filter_map(|v| v.trim().parse().ok()).collect(),
            "title" => toc.title = Some(strip_codes(value)).filter(|t| !t.is_empty()),
            // `@project-version@` is the packager's placeholder in an unbuilt source tree.
            "version" if !value.starts_with('@') => toc.version = Some(value.to_string()),
            "allowloadgametype" => {
                toc.game_types = value.split([',', ' ']).filter(|t| !t.is_empty()).map(str::to_ascii_lowercase).collect()
            }
            _ => {}
        }
    }
    toc
}

/// Removes WoW's inline escape codes: colours (`|cffRRGGBB` … `|r`), textures (`|T…|t`) and
/// atlases (`|A…|a`).
pub fn strip_codes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '|' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('c') => {
                for _ in 0..8 {
                    chars.next();
                }
            }
            Some('r') => {}
            Some(open @ ('T' | 'A')) => {
                let close = open.to_ascii_lowercase();
                while let Some(c) = chars.next() {
                    if c == '|' && chars.peek() == Some(&close) {
                        chars.next();
                        break;
                    }
                }
            }
            Some('|') => out.push('|'),
            Some(other) => {
                out.push('|');
                out.push(other);
            }
            None => out.push('|'),
        }
    }
    out.trim().to_string()
}

pub fn fit(toc: &Toc) -> Fit {
    let allowed = ["forever", "camelot", "mainline"];
    if !toc.game_types.is_empty() && !toc.game_types.iter().any(|t| allowed.contains(&t.as_str())) {
        return Fit::Blocked;
    }
    if toc.interface.iter().any(|i| FOREVER_INTERFACES.contains(i)) { Fit::Forever } else { Fit::OtherClient }
}

/// The TOC the Forever client reads for `folder`, from the folder's file names: the client's
/// own suffix first, then the old code name, then the Mainline family, then the plain name.
/// Matched without case, as Windows and macOS do.
pub fn pick<'a>(folder: &str, files: impl IntoIterator<Item = &'a str> + Clone) -> Option<&'a str> {
    let candidates = [
        format!("{folder}_Forever.toc"),
        format!("{folder}_Camelot.toc"),
        format!("{folder}_Mainline.toc"),
        format!("{folder}.toc"),
    ];
    candidates.iter().find_map(|want| files.clone().into_iter().find(|f| f.eq_ignore_ascii_case(want)))
}

/// Reads one addon folder on disk.
pub fn read_folder(dir: &Path) -> Folder {
    let name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let files: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    let has_any_toc = files.iter().any(|f| f.to_ascii_lowercase().ends_with(".toc"));
    let picked = pick(&name, files.iter().map(String::as_str));
    let toc = picked.and_then(|f| std::fs::read(dir.join(f)).ok()).map(|bytes| parse(&String::from_utf8_lossy(&bytes)));
    let fit = match (&toc, has_any_toc) {
        (Some(toc), _) => fit(toc),
        (None, true) => Fit::OtherClient,
        (None, false) => Fit::NoToc,
    };
    let toc = toc.unwrap_or_default();
    Folder { name, title: toc.title, version: toc.version, interface: toc.interface, fit }
}

/// Every folder in an AddOns directory, by name.
pub fn scan(addons: &Path) -> Vec<Folder> {
    let mut folders: Vec<Folder> = std::fs::read_dir(addons)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .map(|e| read_folder(&e.path()))
                .collect()
        })
        .unwrap_or_default();
    folders.sort_by_key(|f| f.name.to_ascii_lowercase());
    folders
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_header() {
        let toc = parse("\u{feff}## Interface: 120107, 11509, 16001\n## Title: |cffe6b85cOpen|rMap\n## Version: @project-version@\n## X-GameType: Camelot [AllowLoadGameType camelot]\nOpenMap.lua\n");
        assert_eq!(toc.interface, vec![120107, 11509, 16001]);
        assert_eq!(toc.title.as_deref(), Some("OpenMap"));
        assert_eq!(toc.version, None);
        assert_eq!(fit(&toc), Fit::Forever);
    }

    #[test]
    fn other_clients_and_blocks() {
        assert_eq!(fit(&parse("## Interface: 11508")), Fit::OtherClient);
        assert_eq!(fit(&parse("## Interface: 16001\n## AllowLoadGameType: vanilla, tbc")), Fit::Blocked);
        assert_eq!(fit(&parse("## Interface: 16001\n## AllowLoadGameType: standard, camelot")), Fit::Forever);
    }

    #[test]
    fn picks_the_most_specific_toc() {
        let files = ["Questie.toc", "Questie_Vanilla.toc", "questie_mainline.toc", "Questie_Forever.toc"];
        assert_eq!(pick("Questie", files), Some("Questie_Forever.toc"));
        assert_eq!(pick("Questie", ["Questie.toc", "questie_mainline.toc"]), Some("questie_mainline.toc"));
        assert_eq!(pick("Questie", ["Questie_Vanilla.toc"]), None);
    }

    #[test]
    fn strips_texture_codes() {
        assert_eq!(strip_codes("|TInterface\\Icons\\x:16|t Spoken |cffb48cffQuests|r"), "Spoken Quests");
    }
}
