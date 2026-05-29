use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    PdfText,
    PdfScanned,
    Docx,
    Image,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    ListenAndChoose,
    ListenAndNumber,
    ListenAndJudge,
    ListenAndWrite,
    ListenAndCircle,
    ListenPassage,
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Item {
    pub id: String,
    pub number: Option<u32>,
    pub text: String,
    pub enabled: bool,
    pub repeat: u8,
    pub gap_after_ms: u32,
    pub read_number: bool,
    pub override_voice: Option<String>,
    #[serde(default)]
    pub speaker: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Part {
    pub id: String,
    pub index: u32,
    pub label: String,
    pub task_type: TaskType,
    pub read_label: bool,
    pub zh_instruction: Option<String>,
    pub read_zh_instruction: bool,
    pub items: Vec<Item>,
    pub gap_after_ms: u32,
}

fn default_teacher_voice() -> String {
    "en-US-GuyNeural".to_string()
}

fn default_student_voice() -> String {
    "en-US-AnaNeural".to_string()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct VoiceConfig {
    pub provider: String,
    pub en_voice: String,
    pub zh_voice: String,
    pub rate: i32,
    pub pitch: i32,
    pub volume: u32,
    #[serde(default = "default_teacher_voice")]
    pub teacher_voice: String,
    #[serde(default = "default_student_voice")]
    pub student_voice: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ExportConfig {
    pub output_dir: String,
    pub generate_full: bool,
    pub generate_per_part: bool,
    pub generate_script_txt: bool,
    pub generate_script_docx: bool,
    pub generate_ssml: bool,
    pub zip_all: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub source_file: String,
    pub source_type: SourceType,
    pub created_at: String,
    pub parts: Vec<Part>,
    pub voice_config: VoiceConfig,
    pub export_config: ExportConfig,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            provider: "edge".into(),
            en_voice: "en-GB-SoniaNeural".into(),
            zh_voice: "zh-CN-XiaoxiaoNeural".into(),
            rate: 0,
            pitch: 0,
            volume: 100,
            teacher_voice: "en-US-GuyNeural".into(),
            student_voice: "en-US-AnaNeural".into(),
        }
    }
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            output_dir: String::new(),
            generate_full: true,
            generate_per_part: true,
            generate_script_txt: true,
            generate_script_docx: false,
            generate_ssml: false,
            zip_all: false,
        }
    }
}

impl Project {
    /// 创建一个空项目(用于新建 / 测试)。
    pub fn new(title: &str, source_file: &str, source_type: SourceType) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            title: title.into(),
            source_file: source_file.into(),
            source_type,
            created_at: chrono::Utc::now().to_rfc3339(),
            parts: Vec::new(),
            voice_config: VoiceConfig::default(),
            export_config: ExportConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_json_round_trip_preserves_all_fields() {
        let mut p = Project::new("Unit 2", "C:/x/unit2.pdf", SourceType::PdfText);
        p.parts.push(Part {
            id: "part-1".into(),
            index: 0,
            label: "Part One. Listen and choose.".into(),
            task_type: TaskType::ListenAndChoose,
            read_label: true,
            zh_instruction: Some("第一大题,听录音选择。".into()),
            read_zh_instruction: true,
            items: vec![Item {
                id: "item-1".into(),
                number: Some(1),
                text: "I can take the dishes to the kitchen.".into(),
                enabled: true,
                repeat: 2,
                gap_after_ms: 3000,
                read_number: true,
                override_voice: None,
                speaker: Some("A".into()),
            }],
            gap_after_ms: 5000,
        });

        let json = serde_json::to_string(&p).expect("serialize");
        let back: Project = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(p, back);
    }

    #[test]
    fn enums_serialize_as_snake_case() {
        let json = serde_json::to_string(&SourceType::PdfScanned).unwrap();
        assert_eq!(json, "\"pdf_scanned\"");
        let json = serde_json::to_string(&TaskType::ListenAndNumber).unwrap();
        assert_eq!(json, "\"listen_and_number\"");
    }

    #[test]
    fn voice_config_default_has_teacher_student_voices() {
        let cfg = VoiceConfig::default();
        assert_eq!(cfg.teacher_voice, "en-US-GuyNeural");
        assert_eq!(cfg.student_voice, "en-US-AnaNeural");
    }
}
