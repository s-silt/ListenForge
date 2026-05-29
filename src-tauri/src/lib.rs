mod model;

#[tauri::command]
fn health() -> String {
    "ok".to_string()
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
        .invoke_handler(tauri::generate_handler![health])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
