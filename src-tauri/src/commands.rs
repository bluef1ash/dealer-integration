//! Tauri command 边界：薄转发到各模块，错误转字符串给前端。

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::credentials;
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::judongche::client::{Client as JdClient, LeadsWindow};
use crate::models::{AdAccount, Dashboard, LeadPage, LeadsFilter, Range, Store};
use crate::{collector, login, ocean, query, repo, settings, snapshot};

/// 共享 HTTP 客户端
pub struct Clients {
    pub jd: JdClient,
    pub ocean: ocean::Client,
}

fn err_string(e: AppError) -> String {
    e.to_string()
}

fn get_store(db: &Db, id: i64) -> AppResult<Store> {
    db.with(|conn| repo::stores::get(conn, id))
}

// ---------- 门店管理（issue #5） ----------

#[tauri::command]
pub fn list_stores(db: State<'_, Db>) -> Result<Vec<Store>, String> {
    db.with(repo::stores::list).map_err(err_string)
}

#[tauri::command]
pub fn add_store(
    db: State<'_, Db>,
    name: String,
    account: String,
    password: Option<String>,
) -> Result<Store, String> {
    let name = name.trim();
    let account = account.trim();
    if name.is_empty() || account.is_empty() {
        return Err("门店名称与账号标识不能为空".into());
    }
    let store = db
        .with(|conn| repo::stores::insert(conn, name, account))
        .map_err(err_string)?;
    if let Some(password) = password.as_deref().filter(|p| !p.trim().is_empty()) {
        credentials::save_store_password(account, password.trim()).map_err(err_string)?;
    }
    Ok(store)
}

#[tauri::command]
pub fn remove_store(db: State<'_, Db>, store_id: i64) -> Result<(), String> {
    let store = get_store(&db, store_id).map_err(err_string)?;
    credentials::delete_store(&store.account).map_err(err_string)?;
    db.with(|conn| repo::stores::delete(conn, store_id))
        .map_err(err_string)
}

/// 会话状态列表（设置页 + 过期角标）
#[derive(Serialize)]
pub struct StoreStatus {
    pub store: Store,
    pub last_success_at: Option<i64>,
    pub last_error: String,
    pub backfilled: bool,
    /// 凭证库中是否有会话 cookie
    pub has_session: bool,
}

#[tauri::command]
pub fn get_collection_status(db: State<'_, Db>) -> Result<Vec<StoreStatus>, String> {
    db.with(|conn| -> AppResult<Vec<StoreStatus>> {
        let states: std::collections::HashMap<i64, crate::models::PullState> =
            repo::stores::pull_states(conn)?
                .into_iter()
                .map(|s| (s.store_id, s))
                .collect();
        Ok(repo::stores::list(conn)?
            .into_iter()
            .map(|store| {
                let state = states.get(&store.id);
                let has_session = match credentials::load_store_cookie(&store.account) {
                    Ok(c) => c.is_some(),
                    // 凭证库读失败会直接显示「未登录」，必须留痕否则无从排查
                    Err(e) => {
                        eprintln!("[状态] 凭证库读取失败 account={}: {e}", store.account);
                        false
                    }
                };
                StoreStatus {
                    has_session,
                    store: store.clone(),
                    last_success_at: state.and_then(|s| s.last_success_at),
                    last_error: state.map(|s| s.last_error.clone()).unwrap_or_default(),
                    backfilled: state.map(|s| s.backfilled).unwrap_or(false),
                }
            })
            .collect())
    })
    .map_err(err_string)
}

/// 发起 WebView 登录；凭证库里有密码则自动回填（issue #11 自动重登同一路径）。
/// 必须 async：同步 command 在主线程 IPC 路径里建窗口会触发 WebView2 断点异常
/// 0x80000003（tauri#12982，真机实证）；async 后在建制线程执行、经事件循环代理创建
#[tauri::command]
pub async fn start_login(app: AppHandle, db: State<'_, Db>, store_id: i64) -> Result<(), String> {
    let store = get_store(&db, store_id).map_err(err_string)?;
    let password = credentials::load_store_password(&store.account).map_err(err_string)?;
    login::open_login(&app, store_id, &store.account, password).map_err(err_string)
}

// ---------- 拉取（issue #6 / #10） ----------

#[tauri::command]
pub async fn pull_now(
    db: State<'_, Db>,
    clients: State<'_, Clients>,
    store_id: Option<i64>,
) -> Result<Vec<collector::PullOutcome>, String> {
    let stores: Vec<Store> = match store_id {
        Some(id) => vec![get_store(&db, id).map_err(err_string)?],
        None => db.with(repo::stores::list).map_err(err_string)?,
    };
    let mut outcomes = Vec::new();
    for store in stores {
        let backfilled = db
            .with(|conn| Ok(repo::stores::pull_state(conn, store.id)?.backfilled))
            .map_err(err_string)?;
        let days = collector::window_days_for(backfilled);
        let outcome = collector::pull_store(&db, &clients.jd, &store, days).await;
        if outcome.ok && !backfilled {
            db.with(|conn| repo::stores::mark_backfilled(conn, store.id))
                .map_err(err_string)?;
        }
        outcomes.push(outcome);
    }
    Ok(outcomes)
}

// ---------- 查询与导出（issue #7 / #9） ----------

#[tauri::command]
pub fn get_leads(db: State<'_, Db>, filter: LeadsFilter) -> Result<LeadPage, String> {
    db.with(|conn| query::leads_page(conn, &filter))
        .map_err(err_string)
}

#[tauri::command]
pub fn get_status_options(db: State<'_, Db>, store_id: Option<i64>) -> Result<Vec<String>, String> {
    db.with(|conn| query::distinct_statuses(conn, store_id))
        .map_err(err_string)
}

#[tauri::command]
pub fn get_dashboard(
    db: State<'_, Db>,
    range: Range,
    store_id: Option<i64>,
) -> Result<Dashboard, String> {
    db.with(|conn| query::dashboard(conn, &range, store_id))
        .map_err(err_string)
}

/// 导出明细到应用数据目录，返回文件路径
#[tauri::command]
pub fn export_leads(
    app: AppHandle,
    db: State<'_, Db>,
    filter: LeadsFilter,
) -> Result<String, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("解析数据目录失败: {e}"))?
        .join("exports");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建导出目录失败: {e}"))?;
    let path = dir.join(format!(
        "线索明细-{}.xlsx",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    let written = db
        .with(|conn| crate::export::export_leads(conn, &filter, &path))
        .map_err(err_string)?;
    if written == 0 {
        return Err("筛选结果为空，未生成文件".into());
    }
    Ok(path.to_string_lossy().to_string())
}

// ---------- 巨量引擎（issue #8） ----------

#[tauri::command]
pub fn get_ocean_credentials(db: State<'_, Db>) -> Result<settings::OceanCredentials, String> {
    db.with_sql(settings::get_ocean_credentials)
        .map_err(err_string)
}

#[tauri::command]
pub fn save_ocean_credentials(
    db: State<'_, Db>,
    app_id: String,
    app_secret: String,
) -> Result<(), String> {
    let app_id = app_id.trim();
    let app_secret = app_secret.trim();
    if app_id.is_empty() || app_secret.is_empty() {
        return Err("App ID 与 App Secret 不能为空".into());
    }
    db.with_sql(|conn| settings::save_ocean_credentials(conn, app_id, app_secret))
        .map_err(err_string)
}

#[derive(Serialize)]
pub struct OceanAuthOutcome {
    pub accounts: Vec<AdAccount>,
}

/// 发起 OAuth：打开系统浏览器 + 本机回环等回调（issue #8）
#[tauri::command]
pub async fn start_ocean_auth(
    app: AppHandle,
    db: State<'_, Db>,
    clients: State<'_, Clients>,
) -> Result<OceanAuthOutcome, String> {
    let (app_id_str, secret) = db
        .with(|conn| {
            let creds = settings::get_ocean_credentials(conn)?;
            Ok::<_, AppError>((
                creds
                    .app_id
                    .ok_or_else(|| AppError::Message("尚未配置 App ID".into()))?,
                creds
                    .app_secret
                    .ok_or_else(|| AppError::Message("尚未配置 App Secret".into()))?,
            ))
        })
        .map_err(err_string)?;
    let app_id: i64 = app_id_str
        .trim()
        .parse()
        .map_err(|_| "App ID 必须是数字".to_string())?;

    open_browser(&ocean::authorize_url(app_id_str.trim()));
    let auth_code = wait_auth_code().await.map_err(err_string)?;
    let tokens = clients
        .ocean
        .exchange_token(app_id, &secret, &auth_code, &ocean::redirect_uri())
        .await
        .map_err(err_string)?;
    credentials::save_ocean_tokens(&tokens.access_token, &tokens.refresh_token)
        .map_err(err_string)?;
    let expires_at = crate::db::now() + tokens.expires_in;
    db.with_sql(|conn| {
        settings::save_setting(
            conn,
            settings::KEY_OCEAN_TOKEN_EXPIRES,
            &expires_at.to_string(),
        )
    })
    .map_err(err_string)?;

    let accounts: Vec<AdAccount> = tokens
        .advertiser_ids
        .iter()
        .map(|v| AdAccount {
            id: json_value_to_string(v),
            alias: String::new(),
        })
        .collect();
    db.with(|conn| {
        for account in &accounts {
            repo::leads::ensure_ad_account(conn, &account.id, &account.alias)?;
        }
        Ok(())
    })
    .map_err(err_string)?;
    let _ = app.emit("ocean://authorized", &accounts);
    Ok(OceanAuthOutcome { accounts })
}

#[tauri::command]
pub fn list_ad_accounts(db: State<'_, Db>) -> Result<Vec<AdAccount>, String> {
    db.with(|conn| {
        Ok(repo::leads::ad_account_ids(conn)?
            .into_iter()
            .map(|id| AdAccount {
                id,
                alias: String::new(),
            })
            .collect())
    })
    .map_err(err_string)
}

/// 门店 ↔ 广告账户映射（多对多）
#[tauri::command]
pub fn set_store_mapping(
    db: State<'_, Db>,
    store_id: i64,
    ad_account_id: String,
    enable: bool,
) -> Result<(), String> {
    db.with(|conn| repo::stores::set_mapping(conn, store_id, &ad_account_id, enable))
        .map_err(err_string)
}

#[tauri::command]
pub fn get_store_ad_accounts(db: State<'_, Db>, store_id: i64) -> Result<Vec<String>, String> {
    db.with(|conn| repo::stores::mapping_account_ids(conn, store_id))
        .map_err(err_string)
}

// ---------- 对账快照（issue #12） ----------

#[derive(Serialize)]
pub struct SnapshotOutcome {
    pub store_id: i64,
    pub ok: bool,
    pub seen: usize,
    pub error: String,
}

#[tauri::command]
pub async fn run_snapshot(
    app: AppHandle,
    store_id: Option<i64>,
) -> Result<Vec<SnapshotOutcome>, String> {
    run_snapshots(&app, store_id).await.map_err(err_string)
}

/// 快照编排（调度器与手动触发共用）：提交导出 → 轮询完成 → 下载 → 合并
pub async fn run_snapshots(
    app: &AppHandle,
    store_id: Option<i64>,
) -> AppResult<Vec<SnapshotOutcome>> {
    let db = app.state::<Db>();
    let clients = app.state::<Clients>();
    let stores: Vec<Store> = match store_id {
        Some(id) => vec![get_store(&db, id)?],
        None => db.with(repo::stores::list)?,
    };
    let snap_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::Message(format!("解析数据目录失败: {e}")))?
        .join("snapshots");
    std::fs::create_dir_all(&snap_dir)?;
    let today = chrono::Local::now().date_naive().to_string();
    let now = crate::db::now();
    let mut outcomes = Vec::new();
    for store in stores {
        let outcome =
            match run_store_snapshot(&db, &clients.jd, &store, &snap_dir, &today, now).await {
                Ok(seen) => SnapshotOutcome {
                    store_id: store.id,
                    ok: true,
                    seen,
                    error: String::new(),
                },
                Err(e) => SnapshotOutcome {
                    store_id: store.id,
                    ok: false,
                    seen: 0,
                    error: e.to_string(),
                },
            };
        outcomes.push(outcome);
    }
    Ok(outcomes)
}

async fn run_store_snapshot(
    db: &Db,
    jd: &JdClient,
    store: &Store,
    dir: &std::path::Path,
    today: &str,
    now: i64,
) -> AppResult<usize> {
    let cookie = credentials::load_store_cookie(&store.account)?
        .ok_or_else(|| AppError::Message(format!("门店 {} 尚未登录", store.name)))?;
    let window = LeadsWindow::export(now - 90 * 86_400, now, false);
    let task_id = jd.create_export(&cookie, &window).await?;
    let mut task = None;
    for _ in 0..30 {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let list = jd.task_list(&cookie, 1, 10).await?;
        if let Some(found) = list.data.tasks.into_iter().find(|t| t.task_id == task_id) {
            if found.status == 2 {
                task = Some(found);
                break;
            }
        }
    }
    task.ok_or_else(|| AppError::Message("导出任务未在 60 秒内完成".into()))?;
    let bytes = jd.download_export(&cookie, &task_id).await?;
    let path = dir.join(format!("{today}-{}.xlsx", store.id));
    std::fs::write(&path, &bytes)?;
    db.with(|conn| snapshot::reconcile(conn, store.id, &path))
        .map(|s| s.seen)
}

// ---------- 快照时刻设置 ----------

#[tauri::command]
pub fn set_snapshot_hour(db: State<'_, Db>, hour: i32) -> Result<(), String> {
    if !(0..=23).contains(&hour) {
        return Err("快照时刻须在 0-23 之间".into());
    }
    db.with_sql(|conn| settings::save_setting(conn, settings::KEY_SNAPSHOT_HOUR, &hour.to_string()))
        .map_err(err_string)
}

// ---------- 自动拉取配置（issue #10 设置页控件） ----------

#[tauri::command]
pub fn get_scheduler_config(
    db: State<'_, Db>,
) -> Result<crate::scheduler::SchedulerConfig, String> {
    Ok(crate::scheduler::read_config(&db))
}

#[tauri::command]
pub fn set_scheduler_config(
    db: State<'_, Db>,
    enabled: bool,
    interval_minutes: i64,
) -> Result<(), String> {
    if !(15..=1440).contains(&interval_minutes) {
        return Err("拉取间隔须在 15-1440 分钟之间".into());
    }
    db.with_sql(|conn| {
        settings::save_setting(
            conn,
            settings::KEY_SCHEDULER_ENABLED,
            if enabled { "1" } else { "0" },
        )?;
        settings::save_setting(
            conn,
            settings::KEY_SCHEDULER_INTERVAL,
            &interval_minutes.to_string(),
        )
    })
    .map_err(err_string)
}

#[tauri::command]
pub fn get_snapshot_hour(db: State<'_, Db>) -> Result<i32, String> {
    db.with(|conn| {
        Ok(settings::get_setting(conn, settings::KEY_SNAPSHOT_HOUR)?
            .and_then(|v| v.parse().ok())
            .unwrap_or(23))
    })
    .map_err(err_string)
}

// ---------- 辅助 ----------

fn json_value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

/// 本机回环等 OAuth 回调，取回授权码（5 分钟超时）
async fn wait_auth_code() -> AppResult<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", ocean::REDIRECT_PORT))
        .await
        .map_err(|e| AppError::Message(format!("回调端口占用: {e}")))?;
    let (mut stream, _) =
        tokio::time::timeout(std::time::Duration::from_secs(300), listener.accept())
            .await
            .map_err(|_| AppError::Message("等待授权回调超时".into()))??;
    let mut buf = vec![0u8; 8192];
    let n = tokio::time::timeout(std::time::Duration::from_secs(5), stream.read(&mut buf))
        .await
        .map_err(|_| AppError::Message("读取回调请求超时".into()))??;
    let request = String::from_utf8_lossy(&buf[..n]);
    let line = request.lines().next().unwrap_or_default();
    let query = line.split_whitespace().nth(1).unwrap_or_default();
    let code = query
        .split("auth_code=")
        .nth(1)
        .unwrap_or_default()
        .split('&')
        .next()
        .unwrap_or_default();
    if code.is_empty() {
        return Err(AppError::Message("回调未携带授权码".into()));
    }
    let response =
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\r\n授权成功，可关闭此页面。";
    let _ = stream.write_all(response.as_bytes()).await;
    Ok(percent_decode(code))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}
