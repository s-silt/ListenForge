mod model;
mod persistence;

use model::Project;

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
        .invoke_handler(tauri::generate_handler![health, save_project_cmd, load_project_cmd])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
