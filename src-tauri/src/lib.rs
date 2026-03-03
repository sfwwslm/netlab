pub mod init;
pub mod invokes;
pub mod modules;
pub mod types;
pub mod utils;

use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 默认情况下，当应用程序已经在运行时启动新实例时，不会采取任何操作。当用户尝试打开一个新实例时，为了聚焦正在运行实例的窗口，修改回调闭包如下。
            let windows = app.webview_windows();
            windows
                .values()
                .next()
                .expect("Sorry, no window found")
                .set_focus()
                .expect("Can't Bring Window to Focus");
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            init::setup(app);
            init::manage(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            invokes::app_log::list_app_logs,
            invokes::app_log::clear_app_logs,
            invokes::network_debug::connect_or_listen,
            invokes::network_debug::send_data,
            invokes::network_debug::start_auto_send,
            invokes::network_debug::stop_auto_send,
            invokes::network_debug::disconnect,
            invokes::proxy::start_proxy,
            invokes::proxy::stop_proxy,
            invokes::socks5::start_socks5,
            invokes::socks5::stop_socks5,
            invokes::load_test::start_load_test,
            invokes::load_test::stop_load_test,
            invokes::load_test::list_load_test_history,
            invokes::load_test::export_load_test_history,
            invokes::load_test::clear_load_test_history,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
