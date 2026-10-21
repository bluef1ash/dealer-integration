//! WebView 登录与会话捕获（issue #5/#11）：
//! 内嵌窗口打开经营后台，未登录会被带到登录页，真人完成账号密码或短信验证码登录；
//! 后台轮询 WebView cookie，捕获到 CRM 会话（motor_dealer_session_id）即存凭证库并关窗。
//! 携带密码时尝试自动回填（滑块等风控环节仍需真人完成，见 ADR-0003）。

use serde::Serialize;
use std::sync::mpsc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

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
pub fn open_login(
    app: &AppHandle,
    store_id: i64,
    account: &str,
    password: Option<String>,
) -> AppResult<()> {
    let label = format!("login-{store_id}");
    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.close();
    }
    let url: tauri::Url = START_URL
        .parse()
        .map_err(|e| AppError::Message(format!("URL 解析失败: {e}")))?;
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

async fn capture_session(
    app: &AppHandle,
    store_id: i64,
    label: String,
    account: &str,
    password: Option<String>,
) {
    let mut autofilled = false;
    let mut human_notified = false;
    for attempt in 0..MAX_POLLS {
        tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
        let Some(window) = app.get_webview_window(&label) else {
            // 用户手动关窗：视为放弃
            let _ = app.emit(
                "login://result",
                LoginEvent {
                    store_id,
                    ok: false,
                    message: "登录窗口被关闭".into(),
                },
            );
            return;
        };
        // 密码自动回填：等页面渲染后注入一次（滑块/短信仍需真人）
        if let Some(password) = password.clone() {
            if !autofilled && attempt == 2 {
                autofilled = true;
                let script = autofill_script(account, &password);
                let _ = eval_on_main(app, &window, &script);
            }
            // 回填已做但 60 秒仍无会话：大概率被滑块/风控拦下，主动提醒人工介入
            if autofilled && !human_notified && attempt == 32 {
                human_notified = true;
                let _ = app.emit(
                    "login://needs-human",
                    serde_json::json!({ "store_id": store_id, "reason": "自动回填未通过，窗口内可能有滑块验证，请人工完成" }),
                );
            }
        }
        let Some(jar) = cookies_on_main(app, &window) else {
            continue; // 主线程繁忙本轮未取到，下轮再试
        };
        if jar.contains(&format!("{CRM_COOKIE}=")) {
            if let Err(e) = credentials::save_store_cookie(account, &jar) {
                let _ = app.emit(
                    "login://result",
                    LoginEvent {
                        store_id,
                        ok: false,
                        message: e.to_string(),
                    },
                );
                return;
            }
            let _ = window.close();
            let _ = app.emit(
                "login://result",
                LoginEvent {
                    store_id,
                    ok: true,
                    message: "会话已捕获".into(),
                },
            );
            return;
        }
    }
    let _ = app.emit(
        "login://result",
        LoginEvent {
            store_id,
            ok: false,
            message: "登录超时：未捕获到会话，请重试".into(),
        },
    );
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.close();
    }
}

/// WebView2 的 COM 接口只能在主线程触碰：把调用投递到主线程执行并取回结果。
/// 返回 None = 主线程未在超时内应答（跳过本轮）。
/// 直接在工作线程调 cookies_for_url 会触发 0x80000003 断点异常（真机实证）。
fn cookies_on_main(app: &AppHandle, window: &WebviewWindow) -> Option<String> {
    let (tx, rx) = mpsc::channel();
    let w = window.clone();
    app.run_on_main_thread(move || {
        let jar = w
            .cookies_for_url(ORIGIN.parse().expect("常量 URL 有效"))
            .ok()
            .map(|cookies| {
                cookies
                    .iter()
                    .map(|c| format!("{}={}", c.name(), c.value()))
                    .collect::<Vec<_>>()
                    .join("; ")
            });
        let _ = tx.send(jar);
    })
    .ok()?;
    rx.recv_timeout(Duration::from_secs(3)).ok().flatten()
}

/// eval 同理走主线程（尽力而为）
fn eval_on_main(app: &AppHandle, window: &WebviewWindow, script: &str) -> AppResult<()> {
    let w = window.clone();
    let script = script.to_string();
    app.run_on_main_thread(move || {
        let _ = w.eval(&script);
    })?;
    Ok(())
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
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('"', "\\\"")
}
