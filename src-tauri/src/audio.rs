//! Audio pipeline: synthesise each segment of a [`Project`] individually and
//! concatenate them with silent MP3 frames.
//!
//! ## Design
//! Edge TTS does **not** support `<break>` elements inside SSML — any SSML
//! that contains a break tag returns no audio.  Instead, this module:
//!
//! 1. Calls [`crate::tts::TtsProvider::synthesize`] once per text segment
//!    (each call uses the single-`<prosody>` SSML that edge-tts accepts).
//! 2. Inserts silent MP3 frames between segments to reproduce the timing that
//!    was previously baked into SSML `<break>` elements.
//! 3. Concatenates all raw MP3 byte streams — no decoding, no re-encoding,
//!    no C libraries needed.

use crate::model::Project;
use crate::ssml::strip_chinese_parens;
use crate::tts::TtsProvider;

// ── Silence frames ────────────────────────────────────────────────────────────

/// Generate approximately `ms` milliseconds of silence as raw MP3 bytes.
///
/// The frame format matches the edge-tts output stream:
/// MPEG-2 Layer III, 24 kHz, 48 kbps, mono.
///
/// Frame header bytes: `FF F3 64 C0`
/// - `FF F3`: sync word + MPEG-2, Layer III, no CRC
/// - `64`: 48 kbps bitrate (index 6), 24 kHz sample-rate (index 1), no padding, private=0
/// - `C0`: channel mode = single-channel (mono), mode ext = 0, copyright = 0,
///         original = 1 (bit 1), emphasis = none
///
/// Each frame is 144 bytes and covers exactly 576 samples = 24 ms at 24 kHz.
/// The 140 bytes after the header are all zero, which decodes as silence.
pub(crate) fn silence_mp3(ms: u32) -> Vec<u8> {
    const FRAME_HEADER: [u8; 4] = [0xFF, 0xF3, 0x64, 0xC0];
    const FRAME_SIZE: usize = 144;
    const FRAME_MS: u32 = 24; // 576 samples / 24000 Hz = 24 ms
    let n = ((ms + FRAME_MS - 1) / FRAME_MS).max(1); // ceiling division, at least 1
    let mut out = Vec::with_capacity(n as usize * FRAME_SIZE);
    for _ in 0..n {
        out.extend_from_slice(&FRAME_HEADER);
        out.extend(std::iter::repeat(0u8).take(FRAME_SIZE - 4));
    }
    out
}

// ── Main pipeline ─────────────────────────────────────────────────────────────

/// Generate full-project and per-part MP3 audio.
///
/// Returns `(full_mp3, [(filename, part_mp3), …])` where the filenames are
/// `"01_PartLabel.mp3"`, `"02_PartLabel.mp3"`, … (zero-padded two digits).
///
/// Each segment is synthesised independently via
/// [`TtsProvider::synthesize`] (which wraps the text in a single
/// `<prosody>` element — the only SSML form edge-tts reliably accepts).
/// Pauses are produced by inserting [`silence_mp3`] frames.
///
/// If any synthesis call fails the function returns `Err` immediately with a
/// message that identifies the failing part and segment.
pub async fn generate_project_audio(
    project: &Project,
    provider: &dyn TtsProvider,
) -> Result<(Vec<u8>, Vec<(String, Vec<u8>)>), String> {
    let vc = &project.voice_config;
    let mut parts_out: Vec<(String, Vec<u8>)> = Vec::new();

    for (idx, part) in project.parts.iter().enumerate() {
        let part_label = idx + 1; // 1-based for error messages
        let mut part_mp3: Vec<u8> = Vec::new();

        // ── Optional Chinese instruction ──────────────────────────────────
        if part.read_zh_instruction {
            if let Some(ref zh) = part.zh_instruction {
                if !zh.is_empty() {
                    let bytes = provider
                        .synthesize(zh, &vc.zh_voice, vc.rate, vc.pitch, vc.volume)
                        .await
                        .map_err(|e| {
                            format!("zh TTS part {part_label} (zh instruction): {e}")
                        })?;
                    part_mp3.extend_from_slice(&bytes);
                    part_mp3.extend_from_slice(&silence_mp3(1000));
                }
            }
        }

        // ── Part label (English portion) ──────────────────────────────────
        if part.read_label {
            let en_label = strip_chinese_parens(&part.label);
            if !en_label.is_empty() {
                let bytes = provider
                    .synthesize(&en_label, &vc.en_voice, vc.rate, vc.pitch, vc.volume)
                    .await
                    .map_err(|e| {
                        format!("en TTS part {part_label} (label): {e}")
                    })?;
                part_mp3.extend_from_slice(&bytes);
                part_mp3.extend_from_slice(&silence_mp3(2000));
            }
        }

        // ── Items ─────────────────────────────────────────────────────────
        for (item_idx, item) in part.items.iter().enumerate() {
            if !item.enabled {
                continue;
            }

            let item_label = item_idx + 1; // 1-based for error messages

            // Number label (read at base_rate − 5)
            if item.read_number {
                if let Some(n) = item.number {
                    let num_text = format!("Number {n}.");
                    let num_rate = (vc.rate - 5).max(-100);
                    let bytes = provider
                        .synthesize(&num_text, &vc.en_voice, num_rate, vc.pitch, vc.volume)
                        .await
                        .map_err(|e| {
                            format!(
                                "en TTS part {part_label} item {item_label} (number): {e}"
                            )
                        })?;
                    part_mp3.extend_from_slice(&bytes);
                    part_mp3.extend_from_slice(&silence_mp3(600));
                }
            }

            // Body text (first reading)
            let bytes = provider
                .synthesize(&item.text, &vc.en_voice, vc.rate, vc.pitch, vc.volume)
                .await
                .map_err(|e| {
                    format!("en TTS part {part_label} item {item_label} (text): {e}")
                })?;
            part_mp3.extend_from_slice(&bytes);

            // Optional second reading (repeat ≥ 2)
            if item.repeat >= 2 {
                part_mp3.extend_from_slice(&silence_mp3(1500));
                let bytes2 = provider
                    .synthesize(&item.text, &vc.en_voice, vc.rate, vc.pitch, vc.volume)
                    .await
                    .map_err(|e| {
                        format!(
                            "en TTS part {part_label} item {item_label} (repeat): {e}"
                        )
                    })?;
                part_mp3.extend_from_slice(&bytes2);
            }

            // Per-item trailing gap
            part_mp3.extend_from_slice(&silence_mp3(item.gap_after_ms));
        }

        // ── Part trailing gap ─────────────────────────────────────────────
        part_mp3.extend_from_slice(&silence_mp3(part.gap_after_ms));

        // ── File name ─────────────────────────────────────────────────────
        let filename = format!("{:02}_{}.mp3", idx + 1, sanitize_part_name(part));
        parts_out.push((filename, part_mp3));
    }

    // Full MP3 = all parts concatenated in order.
    let full_mp3: Vec<u8> = parts_out
        .iter()
        .flat_map(|(_, bytes)| bytes.iter().copied())
        .collect();

    Ok((full_mp3, parts_out))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Sanitize a part's label into a safe filename fragment (≤ 40 chars).
///
/// Keeps ASCII alphanumerics and replaces everything else with `_`.
/// Collapses repeated underscores and strips leading/trailing underscores.
fn sanitize_part_name(part: &crate::model::Part) -> String {
    let raw: String = part
        .label
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
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

    // ── silence_mp3 ───────────────────────────────────────────────────────

    #[test]
    fn silence_mp3_starts_with_sync_bytes() {
        let data = silence_mp3(100);
        assert_eq!(
            &data[..2],
            &[0xFF, 0xF3],
            "must start with MPEG-2 sync bytes FF F3"
        );
    }

    #[test]
    fn silence_mp3_full_header() {
        let data = silence_mp3(100);
        assert_eq!(
            &data[..4],
            &[0xFF, 0xF3, 0x64, 0xC0],
            "frame header must be FF F3 64 C0"
        );
    }

    #[test]
    fn silence_mp3_length_1000ms() {
        // 1000 ms / 24 ms per frame = ceil(41.67) = 42 frames
        // 42 frames × 144 bytes = 6048 bytes
        let data = silence_mp3(1000);
        assert_eq!(data.len(), 42 * 144, "1000 ms should produce 42 frames (6048 bytes)");
    }

    #[test]
    fn silence_mp3_length_24ms_exact() {
        // Exactly one frame duration
        let data = silence_mp3(24);
        assert_eq!(data.len(), 144);
    }

    #[test]
    fn silence_mp3_minimum_one_frame() {
        // Even 0 ms should produce at least 1 frame
        let data = silence_mp3(0);
        assert_eq!(data.len(), 144, "0 ms must still produce 1 frame");
    }

    #[test]
    fn silence_mp3_multiple_of_frame_size() {
        for ms in [100, 500, 999, 1000, 2000, 5000] {
            let data = silence_mp3(ms);
            assert_eq!(
                data.len() % 144,
                0,
                "silence_mp3({ms}) length {} is not a multiple of 144",
                data.len()
            );
        }
    }

    #[test]
    fn silence_mp3_payload_is_zero() {
        let data = silence_mp3(24); // one frame
        // Bytes 4..144 must all be 0x00
        assert!(
            data[4..].iter().all(|&b| b == 0),
            "frame payload (bytes 4..) must be all zeros (silence)"
        );
    }

    // ── sanitize_part_name ────────────────────────────────────────────────

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

    // ── E2E (network — run with `cargo test -- --ignored`) ────────────────

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
            data.starts_with(b"ID3")
                || (data.len() >= 2 && data[0] == 0xFF && data[1] & 0xE0 == 0xE0)
        };
        assert!(
            is_mp3_like(&full),
            "full output doesn't look like MP3: first bytes {:?}",
            &full[..full.len().min(4)]
        );
    }
}
