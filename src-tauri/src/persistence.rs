use crate::model::Project;
use std::path::{Path, PathBuf};

/// 默认项目目录:~/Documents/ListenForge/。定位失败时返回 Err，
/// 不再静默兜底到当前工作目录（避免项目文件落到不可预测的位置）。
fn default_project_dir() -> Result<PathBuf, String> {
    dirs::document_dir()
        .map(|d| d.join("ListenForge"))
        .ok_or_else(|| "无法定位 Documents 目录".to_string())
}

/// 把标题清洗成安全文件名。
fn sanitize(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| if r#"\/:*?"<>|"#.contains(c) { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() { "untitled".to_string() } else { trimmed.to_string() }
}

/// 保存到指定目录(便于测试),返回写入路径。
pub fn save_project_to(dir: &Path, project: &Project) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.lfproj", sanitize(&project.title)));
    let json = serde_json::to_string_pretty(project).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path)
}

/// 保存到默认目录。
pub fn save_project(project: &Project) -> Result<PathBuf, String> {
    save_project_to(&default_project_dir()?, project)
}

/// 从路径加载。
pub fn load_project(path: &str) -> Result<Project, String> {
    let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Project, SourceType};

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let project = Project::new("Unit 2 听力", "C:/x/unit2.pdf", SourceType::PdfText);

        let path = save_project_to(dir.path(), &project).expect("save");
        assert!(path.exists());
        assert_eq!(path.extension().unwrap(), "lfproj");

        let loaded = load_project(path.to_str().unwrap()).expect("load");
        assert_eq!(loaded, project);
    }

    #[test]
    fn sanitize_removes_illegal_chars() {
        let dir = tempfile::tempdir().unwrap();
        let project = Project::new("Unit/2:test?", "x", SourceType::Image);
        let path = save_project_to(dir.path(), &project).expect("save");
        let name = path.file_name().unwrap().to_string_lossy();
        assert_eq!(name, "Unit_2_test_.lfproj");
    }
}
