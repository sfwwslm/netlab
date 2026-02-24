use std::net::SocketAddr;

use engine::net_debug::{
    self, DebugConfig, DebugEvent, DebugHandle, IpVersion, Protocol, Role, SendPlan,
};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::invokes::app_log;

// Global state for network debug session.
#[derive(Default)]
pub struct DebuggerState {
    handle: Mutex<Option<DebugHandle>>,
    forward_task: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Clone, serde::Serialize)]
struct LogPayload {
    r#type: String,
    content: String,
    source: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct AutoSendArgs {
    data: Vec<u8>,
    interval_ms: u64,
    batch_size: usize,
    repeat: bool,
    max_speed: bool,
    remote_host: String,
    remote_port: u16,
}

async fn emit_log(app_handle: &AppHandle, log_type: &str, content: String, source: Option<String>) {
    let _ = app_handle.emit_to(
        "main",
        "network-log-event",
        LogPayload {
            r#type: log_type.to_string(),
            content: content.clone(),
            source: source.clone(),
        },
    );
    let level = match log_type {
        "status" => "info",
        "sent" => "info",
        "received" => "info",
        _ => "info",
    };
    app_log::record_log(app_handle, "network", level, content, source).await;
}

#[tauri::command]
pub async fn disconnect(state: State<'_, DebuggerState>) -> Result<(), String> {
    if let Some(handle) = state.handle.lock().await.take() {
        handle.stop();
    }
    if let Some(task) = state.forward_task.lock().await.take() {
        task.abort();
    }

    Ok(())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn connect_or_listen(
    app_handle: AppHandle,
    state: State<'_, DebuggerState>,
    protocol: String,
    role: String,
    ip_version: String,
    host: String,
    local_port: u16,
    remote_host: String,
    remote_port: u16,
) -> Result<(), String> {
    disconnect(state.clone()).await?;
    emit_log(&app_handle, "status", "正在初始化...".into(), None).await;

    let protocol = match protocol.as_str() {
        "tcp" => Protocol::Tcp,
        "udp" => Protocol::Udp,
        _ => return Err("无效协议".into()),
    };
    let role = match role.as_str() {
        "client" => Role::Client,
        "server" => Role::Server,
        _ => return Err("无效角色".into()),
    };
    let ip_version = match ip_version.as_str() {
        "ipv6" => IpVersion::Ipv6,
        _ => IpVersion::Ipv4,
    };

    let config = DebugConfig {
        protocol,
        role,
        ip_version,
        host,
        local_port,
        remote_host,
        remote_port,
        buffer_size: 1024,
        send_plan: None,
    };

    let session = net_debug::start(config);
    let mut events = session.events;
    let app_handle_clone = app_handle.clone();

    let forward_task = tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            match event {
                DebugEvent::Status(message) => {
                    emit_log(&app_handle_clone, "status", message, None).await;
                }
                DebugEvent::Sent(message) => {
                    emit_log(&app_handle_clone, "sent", message, None).await;
                }
                DebugEvent::Received { content, source } => {
                    emit_log(&app_handle_clone, "received", content, source).await;
                }
            }
        }
    });

    *state.handle.lock().await = Some(session.handle);
    *state.forward_task.lock().await = Some(forward_task);

    Ok(())
}

#[tauri::command]
pub async fn send_data(
    state: State<'_, DebuggerState>,
    data: Vec<u8>,
    remote_host: String,
    remote_port: u16,
) -> Result<(), String> {
    let sender = {
        let guard = state.handle.lock().await;
        guard.as_ref().map(|handle| handle.sender())
    };

    if let Some(sender) = sender {
        let remote_addr: Option<SocketAddr> =
            format!("{}:{}", remote_host, remote_port).parse().ok();
        sender
            .send(net_debug::SendRequest {
                data,
                target: remote_addr,
            })
            .await
            .map_err(|e| format!("发送数据失败: {e}"))?;
        Ok(())
    } else {
        Err("当前无有效连接".into())
    }
}

#[tauri::command]
pub async fn start_auto_send(
    state: State<'_, DebuggerState>,
    args: AutoSendArgs,
) -> Result<(), String> {
    let target = if args.remote_host.is_empty() || args.remote_port == 0 {
        None
    } else {
        format!("{}:{}", args.remote_host, args.remote_port)
            .parse()
            .ok()
    };

    let plan = SendPlan {
        payloads: vec![args.data],
        interval: std::time::Duration::from_millis(args.interval_ms.max(1)),
        batch_size: args.batch_size.max(1),
        repeat: args.repeat,
        max_speed: args.max_speed,
    };

    let mut guard = state.handle.lock().await;
    if let Some(handle) = guard.as_mut() {
        handle.start_auto_send(plan, target)
    } else {
        Err("当前无有效连接".into())
    }
}

#[tauri::command]
pub async fn stop_auto_send(state: State<'_, DebuggerState>) -> Result<(), String> {
    let mut guard = state.handle.lock().await;
    if let Some(handle) = guard.as_mut() {
        handle.stop_auto_send();
        Ok(())
    } else {
        Err("当前无有效连接".into())
    }
}
