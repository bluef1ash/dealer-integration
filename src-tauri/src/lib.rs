pub mod collector;
pub mod commands;
pub mod credentials;
pub mod db;
pub mod error;
pub mod export;
pub mod ingest;
pub mod judongche;
pub mod login;
pub mod models;
pub mod ocean;
pub mod query;
pub mod repo;
pub mod scheduler;
pub mod settings;
pub mod snapshot;
pub mod tray;

use std::fs;

use tauri::Manager;

use commands::Clients;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // 库文件放应用数据目录（随用户配置走），不进安装目录
            let dir = app.path().app_data_dir().expect("无法解析应用数据目录");
            fs::create_dir_all(&dir).expect("无法创建应用数据目录");
            let database =
                db::Db::open(&dir.join("dealer-integration.db")).expect("无法打开本地数据库");
            app.manage(database);
            app.manage(Clients {
                jd: judongche::client::Client::new(),
                ocean: ocean::Client::new(),
            });
            scheduler::start(app.handle().clone());
            tray::setup(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            tray::on_window_event(window.app_handle(), window.label(), event);
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_stores,
            commands::add_store,
            commands::remove_store,
            commands::get_collection_status,
            commands::start_login,
            commands::pull_now,
            commands::pull_ocean_now,
            commands::get_leads,
            commands::get_leads_filter_options,
            commands::get_dashboard,
            commands::export_leads,
            commands::ocean::list_ocean_apps,
            commands::ocean::add_ocean_app,
            commands::ocean::remove_ocean_app,
            commands::ocean::start_ocean_auth,
            commands::ocean::list_ad_accounts,
            commands::set_store_mapping,
            commands::get_store_ad_accounts,
            commands::run_snapshot,
            commands::set_snapshot_hour,
            commands::get_snapshot_hour,
            commands::get_scheduler_config,
            commands::set_scheduler_config,
            commands::get_close_to_tray,
            commands::set_close_to_tray,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
