use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use engine::conn::ConnectionMode;
use engine::scenario::load_test::LoadTestEvent;
use engine::scenario::load_test::MetricsMode;
use engine::scenario::load_test::ResponseMode;
use engine::scheduler::RpsMode;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::{sync::Mutex, sync::mpsc, task::JoinHandle};

use crate::invokes::app_log;

// 注意：本文件属于 Tauri 壳层（adapter）。
// 仅做三件事：
// 1) 将前端配置转换为 engine 配置（不实现任何调度/指标逻辑）
// 2) 转发 engine 事件到前端（`loadtest:metrics` / `loadtest:log`）
// 3) 在测试结束时写入 SQLite 历史记录
//
// 任何“压测核心逻辑”都应放在 `crates/engine` 中，遵循 `ARCHITECTURE_V1.md` / `CONTRIBUTING.md` 的边界要求。

pub struct LoadTestState {
    task_handle: Mutex<Option<JoinHandle<()>>>,
    stop_flag: Mutex<Option<Arc<AtomicBool>>>,
    db_path: PathBuf,
}

impl LoadTestState {
    pub fn new(db_path: PathBuf) -> Self {
        Self {
            task_handle: Mutex::new(None),
            stop_flag: Mutex::new(None),
            db_path,
        }
    }

    pub fn db_path(&self) -> PathBuf {
        self.db_path.clone()
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LoadTestConfig {
    /// 目标 URL（http/https）。
    pub url: String,
    /// HTTP method（前端传入字符串，例如 "GET"/"POST"）。
    pub method: String,
    /// 并发 worker 数。
    pub concurrency: usize,
    /// 并发 ramp-up 时长（秒）。
    pub ramp_up_secs: Option<u64>,
    /// 每个 worker 最大请求次数（可选）。
    pub iterations_per_worker: Option<u64>,
    /// 全局总请求数上限（可选）。
    pub total_requests_limit: Option<u64>,
    /// 持续时间（秒）。
    pub duration_secs: u64,
    /// 超时（毫秒）。
    pub timeout_ms: u64,
    /// 请求体（POST 可选）。
    pub payload: Option<String>,
    /// 额外 header（可选）。
    pub headers: Option<HashMap<String, String>>,
    /// 响应处理模式（可选，默认 countBytes）。
    pub response_mode: Option<String>,
    /// 指标计算模式（可选，默认 full）。
    pub metrics_mode: Option<String>,
    /// 连接模式（可选，默认 keepAlive）。
    pub connection_mode: Option<String>,
    /// 限速 RPS（可选，空表示不限速）。
    pub rps: Option<f64>,
    /// RPS 调度模式（可选，默认 global）。
    pub rps_mode: Option<String>,
    /// 允许不安全证书（可选，默认 false；仅对 https 有效）。
    pub allow_insecure_certs: Option<bool>,
}

#[derive(Serialize, Clone)]
pub struct CompletionBucket {
    percentile: u64,
    latency_ms: f64,
}

#[derive(Serialize, Clone)]
pub struct StatusCodeStat {
    code: u16,
    count: u64,
}

#[derive(Serialize, Clone)]
pub struct MetricsSnapshot {
    timestamp_ms: u128,
    progress: f64,
    total_requests: u64,
    success: u64,
    failures: u64,
    avg_latency_ms: f64,
    p50_ms: f64,
    p90_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
    rps: f64,
    throughput_bps: f64,
    throughput_bps_up: f64,
    total_bytes: u64,
    total_bytes_up: u64,
    avg_bytes_per_request: f64,
    avg_bytes_per_request_up: f64,
    status_codes: Vec<StatusCodeStat>,
    status_no_response: u64,
    status_other: u64,
    completion_buckets: Vec<CompletionBucket>,
    done: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRecord {
    pub id: i64,
    pub timestamp: u128,
    pub url: String,
    pub method: String,
    pub concurrency: u64,
    pub ramp_up_secs: u64,
    pub iterations_per_worker: Option<u64>,
    pub total_requests_limit: Option<u64>,
    pub duration: u64,
    pub timeout: u64,
    pub connection_mode: String,
    pub rps_limit: Option<f64>,
    pub rps_mode: String,
    pub headers: Option<HashMap<String, String>>,
    pub summary: HistorySummary,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HistorySummary {
    pub total_requests: u64,
    pub success_rate: f64,
    pub avg_latency: f64,
    pub rps: f64,
    pub p50: f64,
    pub p90: f64,
    pub p95: f64,
    pub p99: f64,
}

#[derive(Serialize, Clone, Debug)]
pub struct HistoryPage {
    pub total: u64,
    pub items: Vec<HistoryRecord>,
}

const CREATE_HISTORY_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS load_test_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL,
    url TEXT NOT NULL,
    method TEXT NOT NULL,
    concurrency INTEGER NOT NULL,
    ramp_up INTEGER NOT NULL DEFAULT 0,
    iterations_per_worker INTEGER,
    total_requests_limit INTEGER,
    duration INTEGER NOT NULL,
    timeout INTEGER NOT NULL,
    connection_mode TEXT NOT NULL DEFAULT 'keepAlive',
    rps_limit REAL,
    rps_mode TEXT NOT NULL DEFAULT 'global',
    headers_json TEXT,
    total_requests INTEGER NOT NULL,
    success_rate REAL NOT NULL,
    avg_latency REAL NOT NULL,
    rps REAL NOT NULL,
    p50 REAL NOT NULL,
    p90 REAL NOT NULL,
    p95 REAL NOT NULL,
    p99 REAL NOT NULL
);"#;

fn open_db(db_path: &PathBuf) -> Result<Connection, String> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建数据库目录失败: {e}"))?;
    }
    let conn = Connection::open(db_path).map_err(|e| format!("打开数据库失败: {e}"))?;
    conn.execute(CREATE_HISTORY_TABLE, [])
        .map_err(|e| format!("创建历史表失败: {e}"))?;
    ensure_history_columns(&conn)?;
    Ok(conn)
}

fn ensure_history_columns(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(load_test_history)")
        .map_err(|e| format!("检查历史表结构失败: {e}"))?;
    let mut rows = stmt
        .query([])
        .map_err(|e| format!("读取历史表结构失败: {e}"))?;
    let mut has_connection_mode = false;
    let mut has_p50 = false;
    let mut has_p90 = false;
    let mut has_rps_limit = false;
    let mut has_rps_mode = false;
    let mut has_ramp_up = false;
    let mut has_iterations_per_worker = false;
    let mut has_total_requests_limit = false;
    while let Some(row) = rows
        .next()
        .map_err(|e| format!("扫描历史表结构失败: {e}"))?
    {
        let name: String = row.get(1).map_err(|e| format!("读取列名失败: {e}"))?;
        match name.as_str() {
            "connection_mode" => has_connection_mode = true,
            "rps_limit" => has_rps_limit = true,
            "rps_mode" => has_rps_mode = true,
            "p50" => has_p50 = true,
            "p90" => has_p90 = true,
            "ramp_up" => has_ramp_up = true,
            "iterations_per_worker" => has_iterations_per_worker = true,
            "total_requests_limit" => has_total_requests_limit = true,
            _ => {}
        }
    }
    if !has_ramp_up {
        conn.execute(
            "ALTER TABLE load_test_history ADD COLUMN ramp_up INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| format!("添加 ramp_up 列失败: {e}"))?;
    }
    if !has_iterations_per_worker {
        conn.execute(
            "ALTER TABLE load_test_history ADD COLUMN iterations_per_worker INTEGER",
            [],
        )
        .map_err(|e| format!("添加 iterations_per_worker 列失败: {e}"))?;
    }
    if !has_total_requests_limit {
        conn.execute(
            "ALTER TABLE load_test_history ADD COLUMN total_requests_limit INTEGER",
            [],
        )
        .map_err(|e| format!("添加 total_requests_limit 列失败: {e}"))?;
    }
    if !has_connection_mode {
        conn.execute(
            "ALTER TABLE load_test_history ADD COLUMN connection_mode TEXT NOT NULL DEFAULT 'keepAlive'",
            [],
        )
        .map_err(|e| format!("添加 connection_mode 列失败: {e}"))?;
    }
    if !has_rps_limit {
        conn.execute(
            "ALTER TABLE load_test_history ADD COLUMN rps_limit REAL",
            [],
        )
        .map_err(|e| format!("添加 rps_limit 列失败: {e}"))?;
    }
    if !has_rps_mode {
        conn.execute(
            "ALTER TABLE load_test_history ADD COLUMN rps_mode TEXT NOT NULL DEFAULT 'global'",
            [],
        )
        .map_err(|e| format!("添加 rps_mode 列失败: {e}"))?;
    }
    if !has_p50 {
        conn.execute(
            "ALTER TABLE load_test_history ADD COLUMN p50 REAL NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| format!("添加 p50 列失败: {e}"))?;
    }
    if !has_p90 {
        conn.execute(
            "ALTER TABLE load_test_history ADD COLUMN p90 REAL NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| format!("添加 p90 列失败: {e}"))?;
    }
    Ok(())
}

pub fn init_history_store(db_path: &PathBuf) -> Result<(), String> {
    // 初始化 DB（建表），在 app 启动时调用。
    let _ = open_db(db_path)?;
    Ok(())
}

fn build_history_record(config: &LoadTestConfig, snapshot: &MetricsSnapshot) -> HistoryRecord {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_else(|_| 0);

    let success_rate = if snapshot.total_requests == 0 {
        0.0
    } else {
        (snapshot.success as f64 / snapshot.total_requests as f64) * 100.0
    };

    HistoryRecord {
        id: 0,
        timestamp,
        url: config.url.clone(),
        method: config.method.clone(),
        concurrency: config.concurrency as u64,
        ramp_up_secs: config.ramp_up_secs.unwrap_or(0),
        iterations_per_worker: config.iterations_per_worker,
        total_requests_limit: config.total_requests_limit,
        duration: config.duration_secs,
        timeout: config.timeout_ms / 1000,
        connection_mode: map_connection_mode(config.connection_mode.as_deref())
            .as_str()
            .to_string(),
        rps_limit: config.rps,
        rps_mode: map_rps_mode(config.rps_mode.as_deref())
            .as_str()
            .to_string(),
        headers: config.headers.clone(),
        summary: HistorySummary {
            total_requests: snapshot.total_requests,
            success_rate,
            avg_latency: snapshot.avg_latency_ms,
            rps: snapshot.rps,
            p50: snapshot.p50_ms,
            p90: snapshot.p90_ms,
            p95: snapshot.p95_ms,
            p99: snapshot.p99_ms,
        },
    }
}

async fn save_history(db_path: PathBuf, record: HistoryRecord) -> Result<(), String> {
    // SQLite 属于 blocking I/O，必须放到 spawn_blocking 避免阻塞 tokio runtime。
    tokio::task::spawn_blocking(move || {
        let conn = open_db(&db_path)?;
        let headers_json = record
            .headers
            .as_ref()
            .map(|h| serde_json::to_string(h).unwrap_or_default());
        conn.execute(
            "INSERT INTO load_test_history (
                timestamp, url, method, concurrency, ramp_up, iterations_per_worker, total_requests_limit,
                duration, timeout, connection_mode, rps_limit, rps_mode, headers_json,
                total_requests, success_rate, avg_latency, rps, p50, p90, p95, p99
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
            params![
                record.timestamp as i64,
                record.url,
                record.method,
                record.concurrency as i64,
                record.ramp_up_secs as i64,
                record.iterations_per_worker.map(|v| v as i64),
                record.total_requests_limit.map(|v| v as i64),
                record.duration as i64,
                record.timeout as i64,
                record.connection_mode,
                record.rps_limit,
                record.rps_mode,
                headers_json,
                record.summary.total_requests as i64,
                record.summary.success_rate,
                record.summary.avg_latency,
                record.summary.rps,
                record.summary.p50,
                record.summary.p90,
                record.summary.p95,
                record.summary.p99,
            ],
        )
        .map_err(|e| format!("写入历史记录失败: {e}"))?;
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| format!("写入历史记录线程错误: {e}"))?
}

async fn query_history(db_path: PathBuf, page: u32, page_size: u32) -> Result<HistoryPage, String> {
    // 读取历史也放到 spawn_blocking，避免阻塞。
    tokio::task::spawn_blocking(move || {
        let conn = open_db(&db_path)?;
        let mut count_stmt = conn
            .prepare("SELECT COUNT(*) FROM load_test_history")
            .map_err(|e| format!("查询历史条数失败: {e}"))?;
        let total: i64 = count_stmt
            .query_row([], |row| row.get(0))
            .map_err(|e| format!("读取历史条数失败: {e}"))?;

        let limit = page_size.max(1) as i64;
        let offset = (page.max(1) as i64 - 1) * limit;
        let mut stmt = conn
            .prepare(
                "SELECT id, timestamp, url, method, concurrency, ramp_up, iterations_per_worker, total_requests_limit,
                    duration, timeout, connection_mode, rps_limit, rps_mode, headers_json,
                    total_requests, success_rate, avg_latency, rps, p50, p90, p95, p99
                FROM load_test_history
                ORDER BY timestamp DESC
                LIMIT ?1 OFFSET ?2",
            )
            .map_err(|e| format!("查询历史记录失败: {e}"))?;

        let rows = stmt
            .query_map(params![limit, offset], |row| {
                let headers_json: Option<String> = row.get(13)?;
                let headers = headers_json.and_then(|v| serde_json::from_str(&v).ok());
                Ok(HistoryRecord {
                    id: row.get(0)?,
                    timestamp: row.get::<_, i64>(1)? as u128,
                    url: row.get(2)?,
                    method: row.get(3)?,
                    concurrency: row.get::<_, i64>(4)? as u64,
                    ramp_up_secs: row.get::<_, i64>(5)? as u64,
                    iterations_per_worker: row.get::<_, Option<i64>>(6)?.map(|v| v as u64),
                    total_requests_limit: row.get::<_, Option<i64>>(7)?.map(|v| v as u64),
                    duration: row.get::<_, i64>(8)? as u64,
                    timeout: row.get::<_, i64>(9)? as u64,
                    connection_mode: row.get(10)?,
                    rps_limit: row.get(11)?,
                    rps_mode: row.get(12)?,
                    headers,
                    summary: HistorySummary {
                        total_requests: row.get::<_, i64>(14)? as u64,
                        success_rate: row.get(15)?,
                        avg_latency: row.get(16)?,
                        rps: row.get(17)?,
                        p50: row.get(18)?,
                        p90: row.get(19)?,
                        p95: row.get(20)?,
                        p99: row.get(21)?,
                    },
                })
            })
            .map_err(|e| format!("映射历史记录失败: {e}"))?;

        let mut items = Vec::new();
        for item in rows {
            items.push(item.map_err(|e| format!("读取历史记录失败: {e}"))?);
        }

        Ok::<HistoryPage, String>(HistoryPage {
            total: total as u64,
            items,
        })
    })
    .await
    .map_err(|e| format!("查询历史记录线程错误: {e}"))?
}

async fn query_all_history(db_path: PathBuf) -> Result<Vec<HistoryRecord>, String> {
    // 导出全量历史记录。
    tokio::task::spawn_blocking(move || {
        let conn = open_db(&db_path)?;
        let mut stmt = conn
            .prepare(
                "SELECT id, timestamp, url, method, concurrency, ramp_up, iterations_per_worker, total_requests_limit,
                    duration, timeout, connection_mode, rps_limit, rps_mode, headers_json,
                    total_requests, success_rate, avg_latency, rps, p50, p90, p95, p99
                FROM load_test_history
                ORDER BY timestamp DESC",
            )
            .map_err(|e| format!("查询历史记录失败: {e}"))?;

        let rows = stmt
            .query_map([], |row| {
                let headers_json: Option<String> = row.get(13)?;
                let headers = headers_json.and_then(|v| serde_json::from_str(&v).ok());
                Ok(HistoryRecord {
                    id: row.get(0)?,
                    timestamp: row.get::<_, i64>(1)? as u128,
                    url: row.get(2)?,
                    method: row.get(3)?,
                    concurrency: row.get::<_, i64>(4)? as u64,
                    ramp_up_secs: row.get::<_, i64>(5)? as u64,
                    iterations_per_worker: row.get::<_, Option<i64>>(6)?.map(|v| v as u64),
                    total_requests_limit: row.get::<_, Option<i64>>(7)?.map(|v| v as u64),
                    duration: row.get::<_, i64>(8)? as u64,
                    timeout: row.get::<_, i64>(9)? as u64,
                    connection_mode: row.get(10)?,
                    rps_limit: row.get(11)?,
                    rps_mode: row.get(12)?,
                    headers,
                    summary: HistorySummary {
                        total_requests: row.get::<_, i64>(14)? as u64,
                        success_rate: row.get(15)?,
                        avg_latency: row.get(16)?,
                        rps: row.get(17)?,
                        p50: row.get(18)?,
                        p90: row.get(19)?,
                        p95: row.get(20)?,
                        p99: row.get(21)?,
                    },
                })
            })
            .map_err(|e| format!("映射历史记录失败: {e}"))?;

        let mut items = Vec::new();
        for item in rows {
            items.push(item.map_err(|e| format!("读取历史记录失败: {e}"))?);
        }

        Ok::<Vec<HistoryRecord>, String>(items)
    })
    .await
    .map_err(|e| format!("查询全部历史线程错误: {e}"))?
}

async fn clear_history(db_path: PathBuf) -> Result<(), String> {
    // 清空历史记录。
    tokio::task::spawn_blocking(move || {
        let conn = open_db(&db_path)?;
        conn.execute("DELETE FROM load_test_history", [])
            .map_err(|e| format!("清空历史记录失败: {e}"))?;
        Ok::<(), String>(())
    })
    .await
    .map_err(|e| format!("清空历史记录线程错误: {e}"))?
}

fn map_connection_mode(mode: Option<&str>) -> ConnectionMode {
    match mode {
        Some("newConnection") => ConnectionMode::NewConnection,
        Some("keepAlive") | None => ConnectionMode::KeepAlive,
        Some(other) => {
            let _ = other;
            ConnectionMode::KeepAlive
        }
    }
}

fn map_rps_mode(mode: Option<&str>) -> RpsMode {
    match mode {
        Some("perWorker") => RpsMode::PerWorker,
        Some("global") | None => RpsMode::Global,
        Some(other) => {
            let _ = other;
            RpsMode::Global
        }
    }
}

fn to_engine_config(config: &LoadTestConfig) -> engine::scenario::load_test::LoadTestConfig {
    // 壳层只做参数转换：字符串/单位转换 + mode 映射。
    let mut engine_config = engine::scenario::load_test::LoadTestConfig::new(
        config.url.clone(),
        config.method.clone(),
        config.concurrency,
        Duration::from_secs(config.duration_secs.max(1)),
        Duration::from_millis(config.timeout_ms.max(100)),
    );

    engine_config.body = config.payload.clone().map(|v| v.into_bytes());
    engine_config.headers = config
        .headers
        .clone()
        .unwrap_or_default()
        .into_iter()
        .collect();
    engine_config.response_mode = match config.response_mode.as_deref() {
        Some("discardBody") => ResponseMode::DiscardBody,
        Some("countBytes") | None => ResponseMode::CountBytes,
        Some(other) => {
            let _ = other;
            ResponseMode::CountBytes
        }
    };
    engine_config.metrics_mode = match config.metrics_mode.as_deref() {
        Some("minimal") => MetricsMode::Minimal,
        Some("full") | None => MetricsMode::Full,
        Some(other) => {
            let _ = other;
            MetricsMode::Full
        }
    };
    engine_config.connection_mode = map_connection_mode(config.connection_mode.as_deref());
    engine_config.rps_limit = config.rps;
    engine_config.rps_mode = map_rps_mode(config.rps_mode.as_deref());
    engine_config.allow_insecure_certs = config.allow_insecure_certs.unwrap_or(false);
    engine_config.ramp_up = Duration::from_secs(config.ramp_up_secs.unwrap_or(0));
    engine_config.iterations_per_worker = config.iterations_per_worker;
    engine_config.total_requests_limit = config.total_requests_limit;

    engine_config
}

fn to_metrics_snapshot(snapshot: &engine::metrics::MetricsSnapshot) -> MetricsSnapshot {
    MetricsSnapshot {
        timestamp_ms: snapshot.elapsed.as_millis(),
        progress: snapshot.progress,
        total_requests: snapshot.total_requests,
        success: snapshot.success,
        failures: snapshot.failures,
        avg_latency_ms: snapshot.avg_latency.as_secs_f64() * 1000.0,
        p50_ms: snapshot.p50.as_secs_f64() * 1000.0,
        p90_ms: snapshot.p90.as_secs_f64() * 1000.0,
        p95_ms: snapshot.p95.as_secs_f64() * 1000.0,
        p99_ms: snapshot.p99.as_secs_f64() * 1000.0,
        rps: snapshot.rps,
        throughput_bps: snapshot.throughput_bps,
        throughput_bps_up: snapshot.throughput_bps_up,
        total_bytes: snapshot.total_bytes,
        total_bytes_up: snapshot.total_bytes_up,
        avg_bytes_per_request: snapshot.avg_bytes_per_request,
        avg_bytes_per_request_up: snapshot.avg_bytes_per_request_up,
        status_codes: snapshot
            .status_codes
            .iter()
            .map(|s| StatusCodeStat {
                code: s.code,
                count: s.count,
            })
            .collect(),
        status_no_response: snapshot.status_no_response,
        status_other: snapshot.status_other,
        completion_buckets: snapshot
            .completion_buckets
            .iter()
            .map(|b| CompletionBucket {
                percentile: b.percentile,
                latency_ms: b.latency.as_secs_f64() * 1000.0,
            })
            .collect(),
        done: snapshot.done,
    }
}

async fn forward_events(
    app: AppHandle,
    mut events: mpsc::UnboundedReceiver<LoadTestEvent>,
    config_for_history: LoadTestConfig,
    db_path: PathBuf,
) {
    // 将 engine 的事件流转发到前端，同时在 done 时写入历史记录。
    while let Some(event) = events.recv().await {
        match event {
            LoadTestEvent::Metrics(snapshot) => {
                let snapshot_for_ui = to_metrics_snapshot(&snapshot);
                let _ = app.emit_to("main", "loadtest:metrics", snapshot_for_ui.clone());

                if snapshot_for_ui.done {
                    let history_record =
                        build_history_record(&config_for_history, &snapshot_for_ui);
                    if let Err(err) = save_history(db_path, history_record).await {
                        let message = format!("保存测试历史失败: {err}");
                        let _ = app.emit_to("main", "loadtest:log", message.clone());
                        app_log::record_log(&app, "loadtest", "error", message, None).await;
                    }
                    break;
                }
            }
            LoadTestEvent::Log(msg) => {
                let _ = app.emit_to("main", "loadtest:log", msg.clone());
                app_log::record_log(&app, "loadtest", "error", msg, None).await;
            }
        }
    }
}

#[tauri::command]
pub async fn start_load_test(
    app: AppHandle,
    state: State<'_, LoadTestState>,
    config: LoadTestConfig,
) -> Result<(), String> {
    // 停掉已在运行的任务
    stop_load_test(state.clone()).await?;

    let engine_config = to_engine_config(&config);
    let run = engine::scenario::load_test::spawn(engine_config)?;
    let engine::scenario::load_test::LoadTestRun {
        task,
        stop_flag,
        events,
    } = run;

    {
        let mut guard = state.stop_flag.lock().await;
        *guard = Some(stop_flag.clone());
    }
    let db_path = state.db_path();
    *state.task_handle.lock().await = Some(task);

    tokio::spawn(forward_events(app, events, config.clone(), db_path));
    Ok(())
}

#[tauri::command]
pub async fn list_load_test_history(
    state: State<'_, LoadTestState>,
    page: u32,
    page_size: u32,
) -> Result<HistoryPage, String> {
    query_history(state.db_path(), page, page_size).await
}

#[tauri::command]
pub async fn export_load_test_history(
    state: State<'_, LoadTestState>,
) -> Result<Vec<HistoryRecord>, String> {
    query_all_history(state.db_path()).await
}

#[tauri::command]
pub async fn clear_load_test_history(state: State<'_, LoadTestState>) -> Result<(), String> {
    clear_history(state.db_path()).await
}

#[tauri::command]
pub async fn stop_load_test(state: State<'_, LoadTestState>) -> Result<(), String> {
    if let Some(flag) = state.stop_flag.lock().await.take() {
        flag.store(true, Ordering::Relaxed);
    }
    if let Some(handle) = state.task_handle.lock().await.take() {
        handle.abort();
    }
    Ok(())
}
