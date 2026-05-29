pub mod schema;

use serde::{Deserialize, Serialize};
use crate::model::TaskType;

// ─── 提取结果类型 ────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ExtractedScript {
    pub title: Option<String>,
    pub parts: Vec<ExtractedPart>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ExtractedPart {
    pub label: String,
    pub task_type: TaskType,
    pub zh_instruction: Option<String>,
    pub items: Vec<ExtractedItem>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ExtractedItem {
    pub number: Option<u32>,
    pub text: String,
}

// ─── LLM 配置 ────────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    pub base_url: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: "openai".into(),
            model: "gpt-5.4-mini".into(),
            base_url: "https://api.openai.com/v1".into(),
        }
    }
}

// ─── .env 解析 ───────────────────────────────────────────────────────────────

/// 从 .env 文件内容中解析 OPENAI_BASE_URL / OPENAI_MODEL / OPENAI_API_KEY。
/// 返回 (base_url, model, api_key)。
/// 值为空、纯空白、或以"在此"/"请填"开头的占位则视为 None。
pub fn parse_env_config(content: &str) -> (Option<String>, Option<String>, Option<String>) {
    let mut base_url: Option<String> = None;
    let mut model: Option<String> = None;
    let mut api_key: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();
        // 跳过注释和空行
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((key, val)) = line.split_once('=') {
            let key = key.trim();
            let val = val.trim();
            let cleaned = normalize_env_val(val);

            match key {
                "OPENAI_BASE_URL" => base_url = cleaned,
                "OPENAI_MODEL" => model = cleaned,
                "OPENAI_API_KEY" => api_key = cleaned,
                _ => {}
            }
        }
    }

    (base_url, model, api_key)
}

/// 空字符串和常见占位返回 None，否则返回 Some(s)。
fn normalize_env_val(val: &str) -> Option<String> {
    if val.is_empty() {
        return None;
    }
    // 占位判断：以"在此"/"请填"/"<"/"your-"开头
    let placeholders = ["在此", "请填", "<", "your-", "YOUR_"];
    for p in &placeholders {
        if val.starts_with(p) {
            return None;
        }
    }
    Some(val.to_string())
}

// ─── 读取最终配置 ─────────────────────────────────────────────────────────────

/// 读取 LlmConfig 和 api_key。
/// 优先级:环境变量 > `~/Documents/ListenForge/.env` 文件。
/// base_url / model 缺省时用 Default 的值;api_key 找不到则 Err。
pub fn read_llm_config() -> Result<(LlmConfig, String), String> {
    let default = LlmConfig::default();

    // 1. 先从环境变量读
    let env_key = std::env::var("OPENAI_API_KEY").ok().and_then(|v| normalize_env_val(&v));
    let env_base_url = std::env::var("OPENAI_BASE_URL").ok().and_then(|v| normalize_env_val(&v));
    let env_model = std::env::var("OPENAI_MODEL").ok().and_then(|v| normalize_env_val(&v));

    // 2. 从 .env 文件补缺
    let (file_base_url, file_model, file_key) = read_dotenv_file();

    // 3. 合并:环境变量优先
    let base_url = env_base_url.or(file_base_url).unwrap_or(default.base_url);
    let model = env_model.or(file_model).unwrap_or(default.model);
    let api_key = env_key
        .or(file_key)
        .ok_or_else(|| "未找到 OPENAI_API_KEY(环境变量或 ~/Documents/ListenForge/.env)".to_string())?;

    let config = LlmConfig {
        provider: "openai".into(),
        model,
        base_url,
    };

    Ok((config, api_key))
}

/// 读取 `~/Documents/ListenForge/.env` 并解析。找不到文件则返回全 None。
fn read_dotenv_file() -> (Option<String>, Option<String>, Option<String>) {
    let path = match dirs::document_dir() {
        Some(d) => d.join("ListenForge").join(".env"),
        None => return (None, None, None),
    };

    match std::fs::read_to_string(&path) {
        Ok(content) => parse_env_config(&content),
        Err(_) => (None, None, None),
    }
}

// ─── 测试 ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TaskType;

    #[test]
    fn extracted_script_serde_round_trip() {
        let script = ExtractedScript {
            title: Some("Unit 2 听力".to_string()),
            parts: vec![
                ExtractedPart {
                    label: "Part One. Listen and choose.".to_string(),
                    task_type: TaskType::ListenAndChoose,
                    zh_instruction: Some("听录音选择正确答案。".to_string()),
                    items: vec![
                        ExtractedItem {
                            number: Some(1),
                            text: "I can take the dishes to the kitchen.".to_string(),
                        },
                        ExtractedItem {
                            number: Some(2),
                            text: "She is reading a book.".to_string(),
                        },
                    ],
                },
                ExtractedPart {
                    label: "Part Two. Listen passage.".to_string(),
                    task_type: TaskType::ListenPassage,
                    zh_instruction: None,
                    items: vec![ExtractedItem {
                        number: None,
                        text: "Once upon a time there was a brave knight.".to_string(),
                    }],
                },
            ],
        };

        let json = serde_json::to_string(&script).expect("序列化应成功");
        let back: ExtractedScript = serde_json::from_str(&json).expect("反序列化应成功");
        assert_eq!(script, back);
    }

    #[test]
    fn extracted_script_title_none_round_trip() {
        let script = ExtractedScript {
            title: None,
            parts: vec![],
        };
        let json = serde_json::to_string(&script).unwrap();
        let back: ExtractedScript = serde_json::from_str(&json).unwrap();
        assert_eq!(script, back);
    }

    #[test]
    fn parse_env_config_three_lines() {
        let content = "\
OPENAI_API_KEY=sk-test-abc123\n\
OPENAI_BASE_URL=https://custom.api.example.com/v1\n\
OPENAI_MODEL=gpt-4o\n";

        let (base_url, model, api_key) = parse_env_config(content);
        assert_eq!(base_url, Some("https://custom.api.example.com/v1".to_string()));
        assert_eq!(model, Some("gpt-4o".to_string()));
        assert_eq!(api_key, Some("sk-test-abc123".to_string()));
    }

    #[test]
    fn parse_env_config_ignores_placeholder_values() {
        // 值以"在此..."等占位前缀开头应被忽略
        let content = "\
OPENAI_API_KEY=在此填写你的 API Key\n\
OPENAI_BASE_URL=请填写 base URL\n\
OPENAI_MODEL=gpt-5.4-mini\n";

        let (base_url, model, api_key) = parse_env_config(content);
        assert_eq!(base_url, None, "占位 base_url 应为 None");
        assert_eq!(api_key, None, "占位 api_key 应为 None");
        assert_eq!(model, Some("gpt-5.4-mini".to_string()), "正常 model 应保留");
    }

    #[test]
    fn parse_env_config_ignores_empty_values() {
        let content = "\
OPENAI_API_KEY=\n\
OPENAI_BASE_URL=\n\
OPENAI_MODEL=\n";

        let (base_url, model, api_key) = parse_env_config(content);
        assert_eq!(base_url, None);
        assert_eq!(model, None);
        assert_eq!(api_key, None);
    }

    #[test]
    fn parse_env_config_ignores_comments_and_blank_lines() {
        let content = "\
# 这是注释\n\
\n\
OPENAI_API_KEY=sk-real-key\n\
# OPENAI_MODEL=should-be-ignored\n\
OPENAI_BASE_URL=https://api.openai.com/v1\n";

        let (base_url, model, api_key) = parse_env_config(content);
        assert_eq!(base_url, Some("https://api.openai.com/v1".to_string()));
        assert_eq!(model, None, "注释行中的 model 不应被解析");
        assert_eq!(api_key, Some("sk-real-key".to_string()));
    }

    #[test]
    fn llm_config_default_values() {
        let cfg = LlmConfig::default();
        assert_eq!(cfg.provider, "openai");
        assert_eq!(cfg.model, "gpt-5.4-mini");
        assert_eq!(cfg.base_url, "https://api.openai.com/v1");
    }
}
