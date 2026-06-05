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
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex as AsyncMutex;
use tokio_tungstenite::{
    connect_async_tls_with_config,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
    Connector, MaybeTlsStream, WebSocketStream,
};

/// Edge TTS 复用连接的具体类型（native-tls / Schannel）。
type EdgeWs = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

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

    // Escape XML special characters in BOTH the user text and the voice id.
    // voice_id comes from VoiceConfig, which the frontend can set to any string,
    // so it must be escaped to prevent SSML attribute injection.
    let escaped = crate::ssml::xml_escape(text);
    let voice_escaped = crate::ssml::xml_escape(voice_id);

    format!(
        "<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='en-US'>\
         <voice name='{voice_escaped}'>\
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
pub struct EdgeTtsProvider {
    /// 复用单个 WebSocket 连接合成多句，避免每句新建连接累积触发微软限流。
    conn: AsyncMutex<Option<EdgeWs>>,
}

impl EdgeTtsProvider {
    pub fn new() -> Self {
        EdgeTtsProvider {
            conn: AsyncMutex::new(None),
        }
    }
}

impl Default for EdgeTtsProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Core WebSocket synthesis — sends `ssml` verbatim to the Edge TTS endpoint
/// and returns the collected MP3 bytes.
///
/// This is an internal helper shared by both `synthesize` and `synthesize_ssml`.
/// 全局连接节流：保证两次 Edge TTS 连接至少间隔 MIN_CONNECT_INTERVAL，
/// 平滑请求速率，避免连续多句合成触发微软端的限流（表现为返回空音频）。
static LAST_CONNECT: Mutex<Option<Instant>> = Mutex::new(None);
const MIN_CONNECT_INTERVAL: Duration = Duration::from_millis(350);

async fn throttle_connect() {
    let wait = {
        // 节流状态是 advisory 的单个时间戳，无需保护的不变量；
        // 容忍锁中毒（into_inner 取回 guard），避免某次 panic 永久 brick 后续所有 Edge 合成。
        let mut guard = LAST_CONNECT.lock().unwrap_or_else(|p| p.into_inner());
        let now = Instant::now();
        let wait = match *guard {
            Some(prev) => {
                let next = prev + MIN_CONNECT_INTERVAL;
                if next > now { next - now } else { Duration::ZERO }
            }
            None => Duration::ZERO,
        };
        // 预约本次连接的时间槽，确保连续调用串行错开 ≥ MIN_CONNECT_INTERVAL
        *guard = Some(now + wait);
        wait
    }; // 锁在 await 之前释放，不跨 await 持锁
    if !wait.is_zero() {
        tokio::time::sleep(wait).await;
    }
}

/// 建立一个 Edge TTS WebSocket 连接并发送 speech.config（连接可复用）。
async fn open_edge_connection() -> Result<EdgeWs, String> {
    let sec_ms_gec = generate_sec_ms_gec();
    let conn_id = connection_id();

    let url = format!(
        "{WSS_URL}&ConnectionId={conn_id}\
         &Sec-MS-GEC={sec_ms_gec}\
         &Sec-MS-GEC-Version={SEC_MS_GEC_VERSION}"
    );

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

    let tls_connector = native_tls::TlsConnector::new()
        .map_err(|e| format!("tls connector: {e}"))?;
    let connector = Connector::NativeTls(tls_connector);

    // 连接 Edge TTS 端点（境外 wss，可能走系统代理，较慢）。超时宽松（120s）以兜底永久挂死。
    let (mut ws, _) = tokio::time::timeout(
        Duration::from_secs(120),
        connect_async_tls_with_config(request, None, false, Some(connector)),
    )
    .await
    .map_err(|_| "Edge TTS 连接超时(120s)，请检查网络 / 代理".to_string())?
    .map_err(|e| format!("websocket connect: {e}"))?;

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

    Ok(ws)
}

/// 在已建立的连接上合成一句 SSML，返回 MP3 字节。可在同一连接上反复调用。
async fn synth_on(ws: &mut EdgeWs, ssml: &str) -> Result<Vec<u8>, String> {
    // 每句合成请求前节流：平滑「请求(turn)速率」。复用连接后不再新建连接，
    // 所以节流必须放在每个 synthesis 请求前，而非连接建立时（否则节流失效）。
    throttle_connect().await;
    let ts = timestamp_str();
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

    let mut audio_buf: Vec<u8> = Vec::new();
    let mut binary_count = 0usize;
    let mut seen_paths: Vec<String> = Vec::new();

    loop {
        let msg = tokio::time::timeout(Duration::from_secs(120), ws.next())
            .await
            .map_err(|_| "Edge TTS 响应超时(120s)，请检查网络 / 代理".to_string())?
            .ok_or_else(|| "WebSocket closed before turn.end".to_string())?
            .map_err(|e| format!("websocket recv: {e}"))?;

        match msg {
            Message::Binary(data) => {
                binary_count += 1;
                if data.len() < 2 {
                    seen_paths.push("bin(<2B)".to_string());
                    continue;
                }
                let header_len = u16::from_be_bytes([data[0], data[1]]) as usize;
                let hdr_end = 2 + header_len;
                if hdr_end > data.len() {
                    seen_paths.push(format!("bin(hdr{header_len}>len{})", data.len()));
                    continue;
                }
                let header_str =
                    std::str::from_utf8(&data[2..hdr_end]).unwrap_or("<非UTF8>");
                // 诊断：记录这个 binary 的 Path + 音频负载字节数
                if let Some(p) = header_str.lines().find(|l| l.starts_with("Path:")) {
                    seen_paths.push(format!("bin:{}({}B)", p.trim(), data.len() - hdr_end));
                }
                if header_str.contains("Path:audio") {
                    audio_buf.extend_from_slice(&data[hdr_end..]);
                }
            }
            Message::Text(text_msg) => {
                // 诊断：记录收到的每个消息 Path（区分限流 / 错误 / 正常 turn.end）
                if let Some(p) = text_msg.lines().find(|l| l.starts_with("Path:")) {
                    seen_paths.push(p.trim().to_string());
                }
                if text_msg.contains("Path:turn.end") {
                    break;
                }
            }
            Message::Close(frame) => {
                seen_paths.push(format!("Close({frame:?})"));
                break;
            }
            _ => {}
        }
    }

    if audio_buf.is_empty() {
        let ssml_preview: String = ssml.chars().take(240).collect();
        return Err(format!(
            "No audio data received from Edge TTS [诊断: binary={binary_count}, 消息={seen_paths:?}, SSML={ssml_preview:?}]"
        ));
    }

    Ok(audio_buf)
}

impl EdgeTtsProvider {
    /// 在「复用连接」上合成一句：复用现有连接，失败则丢弃连接、退避后重建重试。
    /// 正常情况下整个项目所有句子复用同一个连接（连接数 ≈ 1），从根本上避免
    /// 「每句新建连接」累积触发微软限流（表现为 No audio data）。
    async fn synthesize_reusing(&self, ssml: &str) -> Result<Vec<u8>, String> {
        let mut guard = self.conn.lock().await;
        let mut last_err = String::new();
        for attempt in 0u32..4 {
            if attempt > 0 {
                // 上次失败：丢弃可能已坏的连接，退避后重建（2s / 4s / 8s）
                *guard = None;
                tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
            }
            // 确保有可用连接：复用现有，或新建
            if guard.is_none() {
                match open_edge_connection().await {
                    Ok(ws) => *guard = Some(ws),
                    Err(e) => {
                        last_err = e;
                        continue;
                    }
                }
            }
            // 在连接上合成这一句
            let ws = guard.as_mut().unwrap();
            match synth_on(ws, ssml).await {
                Ok(audio) => return Ok(audio),
                Err(e) => {
                    last_err = e;
                    *guard = None; // 连接可能已坏，丢弃，下一轮重建
                }
            }
        }
        Err(format!("{last_err}（已重试 4 次仍失败）"))
    }
}

#[async_trait::async_trait]
impl TtsProvider for EdgeTtsProvider {
    /// Synthesise plain `text` by wrapping it in a `<prosody>` SSML envelope
    /// and delegating to the shared WebSocket helper.
    async fn synthesize(
        &self,
        text: &str,
        voice_id: &str,
        rate: i32,
        pitch: i32,
        volume: u32,
    ) -> Result<Vec<u8>, String> {
        let ssml = build_ssml(text, voice_id, rate, pitch, volume);
        self.synthesize_reusing(&ssml).await
    }

    /// Send a pre-built SSML document directly — no additional wrapping.
    async fn synthesize_ssml(
        &self,
        ssml: &str,
        _voice_id: &str,
    ) -> Result<Vec<u8>, String> {
        self.synthesize_reusing(ssml).await
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
    fn build_ssml_escapes_voice_id() {
        // 防御 SSML 属性注入：voice_id 里的单引号必须被转义
        let ssml = build_ssml("x", "evil' onload='y", 0, 0, 100);
        assert!(!ssml.contains("evil' onload"), "voice_id 引号应被转义: {ssml}");
        assert!(ssml.contains("&apos;"), "voice_id 单引号应转义为 &apos;: {ssml}");
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
