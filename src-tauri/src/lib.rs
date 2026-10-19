mod store;

use std::fs;

use tauri::{Manager, State};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // 库文件放应用数据目录（随用户配置走），不进安装目录
            let dir = app.path().app_data_dir().expect("无法解析应用数据目录");
            fs::create_dir_all(&dir).expect("无法创建应用数据目录");
            let db = store::Store::open(&dir.join("dealer-integration.db")).expect("无法打开本地数据库");
            app.manage(db);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            get_ocean_credentials,
            save_ocean_credentials
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// IPC 冒烟命令：前端启动时调用一次，验证 Tauri 桥可用
#[tauri::command]
fn ping(name: &str) -> String {
    format!("pong: {name}")
}

/// 读取巨量引擎应用凭证；从未保存过时两字段均为 null
#[tauri::command]
fn get_ocean_credentials(store: State<'_, store::Store>) -> Result<store::OceanCredentials, String> {
    store.get_ocean_credentials().map_err(|e| e.to_string())
}

/// 保存巨量引擎应用凭证（成对覆盖）
#[tauri::command]
fn save_ocean_credentials(
    store: State<'_, store::Store>,
    app_id: String,
    app_secret: String,
) -> Result<(), String> {
    let app_id = app_id.trim();
    let app_secret = app_secret.trim();
    if app_id.is_empty() || app_secret.is_empty() {
        return Err("App ID 与 App Secret 不能为空".into());
    }
    store.save_ocean_credentials(app_id, app_secret).map_err(|e| e.to_string())
}
