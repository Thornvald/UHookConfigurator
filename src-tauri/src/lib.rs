mod unreal;

use tauri::command;

#[command]
fn get_engines() -> std::collections::HashMap<String, unreal::EngineInfo> {
    unreal::get_engines()
}

#[command]
fn parse_project(path: String) -> Result<unreal::ProjectInfo, String> {
    unreal::parse_project(&path)
}

#[command]
fn install_hook(
    uproject_path: String,
    project_name: String,
    ubt_path: String,
) -> Result<(), String> {
    unreal::install_hook(&uproject_path, &project_name, &ubt_path)
}

#[command]
fn remove_hook(uproject_path: String) -> Result<(), String> {
    unreal::remove_hook(&uproject_path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            get_engines,
            parse_project,
            install_hook,
            remove_hook
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
