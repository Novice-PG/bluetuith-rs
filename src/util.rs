//! Small formatting helpers shared by the views.

use std::cmp::Ordering;

use crate::theme::Theme;
use unicode_width::UnicodeWidthStr;

/// ASCII case-insensitive string comparison without allocating.
///
/// Equivalent to `a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase())`, but
/// byte-wise so it can be used inside a sort comparator (called O(n log n)
/// times per frame) without two temporary `String`s per comparison.
pub fn cmp_ascii_ci(a: &str, b: &str) -> Ordering {
    let mut ai = a.bytes();
    let mut bi = b.bytes();
    loop {
        match (ai.next(), bi.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let (x, y) = (x.to_ascii_lowercase(), y.to_ascii_lowercase());
                if x != y {
                    return x.cmp(&y);
                }
            }
        }
    }
}

/// Case-insensitive substring search without allocating. `needle` must
/// already be ASCII-lowercased (as the filter string is). Equivalent to
/// `haystack.to_ascii_lowercase().contains(needle)`.
pub fn contains_ascii_ci(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let h = haystack.as_bytes();
    let n = needle.as_bytes();
    if n.len() > h.len() {
        return false;
    }
    h.windows(n.len())
        .any(|w| w.iter().zip(n).all(|(&x, &y)| x.to_ascii_lowercase() == y))
}

pub fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} {}", UNITS[i])
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

pub fn duration(ms: u64) -> String {
    let total = ms / 1000;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Truncate to a display width, adding an ellipsis when cut.
///
/// Uses `UnicodeWidthStr` on the accumulated prefix, not the sum of per-char
/// `UnicodeWidthChar` widths: the latter disagrees for grapheme clusters
/// (keycap `1️⃣`, ZWJ families `👨‍👩‍👧`, skin-tone `👍🏽`), which made an emoji
/// string overflow the requested width. Reserve one cell for the ellipsis and
/// stop before exceeding `width`.
pub fn truncate(s: &str, width: usize) -> String {
    if UnicodeWidthStr::width(s) <= width {
        return s.to_string();
    }
    if width == 0 {
        return String::new();
    }
    let budget = width - 1; // reserve the ellipsis cell
                            // Fast path: ASCII is one cell per byte, so no grapheme measurement needed.
    if s.is_ascii() {
        let mut out = String::from(&s[..budget.min(s.len())]);
        out.push('…');
        return out;
    }
    let mut out = String::new();
    for c in s.chars() {
        out.push(c);
        if UnicodeWidthStr::width(out.as_str()) > budget {
            out.pop();
            break;
        }
    }
    out.push('…');
    out
}

pub fn pad(s: &str, width: usize) -> String {
    let w = UnicodeWidthStr::width(s);
    if w >= width {
        truncate(s, width)
    } else {
        format!("{s}{}", " ".repeat(width - w))
    }
}

/// Unicode block signal bars, e.g. `▂▄▆█`. Level 0 (no RSSI) is a flat dash
/// so the column keeps its width; `signal_color` renders it dim, not "error".
pub fn signal_glyph(bars: u8) -> &'static str {
    match bars {
        0 => "────",
        1 => "▂───",
        2 => "▂▄──",
        3 => "▂▄▆─",
        _ => "▂▄▆█",
    }
}

/// Colour for a signal level. `0` is "no signal" and is dim so an unpaired or
/// out-of-range device does not read as a broken one.
pub fn signal_color(bars: u8, t: &Theme) -> ratatui::style::Color {
    match bars {
        0 => t.dim,
        1 => t.error,
        2 => t.warning,
        3 => t.accent,
        _ => t.success,
    }
}

pub fn battery_glyph(pct: u8) -> &'static str {
    match pct {
        0..=10 => "▯",
        11..=35 => "▮▯▯",
        36..=65 => "▮▮▯",
        66..=90 => "▮▮▮",
        _ => "▮▮▮",
    }
}

pub fn spinner(tick: u64) -> &'static str {
    const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    FRAMES[(tick as usize / 2) % FRAMES.len()]
}

pub fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_bytes_scales() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(1024), "1.0 KiB");
        assert_eq!(human_bytes(1024 * 1024), "1.0 MiB");
    }

    #[test]
    fn duration_formats() {
        assert_eq!(duration(0), "0:00");
        assert_eq!(duration(161_000), "2:41");
        assert_eq!(duration(3_661_000), "1:01:01");
    }

    #[test]
    fn truncate_respects_display_width() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(
            UnicodeWidthStr::width(truncate("hello world", 6).as_str()),
            6
        );
    }

    #[test]
    fn ascii_ci_matches_to_lowercase() {
        // The no-alloc helpers must be exactly equivalent to the allocating
        // originals, including for non-ASCII bytes (which to_ascii_lowercase
        // leaves untouched).
        let samples = [
            "",
            "a",
            "A",
            "abc",
            "ABC",
            "AbC",
            "device-01",
            "DEVICE-01",
            "名",
            "名👍",
            "aa名BB",
            "1️⃣",
            "0x1101",
            "AA:BB:CC:DD:EE:FF",
        ];
        for &a in &samples {
            for &b in &samples {
                let expected = a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase());
                assert_eq!(cmp_ascii_ci(a, b), expected, "cmp({a:?},{b:?})");

                let needle = a.to_ascii_lowercase();
                let expected_contains = b.to_ascii_lowercase().contains(&needle);
                assert_eq!(
                    contains_ascii_ci(b, &needle),
                    expected_contains,
                    "contains({b:?}, {needle:?})"
                );
            }
        }
    }

    #[test]
    fn truncate_never_overflows_with_emoji_graphemes() {
        // Regression: per-char UnicodeWidthChar width disagrees with the
        // string-level width for keycaps/ZWJ sequences, so summing char widths
        // let the result overflow the requested width.
        for s in ["1️⃣2️⃣", "👨‍👩‍👧", "❤️🧡💛", "👍🏽👍🏽", "🇺🇸🇨🇳"]
        {
            for w in 1..8 {
                let t = truncate(s, w);
                assert!(
                    UnicodeWidthStr::width(t.as_str()) <= w,
                    "truncate({s:?}, {w}) = {t:?} is wider than {w}"
                );
            }
        }
    }
}
