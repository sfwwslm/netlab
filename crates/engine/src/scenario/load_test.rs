use std::error::Error as StdError;
use std::pin::Pin;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use bytes::Bytes;
use http::{HeaderMap, HeaderName, HeaderValue, Uri};
use http_body_util::{BodyExt, Full};
use hyper::{Method, Request, header};
use hyper_rustls::{ConfigBuilderExt, HttpsConnectorBuilder};
use hyper_util::client::legacy::{Client, connect::HttpConnector};
use hyper_util::rt::TokioExecutor;
use tokio::{
    sync::mpsc,
    task::JoinHandle,
    time::{Instant as TokioInstant, MissedTickBehavior, interval, sleep},
};

use crate::conn::ConnectionMode;
use crate::metrics::{LoadMetrics, MetricsSnapshot};
use crate::protocol::{HttpVersionPolicy, Scheme};
use crate::scheduler::{RpsMode, ScheduleConfig, Worker};

/// 负载测试：engine 内的 HTTP 压测场景实现。
///
/// 设计目标（当前实现的取舍）：
/// - **壳层无逻辑**：Tauri/前端只做参数与事件转发，核心执行与统计在 engine 内完成（见 `ARCHITECTURE_V1.md`）。
/// - **吞吐优先**：对“本地/低延迟靶机”场景，尽量减少压测端自扰动（锁竞争、分配、重型统计）。
/// - **可视化友好**：通过事件流实时推送 `MetricsSnapshot`，由壳层转发到 UI。
///
/// 非目标：
/// - 取代 wrk/k6/vegeta 这类专业压测工具的全部能力（固定 RPS、阶段调度、复杂脚本等）。
const DEFAULT_REPORT_INTERVAL: Duration = Duration::from_millis(500);
const DEFAULT_MAX_LATENCY_SAMPLES: usize = 10_000;
const COMPLETION_PERCENTILES: [u64; 5] = [50, 90, 95, 99, 100];

/// 响应处理策略。
///
/// 注意：HTTP/1.1 keep-alive 下，如果不 drain body，连接通常无法安全复用，
/// 因而会导致吞吐下降或连接异常。因此即使是“丢弃内容”，也会把 body 读完。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponseMode {
    /// 读取响应体并统计字节数（用于 `throughput_bps`）。
    CountBytes,
    /// 读取响应体但不统计/不保留内容（更偏“压吞吐”）。
    DiscardBody,
}

impl ResponseMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CountBytes => "countBytes",
            Self::DiscardBody => "discardBody",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricsMode {
    /// 采样并计算分位（P50/P90/P95/P99/P100），指标更完整。
    Full,
    /// 不计算分位（跳过延迟样本采集与排序），尽量压榨吞吐。
    Minimal,
}

impl MetricsMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Minimal => "minimal",
        }
    }
}

#[derive(Clone, Debug)]
pub struct LoadTestConfig {
    /// 目标 URL（http/https）。
    pub url: String,
    /// HTTP method（GET/POST/…）。
    pub method: String,
    /// 并发 worker 数；每个 worker 循环发送请求直到到期或被 stop。
    pub concurrency: usize,
    /// 并发 ramp-up 时长（0 表示同时启动）。
    pub ramp_up: Duration,
    /// 每个 worker 的最大请求次数（None 表示不限制）。
    pub iterations_per_worker: Option<u64>,
    /// 全局总请求数上限（None 表示不限制）。
    pub total_requests_limit: Option<u64>,
    /// 总运行时长。
    pub duration: Duration,
    /// 单次请求超时。
    pub timeout: Duration,
    /// 请求体（通常用于 POST）。
    pub body: Option<Vec<u8>>,
    /// 额外请求头。
    pub headers: Vec<(String, String)>,
    /// 响应处理模式：统计字节 / 丢弃内容。
    pub response_mode: ResponseMode,
    /// 指标模式：完整分位 / 极限吞吐。
    pub metrics_mode: MetricsMode,
    /// 连接模式：keep-alive 复用 / 每请求新建。
    pub connection_mode: ConnectionMode,
    /// 限速 RPS（全局或每 worker，根据 rps_mode）。None 表示不限速。
    pub rps_limit: Option<f64>,
    /// RPS 调度模式（默认全局）。
    pub rps_mode: RpsMode,
    /// 是否允许不安全证书（仅对 https 有效）。
    ///
    /// 当开启时，会跳过证书链校验与域名校验，存在明显安全风险（MITM 等）。
    /// 仅建议用于开发/内网自签证书靶机。
    pub allow_insecure_certs: bool,
    /// 指标上报间隔（通过事件流推送给壳层）。
    pub report_interval: Duration,
    /// 延迟样本的最大保留数量（近似分位计算的 reservoir 上限）。
    pub max_latency_samples: usize,
}

impl LoadTestConfig {
    pub fn new(
        url: String,
        method: String,
        concurrency: usize,
        duration: Duration,
        timeout: Duration,
    ) -> Self {
        Self {
            url,
            method,
            concurrency,
            ramp_up: Duration::ZERO,
            iterations_per_worker: None,
            total_requests_limit: None,
            duration,
            timeout,
            body: None,
            headers: Vec::new(),
            response_mode: ResponseMode::CountBytes,
            metrics_mode: MetricsMode::Full,
            connection_mode: ConnectionMode::KeepAlive,
            rps_limit: None,
            rps_mode: RpsMode::Global,
            allow_insecure_certs: false,
            report_interval: DEFAULT_REPORT_INTERVAL,
            max_latency_samples: DEFAULT_MAX_LATENCY_SAMPLES,
        }
    }
}

#[derive(Clone, Debug)]
pub enum LoadTestEvent {
    Metrics(Box<MetricsSnapshot>),
    Log(String),
}

pub struct LoadTestRun {
    /// 后台运行任务句柄；壳层可选择等待或直接 abort。
    pub task: JoinHandle<()>,
    /// 停止标记（壳层用于请求停止）。
    pub stop_flag: Arc<AtomicBool>,
    /// 事件流（指标快照/日志）。
    pub events: mpsc::UnboundedReceiver<LoadTestEvent>,
}

/// 启动一次负载测试，返回可停止的运行句柄与事件流。
///
/// 该函数只负责启动与串起事件流，不做任何 UI 语义。
pub fn spawn(config: LoadTestConfig) -> Result<LoadTestRun, String> {
    let validated = ValidatedConfig::try_from(config)?;

    let stop_flag = Arc::new(AtomicBool::new(false));
    let (events_tx, events_rx) = mpsc::unbounded_channel::<LoadTestEvent>();
    let task_stop = stop_flag.clone();

    let task = tokio::spawn(async move {
        if let Err(err) = run(validated, task_stop, events_tx.clone()).await {
            let _ = events_tx.send(LoadTestEvent::Log(err));
        }
    });

    Ok(LoadTestRun {
        task,
        stop_flag,
        events: events_rx,
    })
}

#[derive(Clone)]
struct ValidatedConfig {
    uri: Uri,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
    response_mode: ResponseMode,
    metrics_mode: MetricsMode,
    connection_mode: ConnectionMode,
    rps_limit: Option<f64>,
    rps_mode: RpsMode,
    allow_insecure_certs: bool,
    concurrency: usize,
    ramp_up: Duration,
    iterations_per_worker: Option<u64>,
    total_requests_limit: Option<u64>,
    progress_total_requests_limit: Option<u64>,
    duration: Duration,
    timeout: Duration,
    report_interval: Duration,
    max_latency_samples: usize,
    request_bytes_per_request: u64,
}

impl TryFrom<LoadTestConfig> for ValidatedConfig {
    type Error = String;

    fn try_from(value: LoadTestConfig) -> Result<Self, Self::Error> {
        let uri: Uri = value.url.parse().map_err(|e| format!("无效的 URL: {e}"))?;
        let method = Method::from_bytes(value.method.as_bytes())
            .map_err(|e| format!("无效的 Method: {e}"))?;
        let duration = value.duration.max(Duration::from_secs(1));
        let timeout = value.timeout.max(Duration::from_millis(100));
        let report_interval = value.report_interval.max(Duration::from_millis(100));
        let max_latency_samples = value.max_latency_samples.max(1);

        let mut headers = HeaderMap::new();
        for (key, val) in value.headers {
            let name = HeaderName::from_bytes(key.as_bytes())
                .map_err(|e| format!("无效的 header 名称 `{key}`: {e}"))?;
            let value = HeaderValue::from_str(&val)
                .map_err(|e| format!("无效的 header 值 `{val}`: {e}"))?;
            headers.insert(name, value);
        }

        let request_bytes_per_request = estimate_request_bytes(
            &uri,
            &method,
            &headers,
            value.body.as_deref().map(|v| v.len()).unwrap_or(0),
            value.connection_mode,
        )?;
        let derived_total = value
            .iterations_per_worker
            .and_then(|limit| if limit > 0 { Some(limit) } else { None })
            .map(|limit| limit.saturating_mul(value.concurrency.max(1) as u64));
        let total_requests_limit = value
            .total_requests_limit
            .and_then(|limit| if limit > 0 { Some(limit) } else { None });
        let progress_total_requests_limit = match (total_requests_limit, derived_total) {
            (Some(total), Some(derived)) => Some(total.min(derived)),
            (Some(total), None) => Some(total),
            (None, Some(derived)) => Some(derived),
            (None, None) => None,
        };

        Ok(Self {
            uri,
            method,
            headers,
            body: Bytes::from(value.body.unwrap_or_default()),
            response_mode: value.response_mode,
            metrics_mode: value.metrics_mode,
            connection_mode: value.connection_mode,
            rps_limit: value.rps_limit.and_then(|v| {
                if v.is_finite() && v > 0.0 {
                    Some(v)
                } else {
                    None
                }
            }),
            rps_mode: value.rps_mode,
            allow_insecure_certs: value.allow_insecure_certs,
            concurrency: value.concurrency.max(1),
            ramp_up: value.ramp_up.min(duration),
            iterations_per_worker: value
                .iterations_per_worker
                .and_then(|limit| if limit > 0 { Some(limit) } else { None }),
            total_requests_limit,
            progress_total_requests_limit,
            duration,
            timeout,
            report_interval,
            max_latency_samples,
            request_bytes_per_request,
        })
    }
}

/// 一个“接受所有证书”的 verifier（危险）。
///
/// 仅用于支持“开发/内网自签证书靶机”的压测场景。
/// 开启后：证书链/域名均不校验，存在 MITM 风险。
#[derive(Debug)]
struct InsecureCertVerifier {
    supported_schemes: Vec<rustls::SignatureScheme>,
}

impl InsecureCertVerifier {
    fn new() -> Self {
        // 复用当前 provider 的 scheme 列表，避免因为 schemes 不匹配导致握手失败。
        let supported_schemes = rustls::crypto::aws_lc_rs::default_provider()
            .signature_verification_algorithms
            .supported_schemes();
        Self { supported_schemes }
    }
}

impl rustls::client::danger::ServerCertVerifier for InsecureCertVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.supported_schemes.clone()
    }
}

async fn run(
    config: ValidatedConfig,
    stop_flag: Arc<AtomicBool>,
    events_tx: mpsc::UnboundedSender<LoadTestEvent>,
) -> Result<(), String> {
    let scheme = config
        .uri
        .scheme_str()
        .ok_or_else(|| "URL 缺少 scheme（必须是 http/https）".to_string())?;
    let scheme = Scheme::try_from(scheme)?;

    // connector 选择：
    // - http：使用 `HttpConnector`（纯 TCP），避免 TLS/ALPN 相关层，特别适合本地 loopback 靶机压吞吐。
    // - https：使用 `hyper-rustls`，并启用 http1/http2（由 ALPN 协商），同时显式安装 rustls provider。
    match scheme {
        Scheme::Http => {
            let mut http = HttpConnector::new();
            http.enforce_http(true);
            http.set_nodelay(true);

            let mut builder = Client::builder(TokioExecutor::new());
            let pool_config = config.connection_mode.pool_config();
            builder.pool_max_idle_per_host(pool_config.max_idle_per_host);
            builder.pool_idle_timeout(Some(pool_config.idle_timeout));
            let client = builder.build(http);

            run_with_client(client, config, stop_flag, events_tx).await
        }
        Scheme::Https => {
            // rustls 0.23+ 需要在进程级别显式选择 CryptoProvider（当依赖树同时启用多个 provider 时尤为重要）。
            // 这里选择 aws-lc-rs 作为默认 provider；若已安装则会返回 Err(Arc<_>)，可安全忽略。
            let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

            let make_tls_config = || -> Result<rustls::ClientConfig, String> {
                if config.allow_insecure_certs {
                    Ok(rustls::ClientConfig::builder()
                        .dangerous()
                        .with_custom_certificate_verifier(Arc::new(InsecureCertVerifier::new()))
                        .with_no_client_auth())
                } else {
                    let cfg = rustls::ClientConfig::builder()
                        .with_native_roots()
                        .map_err(|e| format!("加载系统根证书失败: {e}"))?
                        .with_no_client_auth();
                    Ok(cfg)
                }
            };

            let version_policy = match config.connection_mode {
                ConnectionMode::KeepAlive => HttpVersionPolicy::Http1OrHttp2,
                ConnectionMode::NewConnection => HttpVersionPolicy::Http1Only,
            };
            let https = if version_policy.allows_http2() {
                HttpsConnectorBuilder::new()
                    .with_tls_config(make_tls_config()?)
                    .https_or_http()
                    .enable_http1()
                    .enable_http2()
                    .build()
            } else {
                HttpsConnectorBuilder::new()
                    .with_tls_config(make_tls_config()?)
                    .https_or_http()
                    .enable_http1()
                    .build()
            };

            let mut builder = Client::builder(TokioExecutor::new());
            let pool_config = config.connection_mode.pool_config();
            builder.pool_max_idle_per_host(pool_config.max_idle_per_host);
            builder.pool_idle_timeout(Some(pool_config.idle_timeout));
            let client = builder.build(https);

            run_with_client(client, config, stop_flag, events_tx).await
        }
    }
}

#[derive(Debug)]
enum RequestError {
    Client(hyper_util::client::legacy::Error),
    Body(String),
    Timeout,
}

type RequestOutcome = Result<(bool, u64, Option<u16>), RequestError>;

impl RequestError {
    fn reason(&self) -> String {
        match self {
            Self::Client(err) => format_error_chain(err),
            Self::Body(err) => err.clone(),
            Self::Timeout => "请求超时".to_string(),
        }
    }

    fn message(&self) -> String {
        match self {
            Self::Client(_) | Self::Body(_) => self.reason(),
            Self::Timeout => self.reason(),
        }
    }

    fn is_cert_related(&self) -> bool {
        match self {
            Self::Client(err) => error_chain_has_cert_issue(err),
            Self::Body(err) => looks_like_cert_error(err),
            Self::Timeout => false,
        }
    }
}

fn format_error_chain(err: &(dyn StdError + 'static)) -> String {
    let mut parts = Vec::new();
    let mut current: Option<&(dyn StdError + 'static)> = Some(err);
    while let Some(err) = current {
        parts.push(err.to_string());
        current = err.source();
    }
    parts.join(" -> ")
}

struct LoadWorker<C> {
    client: Client<C, Full<Bytes>>,
    config: ValidatedConfig,
    metrics: Arc<LoadMetrics>,
    events_tx: mpsc::UnboundedSender<LoadTestEvent>,
    request_template: Request<Full<Bytes>>,
    latency_buf: Option<Vec<u64>>,
    timeout_sleep: Pin<Box<tokio::time::Sleep>>,
    stop_flag: Arc<AtomicBool>,
    request_bytes: u64,
}

impl<C> LoadWorker<C>
where
    C: hyper_util::client::legacy::connect::Connect + Clone + Send + Sync + 'static,
{
    fn new(
        client: Client<C, Full<Bytes>>,
        config: ValidatedConfig,
        metrics: Arc<LoadMetrics>,
        events_tx: mpsc::UnboundedSender<LoadTestEvent>,
        request_template: Request<Full<Bytes>>,
        stop_flag: Arc<AtomicBool>,
    ) -> Self {
        let request_bytes = config.request_bytes_per_request;
        let latency_buf = if config.metrics_mode == MetricsMode::Full {
            Some(Vec::with_capacity(256))
        } else {
            None
        };
        let timeout_sleep = Box::pin(sleep(config.timeout));
        Self {
            client,
            config,
            metrics,
            events_tx,
            request_template,
            latency_buf,
            timeout_sleep,
            stop_flag,
            request_bytes,
        }
    }
}

impl<C> Worker for LoadWorker<C>
where
    C: hyper_util::client::legacy::connect::Connect + Clone + Send + Sync + 'static,
{
    fn step(
        &mut self,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + '_>> {
        Box::pin(async move {
            let started = Instant::now();

            let request = self.request_template.clone();
            let client = self.client.clone();
            let response_mode = self.config.response_mode;
            self.timeout_sleep
                .as_mut()
                .reset(TokioInstant::now() + self.config.timeout);

            let result: Result<RequestOutcome, RequestError> = tokio::select! {
                res = async {
                    let response = client
                        .request(request)
                        .await
                        .map_err(RequestError::Client)?;
                    let status = response.status();
                    let bytes = handle_response_body(response.into_body(), response_mode)
                        .await
                        .map_err(RequestError::Body)?;
                    Ok::<(bool, u64, Option<u16>), RequestError>((status.is_success(), bytes, Some(status.as_u16())))
                } => Ok(res),
                _ = self.timeout_sleep.as_mut() => Err(RequestError::Timeout),
            };

            let latency = started.elapsed();
            match result {
                Ok(Ok((ok, bytes, status_code))) => {
                    self.metrics.record_counters(
                        latency,
                        bytes,
                        self.request_bytes,
                        ok,
                        status_code,
                    );
                    if let Some(buf) = self.latency_buf.as_mut() {
                        buf.push(latency.as_nanos().min(u64::MAX as u128) as u64);
                    }
                }
                Ok(Err(err)) | Err(err) => {
                    self.metrics
                        .record_counters(latency, 0, self.request_bytes, false, None);
                    if let Some(buf) = self.latency_buf.as_mut() {
                        buf.push(latency.as_nanos().min(u64::MAX as u128) as u64);
                    }
                    let cert_related = err.is_cert_related();
                    let log_message = if cert_related && !self.config.allow_insecure_certs {
                        format!(
                            "TLS 证书校验失败：{}；未开启\"允许不安全证书\"，压测已停止",
                            err.reason()
                        )
                    } else {
                        err.message()
                    };
                    let aborted = maybe_abort_for_cert_error(
                        &log_message,
                        cert_related,
                        self.config.allow_insecure_certs,
                        &self.stop_flag,
                    );
                    let _ = self.events_tx.send(LoadTestEvent::Log(log_message));
                    if aborted {
                        let _ = self.events_tx.send(LoadTestEvent::Log(
                            "TLS certificate verification failed; stopping load test".to_string(),
                        ));
                    }
                }
            }

            if let Some(buf) = self.latency_buf.as_mut()
                && buf.len() >= 256
            {
                // 延迟样本批量 flush，减少全局锁竞争（对本地靶机吞吐至关重要）。
                self.metrics.flush_latency_samples(buf);
            }

            Ok(())
        })
    }

    fn finish(&mut self) -> Pin<Box<dyn std::future::Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            if let Some(buf) = self.latency_buf.as_mut() {
                self.metrics.flush_latency_samples(buf);
            }
        })
    }
}

async fn run_with_client<C>(
    client: Client<C, Full<Bytes>>,
    config: ValidatedConfig,
    stop_flag: Arc<AtomicBool>,
    events_tx: mpsc::UnboundedSender<LoadTestEvent>,
) -> Result<(), String>
where
    C: hyper_util::client::legacy::connect::Connect + Clone + Send + Sync + 'static,
{
    let start = Instant::now();
    let metrics = Arc::new(LoadMetrics::new(config.max_latency_samples));
    let done_flag = Arc::new(AtomicBool::new(false));
    // 预构建请求模板：避免在请求热路径中重复解析 header/uri。
    let request_template = build_request(&config)?;

    spawn_metrics_ticker(
        metrics.clone(),
        start,
        config.duration,
        config.report_interval,
        config.metrics_mode,
        stop_flag.clone(),
        done_flag.clone(),
        config.progress_total_requests_limit,
        events_tx.clone(),
    );

    let schedule_config = ScheduleConfig::new(
        start,
        config.duration,
        config.concurrency,
        stop_flag.clone(),
        config.rps_limit,
        config.rps_mode,
        config.ramp_up,
        config.total_requests_limit,
        config.iterations_per_worker,
    );
    let client = client.clone();
    let config_for_workers = config.clone();
    let metrics_for_workers = metrics.clone();
    let events_for_workers = events_tx.clone();
    let request_template = request_template.clone();
    let stop_flag_for_workers = stop_flag.clone();
    let results =
        crate::scheduler::run_fixed_concurrency::<LoadWorker<C>, _>(schedule_config, move |ctx| {
            let _ = ctx;
            LoadWorker::new(
                client.clone(),
                config_for_workers.clone(),
                metrics_for_workers.clone(),
                events_for_workers.clone(),
                request_template.clone(),
                stop_flag_for_workers.clone(),
            )
        })
        .await;

    for result in results {
        if let Err(err) = result {
            let _ = events_tx.send(LoadTestEvent::Log(err));
        }
    }

    done_flag.store(true, Ordering::Relaxed);
    let mut final_snapshot = match config.metrics_mode {
        MetricsMode::Full => metrics.snapshot(
            start,
            config.duration,
            &COMPLETION_PERCENTILES,
            config.progress_total_requests_limit,
            true,
        ),
        MetricsMode::Minimal => minimal_snapshot(
            metrics.as_ref(),
            start,
            config.duration,
            config.progress_total_requests_limit,
            true,
        ),
    };
    final_snapshot.done = true;
    let _ = events_tx.send(LoadTestEvent::Metrics(Box::new(final_snapshot)));

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn spawn_metrics_ticker(
    metrics: Arc<LoadMetrics>,
    start: Instant,
    duration: Duration,
    report_interval: Duration,
    metrics_mode: MetricsMode,
    stop_flag: Arc<AtomicBool>,
    done_flag: Arc<AtomicBool>,
    total_requests_limit: Option<u64>,
    events_tx: mpsc::UnboundedSender<LoadTestEvent>,
) {
    tokio::spawn(async move {
        let mut ticker = interval(report_interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

        // 周期性快照：用于 UI 实时图表/进度展示。
        // 注意：Full 模式下 percentile 计算会带来额外成本；Minimal 模式下会跳过。
        loop {
            ticker.tick().await;
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }

            let snapshot = match metrics_mode {
                MetricsMode::Full => metrics.snapshot(
                    start,
                    duration,
                    &COMPLETION_PERCENTILES,
                    total_requests_limit,
                    done_flag.load(Ordering::Relaxed),
                ),
                MetricsMode::Minimal => minimal_snapshot(
                    metrics.as_ref(),
                    start,
                    duration,
                    total_requests_limit,
                    done_flag.load(Ordering::Relaxed),
                ),
            };
            let done = snapshot.done;
            let _ = events_tx.send(LoadTestEvent::Metrics(Box::new(snapshot)));
            if done {
                break;
            }
        }
    });
}

fn minimal_snapshot(
    metrics: &LoadMetrics,
    start: Instant,
    duration: Duration,
    total_requests_limit: Option<u64>,
    force_done: bool,
) -> MetricsSnapshot {
    // Minimal 模式的快照只做计数与均值，p50/p90/p95/p99/buckets 置空（避免 percentile 的排序/采样成本）。
    let elapsed = start.elapsed();
    let elapsed_secs = elapsed.as_secs_f64().max(0.001);
    let counters = metrics.counters_snapshot();
    let throughput_bps = counters.byte_count as f64 / elapsed_secs;
    let throughput_bps_up = counters.byte_count_up as f64 / elapsed_secs;
    let avg_bytes_per_request = if counters.total_requests > 0 {
        counters.byte_count as f64 / counters.total_requests as f64
    } else {
        0.0
    };
    let avg_bytes_per_request_up = if counters.total_requests > 0 {
        counters.byte_count_up as f64 / counters.total_requests as f64
    } else {
        0.0
    };
    let time_progress = (elapsed_secs / duration.as_secs_f64().max(0.001)).min(1.0);
    let request_progress = total_requests_limit
        .map(|limit| (counters.total_requests as f64 / limit.max(1) as f64).min(1.0))
        .unwrap_or(0.0);
    let progress = if force_done {
        1.0
    } else if total_requests_limit.is_some() {
        time_progress.max(request_progress)
    } else {
        time_progress
    };
    let done = force_done
        || elapsed >= duration
        || total_requests_limit
            .map(|limit| counters.total_requests >= limit)
            .unwrap_or(false);

    MetricsSnapshot {
        elapsed,
        progress,
        total_requests: counters.total_requests,
        success: counters.success,
        failures: counters.failures,
        avg_latency: counters.avg_latency,
        p50: Duration::ZERO,
        p90: Duration::ZERO,
        p95: Duration::ZERO,
        p99: Duration::ZERO,
        rps: counters.total_requests as f64 / elapsed_secs,
        throughput_bps,
        throughput_bps_up,
        total_bytes: counters.byte_count,
        total_bytes_up: counters.byte_count_up,
        avg_bytes_per_request,
        avg_bytes_per_request_up,
        status_codes: counters.status_codes,
        status_no_response: counters.status_no_response,
        status_other: counters.status_other,
        completion_buckets: Vec::new(),
        done,
    }
}

fn estimate_request_bytes(
    uri: &Uri,
    method: &Method,
    headers: &HeaderMap,
    body_len: usize,
    connection_mode: ConnectionMode,
) -> Result<u64, String> {
    let path = uri.path_and_query().map(|v| v.as_str()).unwrap_or("/");
    let mut total = 0u64;
    let version = "HTTP/1.1";
    total += method.as_str().len() as u64;
    total += 1;
    total += path.len() as u64;
    total += 1;
    total += version.len() as u64;
    total += 2;

    let mut has_host = headers.contains_key(header::HOST);
    let mut header_bytes = 0u64;
    for (name, value) in headers.iter() {
        header_bytes += name.as_str().len() as u64;
        header_bytes += 2;
        header_bytes += value.as_bytes().len() as u64;
        header_bytes += 2;
    }

    if !has_host {
        let authority = uri
            .authority()
            .ok_or_else(|| "URL 缺少 authority（host:port）".to_string())?;
        let host_value = authority.as_str();
        header_bytes += header::HOST.as_str().len() as u64;
        header_bytes += 2;
        header_bytes += host_value.len() as u64;
        header_bytes += 2;
        has_host = true;
    }

    if connection_mode.pool_config().close_header && !headers.contains_key(header::CONNECTION) {
        header_bytes += header::CONNECTION.as_str().len() as u64;
        header_bytes += 2;
        header_bytes += "close".len() as u64;
        header_bytes += 2;
    }

    let _ = has_host;
    total += header_bytes;
    total += 2;
    total += body_len as u64;
    Ok(total)
}

async fn consume_body_and_count(mut body: hyper::body::Incoming) -> Result<u64, String> {
    let mut bytes: u64 = 0;
    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(|e| format!("读取响应失败: {e}"))?;
        if let Some(data) = frame.data_ref() {
            bytes += data.len() as u64;
        }
    }
    Ok(bytes)
}

async fn handle_response_body(
    body: hyper::body::Incoming,
    mode: ResponseMode,
) -> Result<u64, String> {
    match mode {
        ResponseMode::CountBytes => consume_body_and_count(body).await,
        ResponseMode::DiscardBody => {
            // 仍然要 drain，保证连接可复用；否则 keep-alive 会出现不可预期行为。
            consume_body_discard(body).await?;
            Ok(0)
        }
    }
}

async fn consume_body_discard(mut body: hyper::body::Incoming) -> Result<(), String> {
    while let Some(frame) = body.frame().await {
        let frame = frame.map_err(|e| format!("读取响应失败: {e}"))?;
        let _ = frame.data_ref();
    }
    Ok(())
}

fn build_request(config: &ValidatedConfig) -> Result<Request<Full<Bytes>>, String> {
    let mut builder = Request::builder()
        .method(config.method.clone())
        .uri(config.uri.clone());

    // 尽量对齐 reqwest 的默认行为：如果用户没手动提供 Host，则根据 URL 自动补齐。
    if !config.headers.contains_key(header::HOST) {
        let authority = config
            .uri
            .authority()
            .ok_or_else(|| "URL 缺少 authority（host:port）".to_string())?;
        builder = builder.header(header::HOST, authority.as_str());
    }

    if !config.headers.is_empty() {
        let headers_mut = builder.headers_mut().ok_or("构建请求头失败")?;
        for (key, val) in config.headers.iter() {
            headers_mut.insert(key, val.clone());
        }
    }

    // 短连接模式下显式添加 Connection: close，避免 keep-alive 复用。
    if config.connection_mode.pool_config().close_header
        && !config.headers.contains_key(header::CONNECTION)
    {
        let headers_mut = builder.headers_mut().ok_or("构建请求头失败")?;
        headers_mut.insert(header::CONNECTION, HeaderValue::from_static("close"));
    }

    builder
        .body(Full::new(config.body.clone()))
        .map_err(|e| format!("构建请求失败: {e}"))
}

fn looks_like_cert_error(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    (lower.contains("certificate") || lower.contains("cert "))
        && (lower.contains("invalid")
            || lower.contains("unknown")
            || lower.contains("untrusted")
            || lower.contains("failed")
            || lower.contains("verify")
            || lower.contains("self signed"))
}

fn error_chain_has_cert_issue(err: &(dyn StdError + 'static)) -> bool {
    let mut current: Option<&(dyn StdError + 'static)> = Some(err);
    while let Some(err) = current {
        if err.downcast_ref::<rustls::Error>().is_some() {
            return true;
        }
        if looks_like_cert_error(&err.to_string()) {
            return true;
        }
        current = err.source();
    }
    false
}

fn maybe_abort_for_cert_error(
    err: &str,
    cert_related: bool,
    allow_insecure_certs: bool,
    stop_flag: &Arc<AtomicBool>,
) -> bool {
    if allow_insecure_certs || !(cert_related || looks_like_cert_error(err)) {
        return false;
    }

    // 第一次触发时替换 stop 标记，提示已报停。
    !stop_flag.swap(true, Ordering::Relaxed)
}
