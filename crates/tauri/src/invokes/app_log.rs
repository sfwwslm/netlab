use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

const MAX_LOG_ENTRIES: usize = 2000;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppLogEntry {
    pub id: u64,
    pub timestamp_ms: u128,
    pub scope: String,
    pub level: String,
    pub message: String,
    pub source: Option<String>,
}

#[derive(Default)]
pub struct AppLogState {
    entries: Mutex<Vec<AppLogEntry>>,
    next_id: AtomicU64,
}

impl AppLogState {
    pub async fn push(
        &self,
        scope: &str,
        level: &str,
        message: String,
        source: Option<String>,
    ) -> AppLogEntry {
        let entry = AppLogEntry {
            id: self.next_id.fetch_add(1, Ordering::Relaxed),
            timestamp_ms: now_ms(),
            scope: scope.to_string(),
            level: level.to_string(),
            message,
            source,
        };
        let mut guard = self.entries.lock().await;
        guard.push(entry.clone());
        if guard.len() > MAX_LOG_ENTRIES {
            let drain_count = guard.len() - MAX_LOG_ENTRIES;
            guard.drain(..drain_count);
        }
        entry
    }

    pub async fn list(&self) -> Vec<AppLogEntry> {
        self.entries.lock().await.clone()
    }

    pub async fn clear(&self) {
        self.entries.lock().await.clear();
    }
}

pub async fn record_log(
    app: &AppHandle,
    scope: &str,
    level: &str,
    message: String,
    source: Option<String>,
) {
    let state = app.state::<AppLogState>();
    let entry = state.push(scope, level, message, source).await;
    let _ = app.emit("app-log:entry", entry);
}

#[tauri::command]
pub async fn list_app_logs(state: State<'_, AppLogState>) -> Result<Vec<AppLogEntry>, String> {
    Ok(state.list().await)
}

#[tauri::command]
pub async fn clear_app_logs(state: State<'_, AppLogState>) -> Result<(), String> {
    state.clear().await;
    Ok(())
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_millis())
        .unwrap_or(0)
}
