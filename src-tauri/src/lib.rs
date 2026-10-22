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
            // 旧版单应用巨量凭证（settings 键 + 全局 token）迁入多应用首行；失败只留痕不阻断启动，
            // 旧键未消费前下次启动会重试
            match database.with(|conn| -> crate::error::AppResult<()> {
                if let Some(app_id) = repo::ocean_apps::migrate_legacy_db(conn)? {
                    credentials::migrate_legacy_tokens_into_db(conn, &app_id)?;
                    repo::ocean_apps::clear_legacy_keys(conn)?;
                }
                Ok(())
            }) {
                Ok(()) => {}
                Err(e) => eprintln!("[迁移] 巨量旧凭证迁移失败（下次启动重试）: {e}"),
            }
            // 巨懂车密码从系统凭证库迁入 SQLite（用户决策：密码随库走）；失败留痕下次重试
            match database.with(repo::stores::migrate_keyring_passwords) {
                Ok(0) => {}
                Ok(n) => eprintln!("[迁移] 已从凭证库迁入 {n} 条巨懂车密码"),
                Err(e) => eprintln!("[迁移] 巨懂车密码迁移失败（下次启动重试）: {e}"),
            }
            // 会话 cookie 与巨量 token 从凭证库迁入 SQLite（用户决策：跨平台随库走）
            match database.with(credentials::migrate_keyring_into_db) {
                Ok((0, 0)) => {}
                Ok((cookies, tokens)) => {
                    eprintln!("[迁移] 已从凭证库迁入 {cookies} 条会话、{tokens} 组巨量 token")
                }
                Err(e) => eprintln!("[迁移] 凭证库迁移失败（下次启动重试）: {e}"),
            }
            app.manage(database);
            app.manage(Clients {
                jd: judongche::client::Client::new(),
                ocean: ocean::Client::new(),
            });
            scheduler::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_stores,
            commands::add_store,
            commands::remove_store,
            commands::get_collection_status,
            commands::start_login,
            commands::pull_now,
            commands::get_leads,
            commands::get_status_options,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
