pub mod openai;
pub mod schema;

use serde::{Deserialize, Serialize};
use crate::model::TaskType;

// ─── ContentBlock ────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum ContentBlock {
    Text(String),
    Image { data_url: String },
}

// ─── LlmProvider trait ───────────────────────────────────────────────────────

#[async_trait::async_trait]
pub trait LlmProvider {
    async fn extract(&self, blocks: Vec<ContentBlock>) -> Result<ExtractedScript, String>;
}

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
    pub speaker: Option<String>,
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

// ─── 编译期内置默认（自用：构建时注入，绝不进 git）────────────────────────────
// 在构建机上设置以下环境变量后再编译，Key 会被烤进可执行文件，运行时无需任何
// 明文配置文件（不会在 ~/Documents/ListenForge 留下 .env）：
//   LISTENFORGE_OPENAI_API_KEY / LISTENFORGE_OPENAI_BASE_URL / LISTENFORGE_OPENAI_MODEL
// 注意：内置只是把明文藏进二进制，仍可被 `strings` 提取——仅适合自用 Key。
// 优先级：运行时环境变量 > .env 文件 > 此处内置默认。
fn builtin_api_key() -> Option<String> {
    option_env!("LISTENFORGE_OPENAI_API_KEY").and_then(|v| normalize_env_val(v))
}
fn builtin_base_url() -> Option<String> {
    option_env!("LISTENFORGE_OPENAI_BASE_URL").and_then(|v| normalize_env_val(v))
}
fn builtin_model() -> Option<String> {
    option_env!("LISTENFORGE_OPENAI_MODEL").and_then(|v| normalize_env_val(v))
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

// ─── GUI 可见的配置视图 ───────────────────────────────────────────────────────

/// 返回给前端的 LLM 配置(不含 key 明文)。
#[derive(Serialize, Clone, Debug)]
pub struct LlmConfigView {
    pub base_url: String,
    pub model: String,
    pub has_api_key: bool,
}

/// 读取配置视图,即使没有 api_key 也返回默认值,不返回 Err。
pub fn read_config_view() -> LlmConfigView {
    let default = LlmConfig::default();

    let env_key = std::env::var("OPENAI_API_KEY").ok().and_then(|v| normalize_env_val(&v));
    let env_base_url = std::env::var("OPENAI_BASE_URL").ok().and_then(|v| normalize_env_val(&v));
    let env_model = std::env::var("OPENAI_MODEL").ok().and_then(|v| normalize_env_val(&v));

    let (file_base_url, file_model, file_key) = read_dotenv_file();

    let base_url = env_base_url.or(file_base_url).or(builtin_base_url()).unwrap_or(default.base_url);
    let model = env_model.or(file_model).or(builtin_model()).unwrap_or(default.model);
    let has_api_key = env_key.or(file_key).or(builtin_api_key()).is_some();

    LlmConfigView { base_url, model, has_api_key }
}

/// 将配置写入指定目录下的 `.env` 文件(可测试的纯函数)。
/// 若 `api_key` 为 None 或空串,尝试保留目标文件中的旧 key。
pub fn write_dotenv_to(dir: &std::path::Path, base_url: &str, model: &str, api_key: Option<&str>) -> Result<(), String> {
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("创建目录失败: {e}"))?;

    let env_path = dir.join(".env");

    // 决定最终 key:传入非空 → 用传入;否则保留现有文件的旧 key
    let final_key: String = match api_key {
        Some(k) if !k.trim().is_empty() => k.to_string(),
        _ => {
            // 未提供新 key：尝试保留现有 .env 里的旧 key。
            // 文件不存在 → 无旧 key（首次写入，允许空）；
            // 存在但读失败 → 报错中止，避免静默写空把已有 key 抹掉。
            match std::fs::read_to_string(&env_path) {
                Ok(old) => {
                    let (_, _, old_key) = parse_env_config(&old);
                    old_key.unwrap_or_default()
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(e) => {
                    return Err(format!(
                        "无法读取现有 .env 以保留 API Key（已中止写入，避免清空 key）: {e}"
                    ))
                }
            }
        }
    };

    let content = format!(
        "# ListenForge LLM configuration\nOPENAI_API_KEY={}\nOPENAI_BASE_URL={}\nOPENAI_MODEL={}\n",
        final_key, base_url, model
    );

    std::fs::write(&env_path, content)
        .map_err(|e| format!("写入 .env 失败: {e}"))
}

/// 将配置写入 `~/Documents/ListenForge/.env`。
pub fn write_dotenv(base_url: &str, model: &str, api_key: Option<&str>) -> Result<(), String> {
    let dir = dirs::document_dir()
        .ok_or_else(|| "无法获取 Documents 目录".to_string())?
        .join("ListenForge");
    write_dotenv_to(&dir, base_url, model, api_key)
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

    // 3. 合并:环境变量 > .env 文件 > 编译期内置默认
    let base_url = env_base_url.or(file_base_url).or(builtin_base_url()).unwrap_or(default.base_url);
    let model = env_model.or(file_model).or(builtin_model()).unwrap_or(default.model);
    let api_key = env_key
        .or(file_key)
        .or(builtin_api_key())
        .ok_or_else(|| {
            // 区分"没填 key"与".env 存在但读不了"，避免误导排查方向
            if let Some(d) = dirs::document_dir() {
                let p = d.join("ListenForge").join(".env");
                if p.exists() {
                    if let Err(e) = std::fs::read_to_string(&p) {
                        return format!("存在 .env 但读取失败（请检查文件权限）: {e}");
                    }
                }
            }
            "未找到 OPENAI_API_KEY(环境变量、~/Documents/ListenForge/.env 或编译期内置)".to_string()
        })?;

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
                            speaker: None,
                        },
                        ExtractedItem {
                            number: Some(2),
                            text: "She is reading a book.".to_string(),
                            speaker: None,
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
                        speaker: None,
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

    // ─── write_dotenv_to 单测 ────────────────────────────────────────────────

    #[test]
    fn write_dotenv_to_round_trip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();

        write_dotenv_to(dir, "https://my.api/v1", "gpt-99", Some("sk-abc"))
            .expect("写入应成功");

        let content = std::fs::read_to_string(dir.join(".env")).expect("文件应存在");
        let (base_url, model, api_key) = parse_env_config(&content);
        assert_eq!(base_url, Some("https://my.api/v1".to_string()));
        assert_eq!(model, Some("gpt-99".to_string()));
        assert_eq!(api_key, Some("sk-abc".to_string()));
    }

    #[test]
    fn write_dotenv_to_preserves_old_key_when_none() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();

        // 先写一个含 key 的 .env
        write_dotenv_to(dir, "https://api.openai.com/v1", "gpt-5.4-mini", Some("sk-old-key"))
            .expect("首次写入应成功");

        // 再次写入,api_key=None → 应保留旧 key
        write_dotenv_to(dir, "https://new.api/v1", "gpt-100", None)
            .expect("第二次写入应成功");

        let content = std::fs::read_to_string(dir.join(".env")).expect("文件应存在");
        let (base_url, model, api_key) = parse_env_config(&content);
        assert_eq!(base_url, Some("https://new.api/v1".to_string()), "base_url 应更新");
        assert_eq!(model, Some("gpt-100".to_string()), "model 应更新");
        assert_eq!(api_key, Some("sk-old-key".to_string()), "key 应被保留");
    }

    #[test]
    fn write_dotenv_to_preserves_old_key_when_empty_string() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();

        write_dotenv_to(dir, "https://api.openai.com/v1", "gpt-5.4-mini", Some("sk-original"))
            .expect("首次写入应成功");

        // 传空串同样保留旧 key
        write_dotenv_to(dir, "https://api.openai.com/v1", "gpt-5.4-mini", Some(""))
            .expect("空串写入应成功");

        let content = std::fs::read_to_string(dir.join(".env")).expect("文件应存在");
        let (_, _, api_key) = parse_env_config(&content);
        assert_eq!(api_key, Some("sk-original".to_string()), "空串时 key 应被保留");
    }

    #[test]
    fn write_dotenv_to_allows_empty_key_on_first_write() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path();
        // 首次写入（无现有 .env）+ 不提供 key → 允许，写入空 key（NotFound 不报错）
        let r = write_dotenv_to(dir, "https://api.openai.com/v1", "gpt-5.4-mini", None);
        assert!(r.is_ok(), "首次无 key 写入应成功: {:?}", r);
        let content = std::fs::read_to_string(dir.join(".env")).unwrap();
        let (_, _, api_key) = parse_env_config(&content);
        assert_eq!(api_key, None, "首次无 key 应写入空 key");
    }

    #[test]
    fn write_dotenv_to_creates_parent_dir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let nested = tmp.path().join("deep").join("nested").join("ListenForge");

        write_dotenv_to(&nested, "https://api.openai.com/v1", "gpt-5.4-mini", Some("sk-x"))
            .expect("应自动创建嵌套目录");

        assert!(nested.join(".env").exists());
    }
}
