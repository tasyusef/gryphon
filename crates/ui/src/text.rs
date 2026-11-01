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
/// leading `v`. `spoken/v3.6.0` → `3.6.0`, `bundle/v12.3.1+v1.0.9` → `12.3.1+v1.0.9`.
pub fn version(tag: &str, source: &Source) -> String {
    let tag = source.tag_prefix.as_deref().and_then(|p| tag.strip_prefix(p)).unwrap_or(tag);
    let tag = tag.rsplit('/').next().unwrap_or(tag);
    tag.strip_prefix(['v', 'V']).unwrap_or(tag).to_string()
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
