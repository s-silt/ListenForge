//! AzureTtsProvider — Microsoft Azure 认知服务 TTS（官方付费，无限流）。
//!
//! Azure TTS 与 edge-tts 是**同一套神经语音**(voice 名如 en-GB-SoniaNeural 通用)
//! 和**同样的 SSML**，所以这里直接复用 [`crate::tts::edge::build_ssml`]。
//! 走标准 REST（POST SSML → 返回 mp3），无 WebSocket、无限流，
//! 因此不需要 edge-tts 那套节流 / 复用连接 / 重试。

use crate::tts::{edge::build_ssml, TtsProvider};
use std::time::Duration;

pub struct AzureTtsProvider {
    key: String,
    region: String,
    client: reqwest::Client,
}

impl AzureTtsProvider {
    /// `region` 形如 "eastasia" / "southeastasia"。
    pub fn new(key: String, region: String) -> Result<Self, String> {
        // 不加 no_proxy：Azure 是境外端点，走系统代理（与 edge-tts 一致）。
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| format!("构建 Azure client 失败: {e}"))?;
        Ok(Self { key, region, client })
    }

    async fn post_ssml(&self, ssml: &str) -> Result<Vec<u8>, String> {
        let url = format!(
            "https://{}.tts.speech.microsoft.com/cognitiveservices/v1",
            self.region
        );
        let resp = self
            .client
            .post(&url)
            .header("Ocp-Apim-Subscription-Key", &self.key)
            .header("Content-Type", "application/ssml+xml")
            // 与 edge-tts 同款输出，silence_mp3 帧可直接拼接
            .header("X-Microsoft-OutputFormat", "audio-24khz-48kbitrate-mono-mp3")
            .header("User-Agent", "ListenForge")
            .body(ssml.to_string())
            .send()
            .await
            .map_err(|e| format!("Azure TTS 请求失败: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let snippet: String = body.chars().take(300).collect();
            return Err(format!("Azure TTS 返回 {status}: {snippet}"));
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("读取 Azure 音频失败: {e}"))?;
        if bytes.is_empty() {
            return Err("Azure TTS 返回空音频".to_string());
        }
        Ok(bytes.to_vec())
    }
}

#[async_trait::async_trait]
impl TtsProvider for AzureTtsProvider {
    async fn synthesize(
        &self,
        text: &str,
        voice_id: &str,
        rate: i32,
        pitch: i32,
        volume: u32,
    ) -> Result<Vec<u8>, String> {
        let ssml = build_ssml(text, voice_id, rate, pitch, volume);
        self.post_ssml(&ssml).await
    }

    async fn synthesize_ssml(&self, ssml: &str, _voice_id: &str) -> Result<Vec<u8>, String> {
        self.post_ssml(ssml).await
    }
}

// ─── Azure TTS 配置（独立存 ~/Documents/ListenForge/azure_tts.json，不混 .env）──

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
struct AzureConfigFile {
    key: String,
    region: String,
}

fn azure_config_path() -> Option<std::path::PathBuf> {
    dirs::document_dir().map(|d| d.join("ListenForge").join("azure_tts.json"))
}

// ─── 编译期内置默认（自用：构建时注入，绝不进 git）────────────────────────────
// 在构建机上设置 LISTENFORGE_AZURE_TTS_KEY / LISTENFORGE_AZURE_TTS_REGION 后编译，
// Key 会被烤进可执行文件，运行时无需 azure_tts.json。
// 注意：内置只是把明文藏进二进制，仍可被 `strings` 提取——仅适合自用 Key。
// 优先级：azure_tts.json 文件 > 此处内置默认。
fn builtin_azure_config() -> Option<(String, String)> {
    let key = option_env!("LISTENFORGE_AZURE_TTS_KEY").map(str::trim).filter(|v| !v.is_empty())?;
    let region = option_env!("LISTENFORGE_AZURE_TTS_REGION").map(str::trim).filter(|v| !v.is_empty())?;
    Some((key.to_string(), region.to_string()))
}

/// 读取 (key, region)；文件缺失或为空时回退到编译期内置；都没有 → None。
pub fn read_azure_config() -> Option<(String, String)> {
    if let Some(path) = azure_config_path() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<AzureConfigFile>(&content) {
                if !cfg.key.trim().is_empty() && !cfg.region.trim().is_empty() {
                    return Some((cfg.key, cfg.region));
                }
            }
        }
    }
    builtin_azure_config()
}

/// 写入 Azure key + region。
pub fn write_azure_config(key: &str, region: &str) -> Result<(), String> {
    let path = azure_config_path().ok_or_else(|| "无法定位 Documents 目录".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
    }
    // key 留空 → 保留现有旧 key（只改 region 时不必重填 key）
    let final_key = if key.trim().is_empty() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|c| serde_json::from_str::<AzureConfigFile>(&c).ok())
            .map(|c| c.key)
            .unwrap_or_default()
    } else {
        key.trim().to_string()
    };
    let cfg = AzureConfigFile {
        key: final_key,
        region: region.trim().to_string(),
    };
    let json = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("写入 azure_tts.json 失败: {e}"))
}

/// 给前端的视图：region + 是否已配置 key（不返回 key 明文）。
#[derive(Serialize)]
pub struct AzureConfigView {
    pub region: String,
    pub has_key: bool,
}

pub fn read_azure_config_view() -> AzureConfigView {
    let raw = azure_config_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|c| serde_json::from_str::<AzureConfigFile>(&c).ok())
        .unwrap_or_default();
    // 文件未配置时，反映编译期内置默认，让前端不再提示"未配置"。
    let builtin = builtin_azure_config();
    let region = if !raw.region.trim().is_empty() {
        raw.region
    } else {
        builtin.as_ref().map(|(_, r)| r.clone()).unwrap_or_default()
    };
    AzureConfigView {
        region,
        has_key: !raw.key.trim().is_empty() || builtin.is_some(),
    }
}
