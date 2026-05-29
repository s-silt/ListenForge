use async_trait::async_trait;
use serde_json::{json, Value};

use super::{ContentBlock, ExtractedScript, LlmConfig, LlmProvider};
use crate::llm::schema::extracted_script_schema;

// ─── System prompt ───────────────────────────────────────────────────────────

const SYSTEM_PROMPT: &str = r#"You are a Chinese primary-school English listening-test script extractor.

Your job: given scanned pages (images) or text of a practice worksheet, extract ONLY the "Listening Script" / "Tapescript" / "听力原文" section — the sentences that will be read aloud in the exam.

Rules:
1. DO NOT include answer keys (e.g. "1.B 2.A", "答案:", "→4,3,2,1", "( B )").
2. DO NOT include Chinese translations of English sentences.
3. Strip item numbers from `text`; put the digit in `number` (integer). For passage-style items set number to null.
4. Chinese task instructions (第一大题…) → `zh_instruction`. Discard Chinese translations of English content.
5. Infer `task_type` from the section heading:
   - "listen and choose" / "选择" → listen_and_choose
   - "number" / "排序" / "编号" → listen_and_number
   - "judge" / "判断" → listen_and_judge
   - "write" / "填写" → listen_and_write
   - "circle" / "圈出" → listen_and_circle
   - continuous passage / short text → listen_passage
   - unclear → unknown
6. A continuous short passage (no numbered items) uses one item with number=null.
7. Output STRICT JSON with EXACTLY these field names — do not rename them:

{"title": "nullable string or null", "parts": [{"label": "Part One. Listen and choose.", "task_type": "listen_and_choose", "zh_instruction": "第一大题说明或null", "items": [{"number": 1, "text": "I can take the dishes to the kitchen."}]}]}

Only output the JSON object, no markdown fences, no extra keys."#;

// ─── OpenAiProvider ──────────────────────────────────────────────────────────

pub struct OpenAiProvider {
    cfg: LlmConfig,
    api_key: String,
    client: reqwest::Client,
}

impl OpenAiProvider {
    /// 构造函数。client 使用 no_proxy() 绕过系统代理(关键：否则局域网中转超时)。
    pub fn new(cfg: LlmConfig, api_key: String) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .no_proxy()
            .build()
            .map_err(|e| format!("构建 reqwest client 失败: {e}"))?;
        Ok(Self { cfg, api_key, client })
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
            "messages": [
                {
                    "role": "system",
                    "content": SYSTEM_PROMPT
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
            let body_text = resp.text().await.unwrap_or_default();
            return Err(format!(
                "API 返回非 2xx 状态 {status}: {body_text}"
            ));
        }

        let json: Value = resp
            .json()
            .await
            .map_err(|e| format!("解析响应 JSON 失败: {e}"))?;

        let content = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| {
                format!(
                    "响应中未找到 choices[0].message.content，实际响应:\n{}",
                    serde_json::to_string_pretty(&json).unwrap_or_default()
                )
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
        OpenAiProvider::new(cfg, "test-key".into()).expect("构建 provider 应成功")
    }

    #[test]
    fn build_body_model_matches_config() {
        let p = make_provider();
        let body = p.build_body(&[]);
        assert_eq!(body["model"], "gpt-4o-mini");
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
}
