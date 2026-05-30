use async_trait::async_trait;
use serde_json::{json, Value};

use super::{ContentBlock, ExtractedScript, LlmConfig, LlmProvider};
use crate::llm::schema::extracted_script_schema;

/// 对返回给前端的错误文本做密钥脱敏：把形如 `sk-XXXX` 的 API key 片段替换为 `[REDACTED]`。
/// 第三方 / 中转 LLM 的错误响应体可能回显鉴权信息，截断 + 脱敏后再外露。
fn redact_secrets(s: &str) -> String {
    fn is_token_char(b: u8) -> bool {
        b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.'
    }
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        let rest = &s[i..];
        // 1) sk- 前缀的 API key（OpenAI 兼容，含 sk-ant- 等变体）
        if rest.starts_with("sk-") {
            out.push_str("[REDACTED]");
            i += 3;
            while i < bytes.len() && is_token_char(bytes[i]) {
                i += 1;
            }
            continue;
        }
        // 2) "Bearer <token>"（大小写不敏感）：保留 "Bearer "，脱敏其后 token
        if let Some(prefix) = rest.get(..7) {
            if prefix.eq_ignore_ascii_case("Bearer ") {
                out.push_str(prefix);
                i += 7;
                if i < bytes.len() && is_token_char(bytes[i]) {
                    out.push_str("[REDACTED]");
                    while i < bytes.len() && is_token_char(bytes[i]) {
                        i += 1;
                    }
                }
                continue;
            }
        }
        // 其它：按 UTF-8 字符边界推进，避免切碎多字节字符
        let ch = rest.chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

// ─── OpenAiProvider ──────────────────────────────────────────────────────────

pub struct OpenAiProvider {
    cfg: LlmConfig,
    api_key: String,
    client: reqwest::Client,
    system_prompt: String,
}

impl OpenAiProvider {
    /// 构造函数。client 使用 no_proxy() 绕过系统代理(关键：否则局域网中转超时)。
    /// `system_prompt` 在运行时由调用方传入(通常来自 prompts::selected_prompt_content())。
    pub fn new(cfg: LlmConfig, api_key: String, system_prompt: String) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .no_proxy()
            // 防止请求永久挂起：连接超时短，总超时给 vision 请求留足余量
            .connect_timeout(std::time::Duration::from_secs(15))
            .timeout(std::time::Duration::from_secs(180))
            .build()
            .map_err(|e| format!("构建 reqwest client 失败: {e}"))?;
        Ok(Self { cfg, api_key, client, system_prompt })
    }

    /// 构造请求 body (公开供单测)。
    pub fn build_body(&self, blocks: &[ContentBlock]) -> Value {
        // 用户消息内容：第一个 text 块固定为提示语，后跟各 block
        let mut user_content: Vec<Value> = vec![json!({
            "type": "text",
            "text": "请从这份练习卷提取听力原文,只输出 JSON。"
        })];

        for block in blocks {
            match block {
                ContentBlock::Text(t) => {
                    user_content.push(json!({
                        "type": "text",
                        "text": t
                    }));
                }
                ContentBlock::Image { data_url } => {
                    user_content.push(json!({
                        "type": "image_url",
                        "image_url": { "url": data_url }
                    }));
                }
            }
        }

        json!({
            "model": self.cfg.model,
            // 提取是结构化任务：temperature=0 保证确定性、稳定输出，
            // 避免同一份卷子 / 模板"有时过滤干净答案、有时漏过滤"的随机性。
            "temperature": 0,
            "messages": [
                {
                    "role": "system",
                    "content": self.system_prompt
                },
                {
                    "role": "user",
                    "content": user_content
                }
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "extracted_script",
                    "schema": extracted_script_schema()
                }
            }
        })
    }

    /// 容错解析：去除可能的 ```json / ``` 代码围栏，再反序列化。
    fn parse_content(raw: &str) -> Result<ExtractedScript, String> {
        // 去掉首尾空白
        let s = raw.trim();

        // 去除 ```json ... ``` 或 ``` ... ``` 围栏
        let s = if s.starts_with("```") {
            let after_fence = s
                .find('\n')
                .map(|i| &s[i + 1..])
                .unwrap_or(s);
            let trimmed = after_fence.trim_end();
            if trimmed.ends_with("```") {
                trimmed[..trimmed.len() - 3].trim()
            } else {
                trimmed
            }
        } else {
            s
        };

        serde_json::from_str::<ExtractedScript>(s).map_err(|e| {
            // 截取前 300 字符便于排查字段漂移
            let snippet: String = s.chars().take(300).collect();
            format!(
                "JSON 反序列化失败: {e}\n原始 content (前300字符):\n{snippet}"
            )
        })
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn extract(&self, blocks: Vec<ContentBlock>) -> Result<ExtractedScript, String> {
        let body = self.build_body(&blocks);
        let url = format!("{}/chat/completions", self.cfg.base_url);

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("HTTP 请求失败: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body_text = resp
                .text()
                .await
                .unwrap_or_else(|e| format!("<读取响应体失败: {e}>"));
            let snippet: String = redact_secrets(&body_text).chars().take(500).collect();
            return Err(format!("API 返回非 2xx 状态 {status}: {snippet}"));
        }

        let json: Value = resp
            .json()
            .await
            .map_err(|e| format!("解析响应 JSON 失败: {e}"))?;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| {
                let pretty = serde_json::to_string_pretty(&json).unwrap_or_default();
                let snippet: String = redact_secrets(&pretty).chars().take(500).collect();
                format!("响应中未找到 choices[0].message.content，实际响应(前500字):\n{snippet}")
            })?;

        Self::parse_content(content)
    }
}

// ─── 单测 ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_provider() -> OpenAiProvider {
        let cfg = LlmConfig {
            provider: "openai".into(),
            model: "gpt-4o-mini".into(),
            base_url: "https://api.openai.com/v1".into(),
        };
        OpenAiProvider::new(cfg, "test-key".into(), "test system prompt".into())
            .expect("构建 provider 应成功")
    }

    #[test]
    fn build_body_model_matches_config() {
        let p = make_provider();
        let body = p.build_body(&[]);
        assert_eq!(body["model"], "gpt-4o-mini");
    }

    #[test]
    fn build_body_sets_temperature_zero() {
        // 提取确定性：temperature 必须为 0，避免随机性导致"有时有效有时无效"
        let p = make_provider();
        let body = p.build_body(&[]);
        assert_eq!(body["temperature"], 0);
    }

    #[test]
    fn build_body_messages_count() {
        let p = make_provider();
        let blocks = vec![
            ContentBlock::Text("hi".into()),
            ContentBlock::Image {
                data_url: "data:image/png;base64,AAA".into(),
            },
        ];
        let body = p.build_body(&blocks);
        let messages = body["messages"].as_array().unwrap();
        // system + user = 2
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn build_body_user_content_has_image_url_block() {
        let data_url = "data:image/png;base64,AAA";
        let p = make_provider();
        let blocks = vec![
            ContentBlock::Text("hi".into()),
            ContentBlock::Image {
                data_url: data_url.into(),
            },
        ];
        let body = p.build_body(&blocks);
        let messages = body["messages"].as_array().unwrap();
        let user_msg = &messages[1];
        assert_eq!(user_msg["role"], "user");

        let content_arr = user_msg["content"].as_array().unwrap();

        // 应有: 固定提示语 + Text("hi") + Image = 3 项
        assert_eq!(content_arr.len(), 3);

        // 找到 image_url 类型的项
        let image_block = content_arr
            .iter()
            .find(|c| c["type"] == "image_url")
            .expect("应存在 image_url 块");

        assert_eq!(image_block["image_url"]["url"], data_url);
    }

    #[test]
    fn parse_content_strips_code_fences() {
        // 构造一个带围栏的合法 JSON 字符串
        let raw = "```json\n{\"title\":null,\"parts\":[]}\n```";
        let result = OpenAiProvider::parse_content(raw);
        assert!(result.is_ok(), "应成功解析带围栏的 JSON: {:?}", result);
        assert_eq!(result.unwrap().parts.len(), 0);
    }

    #[test]
    fn parse_content_plain_json() {
        let raw = r#"{"title":"Unit 2","parts":[]}"#;
        let result = OpenAiProvider::parse_content(raw);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().title, Some("Unit 2".into()));
    }

    #[test]
    fn parse_content_bad_json_returns_err_with_snippet() {
        let raw = r#"{"bad_field": 123}"#;
        let err = OpenAiProvider::parse_content(raw).unwrap_err();
        // 错误信息应包含原始片段
        assert!(
            err.contains("bad_field") || err.contains("原始 content"),
            "错误信息应包含片段: {err}"
        );
    }

    #[test]
    fn no_proxy_client_builds_successfully() {
        // 验证 no_proxy 构建不报错
        let client = reqwest::Client::builder()
            .no_proxy()
            .build();
        assert!(client.is_ok(), "no_proxy client 应构建成功");
    }

    #[test]
    fn redact_secrets_masks_sk_keys() {
        let input = "error: invalid key sk-abc123XYZ_-tail used for auth";
        let out = redact_secrets(input);
        assert!(!out.contains("sk-abc123XYZ"), "sk- key 应被脱敏: {out}");
        assert!(out.contains("[REDACTED]"), "应含脱敏标记: {out}");
        assert!(out.contains("used for auth"), "非密钥文本应保留: {out}");
    }

    #[test]
    fn redact_secrets_preserves_plain_text() {
        let input = "纯中文错误信息，无密钥 plain ascii";
        assert_eq!(redact_secrets(input), input);
    }

    #[test]
    fn redact_secrets_masks_bearer_token() {
        let input = "Authorization: Bearer abc123tokenXYZ failed";
        let out = redact_secrets(input);
        assert!(!out.contains("abc123tokenXYZ"), "Bearer token 应脱敏: {out}");
        assert!(out.contains("Bearer [REDACTED]"), "应保留 Bearer 并脱敏其后: {out}");
        assert!(out.contains("failed"), "尾部普通词应保留: {out}");
    }
}
