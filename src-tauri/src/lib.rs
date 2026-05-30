pub mod assembler;
pub mod audio;
pub mod export;
pub mod input_builder;
pub mod llm;
pub mod model;
pub mod persistence;
pub mod prompts;
pub mod render;
pub mod ssml;
pub mod tts;

use model::{Project, SourceType};
use std::path::Path;
use tauri::Emitter;
use serde::Serialize;

#[derive(Clone, Serialize)]
struct ProgressPayload {
    current: u32,
    total: u32,
    message: String,
}

fn infer_source_type(path: &str) -> SourceType {
    match Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .as_deref()
    {
        Some("docx") | Some("doc") => SourceType::Docx,
        Some("jpg") | Some("jpeg") | Some("png") | Some("webp") => SourceType::Image,
        _ => SourceType::PdfText,
    }
}

#[tauri::command]
async fn extract_script(path: String, prompt_override: Option<String>) -> Result<Project, String> {
    let blocks = input_builder::build_blocks(&path)?;
    let (cfg, api_key) = llm::read_llm_config()?;
    // 优先用前端传来的「当前界面模板内容」(所见即所得)；
    // 为空时才回退到持久化的 selected，避免"选了模板但没点应用→提取仍用默认"的陷阱。
    let prompt = match prompt_override {
        Some(p) if !p.trim().is_empty() => p,
        _ => prompts::selected_prompt_content(),
    };
    let provider = llm::openai::OpenAiProvider::new(cfg, api_key, prompt)?;
    let extracted = { use llm::LlmProvider; provider.extract(blocks).await? };
    Ok(assembler::build_project(extracted, &path, infer_source_type(&path)))
}

#[tauri::command]
fn get_prompt_templates() -> Vec<prompts::PromptTemplate> {
    prompts::all_templates()
}

#[tauri::command]
fn save_prompt_selection(id: String) -> Result<(), String> {
    prompts::set_selected(&id)
}

#[tauri::command]
fn save_custom_prompt(name: String, content: String) -> Result<String, String> {
    prompts::save_custom(&name, &content)
}

#[tauri::command]
async fn demo_progress(app: tauri::AppHandle) -> Result<(), String> {
    let total = 5;
    for i in 1..=total {
        app.emit(
            "progress",
            ProgressPayload { current: i, total, message: format!("step {i}") },
        )
        .map_err(|e| e.to_string())?;
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }
    Ok(())
}

#[tauri::command]
fn health() -> String {
    "ok".to_string()
}

#[tauri::command]
fn save_project_cmd(project: Project) -> Result<String, String> {
    persistence::save_project(&project).map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
fn load_project_cmd(path: String) -> Result<Project, String> {
    persistence::load_project(&path)
}

#[tauri::command]
async fn generate_audio(project: model::Project, output_dir: String) -> Result<Vec<String>, String> {
    // 配置了 Azure（key + region）就自动用 Azure（付费，无限流），否则用免费 edge-tts。
    let (full, parts) = match tts::azure::read_azure_config() {
        Some((key, region)) => {
            let provider = tts::azure::AzureTtsProvider::new(key, region)?;
            audio::generate_project_audio(&project, &provider).await?
        }
        None => {
            let provider = tts::edge::EdgeTtsProvider::new();
            audio::generate_project_audio(&project, &provider).await?
        }
    };
    export::save_audio(&full, &parts, &output_dir, &project.title)
}

#[tauri::command]
fn get_azure_tts_config() -> tts::azure::AzureConfigView {
    tts::azure::read_azure_config_view()
}

#[tauri::command]
fn save_azure_tts_config(key: String, region: String) -> Result<(), String> {
    tts::azure::write_azure_config(&key, &region)
}

#[tauri::command]
fn get_llm_config() -> llm::LlmConfigView {
    llm::read_config_view()
}

#[tauri::command]
fn save_llm_config(base_url: String, model: String, api_key: Option<String>) -> Result<(), String> {
    llm::write_dotenv(&base_url, &model, api_key.as_deref())
}

#[tauri::command]
fn get_voices() -> Vec<tts::Voice> {
    tts::preset_voices()
}

#[cfg(test)]
mod cmd_tests {
    use super::*;

    #[test]
    fn health_returns_ok() {
        assert_eq!(health(), "ok");
    }

    #[test]
    fn infer_source_type_pdf() {
        assert_eq!(infer_source_type("foo/bar.pdf"), SourceType::PdfText);
    }

    #[test]
    fn infer_source_type_docx() {
        assert_eq!(infer_source_type("foo/bar.docx"), SourceType::Docx);
    }

    #[test]
    fn infer_source_type_image() {
        assert_eq!(infer_source_type("foo/bar.png"), SourceType::Image);
        assert_eq!(infer_source_type("foo/bar.jpg"), SourceType::Image);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            health,
            save_project_cmd,
            load_project_cmd,
            demo_progress,
            extract_script,
            generate_audio,
            get_llm_config,
            save_llm_config,
            get_prompt_templates,
            save_prompt_selection,
            save_custom_prompt,
            get_voices,
            get_azure_tts_config,
            save_azure_tts_config
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
