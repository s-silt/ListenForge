mod model;
mod persistence;
pub mod render;

use model::Project;
use tauri::Emitter;
use serde::Serialize;

#[derive(Clone, Serialize)]
struct ProgressPayload {
    current: u32,
    total: u32,
    message: String,
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

#[cfg(test)]
mod cmd_tests {
    use super::*;

    #[test]
    fn health_returns_ok() {
        assert_eq!(health(), "ok");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![health, save_project_cmd, load_project_cmd, demo_progress])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
