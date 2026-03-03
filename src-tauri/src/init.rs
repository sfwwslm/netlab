use crate::invokes::app_log::AppLogState;
use crate::invokes::load_test::{self, LoadTestState};
use crate::utils::{HttpClientConfig, app_data_dir_path, build_http_client};
use crate::{
    invokes::{network_debug, proxy, socks5},
    modules::{logger, tray},
};
use log::{error, info};
use tauri::{Manager, Runtime};

pub trait CustomInit {
    fn init_plugin(self) -> Self;
}

impl<R: Runtime> CustomInit for tauri::Builder<R> {
    fn init_plugin(self) -> Self {
        self
    }
}

pub fn setup(app: &mut tauri::App) {
    if let Err(e) = tray::create(app) {
        error!("Failed to create system tray: {e}");
    }

    tray::listener(app);

    if let Err(e) = logger::init(app) {
        error!("Failed to initialize logging: {e}");
    }

    info!("【初始化】`setup` 设置完成");
}

pub fn manage(app: &mut tauri::App) {
    let client = build_http_client(HttpClientConfig::builder()).unwrap();
    let data_dir = app_data_dir_path(app);
    let db_path = data_dir.join("loadtest.db");
    if let Err(e) = load_test::init_history_store(&db_path) {
        error!("初始化负载测试历史数据库失败: {e}");
    }
    app.manage(client);
    app.manage(network_debug::DebuggerState::default());
    app.manage(LoadTestState::new(db_path));
    app.manage(proxy::ProxyState::default());
    app.manage(socks5::Socks5State::default());
    app.manage(AppLogState::default());

    info!("【初始化】`state` 设置完成")
}
