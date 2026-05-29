//! End-to-end audio pipeline integration test.
//!
//! Marked `#[ignore]` — requires live network access to the Edge TTS service.
//! Run explicitly with:
//!
//!   cargo test --test audio_e2e -- --ignored
//!
//! Requires:
//!   - Outbound WSS to speech.platform.bing.com
//!   - A system proxy configured if needed (HTTP_PROXY / HTTPS_PROXY)

use listenforge_lib::{
    audio::generate_project_audio,
    model::{ExportConfig, Item, Part, Project, SourceType, TaskType, VoiceConfig},
    tts::edge::EdgeTtsProvider,
};

fn make_test_project() -> Project {
    let mut project = Project::new("E2E_Test", "test.pdf", SourceType::PdfText);
    project.voice_config = VoiceConfig {
        provider: "edge".into(),
        en_voice: "en-GB-SoniaNeural".into(),
        zh_voice: "zh-CN-XiaoxiaoNeural".into(),
        rate: 0,
        pitch: 0,
        volume: 100,
        teacher_voice: "en-US-GuyNeural".into(),
        student_voice: "en-US-AnaNeural".into(),
    };
    project.export_config = ExportConfig::default();

    // Part 1: two items, one with repeat=2
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
                speaker: None,
            },
            Item {
                id: "i2".into(),
                number: Some(2),
                text: "The dog is sitting under the table.".into(),
                enabled: true,
                repeat: 1,
                gap_after_ms: 3000,
                read_number: true,
                override_voice: None,
                speaker: None,
            },
        ],
        gap_after_ms: 5000,
    });

    // Part 2: single item, no zh instruction
    project.parts.push(Part {
        id: "part-2".into(),
        index: 1,
        label: "Part Two. Listen and fill in the blanks.".into(),
        task_type: TaskType::ListenAndWrite,
        read_label: true,
        zh_instruction: None,
        read_zh_instruction: false,
        items: vec![Item {
            id: "i3".into(),
            number: Some(1),
            text: "The sun rises in the east every morning.".into(),
            enabled: true,
            repeat: 1,
            gap_after_ms: 3000,
            read_number: false,
            override_voice: None,
            speaker: None,
        }],
        gap_after_ms: 5000,
    });

    project
}

/// Full pipeline: SSML → Edge TTS → MP3 byte concat.
#[tokio::test]
#[ignore = "requires live Edge TTS network access — run with --ignored"]
async fn e2e_full_project_audio_non_empty_and_mp3_like() {
    let provider = EdgeTtsProvider::new();
    let project = make_test_project();

    let result = generate_project_audio(&project, &provider).await;
    assert!(result.is_ok(), "generate_project_audio failed: {:?}", result);

    let (full, parts) = result.unwrap();

    // Full audio must be non-empty.
    assert!(!full.is_empty(), "full mp3 must not be empty");

    // Must have the right number of parts.
    assert_eq!(parts.len(), 2, "expected 2 part files");
    assert!(!parts[0].1.is_empty(), "part 1 audio must not be empty");
    assert!(!parts[1].1.is_empty(), "part 2 audio must not be empty");

    // Filenames must follow the 01_xxx.mp3 / 02_xxx.mp3 pattern.
    assert!(
        parts[0].0.starts_with("01_"),
        "part 1 filename: {}",
        parts[0].0
    );
    assert!(
        parts[1].0.starts_with("02_"),
        "part 2 filename: {}",
        parts[1].0
    );

    // Full audio should start with MP3 magic bytes (ID3 or MPEG sync).
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
