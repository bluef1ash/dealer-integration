//! 巨量引擎多应用 command：应用 CRUD、按应用 OAuth 授权、广告账户列表。

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

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
    // 留痕实际打开的链接，便于与开发者后台生成的官方授权链接比对排查
    eprintln!("[OAuth] 打开授权链接: {authorize_link}");
    open_browser(&authorize_link);
    let auth_code = wait_auth_code().await.map_err(err_string)?;
    let tokens = clients
        .ocean
        .exchange_token(
            app_id,
            ocean_app.app_secret.trim(),
            &auth_code,
            &ocean::redirect_uri(),
        )
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

    // 授权返回的账户归属该应用；同账户在其他应用再授权时改判归属
    let accounts: Vec<AdAccount> = tokens
        .advertiser_ids
        .iter()
        .map(|v| AdAccount {
            id: json_value_to_string(v),
            alias: String::new(),
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
