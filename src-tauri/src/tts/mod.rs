pub mod edge;

/// Trait implemented by every TTS backend.
///
/// All parameters use neutral defaults so callers don't have to think about
/// them unless they need customisation:
/// - `rate`:   percentage relative to normal speed (-100 … +100, 0 = normal)
/// - `pitch`:  percentage relative to normal pitch  (-100 … +100, 0 = normal)
/// - `volume`: absolute volume 0–100 (100 = full)
#[async_trait::async_trait]
pub trait TtsProvider: Send + Sync {
    /// Synthesise `text` with the given voice and prosody settings.
    ///
    /// Returns raw MP3 bytes on success or an error string on failure.
    async fn synthesize(
        &self,
        text: &str,
        voice_id: &str,
        rate: i32,
        pitch: i32,
        volume: u32,
    ) -> Result<Vec<u8>, String>;
}

/// A single pre-configured voice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Voice {
    /// Neural voice name accepted by the Microsoft Edge TTS endpoint,
    /// e.g. `"en-GB-SoniaNeural"`.
    pub id: String,
    /// Human-readable label shown in the UI.
    pub label: String,
}

/// Returns the 7 preset voices defined in §6 of the spec.
pub fn preset_voices() -> Vec<Voice> {
    vec![
        Voice { id: "en-GB-SoniaNeural".into(),  label: "英式女声 (Sonia)".into() },
        Voice { id: "en-GB-RyanNeural".into(),   label: "英式男声 (Ryan)".into() },
        Voice { id: "en-US-AriaNeural".into(),   label: "美式女声 (Aria)".into() },
        Voice { id: "en-US-GuyNeural".into(),    label: "美式男声 (Guy)".into() },
        Voice { id: "en-US-AnaNeural".into(),    label: "儿童女声 (Ana)".into() },
        Voice { id: "en-US-JennyNeural".into(),  label: "美式女声 (Jenny)".into() },
        Voice { id: "zh-CN-XiaoxiaoNeural".into(), label: "中文女声 (晓晓)".into() },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_voices_returns_seven() {
        assert_eq!(preset_voices().len(), 7);
    }

    #[test]
    fn preset_voices_contains_xiaoxiao() {
        let voices = preset_voices();
        assert!(
            voices.iter().any(|v| v.id == "zh-CN-XiaoxiaoNeural"),
            "zh-CN-XiaoxiaoNeural must be in preset_voices()"
        );
    }

    #[test]
    fn preset_voices_contains_all_required_ids() {
        let voices = preset_voices();
        let ids: Vec<&str> = voices.iter().map(|v| v.id.as_str()).collect();
        for required in &[
            "en-GB-SoniaNeural",
            "en-GB-RyanNeural",
            "en-US-AriaNeural",
            "en-US-GuyNeural",
            "en-US-AnaNeural",
            "en-US-JennyNeural",
            "zh-CN-XiaoxiaoNeural",
        ] {
            assert!(ids.contains(required), "missing voice id: {required}");
        }
    }

    #[test]
    fn voice_fields_non_empty() {
        for v in preset_voices() {
            assert!(!v.id.is_empty(),    "voice id must not be empty");
            assert!(!v.label.is_empty(), "voice label must not be empty");
        }
    }
}
