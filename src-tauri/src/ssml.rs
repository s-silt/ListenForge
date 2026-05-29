//! SSML / text helpers shared across the audio pipeline.
//!
//! `render_part_english_ssml` has been removed — edge-tts does not support
//! `<break>` elements in SSML.  The audio pipeline now uses per-segment
//! synthesis with silent MP3 frames instead (see [`crate::audio`]).

// ── XML escape ────────────────────────────────────────────────────────────────

/// Escape the five XML special characters so they are safe inside SSML text.
#[allow(dead_code)]
pub(crate) fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ── Label helpers ─────────────────────────────────────────────────────────────

/// Strip the Chinese portion from a label string.
///
/// Removes everything inside `（…）` (full-width brackets) and `(…)` (half-width).
/// E.g. `"Part One (第一大题)"` → `"Part One"`.
pub(crate) fn strip_chinese_parens(label: &str) -> String {
    let mut out = String::new();
    let mut depth: u32 = 0;
    for ch in label.chars() {
        match ch {
            '（' | '(' => depth += 1,
            '）' | ')' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            _ => {
                if depth == 0 {
                    out.push(ch);
                }
            }
        }
    }
    out.trim().to_string()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_chinese_parens_removes_half_width() {
        assert_eq!(
            strip_chinese_parens("Part One (第一大题)"),
            "Part One"
        );
    }

    #[test]
    fn strip_chinese_parens_removes_full_width() {
        assert_eq!(
            strip_chinese_parens("Part Two（二）. Listen."),
            "Part Two. Listen."
        );
    }

    #[test]
    fn strip_chinese_parens_no_parens() {
        assert_eq!(
            strip_chinese_parens("Plain label"),
            "Plain label"
        );
    }

    #[test]
    fn strip_chinese_parens_nested() {
        // Nested brackets: everything inside the outer brackets is stripped.
        // The space before the opening bracket is preserved, producing two
        // spaces between A and B — trim() only strips leading/trailing.
        assert_eq!(
            strip_chinese_parens("A (outer (inner) still outer) B"),
            "A  B"
        );
    }

    #[test]
    fn xml_escape_special_chars() {
        let escaped = xml_escape("a & b < c > d \"e\" 'f'");
        assert!(escaped.contains("&amp;"));
        assert!(escaped.contains("&lt;"));
        assert!(escaped.contains("&gt;"));
        assert!(escaped.contains("&quot;"));
        assert!(escaped.contains("&apos;"));
    }
}
