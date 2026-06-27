use crate::llm::ExtractedScript;
use crate::model::{ExportConfig, Item, Part, Project, SourceType, TaskType, VoiceConfig};
use std::path::Path;

/// ExtractedScript(纯提取结果)→ Project(应用状态):补 id/时间/默认朗读与导出配置。
pub fn build_project(extracted: ExtractedScript, source_file: &str, source_type: SourceType) -> Project {
    let parts = extracted.parts.into_iter().enumerate().map(|(i, ep)| {
        // 音标单词卡:节奏更紧凑、不报题号、整卡只在开头(第 0 个分组)报一次音标。
        // 其余题型沿用听力卷的默认值。详见 docs / build_project 文档。
        let is_phonics = ep.task_type == TaskType::Phonics;
        let item_gap_ms = if is_phonics { 1500 } else { 3000 };
        let item_read_number = !is_phonics;
        let part_gap_ms = if is_phonics { 1500 } else { 5000 };
        // phonics:仅首个拼写分组朗读 label(承载音标播报),其余分组的拼写字母不读。
        let read_label = if is_phonics { i == 0 } else { true };

        let items = ep.items.into_iter().map(|ei| Item {
            id: uuid::Uuid::new_v4().to_string(),
            number: ei.number,
            text: ei.text,
            enabled: true,
            repeat: 1,
            gap_after_ms: item_gap_ms,
            read_number: item_read_number,
            override_voice: None,
            speaker: ei.speaker,
        }).collect();
        let has_zh = ep.zh_instruction.is_some();
        Part {
            id: uuid::Uuid::new_v4().to_string(),
            index: i as u32,
            label: ep.label,
            task_type: ep.task_type,
            read_label,
            zh_instruction: ep.zh_instruction,
            read_zh_instruction: has_zh,
            items,
            gap_after_ms: part_gap_ms,
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
                    speaker: Some("A".to_string()),
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

    #[test]
    fn build_project_speaker_passed_through() {
        let project = build_project(
            make_extracted(),
            "C:/test/unit2.pdf",
            SourceType::PdfText,
        );
        let item = &project.parts[0].items[0];
        assert_eq!(item.speaker, Some("A".to_string()), "speaker 应从 ExtractedItem 传入 Item");
    }

    /// 构造一张音标卡:两个拼写分组(主拼写 i + 变体 y),task_type=phonics。
    fn make_phonics_extracted() -> ExtractedScript {
        ExtractedScript {
            title: Some("短音 /ɪ/".to_string()),
            parts: vec![
                ExtractedPart {
                    label: "The short i sound.".to_string(),
                    task_type: TaskType::Phonics,
                    zh_instruction: None,
                    items: vec![
                        ExtractedItem { number: None, text: "big".to_string(), speaker: None },
                        ExtractedItem { number: None, text: "chip".to_string(), speaker: None },
                    ],
                },
                ExtractedPart {
                    label: "y".to_string(),
                    task_type: TaskType::Phonics,
                    zh_instruction: None,
                    items: vec![
                        ExtractedItem { number: None, text: "physical".to_string(), speaker: None },
                    ],
                },
            ],
        }
    }

    #[test]
    fn build_project_phonics_item_defaults() {
        let project = build_project(make_phonics_extracted(), "C:/cards/i.png", SourceType::Image);
        let item = &project.parts[0].items[0];
        // 音标卡:间隔 1.5s、读一遍、不报题号。
        assert_eq!(item.gap_after_ms, 1500, "音标卡单词间隔应为 1500ms");
        assert_eq!(item.repeat, 1, "音标卡单词读一遍");
        assert!(!item.read_number, "音标卡不报题号");
    }

    #[test]
    fn build_project_phonics_announces_phoneme_only_on_first_group() {
        let project = build_project(make_phonics_extracted(), "C:/cards/i.png", SourceType::Image);
        // 仅第 0 个分组(承载音标播报)read_label=true;其余拼写分组不读字母。
        assert!(project.parts[0].read_label, "首个分组应朗读 label(报音标)");
        assert!(!project.parts[1].read_label, "其余拼写分组不读 label(字母)");
    }

    #[test]
    fn build_project_phonics_part_gap_is_tight() {
        let project = build_project(make_phonics_extracted(), "C:/cards/i.png", SourceType::Image);
        for part in &project.parts {
            assert_eq!(part.gap_after_ms, 1500, "音标卡分组间间隔应为 1500ms(听感连续)");
        }
    }

    #[test]
    fn build_project_non_phonics_keeps_listening_defaults() {
        // 回归:非 phonics 题型仍用听力卷默认值(3s/5s、报题号、每组读 label)。
        let project = build_project(make_extracted(), "C:/test/unit2.pdf", SourceType::PdfText);
        let item = &project.parts[0].items[0];
        assert_eq!(item.gap_after_ms, 3000);
        assert!(item.read_number);
        assert!(project.parts[0].read_label);
        assert_eq!(project.parts[0].gap_after_ms, 5000);
    }
}
