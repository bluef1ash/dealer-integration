//! 调度器（issue #10/#11/#12）：应用启动即跑，每小时拉取；快照按配置时刻每日一次；
//! 会话失效自动触发 WebView 重登（有密码自动回填，无密码提醒人工）；单店失败不中断整体调度。

use std::time::Duration;

use chrono::Timelike;
use tauri::{AppHandle, Emitter, Manager};

use crate::commands::Clients;
use crate::credentials;
use crate::db::Db;
use crate::models::Store;
use crate::{collector, login, settings};

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // 首个 tick 立即触发：启动即保鲜
        let mut ticker = tokio::time::interval(Duration::from_secs(3600));
        loop {
            ticker.tick().await;
            if let Err(e) = run_hourly(&app).await {
                eprintln!("[调度] 小时拉取失败（不影响后续调度）: {e}");
            }
            if let Err(e) = maybe_daily_snapshot(&app).await {
                eprintln!("[调度] 对账快照失败（不影响后续调度）: {e}");
            }
        }
    });
}

/// 每小时：逐店拉取（未回填的店走 90 天，否则 7 天窗口）+ 巨量报表
async fn run_hourly(app: &AppHandle) -> crate::error::AppResult<()> {
    let db = app.state::<Db>();
    let clients = app.state::<Clients>();
    let stores: Vec<Store> = db.with(crate::repo::stores::list)?;
    for store in stores {
        // 单店环节全部尽力而为：任何失败只记日志，不中断其余门店与报表步（issue #10）
        let backfilled = db
            .with(|conn| Ok(crate::repo::stores::pull_state(conn, store.id)?.backfilled))
            .unwrap_or(false);
        let outcome = collector::pull_store(
            &db,
            &clients.jd,
            &store,
            collector::window_days_for(backfilled),
        )
        .await;
        if outcome.ok && !backfilled {
            let _ = db.with(|conn| crate::repo::stores::mark_backfilled(conn, store.id));
        }
        // 会话失效 → 自动重登（issue #11）：有密码自动回填，无密码提醒人工介入
        if outcome.session_expired {
            match credentials::load_store_password(&store.account) {
                Ok(password) => {
                    let has_password = password.is_some();
                    if let Err(e) = login::open_login(app, store.id, &store.account, password) {
                        eprintln!("[调度] {} 自动重登开窗失败: {e}", store.name);
                    }
                    if !has_password {
                        let _ = app.emit(
                            "login://needs-human",
                            serde_json::json!({ "store_id": store.id, "reason": "会话失效且未存密码，请人工登录" }),
                        );
                    }
                }
                Err(e) => eprintln!("[调度] {} 读取密码失败: {e}", store.name),
            }
        }
    }
    refresh_and_pull_ocean(app).await?;
    Ok(())
}

/// 巨量 token 过期前自动刷新，随后按日拉取近 7 天报表
async fn refresh_and_pull_ocean(app: &AppHandle) -> crate::error::AppResult<()> {
    let db = app.state::<Db>();
    let clients = app.state::<Clients>();
    let (app_id_str, secret, access, refresh) = db.with(|conn| {
        let creds = settings::get_ocean_credentials(conn)?;
        let tokens = credentials::load_ocean_tokens()?;
        Ok::<_, crate::error::AppError>((creds.app_id, creds.app_secret, tokens.0, tokens.1))
    })?;
    let (Some(app_id_str), Some(secret)) = (app_id_str, secret) else {
        return Ok(()); // 未配置应用凭证：跳过
    };
    let (Some(access), Some(refresh)) = (access, refresh) else {
        return Ok(()); // 未授权：跳过
    };
    let app_id: i64 = app_id_str.trim().parse().unwrap_or(0);

    // 过期前 5 分钟自动刷新
    let now = crate::db::now();
    let valid_until: i64 = db.with(|conn| {
        Ok(
            settings::get_setting(conn, settings::KEY_OCEAN_TOKEN_EXPIRES)?
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(0),
        )
    })?;
    let access = if valid_until.saturating_sub(now) < 300 {
        let tokens = clients
            .ocean
            .refresh_token(app_id, secret.trim(), &refresh)
            .await?;
        credentials::save_ocean_tokens(&tokens.access_token, &tokens.refresh_token)?;
        db.with_sql(|conn| {
            settings::save_setting(
                conn,
                settings::KEY_OCEAN_TOKEN_EXPIRES,
                &(now + tokens.expires_in).to_string(),
            )
        })?;
        tokens.access_token
    } else {
        access
    };
    collector::pull_ocean_reports(&db, &clients.ocean, &access, 7).await?;
    Ok(())
}

/// 到达配置时刻（默认 23 点）且今日尚无快照时执行对账
async fn maybe_daily_snapshot(app: &AppHandle) -> crate::error::AppResult<()> {
    let db = app.state::<Db>();
    let hour: i32 = db.with(|conn| {
        Ok(settings::get_setting(conn, settings::KEY_SNAPSHOT_HOUR)?
            .and_then(|v| v.parse().ok())
            .unwrap_or(23))
    })?;
    let now = chrono::Local::now();
    if now.hour() != hour as u32 {
        return Ok(());
    }
    let today = now.date_naive().to_string();
    let has_today = db.with(|conn| crate::snapshot::has_snapshot_for(conn, &today))?;
    if has_today {
        return Ok(());
    }
    // 对所有有会话的店各跑一次（内部跳过未登录店）
    crate::commands::run_snapshots(app, None).await?;
    Ok(())
}
