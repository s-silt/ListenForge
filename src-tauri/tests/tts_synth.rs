//! Integration test for EdgeTtsProvider.
//!
//! These tests are marked `#[ignore]` because they require outbound HTTPS/WSS
//! access to Microsoft's speech service.  Run them explicitly with:
//!
//!   cargo test --test tts_synth -- --ignored
//!
//! Requires:
//!   - `HTTP_PROXY=127.0.0.1:7891` (or equivalent system proxy for 外网 access)
//!   - Network connectivity to speech.platform.bing.com

use listenforge_lib::tts::{edge::EdgeTtsProvider, TtsProvider};

#[tokio::test]
#[ignore = "requires outbound WSS to Microsoft — run with --ignored"]
async fn synth_sonia_hello_world_produces_mp3() {
    let provider = EdgeTtsProvider::new();
    let result = provider
        .synthesize("Hello, world.", "en-GB-SoniaNeural", 0, 0, 100)
        .await;

    assert!(result.is_ok(), "synthesis failed: {:?}", result.err());

    let bytes = result.unwrap();

    // Must be non-trivially long (a real MP3 will be several KB).
    assert!(
        bytes.len() > 1000,
        "expected > 1000 bytes, got {}",
        bytes.len()
    );

    // Check for valid MP3 magic: ID3 tag (0x49 0x44 0x33) or MPEG sync frame
    // (0xFF 0xFB, 0xFF 0xF3, or 0xFF 0xFA).
    let is_id3 = bytes.len() >= 3 && bytes[0] == 0x49 && bytes[1] == 0x44 && bytes[2] == 0x33;
    let is_mpeg = bytes.len() >= 2
        && bytes[0] == 0xFF
        && matches!(bytes[1], 0xFA | 0xFB | 0xF3 | 0xF2);

    assert!(
        is_id3 || is_mpeg,
        "first bytes {:02X} {:02X} {:02X} don't look like MP3",
        bytes.first().copied().unwrap_or(0),
        bytes.get(1).copied().unwrap_or(0),
        bytes.get(2).copied().unwrap_or(0),
    );
}
