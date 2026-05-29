//! SSML rendering for the "teacher reading" audio pipeline.
//!
//! Generates SSML for a single [`Part`] following the spec §5.1 rhythm:
//! - Number label read at −5% rate + 600ms pause
//! - Body repeated twice with 1500ms gap when `repeat == 2`
//! - Per-item gap (`item.gap_after_ms`)
//! - Part label (English portion) + 2s pause when `read_label == true`

use crate::model::Part;

// ── XML escape ────────────────────────────────────────────────────────────────

/// Escape the five XML special characters so they are safe inside SSML text.
fn xml_escape(s: &str) -> String {
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
fn strip_chinese_parens(label: &str) -> String {
    // Remove content in full-width brackets （…）
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

// ── Main renderer ─────────────────────────────────────────────────────────────

/// Render a full SSML document for one [`Part`] in "teacher reading" style.
///
/// # Arguments
/// * `part`      – the part to render
/// * `voice_id`  – Edge TTS voice name, e.g. `"en-GB-SoniaNeural"`
/// * `base_rate` – the project's base rate offset (−100 … +100); the number
///                 label is further shifted by −5 percentage points
///
/// # SSML structure
/// ```xml
/// <speak version="1.0" xmlns="…" xml:lang="en-US">
///   <voice name="…">
///     <!-- optional part label -->
///     Part One.<break time="2s"/>
///     <!-- per-item blocks -->
///     <prosody rate="-5%">Number 1.</prosody><break time="600ms"/>
///     The cat sat on the mat.<break time="1500ms"/>
///     The cat sat on the mat.<break time="3000ms"/>
///   </voice>
/// </speak>
/// ```
pub fn render_part_english_ssml(part: &Part, voice_id: &str, base_rate: i32) -> String {
    // Build the rate string for normal text (base_rate).
    let rate_str = if base_rate >= 0 {
        format!("+{}%", base_rate)
    } else {
        format!("{}%", base_rate)
    };

    // Rate for number labels is base_rate − 5 (clamped to −100).
    let num_rate = (base_rate - 5).max(-100);
    let num_rate_str = if num_rate >= 0 {
        format!("+{}%", num_rate)
    } else {
        format!("{}%", num_rate)
    };

    let voice_escaped = xml_escape(voice_id);

    let mut inner = String::new();

    // ── Part label (English portion only) ────────────────────────────────
    if part.read_label && !part.label.is_empty() {
        let en_label = strip_chinese_parens(&part.label);
        if !en_label.is_empty() {
            inner.push_str(&xml_escape(&en_label));
            inner.push_str(r#"<break time="2s"/>"#);
        }
    }

    // ── Items ─────────────────────────────────────────────────────────────
    for item in &part.items {
        if !item.enabled {
            continue;
        }

        // Number label
        if item.read_number {
            if let Some(n) = item.number {
                inner.push_str(&format!(
                    r#"<prosody rate="{num_rate_str}">Number {n}.</prosody><break time="600ms"/>"#
                ));
            }
        }

        // Body text (with optional outer prosody for base_rate when non-zero)
        let body = xml_escape(&item.text);

        if base_rate != 0 {
            inner.push_str(&format!(
                r#"<prosody rate="{rate_str}">{body}</prosody>"#
            ));
        } else {
            inner.push_str(&body);
        }

        // Repeat gap + second reading
        if item.repeat >= 2 {
            inner.push_str(r#"<break time="1500ms"/>"#);
            if base_rate != 0 {
                inner.push_str(&format!(
                    r#"<prosody rate="{rate_str}">{body}</prosody>"#
                ));
            } else {
                inner.push_str(&body);
            }
        }

        // Item gap
        let gap = item.gap_after_ms;
        inner.push_str(&format!(r#"<break time="{gap}ms"/>"#));
    }

    format!(
        r#"<speak version="1.0" xmlns="http://www.w3.org/2001/10/synthesis" xml:lang="en-US"><voice name="{voice_escaped}">{inner}</voice></speak>"#
    )
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Item, Part, TaskType};

    fn make_part(items: Vec<Item>) -> Part {
        Part {
            id: "p1".into(),
            index: 0,
            label: "Part One (第一大题). Listen and choose.".into(),
            task_type: TaskType::ListenAndChoose,
            read_label: true,
            zh_instruction: Some("听录音选择。".into()),
            read_zh_instruction: true,
            items,
            gap_after_ms: 5000,
        }
    }

    fn make_item(number: u32, text: &str, repeat: u8, read_number: bool) -> Item {
        Item {
            id: format!("i{number}"),
            number: Some(number),
            text: text.into(),
            enabled: true,
            repeat,
            gap_after_ms: 3000,
            read_number,
            override_voice: None,
        }
    }

    #[test]
    fn ssml_contains_number_and_600ms_break() {
        let part = make_part(vec![make_item(1, "The cat sat on the mat.", 1, true)]);
        let ssml = render_part_english_ssml(&part, "en-GB-SoniaNeural", 0);
        assert!(ssml.contains("Number 1."), "must contain 'Number 1.'");
        assert!(ssml.contains(r#"<break time="600ms"/>"#), "must have 600ms break");
    }

    #[test]
    fn ssml_repeat2_contains_1500ms_and_two_occurrences_of_text() {
        let text = "The cat sat on the mat.";
        let part = make_part(vec![make_item(2, text, 2, true)]);
        let ssml = render_part_english_ssml(&part, "en-GB-SoniaNeural", 0);
        assert!(ssml.contains(r#"<break time="1500ms"/>"#), "must have 1500ms break");
        // Text must appear twice
        let occurrences = ssml.matches(text).count();
        assert_eq!(occurrences, 2, "text should appear twice when repeat==2, got: {occurrences}");
    }

    #[test]
    fn ssml_label_read_and_2s_break() {
        let part = make_part(vec![make_item(1, "Hello.", 1, false)]);
        let ssml = render_part_english_ssml(&part, "en-GB-SoniaNeural", 0);
        // Should contain the English portion of the label (without Chinese brackets)
        assert!(ssml.contains("Part One"), "must contain 'Part One' from label");
        assert!(ssml.contains(r#"<break time="2s"/>"#), "must have 2s break after label");
    }

    #[test]
    fn ssml_disabled_item_skipped() {
        let mut item = make_item(1, "Should not appear.", 1, true);
        item.enabled = false;
        let part = make_part(vec![item]);
        let ssml = render_part_english_ssml(&part, "en-GB-SoniaNeural", 0);
        assert!(!ssml.contains("Should not appear."), "disabled item must be skipped");
    }

    #[test]
    fn ssml_item_gap_present() {
        let part = make_part(vec![make_item(1, "Hello.", 1, false)]);
        let ssml = render_part_english_ssml(&part, "en-GB-SoniaNeural", 0);
        assert!(ssml.contains(r#"<break time="3000ms"/>"#), "must have per-item gap");
    }

    #[test]
    fn ssml_two_items_repeat2_and_read_number() {
        // Comprehensive test: 2 items, one with repeat=2 and read_number=true
        let items = vec![
            make_item(1, "Hello world.", 1, true),
            make_item(2, "Goodbye world.", 2, true),
        ];
        let part = make_part(items);
        let ssml = render_part_english_ssml(&part, "en-GB-SoniaNeural", 0);

        // Number labels
        assert!(ssml.contains("Number 1."), "item 1 number");
        assert!(ssml.contains("Number 2."), "item 2 number");
        // 600ms break after each number
        assert!(ssml.contains(r#"<break time="600ms"/>"#));
        // 1500ms break only for item 2 (repeat=2)
        assert!(ssml.contains(r#"<break time="1500ms"/>"#));
        // "Goodbye world." appears twice
        assert_eq!(ssml.matches("Goodbye world.").count(), 2);
        // "Hello world." appears once (repeat=1)
        assert_eq!(ssml.matches("Hello world.").count(), 1);
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

    #[test]
    fn strip_chinese_parens_removes_brackets() {
        assert_eq!(
            strip_chinese_parens("Part One (第一大题)"),
            "Part One"
        );
        assert_eq!(
            strip_chinese_parens("Part Two（二）. Listen."),
            "Part Two. Listen."
        );
    }
}
