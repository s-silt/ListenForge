use crate::llm::ExtractedScript;
use crate::model::{ExportConfig, Item, Part, Project, SourceType, VoiceConfig};
use std::path::Path;

/// ExtractedScript(纯提取结果)→ Project(应用状态):补 id/时间/默认朗读与导出配置。
pub fn build_project(extracted: ExtractedScript, source_file: &str, source_type: SourceType) -> Project {
    let parts = extracted.parts.into_iter().enumerate().map(|(i, ep)| {
        let items = ep.items.into_iter().map(|ei| Item {
            id: uuid::Uuid::new_v4().to_string(),
            number: ei.number,
            text: ei.text,
            enabled: true,
            repeat: 1,
            gap_after_ms: 3000,
            read_number: true,
            override_voice: None,
        }).collect();
        let has_zh = ep.zh_instruction.is_some();
        Part {
            id: uuid::Uuid::new_v4().to_string(),
            index: i as u32,
            label: ep.label,
            task_type: ep.task_type,
            read_label: true,
            zh_instruction: ep.zh_instruction,
            read_zh_instruction: has_zh,
            items,
            gap_after_ms: 5000,
        }
    }).collect();

    let title = extracted.title.unwrap_or_else(|| {
        Path::new(source_file)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string()
    });

    Project {
        id: uuid::Uuid::new_v4().to_string(),
        title,
        source_file: source_file.to_string(),
        source_type,
        created_at: chrono::Utc::now().to_rfc3339(),
        parts,
        voice_config: VoiceConfig::default(),
        export_config: ExportConfig::default(),
    }
}

// ─── 单测 ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{ExtractedItem, ExtractedPart, ExtractedScript};
    use crate::model::TaskType;

    fn make_extracted() -> ExtractedScript {
        ExtractedScript {
            title: None,
            parts: vec![ExtractedPart {
                label: "Part One. Listen and choose.".to_string(),
                task_type: TaskType::ListenAndChoose,
                zh_instruction: Some("第一大题，听录音选择。".to_string()),
                items: vec![ExtractedItem {
                    number: Some(1),
                    text: "I can take the dishes to the kitchen.".to_string(),
                }],
            }],
        }
    }

    #[test]
    fn build_project_item_defaults() {
        let project = build_project(
            make_extracted(),
            "C:/test/unit2.pdf",
            SourceType::PdfText,
        );
        assert_eq!(project.parts.len(), 1);
        let part = &project.parts[0];
        assert_eq!(part.items.len(), 1);

        let item = &part.items[0];
        assert_eq!(item.repeat, 1);
        assert_eq!(item.gap_after_ms, 3000);
        assert!(item.enabled);
        assert!(item.read_number);
        assert!(item.override_voice.is_none());
    }

    #[test]
    fn build_project_part_defaults() {
        let project = build_project(
            make_extracted(),
            "C:/test/unit2.pdf",
            SourceType::PdfText,
        );
        let part = &project.parts[0];
        assert_eq!(part.gap_after_ms, 5000);
        assert!(part.read_zh_instruction);
        assert_eq!(part.index, 0);
    }

    #[test]
    fn build_project_ids_nonempty() {
        let project = build_project(
            make_extracted(),
            "C:/test/unit2.pdf",
            SourceType::PdfText,
        );
        assert!(!project.id.is_empty());
        assert!(!project.parts[0].id.is_empty());
        assert!(!project.parts[0].items[0].id.is_empty());
    }

    #[test]
    fn build_project_title_fallback_to_file_stem() {
        let project = build_project(
            make_extracted(), // title = None
            "C:/test/unit2.pdf",
            SourceType::PdfText,
        );
        // title=None → 回退到文件名 stem
        assert_eq!(project.title, "unit2");
    }

    #[test]
    fn build_project_title_from_extracted() {
        let mut extracted = make_extracted();
        extracted.title = Some("Unit 2 Listening".to_string());
        let project = build_project(extracted, "C:/test/unit2.pdf", SourceType::PdfText);
        assert_eq!(project.title, "Unit 2 Listening");
    }
}
