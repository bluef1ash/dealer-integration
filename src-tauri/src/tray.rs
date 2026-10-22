//! 系统托盘：托盘左键/菜单「显示主窗口」唤起，菜单「退出」真正退出。
//! 关闭按钮是否隐藏到托盘由设置页「系统设置」控制（默认开）。

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};

/// 主窗口标签（tauri.conf.json 未显式命名时的默认值）
const MAIN: &str = "main";

/// 建托盘：图标用应用默认窗口图标，tooltip 与菜单均为中文
pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let icon = app
        .default_window_icon()
        .cloned()
        .expect("应用图标缺失：bundle.icon 未生成默认窗口图标");
    TrayIconBuilder::with_id("main-tray")
        .icon(icon)
        .tooltip("经销商整合")
        .menu(&menu)
        // 左键单击直接唤起窗口；菜单只挂右键（macOS 默认左键开菜单，此处统一关闭）
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Windows 单击 / macOS 常见双击均视为唤起
            let clicked = matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            );
            if clicked {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// 唤起主窗口（隐藏态先显示，再置前）
fn show_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(MAIN) {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

/// 主窗口关闭请求按设置三态分流：
/// - Some(true)：隐藏到托盘（采集调度继续）
/// - None（首次，未记录设置）：拦截并请前端弹窗询问，选择后记住
/// - Some(false)：放行关闭，应用退出
///
/// 其余窗口（登录/OAuth 弹窗）保持原生关闭语义
pub fn on_window_event(app: &AppHandle, label: &str, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        if label != MAIN {
            return;
        }
        let choice = read_choice(app);
        match choice {
            Some(true) => {
                api.prevent_close();
                if let Some(win) = app.get_webview_window(MAIN) {
                    let _ = win.hide();
                }
            }
            None => {
                api.prevent_close();
                let _ = app.emit("tray://confirm-close", ());
            }
            Some(false) => {}
        }
    }
}

/// 读「关闭时隐藏到托盘」三态设置；读取失败按未设置处理（再问一次，避免误退）
fn read_choice(app: &AppHandle) -> Option<bool> {
    let db = app.state::<crate::db::Db>();
    db.with(crate::commands::close_to_tray_choice)
        .unwrap_or(None)
}

#[cfg(test)]
mod tests {
    /// tray 建在 Tauri 应用实例上，无法单测；此处仅钉住主窗口标签口径，
    /// 若 tauri.conf.json 显式命名了窗口需同步更新
    #[test]
    fn main_window_label_is_default() {
        assert_eq!(super::MAIN, "main");
    }
}
