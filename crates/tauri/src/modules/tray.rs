use std::collections::HashMap;

use tauri::{
    AppHandle, Listener, Manager, Runtime,
    menu::{IsMenuItem, Menu, MenuEvent, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
};

use log::error;
use serde::Deserialize;
use std::fs;

// 定义需要的配置项
#[derive(Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct AppConfig {
    #[serde(default = "default_bool_false")]
    start_minimized: bool,
}

fn default_bool_false() -> bool {
    false
}

fn read_config(app_handle: &AppHandle) -> AppConfig {
    let config_path = app_handle
        .path()
        .home_dir()
        .expect("failed to get app config dir")
        .join(app_handle.path().app_config_dir().unwrap())
        .join(crate::types::APP_CONFIG_FILE);

    if config_path.exists()
        && let Ok(content) = fs::read_to_string(config_path)
        && !content.trim().is_empty()
    {
        return serde_json::from_str::<AppConfig>(&content).unwrap_or_default();
    }
    AppConfig::default()
}

fn handle_exit(app_handle: &AppHandle) {
    let config = read_config(app_handle);
    if config.start_minimized {
        // 如果用户希望下次启动时最小化，则先隐藏所有窗口再退出
        let window = app_handle.get_webview_window("main").unwrap();
        if let Err(error) = window.hide() {
            log::error!("Tried and failed to hide the window before exiting: {error}");
        }
    }
    app_handle.exit(0);
}

// 定义一个类型别名，方便处理事件 payload
pub type TrayMenuItemsPayload = HashMap<String, String>;

/// 更新托盘菜单：接收一个 `HashMap`，用 ID 和标题重建菜单项
pub fn update_tray_menu_items<R: Runtime>(
    app: &AppHandle<R>,
    mut new_items: TrayMenuItemsPayload,
) -> tauri::Result<()> {
    let tray = app.tray_by_id("main_tray").unwrap();

    let title = new_items.remove("title").unwrap_or_default();

    // 直接用栈上的 MenuItem<R>
    let mut menu_items = Vec::with_capacity(new_items.len());

    for (id, text) in new_items {
        let item = MenuItem::with_id(app, id, text, true, None::<&str>)?;
        menu_items.push(item);
    }

    // 构造 &[&dyn IsMenuItem] 切片
    let item_refs: Vec<&dyn IsMenuItem<R>> =
        menu_items.iter().map(|i| i as &dyn IsMenuItem<R>).collect();

    let new_menu = Menu::with_items(app, &item_refs)?;
    tray.set_menu(Some(new_menu))?;

    #[cfg(target_os = "windows")]
    tray.set_tooltip(Some(title))?;

    #[cfg(target_os = "linux")]
    tray.set_title(Some(title))?;

    log::debug!("Update tray menu success!");
    Ok(())
}

/// 创建系统托盘
pub fn create(app: &mut tauri::App) -> tauri::Result<()> {
    let menu = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    TrayIconBuilder::with_id("main_tray")
        .tooltip("Toolkit")
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&Menu::with_items(app, &[&menu])?)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray: &TrayIcon, event: TrayIconEvent| {
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    let app = tray.app_handle();
                    if let Some(window) = app.get_webview_window("main") {
                        if window.is_minimized().expect("获取窗口最小化状态失败") {
                            window.unminimize().expect("取消最小化窗口失败");
                        }
                        window.show().expect("显示窗口失败");
                        window.set_focus().expect("聚焦窗口失败");
                    }
                }
                _ => {
                    // debug!("未处理的事件 {event:?}");
                }
            }
        })
        .on_menu_event(
            |app: &tauri::AppHandle, event: MenuEvent| match event.id.as_ref() {
                "quit" => {
                    handle_exit(app);
                    log::debug!("Click quit menu");
                    if let Some(window) = app.get_webview_window("main")
                        && let Err(err) = window.close()
                    {
                        log::warn!("Tried and failed to close the window before exiting: {err}",);
                    }
                    app.exit(0);
                }
                _ => {
                    log::debug!("menu item {:?} not handled", event.id);
                }
            },
        )
        .build(app)?;

    Ok(())
}

pub fn listener(app: &mut tauri::App) {
    // 监听前端发来的事件
    let app_handle = app.handle().clone();
    app.listen(
        "update-tray-menu",
        move |event| match serde_json::from_str::<TrayMenuItemsPayload>(event.payload()) {
            Ok(payload) => {
                if let Err(e) = update_tray_menu_items(&app_handle, payload) {
                    error!("Error updating tray menu: {e}");
                }
            }
            Err(e) => {
                error!("Failed to parse tray menu payload: {e}");
            }
        },
    );
}
