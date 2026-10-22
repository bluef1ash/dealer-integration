//! 巨量引擎多应用 command：应用 CRUD、按应用 OAuth 授权、广告账户列表。

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

use super::{err_string, Clients};
use crate::credentials;
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::models::{AdAccount, OceanAppStatus};
use crate::{ocean, repo};

#[derive(Serialize)]
pub struct OceanAuthOutcome {
    pub accounts: Vec<AdAccount>,
}

/// 应用列表（设置页）；token 有无查自 credentials 表，读失败按未授权展示但必须留痕
#[tauri::command]
pub fn list_ocean_apps(db: State<'_, Db>) -> Result<Vec<OceanAppStatus>, String> {
    db.with(|conn| -> AppResult<Vec<OceanAppStatus>> {
        let mut out = Vec::new();
        for app in repo::ocean_apps::list(conn)? {
            let has_token = match credentials::load_ocean_tokens(conn, &app.app_id) {
                Ok((access, _)) => access.is_some(),
                Err(e) => {
                    eprintln!("[状态] 巨量 token 读取失败 app_id={}: {e}", app.app_id);
                    false
                }
            };
            out.push(OceanAppStatus {
                id: app.id,
                name: app.name,
                app_id: app.app_id,
                token_expires_at: app.token_expires_at,
                has_token,
            });
        }
        Ok(out)
    })
    .map_err(err_string)
}

#[tauri::command]
pub fn add_ocean_app(
    db: State<'_, Db>,
    name: String,
    app_id: String,
    app_secret: String,
) -> Result<OceanAppStatus, String> {
    let name = name.trim();
    let app_id = app_id.trim();
    let app_secret = app_secret.trim();
    if name.is_empty() || app_id.is_empty() || app_secret.is_empty() {
        return Err("应用名称、App ID 与 App Secret 均不能为空".into());
    }
    // OAuth client_id 为数字串，录入时早失败，授权环节不再二次校验
    if app_id.parse::<i64>().is_err() {
        return Err("App ID 必须是数字".into());
    }
    let app = db
        .with(|conn| repo::ocean_apps::insert(conn, name, app_id, app_secret))
        .map_err(err_string)?;
    Ok(OceanAppStatus {
        id: app.id,
        name: app.name,
        app_id: app.app_id,
        token_expires_at: app.token_expires_at,
        has_token: false,
    })
}

#[tauri::command]
pub fn remove_ocean_app(db: State<'_, Db>, ocean_id: i64) -> Result<(), String> {
    let app = db
        .with(|conn| repo::ocean_apps::get(conn, ocean_id))
        .map_err(err_string)?;
    db.with(|conn| credentials::delete_ocean_tokens(conn, &app.app_id))
        .map_err(err_string)?;
    db.with(|conn| repo::ocean_apps::delete(conn, ocean_id))
        .map_err(err_string)
}

/// 发起某应用的 OAuth：打开系统浏览器 + 本机回环等回调（回调端点联调在手动冒烟校正）
#[tauri::command]
pub async fn start_ocean_auth(
    app: AppHandle,
    db: State<'_, Db>,
    clients: State<'_, Clients>,
    ocean_id: i64,
) -> Result<OceanAuthOutcome, String> {
    let ocean_app = db
        .with(|conn| repo::ocean_apps::get(conn, ocean_id))
        .map_err(err_string)?;
    let app_id: i64 = ocean_app
        .app_id
        .trim()
        .parse()
        .map_err(|_| "App ID 必须是数字".to_string())?;

    let authorize_link = ocean::authorize_url(ocean_app.app_id.trim());
    eprintln!("[OAuth] 打开授权弹窗: {authorize_link}");
    let auth_code = oauth_popup(&app, ocean_app.id, &authorize_link)
        .await
        .map_err(err_string)?;
    let tokens = clients
        .ocean
        .exchange_token(app_id, ocean_app.app_secret.trim(), &auth_code)
        .await
        .map_err(err_string)?;
    db.with(|conn| {
        credentials::save_ocean_tokens(
            conn,
            ocean_app.app_id.trim(),
            &tokens.access_token,
            &tokens.refresh_token,
        )
    })
    .map_err(err_string)?;
    db.with(|conn| {
        repo::ocean_apps::set_token_expires(
            conn,
            ocean_app.id,
            crate::db::now() + tokens.expires_in,
        )
    })
    .map_err(err_string)?;

    // 授权范围展开为可核算广告账户：token 返回的 id 可能是工作台等角色（报表 40002），
    // 按角色展开为真实广告账户；同 id 在其他应用再授权时改判归属
    let accounts: Vec<AdAccount> = ocean::accounts::sync_advertiser_accounts(
        &clients.ocean,
        &tokens.access_token,
        &tokens.advertiser_ids,
    )
    .await
    .into_iter()
    .map(|acc| AdAccount {
        id: acc.id,
        alias: acc.name,
        ocean_app_id: Some(ocean_app.id),
        app_name: Some(ocean_app.name.clone()),
    })
    .collect();
    db.with(|conn| {
        for account in &accounts {
            repo::leads::attach_ad_account(conn, &account.id, &account.alias, ocean_app.id)?;
        }
        Ok(())
    })
    .map_err(err_string)?;
    let _ = app.emit("ocean://authorized", &accounts);
    Ok(OceanAuthOutcome { accounts })
}

#[tauri::command]
pub fn list_ad_accounts(db: State<'_, Db>) -> Result<Vec<AdAccount>, String> {
    db.with(repo::leads::ad_accounts).map_err(err_string)
}

// ---------- 辅助 ----------

/// 应用内弹窗完成 OAuth：on_navigation 拦截回调导航（127.0.0.1:1720）原生侧提取授权码，
/// 无需系统浏览器与本机端口监听（浏览器方案的竞态与超时问题一并消除）
async fn oauth_popup(app: &AppHandle, ocean_row: i64, authorize_link: &str) -> AppResult<String> {
    let label = format!("oauth-{ocean_row}");
    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.close();
    }
    let url: tauri::Url = authorize_link
        .parse()
        .map_err(|e| AppError::Message(format!("授权链接解析失败: {e}")))?;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url))
        .title("巨量引擎授权")
        .inner_size(1024.0, 780.0)
        .on_navigation(move |nav| {
            if let Some(code) = ocean::extract_auth_code(nav.as_str()) {
                // 拦截回调导航：不放行到本机地址，直接取走授权码
                let _ = tx.send(code);
                return false;
            }
            true
        })
        .build()?;
    let received = tokio::time::timeout(std::time::Duration::from_secs(600), rx.recv()).await;
    let _ = window.close();
    match received {
        Ok(Some(code)) => Ok(code),
        Ok(None) => Err(AppError::Message("授权窗口已关闭，未完成授权".into())),
        Err(_) => Err(AppError::Message(
            "授权超时（10 分钟），请重新发起授权".into(),
        )),
    }
}
