pub mod db;
pub mod error;
pub mod ingest;
pub mod judongche;
pub mod models;
pub mod repo;
pub mod settings;

use std::fs;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // 库文件放应用数据目录（随用户配置走），不进安装目录
            let dir = app.path().app_data_dir().expect("无法解析应用数据目录");
            fs::create_dir_all(&dir).expect("无法创建应用数据目录");
            let database = db::Db::open(&dir.join("dealer-integration.db")).expect("无法打开本地数据库");
            app.manage(database);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![ping])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// IPC 冒烟命令：前端启动时调用一次，验证 Tauri 桥可用
#[tauri::command]
fn ping(name: &str) -> String {
    format!("pong: {name}")
}
