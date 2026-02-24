use std::time::Duration;

use engine::proxy::{ProxyEvent, ProxyHandle};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::invokes::app_log;

#[derive(Default)]
pub struct ProxyState {
    handle: Mutex<Option<ProxyHandle>>,
    forward_task: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProxyConfigPayload {
    pub listen_host: String,
    pub listen_port: u16,
    pub report_interval_ms: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxySnapshotPayload {
    pub uptime_ms: u64,
    pub active_connections: u64,
    pub total_connections: u64,
    pub total_requests: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub clients: Vec<ClientSnapshotPayload>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientSnapshotPayload {
    pub ip: String,
    pub active_connections: u64,
    pub total_connections: u64,
    pub total_requests: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub last_seen_ms: u64,
    pub top_targets: Vec<TargetStatPayload>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetStatPayload {
    pub target: String,
    pub count: u64,
}

fn to_snapshot_payload(snapshot: &engine::proxy::ProxySnapshot) -> ProxySnapshotPayload {
    ProxySnapshotPayload {
        uptime_ms: snapshot.uptime.as_millis() as u64,
        active_connections: snapshot.active_connections,
        total_connections: snapshot.total_connections,
        total_requests: snapshot.total_requests,
        bytes_in: snapshot.bytes_in,
        bytes_out: snapshot.bytes_out,
        clients: snapshot
            .clients
            .iter()
            .map(|client| ClientSnapshotPayload {
                ip: client.ip.clone(),
                active_connections: client.active_connections,
                total_connections: client.total_connections,
                total_requests: client.total_requests,
                bytes_in: client.bytes_in,
                bytes_out: client.bytes_out,
                last_seen_ms: client.last_seen_ms,
                top_targets: client
                    .top_targets
                    .iter()
                    .map(|entry| TargetStatPayload {
                        target: entry.target.clone(),
                        count: entry.count,
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[tauri::command]
pub async fn start_proxy(
    app_handle: AppHandle,
    state: State<'_, ProxyState>,
    config: ProxyConfigPayload,
) -> Result<(), String> {
    stop_proxy(state.clone()).await?;

    let engine_config = engine::proxy::ProxyConfig {
        listen_host: config.listen_host,
        listen_port: config.listen_port,
        report_interval: Duration::from_millis(config.report_interval_ms.max(100)),
        client_idle_ttl: Duration::from_secs(300),
    };
    let session = engine::proxy::start(engine_config);
    let mut events = session.events;
    let app_handle_clone = app_handle.clone();

    let forward_task = tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            match event {
                ProxyEvent::Status(message) => {
                    let _ = app_handle_clone.emit_to("main", "proxy:status", message.clone());
                    app_log::record_log(&app_handle_clone, "proxy", "info", message, None).await;
                }
                ProxyEvent::Snapshot(snapshot) => {
                    let payload = to_snapshot_payload(&snapshot);
                    let _ = app_handle_clone.emit_to("main", "proxy:snapshot", payload);
                }
            }
        }
    });

    *state.handle.lock().await = Some(session.handle);
    *state.forward_task.lock().await = Some(forward_task);
    Ok(())
}

#[tauri::command]
pub async fn stop_proxy(state: State<'_, ProxyState>) -> Result<(), String> {
    if let Some(handle) = state.handle.lock().await.take() {
        handle.stop();
    }
    if let Some(task) = state.forward_task.lock().await.take() {
        task.abort();
    }
    Ok(())
}
