//! WebView 登录与会话捕获（issue #5/#11）：
//! 内嵌窗口打开经营后台，未登录会被带到登录页，真人完成账号密码或短信验证码登录；
//! 后台轮询 WebView cookie，捕获到 CRM 会话（motor_dealer_session_id）即存凭证库并关窗。
//! 携带密码时尝试自动回填（滑块等风控环节仍需真人完成，见 ADR-0003）。

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::credentials;
use crate::error::{AppError, AppResult};

pub const ORIGIN: &str = "https://www.autoengine.com";
/// 打开经营后台；未登录时平台自动跳转登录页
pub const START_URL: &str = "https://www.autoengine.com/jdc/one_saas/leads-manage";
/// CRM 层会话 cookie 名（spike 实测：仅 passport sessionid 不够）
const CRM_COOKIE: &str = "motor_dealer_session_id";
const POLL_INTERVAL_MS: u64 = 2000;
const MAX_POLLS: u32 = 300; // 10 分钟

#[derive(Serialize, Clone)]
pub struct LoginEvent {
    pub store_id: i64,
    pub ok: bool,
    pub message: String,
}

/// 打开登录窗口并启动捕获任务；重复调用先关旧窗
pub fn open_login(app: &AppHandle, store_id: i64, account: &str, password: Option<String>) -> AppResult<()> {
    let label = format!("login-{store_id}");
    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.close();
    }
    let url: tauri::Url = START_URL.parse().map_err(|e| AppError::Message(format!("URL 解析失败: {e}")))?;
    WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url))
        .title(format!("巨懂车登录 - {}", account))
        .inner_size(1100.0, 800.0)
        .build()?;
    let app = app.clone();
    let account = account.to_string();
    tauri::async_runtime::spawn(async move {
        capture_session(&app, store_id, label, &account, password).await;
    });
    Ok(())
}

async fn capture_session(app: &AppHandle, store_id: i64, label: String, account: &str, password: Option<String>) {
    let mut autofilled = false;
    for attempt in 0..MAX_POLLS {
        tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
        let Some(window) = app.get_webview_window(&label) else {
            // 用户手动关窗：视为放弃
            let _ = app.emit("login://result", LoginEvent {
                store_id,
                ok: false,
                message: "登录窗口被关闭".into(),
            });
            return;
        };
        // 密码自动回填：等页面渲染后注入一次（滑块/短信仍需真人）
        if let Some(password) = password.clone() {
            if !autofilled && attempt == 2 {
                autofilled = true;
                let account = account.to_string();
                let _ = window.eval(&autofill_script(&account, &password));
            }
        }
        let cookies = window.cookies_for_url(ORIGIN.parse().expect("常量 URL 有效"));
        if let Ok(cookies) = cookies {
            let jar = cookies
                .iter()
                .map(|c| format!("{}={}", c.name(), c.value()))
                .collect::<Vec<_>>()
                .join("; ");
            if jar.contains(&format!("{CRM_COOKIE}=")) {
                if let Err(e) = credentials::save_store_cookie(account, &jar) {
                    let _ = app.emit("login://result", LoginEvent { store_id, ok: false, message: e.to_string() });
                    return;
                }
                let _ = window.close();
                let _ = app.emit("login://result", LoginEvent {
                    store_id,
                    ok: true,
                    message: "会话已捕获".into(),
                });
                return;
            }
        }
    }
    let _ = app.emit("login://result", LoginEvent {
        store_id,
        ok: false,
        message: "登录超时：未捕获到会话，请重试".into(),
    });
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.close();
    }
}

/// 自动回填脚本：命中第一个可输入账号的文本框与密码框后提交。
/// 平台页面结构可能变化，失败时人工在窗口内登录即可（尽力而为）。
fn autofill_script(account: &str, password: &str) -> String {
    let account = js_escape(account);
    let password = js_escape(password);
    format!(
        r#"(function() {{
            const text = document.querySelector('input[type=tel], input[type=text], input[name*=user], input[placeholder*=账号]');
            const pass = document.querySelector('input[type=password]');
            if (text && pass) {{
                const set = (el, v) => {{
                    const setter = Object.getOwnPropertyDescriptor(el.__proto__, 'value').set;
                    setter.call(el, v);
                    el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                }};
                set(text, '{account}');
                set(pass, '{password}');
                setTimeout(() => {{
                    const btn = document.querySelector('button[type=submit], .submit-btn, button');
                    if (btn) btn.click();
                }}, 300);
            }}
        }})();"#
    )
}

fn js_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'").replace('"', "\\\"")
}
