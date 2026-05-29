//! Audio pipeline: synthesise all parts of a [`Project`] and concatenate them.
//!
//! ## Design
//! - Zero C-library decoding/re-encoding.  All audio is kept as raw MP3 byte
//!   streams returned by the Edge TTS WebSocket and concatenated with plain
//!   `Vec::extend_from_slice`.
//! - Pauses, repetitions, and number-label prosody are baked into the SSML
//!   sent to the TTS engine (see [`crate::ssml`]) — no local DSP needed.
//! - Chinese instructions are synthesised separately with the project's
//!   `zh_voice` and prepended to the English audio for each part.

use crate::model::Project;
use crate::ssml::render_part_english_ssml;
use crate::tts::TtsProvider;

/// Generate full-project and per-part MP3 audio.
///
/// Returns `(full_mp3, [(filename, part_mp3), …])` where the filenames are
/// `"01_PartLabel.mp3"`, `"02_PartLabel.mp3"`, … (zero-padded two digits).
///
/// The overall audio is the parts concatenated in index order with no
/// additional silence inserted at the boundary (silence between parts is
/// already encoded by `render_part_english_ssml` via the part's SSML).
pub async fn generate_project_audio(
    project: &Project,
    provider: &dyn TtsProvider,
) -> Result<(Vec<u8>, Vec<(String, Vec<u8>)>), String> {
    let vc = &project.voice_config;
    let mut parts_out: Vec<(String, Vec<u8>)> = Vec::new();

    for (idx, part) in project.parts.iter().enumerate() {
        let mut part_mp3: Vec<u8> = Vec::new();

        // ── Optional Chinese instruction ──────────────────────────────────
        if part.read_zh_instruction {
            if let Some(ref zh) = part.zh_instruction {
                if !zh.is_empty() {
                    let zh_bytes = provider
                        .synthesize(zh, &vc.zh_voice, vc.rate, vc.pitch, vc.volume)
                        .await
                        .map_err(|e| format!("zh TTS part {}: {e}", idx + 1))?;
                    part_mp3.extend_from_slice(&zh_bytes);
                }
            }
        }

        // ── English SSML ──────────────────────────────────────────────────
        let ssml = render_part_english_ssml(part, &vc.en_voice, vc.rate);
        let en_bytes = provider
            .synthesize_ssml(&ssml, &vc.en_voice)
            .await
            .map_err(|e| format!("en TTS part {}: {e}", idx + 1))?;
        part_mp3.extend_from_slice(&en_bytes);

        // ── File name ─────────────────────────────────────────────────────
        let filename = format!(
            "{:02}_{}.mp3",
            idx + 1,
            sanitize_part_name(part)
        );

        parts_out.push((filename, part_mp3));
    }

    // Full MP3 = all parts concatenated.
    let full_mp3: Vec<u8> = parts_out
        .iter()
        .flat_map(|(_, bytes)| bytes.iter().copied())
        .collect();

    Ok((full_mp3, parts_out))
}

/// Sanitize a part's label into a safe filename fragment (≤ 40 chars).
///
/// Keeps ASCII alphanumerics and replaces everything else with `_`.
/// Strips leading/trailing underscores.
fn sanitize_part_name(part: &crate::model::Part) -> String {
    let raw: String = part
        .label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c
            } else {
                '_'
            }
        })
        .collect();
    // Collapse repeated underscores and trim.
    let collapsed = raw
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    // Truncate to 40 chars to keep filenames reasonable.
    collapsed.chars().take(40).collect::<String>()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Item, Part, TaskType};

    #[test]
    fn sanitize_part_name_ascii() {
        let part = Part {
            id: "x".into(),
            index: 0,
            label: "Part One. Listen & choose!".into(),
            task_type: TaskType::ListenAndChoose,
            read_label: false,
            zh_instruction: None,
            read_zh_instruction: false,
            items: vec![],
            gap_after_ms: 5000,
        };
        let name = sanitize_part_name(&part);
        assert!(
            name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "unexpected chars: {name}"
        );
        assert!(!name.starts_with('_'), "should not start with _: {name}");
        assert!(!name.ends_with('_'), "should not end with _: {name}");
    }

    #[test]
    fn sanitize_part_name_truncates() {
        let part = Part {
            id: "x".into(),
            index: 0,
            label: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into(),
            task_type: TaskType::ListenAndChoose,
            read_label: false,
            zh_instruction: None,
            read_zh_instruction: false,
            items: vec![],
            gap_after_ms: 5000,
        };
        let name = sanitize_part_name(&part);
        assert!(name.len() <= 40, "should be truncated: {}", name.len());
    }

    // ── E2E test (requires network — run with `cargo test -- --ignored`) ──

    /// End-to-end: build a tiny Project, call generate_project_audio,
    /// assert full mp3 non-empty and starts with ID3 or MPEG sync byte.
    #[tokio::test]
    #[ignore = "requires live Edge TTS network access"]
    async fn e2e_generate_project_audio() {
        use crate::model::{ExportConfig, Project, SourceType, VoiceConfig};
        use crate::tts::edge::EdgeTtsProvider;

        let provider = EdgeTtsProvider::new();

        let mut project = Project::new("TestProject", "test.pdf", SourceType::PdfText);
        project.voice_config = VoiceConfig {
            provider: "edge".into(),
            en_voice: "en-GB-SoniaNeural".into(),
            zh_voice: "zh-CN-XiaoxiaoNeural".into(),
            rate: 0,
            pitch: 0,
            volume: 100,
        };
        project.export_config = ExportConfig::default();

        // Part 1: 2 items
        project.parts.push(Part {
            id: "part-1".into(),
            index: 0,
            label: "Part One. Listen and choose.".into(),
            task_type: TaskType::ListenAndChoose,
            read_label: true,
            zh_instruction: Some("第一大题,听录音选择。".into()),
            read_zh_instruction: true,
            items: vec![
                Item {
                    id: "i1".into(),
                    number: Some(1),
                    text: "I can take the dishes to the kitchen.".into(),
                    enabled: true,
                    repeat: 2,
                    gap_after_ms: 3000,
                    read_number: true,
                    override_voice: None,
                },
                Item {
                    id: "i2".into(),
                    number: Some(2),
                    text: "The dog is under the table.".into(),
                    enabled: true,
                    repeat: 1,
                    gap_after_ms: 3000,
                    read_number: true,
                    override_voice: None,
                },
            ],
            gap_after_ms: 5000,
        });

        // Part 2: 1 item
        project.parts.push(Part {
            id: "part-2".into(),
            index: 1,
            label: "Part Two. Listen and fill.".into(),
            task_type: TaskType::ListenAndWrite,
            read_label: true,
            zh_instruction: None,
            read_zh_instruction: false,
            items: vec![Item {
                id: "i3".into(),
                number: Some(1),
                text: "The sun rises in the east.".into(),
                enabled: true,
                repeat: 1,
                gap_after_ms: 3000,
                read_number: false,
                override_voice: None,
            }],
            gap_after_ms: 5000,
        });

        let result = generate_project_audio(&project, &provider).await;
        assert!(result.is_ok(), "generate_project_audio failed: {:?}", result);

        let (full, parts) = result.unwrap();
        assert!(!full.is_empty(), "full mp3 must not be empty");
        assert_eq!(parts.len(), 2, "should have 2 part files");
        assert!(!parts[0].1.is_empty(), "part 1 mp3 must not be empty");
        assert!(!parts[1].1.is_empty(), "part 2 mp3 must not be empty");

        // MP3 frames begin with 0xFF 0xFB/0xE0/... or ID3 header "ID3"
        let is_mp3_like = |data: &[u8]| {
            data.starts_with(b"ID3") || (data.len() >= 2 && data[0] == 0xFF && data[1] & 0xE0 == 0xE0)
        };
        assert!(
            is_mp3_like(&full),
            "full output doesn't look like MP3: first bytes {:?}",
            &full[..full.len().min(4)]
        );
    }
}
