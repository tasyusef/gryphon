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
