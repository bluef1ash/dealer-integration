#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![ping])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// IPC 冒烟命令：前端启动时调用一次，验证 Tauri 桥可用
#[tauri::command]
fn ping(name: &str) -> String {
    format!("pong: {name}")
}
