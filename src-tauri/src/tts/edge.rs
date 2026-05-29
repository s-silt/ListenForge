//! EdgeTtsProvider — self-implemented Microsoft Edge TTS over WebSocket.
//!
//! Protocol reference: <https://github.com/rany2/edge-tts>
//!
//! The implementation:
//! 1. Computes a fresh `Sec-MS-GEC` token (SHA-256 of ticks + token constant).
//! 2. Opens a `wss://speech.platform.bing.com/…` connection via
//!    tokio-tungstenite **with native-tls** (Windows Schannel — no ring/aws-lc).
//! 3. Sends `speech.config` then an SSML synthesis request.
//! 4. Reads binary audio frames until the `turn.end` text message arrives.
//! 5. Returns the concatenated MP3 bytes.
//!
//! Proxy: the connection goes via `HTTP_PROXY` / `HTTPS_PROXY` env-vars as
//! resolved by the OS socket layer (native-tls / Schannel follows the system
//! proxy, so the global 127.0.0.1:7891 proxy is used automatically).

use crate::tts::TtsProvider;
use futures_util::{SinkExt, StreamExt};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_tungstenite::{
    connect_async_tls_with_config,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
    Connector,
};

// ── Constants ────────────────────────────────────────────────────────────────

const TRUSTED_CLIENT_TOKEN: &str = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";
const SEC_MS_GEC_VERSION: &str = "1-143.0.3650.75";
const WSS_URL: &str =
    "wss://speech.platform.bing.com/consumer/speech/synthesize/readaloud/edge/v1\
     ?TrustedClientToken=6A5AA1D4EAFF4E9FB37E23D68491D6F4";

/// Seconds between Windows FILETIME epoch (1601-01-01) and Unix epoch (1970-01-01).
const WIN_EPOCH_OFFSET: u64 = 11_644_473_600;

// ── Token generation ─────────────────────────────────────────────────────────

/// Generate the `Sec-MS-GEC` DRM token.
///
/// Algorithm (matches rany2/edge-tts `drm.py`):
/// 1. Get Unix timestamp in whole seconds.
/// 2. Convert to Windows FILETIME ticks (100-nanosecond intervals since 1601-01-01).
/// 3. Round down to nearest 5-minute boundary.
/// 4. Concatenate `"{ticks}{TRUSTED_CLIENT_TOKEN}"`.
/// 5. SHA-256 hash, upper-case hex.
pub fn generate_sec_ms_gec() -> String {
    let unix_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // To Windows FILETIME ticks (100-ns units).
    let win_secs = unix_secs + WIN_EPOCH_OFFSET;
    let ticks = win_secs * 10_000_000_u64; // seconds → 100-ns ticks

    // Round down to 5-minute boundary (5 * 60 * 10_000_000 ticks).
    let five_min_ticks = 5 * 60 * 10_000_000_u64;
    let rounded = (ticks / five_min_ticks) * five_min_ticks;

    let input = format!("{rounded}{TRUSTED_CLIENT_TOKEN}");
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let hash = hasher.finalize();
    format!("{hash:X}") // upper-case hex, no separators
}

// ── SSML construction ─────────────────────────────────────────────────────────

/// Build the SSML prosody wrapper accepted by the Edge TTS endpoint.
///
/// * `rate`   – percentage offset from 0 (e.g. `+20%`, `-10%`, `+0%`)
/// * `pitch`  – percentage offset (mapped to Hz-like string `+0Hz`)
/// * `volume` – absolute 0-100 → percentage string `"100%"`
pub fn build_ssml(text: &str, voice_id: &str, rate: i32, pitch: i32, volume: u32) -> String {
    let rate_str = if rate >= 0 {
        format!("+{rate}%")
    } else {
        format!("{rate}%")
    };
    let pitch_str = if pitch >= 0 {
        format!("+{pitch}Hz")
    } else {
        format!("{pitch}Hz")
    };
    let volume_str = format!("{volume}%");

    // Escape XML special characters in user text.
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;");

    format!(
        "<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='en-US'>\
         <voice name='{voice_id}'>\
         <prosody pitch='{pitch_str}' rate='{rate_str}' volume='{volume_str}'>\
         {escaped}\
         </prosody></voice></speak>"
    )
}

/// Build the `speech.config` JSON payload.
fn build_speech_config() -> String {
    r#"{"context":{"synthesis":{"audio":{"metadataoptions":{"sentenceBoundaryEnabled":"false","wordBoundaryEnabled":"false"},"outputFormat":"audio-24khz-48kbitrate-mono-mp3"}}}}"#.to_string()
}

/// Generate a random-ish connection UUID (no external crate needed).
fn connection_id() -> String {
    // Use current time + PID for a pseudo-unique ID.
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let hi = t.as_secs();
    let lo = t.subsec_nanos();
    format!("{hi:016x}{lo:08x}0000000000000000")
        .chars()
        .take(32)
        .collect()
}

/// Format a timestamp string for message headers (RFC-like, edge-tts style).
fn timestamp_str() -> String {
    // Edge-tts uses "Thu Jan 01 2099 00:00:00 GMT+0000 (Coordinated Universal Time)"
    // We can use any fixed or current string — the server doesn't validate it strictly.
    "Thu Jan 01 2099 00:00:00 GMT+0000 (Coordinated Universal Time)".to_string()
}

// ── EdgeTtsProvider ───────────────────────────────────────────────────────────

/// Synthesises speech via the Microsoft Edge TTS WebSocket API.
///
/// Uses `native-tls` (Windows Schannel) for TLS — no ring or aws-lc-sys.
pub struct EdgeTtsProvider;

impl EdgeTtsProvider {
    pub fn new() -> Self {
        EdgeTtsProvider
    }
}

impl Default for EdgeTtsProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl TtsProvider for EdgeTtsProvider {
    async fn synthesize(
        &self,
        text: &str,
        voice_id: &str,
        rate: i32,
        pitch: i32,
        volume: u32,
    ) -> Result<Vec<u8>, String> {
        let sec_ms_gec = generate_sec_ms_gec();
        let conn_id = connection_id();

        // Build the full URL with dynamic query params.
        let url = format!(
            "{WSS_URL}&ConnectionId={conn_id}\
             &Sec-MS-GEC={sec_ms_gec}\
             &Sec-MS-GEC-Version={SEC_MS_GEC_VERSION}"
        );

        // Build the HTTP upgrade request with required headers.
        let mut request = url
            .as_str()
            .into_client_request()
            .map_err(|e| format!("build request: {e}"))?;

        let headers = request.headers_mut();
        headers.insert(
            "User-Agent",
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
                 AppleWebKit/537.36 (KHTML, like Gecko) \
                 Chrome/143.0.0.0 Safari/537.36 Edg/143.0.0.0",
            ),
        );
        headers.insert(
            "Accept-Encoding",
            HeaderValue::from_static("gzip, deflate, br"),
        );
        headers.insert(
            "Accept-Language",
            HeaderValue::from_static("en-US,en;q=0.9"),
        );
        headers.insert("Pragma", HeaderValue::from_static("no-cache"));
        headers.insert("Cache-Control", HeaderValue::from_static("no-cache"));
        headers.insert(
            "Origin",
            HeaderValue::from_static(
                "chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold",
            ),
        );

        // Use native-tls connector (Windows Schannel).
        let tls_connector = native_tls::TlsConnector::new()
            .map_err(|e| format!("tls connector: {e}"))?;
        let connector = Connector::NativeTls(tls_connector);

        let (mut ws, _) =
            connect_async_tls_with_config(request, None, false, Some(connector))
                .await
                .map_err(|e| format!("websocket connect: {e}"))?;

        // ── 1. Send speech.config ─────────────────────────────────────────
        let ts = timestamp_str();
        let config_body = build_speech_config();
        let speech_config_msg = format!(
            "X-Timestamp:{ts}\r\n\
             Content-Type:application/json; charset=utf-8\r\n\
             Path:speech.config\r\n\r\n\
             {config_body}"
        );
        ws.send(Message::Text(speech_config_msg.into()))
            .await
            .map_err(|e| format!("send speech.config: {e}"))?;

        // ── 2. Send SSML synthesis request ────────────────────────────────
        let ssml = build_ssml(text, voice_id, rate, pitch, volume);
        let request_id = connection_id();
        let synthesis_msg = format!(
            "X-RequestId:{request_id}\r\n\
             Content-Type:application/ssml+xml\r\n\
             X-Timestamp:{ts}\r\n\
             Path:ssml\r\n\r\n\
             {ssml}"
        );
        ws.send(Message::Text(synthesis_msg.into()))
            .await
            .map_err(|e| format!("send ssml: {e}"))?;

        // ── 3. Collect audio frames ───────────────────────────────────────
        let mut audio_buf: Vec<u8> = Vec::new();

        loop {
            let msg = ws
                .next()
                .await
                .ok_or_else(|| "WebSocket closed before turn.end".to_string())?
                .map_err(|e| format!("websocket recv: {e}"))?;

            match msg {
                Message::Binary(data) => {
                    // Binary frame layout:
                    //   [0..2]  big-endian u16 = header section length
                    //   [2..2+hlen]  ASCII headers (one per line, like HTTP)
                    //   [2+hlen..]   raw MP3 bytes
                    if data.len() < 2 {
                        continue;
                    }
                    let header_len = u16::from_be_bytes([data[0], data[1]]) as usize;
                    let hdr_end = 2 + header_len;
                    if hdr_end > data.len() {
                        continue;
                    }
                    let header_str =
                        std::str::from_utf8(&data[2..hdr_end]).unwrap_or("");

                    // Only collect frames that carry audio data.
                    if header_str.contains("Path:audio") {
                        audio_buf.extend_from_slice(&data[hdr_end..]);
                    }
                }

                Message::Text(text_msg) => {
                    // "Path:turn.end" signals end of synthesis for this request.
                    if text_msg.contains("Path:turn.end") {
                        break;
                    }
                    // Other text messages (turn.start, response, etc.) are ignored.
                }

                Message::Close(_) => {
                    break;
                }

                _ => {}
            }
        }

        if audio_buf.is_empty() {
            return Err("No audio data received from Edge TTS".to_string());
        }

        Ok(audio_buf)
    }
}

// ── Unit tests (pure functions only — no network) ────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sec_ms_gec_is_64_hex_chars() {
        let token = generate_sec_ms_gec();
        assert_eq!(token.len(), 64, "SHA-256 hex = 64 chars, got: {token}");
        assert!(
            token.chars().all(|c| c.is_ascii_hexdigit()),
            "token must be hex: {token}"
        );
        // Must be upper-case.
        assert_eq!(token, token.to_uppercase(), "token must be upper-case");
    }

    #[test]
    fn sec_ms_gec_stable_within_same_5min_window() {
        // Two calls very close in time should produce the same token
        // (both fall in the same 5-minute rounded window).
        let a = generate_sec_ms_gec();
        let b = generate_sec_ms_gec();
        assert_eq!(a, b, "tokens should be stable within the same 5-min window");
    }

    #[test]
    fn build_ssml_positive_rate_pitch() {
        let ssml = build_ssml("Hello", "en-GB-SoniaNeural", 20, 10, 100);
        assert!(ssml.contains("rate='+20%'"), "rate positive: {ssml}");
        assert!(ssml.contains("pitch='+10Hz'"), "pitch positive: {ssml}");
        assert!(ssml.contains("volume='100%'"), "volume: {ssml}");
        assert!(ssml.contains("name='en-GB-SoniaNeural'"), "voice: {ssml}");
        assert!(ssml.contains("Hello"), "text: {ssml}");
    }

    #[test]
    fn build_ssml_negative_rate_pitch() {
        let ssml = build_ssml("Hi", "en-US-GuyNeural", -15, -5, 80);
        assert!(ssml.contains("rate='-15%'"), "rate negative: {ssml}");
        assert!(ssml.contains("pitch='-5Hz'"), "pitch negative: {ssml}");
        assert!(ssml.contains("volume='80%'"), "volume: {ssml}");
    }

    #[test]
    fn build_ssml_zero_rate_pitch() {
        let ssml = build_ssml("Test", "en-US-AriaNeural", 0, 0, 100);
        assert!(ssml.contains("rate='+0%'"), "rate zero: {ssml}");
        assert!(ssml.contains("pitch='+0Hz'"), "pitch zero: {ssml}");
    }

    #[test]
    fn build_ssml_escapes_xml() {
        let ssml = build_ssml("a & b < c > d", "en-GB-SoniaNeural", 0, 0, 100);
        assert!(ssml.contains("&amp;"), "& must be escaped");
        assert!(ssml.contains("&lt;"), "< must be escaped");
        assert!(ssml.contains("&gt;"), "> must be escaped");
    }

    #[test]
    fn build_ssml_structure() {
        let ssml = build_ssml("x", "en-GB-SoniaNeural", 0, 0, 100);
        assert!(ssml.starts_with("<speak "), "must start with <speak");
        assert!(ssml.contains("<voice "), "must contain <voice>");
        assert!(ssml.contains("<prosody "), "must contain <prosody>");
        assert!(ssml.ends_with("</prosody></voice></speak>"), "must close all tags");
    }
}
