//! How numbers and tags read on screen.

use gryphon_shared::Source;

/// `88 MB`, `1.4 GB`.
pub fn size(bytes: u64) -> String {
    const MB: u64 = 1 << 20;
    if bytes >= 1000 * MB {
        format!("{:.1} GB", bytes as f64 / (1024 * MB) as f64)
    } else {
        format!("{} MB", (bytes / MB).max(1))
    }
}

/// A release tag as a version: the addon's tag prefix and any `name/` part dropped, then a
/// leading `v`. `spoken/v3.6.0` → `3.6.0`, `bundle/v12.3.1+v1.0.9` → `12.3.1+v1.0.9`. A
/// CurseForge tag is the file's name and its id, `11.2.10#7398127` → `11.2.10`.
pub fn version(tag: &str, source: &Source) -> String {
    if source.curseforge.is_some() {
        let name = tag.rsplit_once('#').map_or(tag, |(name, _)| name);
        return name.strip_prefix(['v', 'V']).filter(|rest| rest.starts_with(|c: char| c.is_ascii_digit())).unwrap_or(name).to_string();
    }
    let tag = source.tag_prefix.as_deref().and_then(|p| tag.strip_prefix(p)).unwrap_or(tag);
    let tag = tag.rsplit('/').next().unwrap_or(tag);
    tag.strip_prefix(['v', 'V']).unwrap_or(tag).to_string()
}

/// [`version`] as it reads on screen: `v3.6.0`, or a name that isn't a number as it is.
pub fn shown_version(tag: &str, source: &Source) -> String {
    let version = version(tag, source);
    if version.starts_with(|c: char| c.is_ascii_digit()) { format!("v{version}") } else { version }
}

/// `1.2M`, `48K`, `950`.
pub fn downloads(n: u64) -> String {
    match n {
        1_000_000.. => format!("{:.1}M", n as f64 / 1_000_000.0).replace(".0M", "M"),
        1_000.. => format!("{}K", n / 1_000),
        _ => n.to_string(),
    }
}

/// `3 updates`, `1 update`.
pub fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// A release's date, `2026-10-28T12:00:00Z` → `28 Oct 2026`. Anything else stays as it is.
pub fn date(iso: &str) -> String {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let mut parts = iso.get(..10).unwrap_or(iso).split('-');
    let (Some(year), Some(month), Some(day)) = (parts.next(), parts.next(), parts.next()) else { return iso.to_string() };
    match (month.parse::<usize>(), day.parse::<u32>()) {
        (Ok(m @ 1..=12), Ok(d)) => format!("{d} {} {year}", MONTHS[m - 1]),
        _ => iso.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_read_the_same_from_either_site() {
        let curse = Source::curseforge(1, "masque");
        assert_eq!(shown_version("11.2.10#7398127", &curse), "v11.2.10");
        assert_eq!(shown_version("v0.4.0-beta#9", &curse), "v0.4.0-beta");
        assert_eq!(shown_version("Masque-11.2.10#7398127", &curse), "Masque-11.2.10");
        assert_eq!(shown_version("bundle/v12.3.1+v1.0.9", &Source::github("Questie/Questie")), "v12.3.1+v1.0.9");
    }

    #[test]
    fn download_counts() {
        assert_eq!(downloads(8_916_674), "8.9M");
        assert_eq!(downloads(2_000_000), "2M");
        assert_eq!(downloads(48_120), "48K");
        assert_eq!(downloads(950), "950");
    }
}
