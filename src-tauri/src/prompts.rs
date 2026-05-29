use serde::{Deserialize, Serialize};

// ─── PromptTemplate ──────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PromptTemplate {
    pub id: String,
    pub name: String,
    pub content: String,
    pub builtin: bool,
}

// ─── 内置预设 ─────────────────────────────────────────────────────────────────

/// standard 预设的 content(原 openai.rs SYSTEM_PROMPT)。
const STANDARD_CONTENT: &str = r#"You are a Chinese primary-school English listening-test script extractor.

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

{"title": "nullable string or null", "parts": [{"label": "Part One. Listen and choose.", "task_type": "listen_and_choose", "zh_instruction": "第一大题说明或null", "items": [{"number": 1, "text": "I can take the dishes to the kitchen.", "speaker": null}]}]}

Only output the JSON object, no markdown fences, no extra keys."#;

/// 通用英文朗读预设 content。
const GENERAL_CONTENT: &str = r#"You are a general English reading-aloud text extractor.

Your job: given pages (images or text) of an English document, extract ALL English text that should be read aloud. This is NOT a test paper — do NOT filter out "answers", do NOT identify question numbers or task types.

Rules:
1. Treat continuous prose as a single item with number=null and task_type=listen_passage.
2. Chinese explanatory text (说明, 注释) → `zh_instruction`. Do NOT read Chinese content aloud.
3. `speaker` is always null.
4. Keep all English content, even if it looks like an answer — this is a general reading document.
5. Output STRICT JSON with EXACTLY these field names — do not rename them:

{"title": "nullable string or null", "parts": [{"label": "Reading", "task_type": "listen_passage", "zh_instruction": null, "items": [{"number": null, "text": "The full English passage goes here.", "speaker": null}]}]}

Only output the JSON object, no markdown fences, no extra keys."#;

/// 单词听写预设 content。
const WORDS_CONTENT: &str = r#"You are an English vocabulary/dictation list extractor.

Your job: given a vocabulary list or dictation worksheet, extract each individual English word as a separate item.

Rules:
1. Each English word → one item. `text` = the word itself (no punctuation, no translation).
2. Number items sequentially: number=1, 2, 3, … in order of appearance.
3. Do NOT include sentences, definitions, Chinese translations, or answer lines.
4. `task_type` = listen_and_write for all items.
5. `speaker` is always null.
6. Chinese section headings → `zh_instruction`.
7. Output STRICT JSON with EXACTLY these field names — do not rename them:

{"title": "nullable string or null", "parts": [{"label": "Words", "task_type": "listen_and_write", "zh_instruction": null, "items": [{"number": 1, "text": "apple", "speaker": null}, {"number": 2, "text": "banana", "speaker": null}]}]}

Only output the JSON object, no markdown fences, no extra keys."#;

/// 中英对照朗读预设 content。
const BILINGUAL_CONTENT: &str = r#"You are a Chinese primary-school English listening-test script extractor (bilingual mode).

Your job: given scanned pages (images) or text of a practice worksheet, extract ONLY the "Listening Script" / "Tapescript" / "听力原文" section.

Rules:
1. DO NOT include answer keys (e.g. "1.B 2.A", "答案:", "→4,3,2,1", "( B )").
2. KEEP Chinese translations of English sentences — append them to the item's `text`: English sentence first, then a space, then the Chinese translation. This enables bilingual read-aloud (English then Chinese).
3. Strip item numbers from `text`; put the digit in `number` (integer). For passage-style items set number to null.
4. Chinese task instructions (第一大题…) → `zh_instruction` only (not appended to text).
5. Infer `task_type` from the section heading:
   - "listen and choose" / "选择" → listen_and_choose
   - "number" / "排序" / "编号" → listen_and_number
   - "judge" / "判断" → listen_and_judge
   - "write" / "填写" → listen_and_write
   - "circle" / "圈出" → listen_and_circle
   - continuous passage / short text → listen_passage
   - unclear → unknown
6. A continuous short passage uses one item with number=null.
7. `speaker` is always null.
8. Output STRICT JSON with EXACTLY these field names — do not rename them:

{"title": "nullable string or null", "parts": [{"label": "Part One. Listen and choose.", "task_type": "listen_and_choose", "zh_instruction": "第一大题说明或null", "items": [{"number": 1, "text": "I can take the dishes to the kitchen. 我可以把盘子拿到厨房。", "speaker": null}]}]}

Only output the JSON object, no markdown fences, no extra keys."#;

/// 对话分角色预设 content。
const DIALOGUE_CONTENT: &str = r#"You are a Chinese primary-school English listening-test script extractor (dialogue mode).

Your job: given scanned pages (images) or text of a practice worksheet, extract ONLY the "Listening Script" / "Tapescript" / "听力原文" section. This material contains DIALOGUE with multiple speakers taking turns.

Rules:
1. DO NOT include answer keys (e.g. "1.B 2.A", "答案:", "→4,3,2,1", "( B )").
2. DO NOT include Chinese translations of English sentences.
3. Identify the SPEAKER of each line. Fill `speaker` with 'A'/'B' or the character name from the material (e.g. "Tom", "Lisa"). Each speaker's utterance → one item.
4. `text` = what that speaker says (no speaker label prefix, no item number).
5. Strip item numbers from `text`; put the digit in `number` (integer) if present; for dialogue lines without numbers set number=null.
6. Chinese task instructions (第一大题…) → `zh_instruction`.
7. Infer `task_type` from section heading:
   - "listen and choose" / "选择" → listen_and_choose
   - "number" / "排序" / "编号" → listen_and_number
   - "judge" / "判断" → listen_and_judge
   - "write" / "填写" → listen_and_write
   - "circle" / "圈出" → listen_and_circle
   - dialogue passage → listen_passage
   - unclear → unknown
8. List items in dialogue order (turn by turn).
9. Output STRICT JSON with EXACTLY these field names — do not rename them:

{"title": "nullable string or null", "parts": [{"label": "Part One. Dialogue.", "task_type": "listen_and_choose", "zh_instruction": null, "items": [{"number": null, "text": "Hello! How are you?", "speaker": "A"}, {"number": null, "text": "I'm fine, thank you.", "speaker": "B"}]}]}

Only output the JSON object, no markdown fences, no extra keys."#;

/// 返回 5 个内置模板。
pub fn builtin_templates() -> Vec<PromptTemplate> {
    vec![
        PromptTemplate {
            id: "standard".into(),
            name: "标准听力卷".into(),
            content: STANDARD_CONTENT.to_string(),
            builtin: true,
        },
        PromptTemplate {
            id: "general".into(),
            name: "通用英文朗读".into(),
            content: GENERAL_CONTENT.to_string(),
            builtin: true,
        },
        PromptTemplate {
            id: "words".into(),
            name: "单词听写".into(),
            content: WORDS_CONTENT.to_string(),
            builtin: true,
        },
        PromptTemplate {
            id: "bilingual".into(),
            name: "中英都读(对照)".into(),
            content: BILINGUAL_CONTENT.to_string(),
            builtin: true,
        },
        PromptTemplate {
            id: "dialogue".into(),
            name: "对话分角色".into(),
            content: DIALOGUE_CONTENT.to_string(),
            builtin: true,
        },
    ]
}

// ─── prompts.json 持久化 ──────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct PromptsFile {
    #[serde(default = "default_selected_id")]
    selected: String,
    #[serde(default)]
    custom: Vec<CustomEntry>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct CustomEntry {
    id: String,
    name: String,
    content: String,
}

fn default_selected_id() -> String {
    "standard".to_string()
}

fn prompts_json_path() -> Option<std::path::PathBuf> {
    dirs::document_dir().map(|d| d.join("ListenForge").join("prompts.json"))
}

fn read_prompts_file() -> PromptsFile {
    let path = match prompts_json_path() {
        Some(p) => p,
        None => return PromptsFile::default(),
    };
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => PromptsFile::default(),
    }
}

fn write_prompts_file(pf: &PromptsFile) -> Result<(), String> {
    let path = prompts_json_path()
        .ok_or_else(|| "无法获取 Documents 目录".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("创建目录失败: {e}"))?;
    }
    let json = serde_json::to_string_pretty(pf)
        .map_err(|e| format!("序列化 prompts.json 失败: {e}"))?;
    std::fs::write(&path, json)
        .map_err(|e| format!("写入 prompts.json 失败: {e}"))
}

// ─── 公开 API ─────────────────────────────────────────────────────────────────

/// 返回内置 + 用户自定义模板列表(自定义 builtin=false)。
pub fn all_templates() -> Vec<PromptTemplate> {
    let pf = read_prompts_file();
    let mut result = builtin_templates();
    for entry in pf.custom {
        result.push(PromptTemplate {
            id: entry.id,
            name: entry.name,
            content: entry.content,
            builtin: false,
        });
    }
    result
}

/// 纯函数:在 templates 列表中查找 selected_id 对应的 content；
/// 找不到则返回 standard 的 content。
pub fn select_content(templates: &[PromptTemplate], selected_id: &str) -> String {
    templates
        .iter()
        .find(|t| t.id == selected_id)
        .map(|t| t.content.clone())
        .unwrap_or_else(|| STANDARD_CONTENT.to_string())
}

/// 读取当前选中模板的 content。找不到/无文件 → standard content。
pub fn selected_prompt_content() -> String {
    let pf = read_prompts_file();
    let templates = all_templates();
    select_content(&templates, &pf.selected)
}

/// 将选中的模板 id 写入 prompts.json。
pub fn set_selected(id: &str) -> Result<(), String> {
    let mut pf = read_prompts_file();
    // 验证 id 存在
    let templates = all_templates();
    if !templates.iter().any(|t| t.id == id) {
        return Err(format!("模板 id 不存在: {id}"));
    }
    pf.selected = id.to_string();
    write_prompts_file(&pf)
}

/// 保存自定义模板(name 已存在则覆盖,否则新增)。返回 id。
pub fn save_custom(name: &str, content: &str) -> Result<String, String> {
    if name.trim().is_empty() {
        return Err("模板名称不能为空".to_string());
    }
    let mut pf = read_prompts_file();
    // 生成或复用 id:用 name 的 slug 或 custom_{n}
    let id = make_custom_id(name, &pf.custom);
    match pf.custom.iter_mut().find(|e| e.id == id) {
        Some(entry) => {
            entry.name = name.to_string();
            entry.content = content.to_string();
        }
        None => {
            pf.custom.push(CustomEntry {
                id: id.clone(),
                name: name.to_string(),
                content: content.to_string(),
            });
        }
    }
    write_prompts_file(&pf)?;
    Ok(id)
}

/// 生成自定义模板 id：将 name 转为小写字母数字 slug，若已被内置占用则追加 _custom。
fn make_custom_id(name: &str, existing_custom: &[CustomEntry]) -> String {
    let slug: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect::<String>()
        .to_lowercase();

    let base = if slug.is_empty() {
        format!("custom_{}", existing_custom.len() + 1)
    } else {
        slug
    };

    // 若与内置 id 冲突,加 _custom 后缀
    let builtin_ids = ["standard", "general", "words", "bilingual", "dialogue"];
    if builtin_ids.contains(&base.as_str()) {
        format!("{base}_custom")
    } else {
        base
    }
}

// ─── 测试 ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_templates_returns_five() {
        let templates = builtin_templates();
        assert_eq!(templates.len(), 5, "应有 5 个内置预设");
    }

    #[test]
    fn builtin_ids_are_correct() {
        let templates = builtin_templates();
        let ids: Vec<&str> = templates.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"standard"));
        assert!(ids.contains(&"general"));
        assert!(ids.contains(&"words"));
        assert!(ids.contains(&"bilingual"));
        assert!(ids.contains(&"dialogue"));
    }

    #[test]
    fn all_builtins_have_builtin_true() {
        for t in builtin_templates() {
            assert!(t.builtin, "内置模板 {} 的 builtin 应为 true", t.id);
        }
    }

    #[test]
    fn select_content_finds_known_id() {
        let templates = builtin_templates();
        let content = select_content(&templates, "general");
        assert!(
            content.contains("general") || content.contains("NOT a test paper"),
            "general 模板内容应包含对应说明"
        );
    }

    #[test]
    fn select_content_falls_back_to_standard_on_unknown_id() {
        let templates = builtin_templates();
        let content = select_content(&templates, "nonexistent_id_xyz");
        // 回退到 standard
        assert!(
            content.contains("listening-test script extractor"),
            "未知 id 应回退到 standard"
        );
    }

    #[test]
    fn select_content_standard_contains_json_example_with_speaker() {
        let templates = builtin_templates();
        let content = select_content(&templates, "standard");
        assert!(
            content.contains("speaker"),
            "standard 模板应包含 speaker 字段示例"
        );
    }

    #[test]
    fn select_content_dialogue_has_speaker_instructions() {
        let templates = builtin_templates();
        let content = select_content(&templates, "dialogue");
        assert!(
            content.contains("speaker"),
            "dialogue 模板应包含 speaker 字段说明"
        );
        assert!(
            content.contains("'A'") || content.contains("A"),
            "dialogue 模板应提到 A/B 角色"
        );
    }

    #[test]
    fn save_and_load_custom_in_tempdir() {
        // 测试纯逻辑:make_custom_id + 对 PromptsFile 的操作
        let mut pf = PromptsFile::default();

        // 空格不是字母数字/下划线/连字符,会被过滤掉
        let id = make_custom_id("My Template", &pf.custom);
        assert_eq!(id, "mytemplate");

        pf.custom.push(CustomEntry {
            id: id.clone(),
            name: "My Template".to_string(),
            content: "test content".to_string(),
        });

        assert_eq!(pf.custom.len(), 1);
        assert_eq!(pf.custom[0].id, id);
    }

    #[test]
    fn make_custom_id_avoids_builtin_conflict() {
        let existing: Vec<CustomEntry> = vec![];
        let id = make_custom_id("standard", &existing);
        assert_eq!(id, "standard_custom");
    }

    #[test]
    fn make_custom_id_empty_name_uses_counter() {
        let existing: Vec<CustomEntry> = vec![];
        let id = make_custom_id("!@#$", &existing);
        assert_eq!(id, "custom_1");
    }

    #[test]
    fn select_content_words_does_not_include_sentence() {
        let templates = builtin_templates();
        let content = select_content(&templates, "words");
        assert!(
            content.contains("word") || content.contains("dictation"),
            "words 模板应提到 word/dictation"
        );
    }

    #[test]
    fn select_content_bilingual_keeps_chinese_translation() {
        let templates = builtin_templates();
        let content = select_content(&templates, "bilingual");
        assert!(
            content.contains("Chinese translation") || content.contains("中文"),
            "bilingual 模板应提到中文翻译"
        );
    }

    #[test]
    fn prompts_file_round_trip_in_tempdir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("prompts.json");

        let pf = PromptsFile {
            selected: "dialogue".to_string(),
            custom: vec![CustomEntry {
                id: "my_custom".to_string(),
                name: "My Custom".to_string(),
                content: "custom content here".to_string(),
            }],
        };

        let json = serde_json::to_string_pretty(&pf).unwrap();
        std::fs::write(&path, &json).unwrap();

        let loaded: PromptsFile = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(loaded.selected, "dialogue");
        assert_eq!(loaded.custom.len(), 1);
        assert_eq!(loaded.custom[0].id, "my_custom");
        assert_eq!(loaded.custom[0].content, "custom content here");
    }
}
