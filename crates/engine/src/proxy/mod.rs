use std::collections::HashMap;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::task::{Context, Poll};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use http::Uri;
use http_body_util::{BodyExt, Empty};
use hyper::body::{Body, Frame, Incoming};
use hyper::header::HOST;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::upgrade::Upgraded;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::client::legacy::{Client, connect::HttpConnector};
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

#[derive(Clone, Debug)]
pub struct ProxyConfig {
    pub listen_host: String,
    pub listen_port: u16,
    pub report_interval: Duration,
    pub client_idle_ttl: Duration,
}

#[derive(Clone, Debug)]
pub enum ProxyEvent {
    Status(String),
    Snapshot(ProxySnapshot),
}

#[derive(Clone, Debug)]
pub struct ProxySnapshot {
    pub uptime: Duration,
    pub active_connections: u64,
    pub total_connections: u64,
    pub total_requests: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub clients: Vec<ClientSnapshot>,
}

#[derive(Clone, Debug)]
pub struct ClientSnapshot {
    pub ip: String,
    pub active_connections: u64,
    pub total_connections: u64,
    pub total_requests: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub last_seen_ms: u64,
    pub top_targets: Vec<TargetStat>,
}

#[derive(Clone, Debug)]
pub struct TargetStat {
    pub target: String,
    pub count: u64,
}

pub struct ProxySession {
    pub events: mpsc::Receiver<ProxyEvent>,
    pub handle: ProxyHandle,
}

pub struct ProxyHandle {
    stop_tx: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
    report_task: JoinHandle<()>,
    stop_flag: Arc<AtomicBool>,
}

impl ProxyHandle {
    pub fn stop(mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        self.task.abort();
        self.report_task.abort();
    }
}

pub fn start(config: ProxyConfig) -> ProxySession {
    let (event_tx, event_rx) = mpsc::channel::<ProxyEvent>(256);
    let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stats = Arc::new(ProxyStats::new(config.client_idle_ttl));
    let listen_addr = format_host_port(&config.listen_host, config.listen_port);

    let report_interval = config.report_interval.max(Duration::from_millis(100));
    let stats_for_report = stats.clone();
    let event_tx_for_report = event_tx.clone();
    let stop_flag_for_report = stop_flag.clone();
    let report_task = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(report_interval);
        loop {
            ticker.tick().await;
            if stop_flag_for_report.load(Ordering::Relaxed) {
                break;
            }
            let snapshot = stats_for_report.snapshot();
            let _ = event_tx_for_report
                .send(ProxyEvent::Snapshot(snapshot))
                .await;
        }
    });

    let task = tokio::spawn(async move {
        let listener = match TcpListener::bind(&listen_addr).await {
            Ok(listener) => listener,
            Err(err) => {
                let _ = event_tx
                    .send(ProxyEvent::Status(format!("proxy listen failed: {err}")))
                    .await;
                return;
            }
        };

        let _ = event_tx
            .send(ProxyEvent::Status(format!(
                "proxy listening on {listen_addr}"
            )))
            .await;

        let mut connector = HttpConnector::new();
        connector.set_nodelay(true);
        let client: ProxyClient = Client::builder(TokioExecutor::new()).build(connector);

        loop {
            tokio::select! {
                _ = &mut stop_rx => {
                    let _ = event_tx.send(ProxyEvent::Status("proxy stopped".into())).await;
                    break;
                }
                accepted = listener.accept() => {
                    match accepted {
                        Ok((stream, addr)) => {
                            let stats = stats.clone();
                            let client = client.clone();
                            let event_tx = event_tx.clone();
                            tokio::spawn(async move {
                                handle_connection(stream, addr, stats, client, event_tx).await;
                            });
                        }
                        Err(err) => {
                            let _ = event_tx
                                .send(ProxyEvent::Status(format!(
                                    "proxy accept failed: {err}"
                                )))
                                .await;
                            break;
                        }
                    }
                }
            }
        }
    });

    ProxySession {
        events: event_rx,
        handle: ProxyHandle {
            stop_tx: Some(stop_tx),
            task,
            report_task,
            stop_flag,
        },
    }
}

type ProxyClient = Client<HttpConnector, CountingBody<Incoming>>;
type ProxyBody = http_body_util::combinators::BoxBody<Bytes, hyper::Error>;

async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    stats: Arc<ProxyStats>,
    client: ProxyClient,
    event_tx: mpsc::Sender<ProxyEvent>,
) {
    let ip = addr.ip().to_string();
    let client_stats = stats.get_or_create_client(&ip);
    stats.on_connect(&client_stats);
    let _ = event_tx
        .send(ProxyEvent::Status(format!("proxy client connected: {ip}")))
        .await;

    let stats_for_service = stats.clone();
    let client_stats_for_service = client_stats.clone();
    let event_tx_for_service = event_tx.clone();
    let service = service_fn(move |req| {
        let stats = stats_for_service.clone();
        let client_stats = client_stats_for_service.clone();
        let client = client.clone();
        let event_tx = event_tx_for_service.clone();
        async move { proxy_request(req, stats, client_stats, client, event_tx).await }
    });

    if let Err(err) = http1::Builder::new()
        .preserve_header_case(true)
        .title_case_headers(true)
        .serve_connection(TokioIo::new(stream), service)
        .with_upgrades()
        .await
    {
        let _ = event_tx
            .send(ProxyEvent::Status(format!(
                "proxy connection error for {ip}: {err}"
            )))
            .await;
    }

    stats.on_disconnect(&client_stats);
    let _ = event_tx
        .send(ProxyEvent::Status(format!(
            "proxy client disconnected: {ip}"
        )))
        .await;
}

async fn proxy_request(
    req: Request<Incoming>,
    stats: Arc<ProxyStats>,
    client_stats: Arc<ClientStats>,
    client: ProxyClient,
    event_tx: mpsc::Sender<ProxyEvent>,
) -> Result<Response<ProxyBody>, hyper::Error> {
    stats.on_request(&client_stats);

    if req.method() == Method::CONNECT {
        return handle_connect(req, stats, client_stats, event_tx).await;
    }

    let uri = match build_forward_uri(&req) {
        Ok(uri) => uri,
        Err(message) => {
            let response = Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(empty_body())
                .unwrap();
            let _ = event_tx
                .send(ProxyEvent::Status(format!(
                    "proxy rejected request: {message}"
                )))
                .await;
            return Ok(response);
        }
    };

    let target = format_http_target(&uri);
    client_stats.record_target(&target);

    let (mut parts, body) = req.into_parts();
    parts.uri = uri;
    parts.headers.remove("proxy-connection");

    let counter = ByteCounter::new(stats.clone(), client_stats.clone(), Direction::In);
    let body = CountingBody::new(body, counter);
    let forward_req = Request::from_parts(parts, body);

    let upstream_resp = match client.request(forward_req).await {
        Ok(resp) => resp,
        Err(err) => {
            let response = Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(empty_body())
                .unwrap();
            let _ = event_tx
                .send(ProxyEvent::Status(format!("proxy upstream error: {err}")))
                .await;
            return Ok(response);
        }
    };

    let (parts, body) = upstream_resp.into_parts();
    let counter = ByteCounter::new(stats, client_stats, Direction::Out);
    let body = CountingBody::new(body, counter);
    Ok(Response::from_parts(parts, body.boxed()))
}

async fn handle_connect(
    req: Request<Incoming>,
    stats: Arc<ProxyStats>,
    client_stats: Arc<ClientStats>,
    event_tx: mpsc::Sender<ProxyEvent>,
) -> Result<Response<ProxyBody>, hyper::Error> {
    let authority = match req.uri().authority() {
        Some(authority) => normalize_connect_authority(authority.as_str()),
        None => {
            let response = Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(empty_body())
                .unwrap();
            let _ = event_tx
                .send(ProxyEvent::Status("proxy CONNECT missing authority".into()))
                .await;
            return Ok(response);
        }
    };

    client_stats.record_target(&authority);
    let on_upgrade = hyper::upgrade::on(req);
    let event_tx_for_tunnel = event_tx.clone();
    tokio::spawn(async move {
        match on_upgrade.await {
            Ok(upgraded) => {
                let event_tx_err = event_tx_for_tunnel.clone();
                if let Err(err) = tunnel_connect(
                    upgraded,
                    authority,
                    stats,
                    client_stats,
                    event_tx_for_tunnel,
                )
                .await
                {
                    let _ = event_tx_err
                        .send(ProxyEvent::Status(format!("proxy CONNECT error: {err}")))
                        .await;
                }
            }
            Err(err) => {
                let _ = event_tx
                    .send(ProxyEvent::Status(format!(
                        "proxy CONNECT upgrade failed: {err}"
                    )))
                    .await;
            }
        }
    });

    Ok(Response::builder()
        .status(StatusCode::OK)
        .body(empty_body())
        .unwrap())
}

async fn tunnel_connect(
    upgraded: Upgraded,
    authority: String,
    stats: Arc<ProxyStats>,
    client_stats: Arc<ClientStats>,
    event_tx: mpsc::Sender<ProxyEvent>,
) -> Result<(), String> {
    let mut server = TcpStream::connect(&authority)
        .await
        .map_err(|e| format!("connect upstream failed: {e}"))?;

    let mut upgraded = TokioIo::new(upgraded);
    let (from_client, from_server) = tokio::io::copy_bidirectional(&mut upgraded, &mut server)
        .await
        .map_err(|e| format!("tunnel copy failed: {e}"))?;

    stats.add_bytes(Direction::In, from_client, &client_stats);
    stats.add_bytes(Direction::Out, from_server, &client_stats);
    let _ = event_tx
        .send(ProxyEvent::Status(format!(
            "proxy CONNECT closed: {authority} ({from_client} in/{from_server} out)"
        )))
        .await;
    Ok(())
}

fn empty_body() -> ProxyBody {
    Empty::<Bytes>::new().map_err(|err| match err {}).boxed()
}

fn normalize_connect_authority(authority: &str) -> String {
    if has_port(authority) {
        authority.to_string()
    } else {
        format!("{authority}:443")
    }
}

fn has_port(authority: &str) -> bool {
    if authority.starts_with('[') {
        authority.contains("]:")
    } else {
        authority.rsplit_once(':').is_some()
    }
}

fn format_http_target(uri: &Uri) -> String {
    if let Some(authority) = uri.authority() {
        let raw = authority.as_str();
        if has_port(raw) {
            raw.to_string()
        } else {
            format!("{raw}:80")
        }
    } else {
        "unknown:80".to_string()
    }
}

fn build_forward_uri(req: &Request<Incoming>) -> Result<Uri, String> {
    let scheme = req
        .uri()
        .scheme_str()
        .unwrap_or("http")
        .to_ascii_lowercase();
    if scheme == "https" {
        return Err("https request must use CONNECT".into());
    }

    let authority = if let Some(authority) = req.uri().authority() {
        authority.as_str().to_string()
    } else if let Some(host) = req.headers().get(HOST) {
        host.to_str()
            .map(|value| value.to_string())
            .map_err(|_| "invalid host header".to_string())?
    } else {
        return Err("missing host".into());
    };

    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|v| v.as_str())
        .unwrap_or("/");

    Uri::builder()
        .scheme("http")
        .authority(authority)
        .path_and_query(path_and_query)
        .build()
        .map_err(|_| "invalid target uri".to_string())
}

#[derive(Clone, Copy)]
enum Direction {
    In,
    Out,
}

struct ByteCounter {
    stats: Arc<ProxyStats>,
    client: Arc<ClientStats>,
    direction: Direction,
}

impl ByteCounter {
    fn new(stats: Arc<ProxyStats>, client: Arc<ClientStats>, direction: Direction) -> Arc<Self> {
        Arc::new(Self {
            stats,
            client,
            direction,
        })
    }

    fn add(&self, amount: usize) {
        if amount == 0 {
            return;
        }
        let value = amount as u64;
        match self.direction {
            Direction::In => {
                self.stats.bytes_in.fetch_add(value, Ordering::Relaxed);
                self.client.bytes_in.fetch_add(value, Ordering::Relaxed);
            }
            Direction::Out => {
                self.stats.bytes_out.fetch_add(value, Ordering::Relaxed);
                self.client.bytes_out.fetch_add(value, Ordering::Relaxed);
            }
        }
        self.client.touch();
    }
}

struct CountingBody<B> {
    inner: B,
    counter: Arc<ByteCounter>,
}

impl<B> CountingBody<B> {
    fn new(inner: B, counter: Arc<ByteCounter>) -> Self {
        Self { inner, counter }
    }
}

impl<B> Body for CountingBody<B>
where
    B: Body<Data = Bytes> + Unpin,
{
    type Data = Bytes;
    type Error = B::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        match Pin::new(&mut self.inner).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    self.counter.add(data.len());
                }
                Poll::Ready(Some(Ok(frame)))
            }
            other => other,
        }
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.size_hint()
    }
}

struct ProxyStats {
    start: Instant,
    active_connections: AtomicU64,
    total_connections: AtomicU64,
    total_requests: AtomicU64,
    bytes_in: AtomicU64,
    bytes_out: AtomicU64,
    clients: Mutex<HashMap<String, Arc<ClientStats>>>,
    client_idle_ttl_ms: u64,
}

impl ProxyStats {
    fn new(client_idle_ttl: Duration) -> Self {
        Self {
            start: Instant::now(),
            active_connections: AtomicU64::new(0),
            total_connections: AtomicU64::new(0),
            total_requests: AtomicU64::new(0),
            bytes_in: AtomicU64::new(0),
            bytes_out: AtomicU64::new(0),
            clients: Mutex::new(HashMap::new()),
            client_idle_ttl_ms: client_idle_ttl.as_millis() as u64,
        }
    }

    fn get_or_create_client(&self, ip: &str) -> Arc<ClientStats> {
        let mut guard = self.clients.lock().unwrap();
        if let Some(entry) = guard.get(ip) {
            return entry.clone();
        }
        let client = Arc::new(ClientStats::new(ip.to_string()));
        guard.insert(ip.to_string(), client.clone());
        client
    }

    fn on_connect(&self, client: &Arc<ClientStats>) {
        self.total_connections.fetch_add(1, Ordering::Relaxed);
        self.active_connections.fetch_add(1, Ordering::Relaxed);
        client.total_connections.fetch_add(1, Ordering::Relaxed);
        client.active_connections.fetch_add(1, Ordering::Relaxed);
        client.touch();
    }

    fn on_disconnect(&self, client: &Arc<ClientStats>) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
        client.active_connections.fetch_sub(1, Ordering::Relaxed);
        client.touch();
    }

    fn on_request(&self, client: &Arc<ClientStats>) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        client.total_requests.fetch_add(1, Ordering::Relaxed);
        client.touch();
    }

    fn add_bytes(&self, direction: Direction, amount: u64, client: &Arc<ClientStats>) {
        if amount == 0 {
            return;
        }
        match direction {
            Direction::In => {
                self.bytes_in.fetch_add(amount, Ordering::Relaxed);
                client.bytes_in.fetch_add(amount, Ordering::Relaxed);
            }
            Direction::Out => {
                self.bytes_out.fetch_add(amount, Ordering::Relaxed);
                client.bytes_out.fetch_add(amount, Ordering::Relaxed);
            }
        }
        client.touch();
    }

    fn snapshot(&self) -> ProxySnapshot {
        let now = now_ms();
        let ttl_ms = self.client_idle_ttl_ms;
        let clients = {
            let mut guard = self.clients.lock().unwrap();
            if ttl_ms > 0 {
                guard.retain(|_, client| {
                    now.saturating_sub(client.last_seen_ms.load(Ordering::Relaxed)) <= ttl_ms
                });
            }
            let mut entries: Vec<ClientSnapshot> =
                guard.values().map(|client| client.snapshot()).collect();
            entries.sort_by_key(|c| std::cmp::Reverse(c.total_requests));
            entries
        };

        ProxySnapshot {
            uptime: self.start.elapsed(),
            active_connections: self.active_connections.load(Ordering::Relaxed),
            total_connections: self.total_connections.load(Ordering::Relaxed),
            total_requests: self.total_requests.load(Ordering::Relaxed),
            bytes_in: self.bytes_in.load(Ordering::Relaxed),
            bytes_out: self.bytes_out.load(Ordering::Relaxed),
            clients,
        }
    }
}

struct ClientStats {
    ip: String,
    active_connections: AtomicU64,
    total_connections: AtomicU64,
    total_requests: AtomicU64,
    bytes_in: AtomicU64,
    bytes_out: AtomicU64,
    last_seen_ms: AtomicU64,
    targets: Mutex<HashMap<String, u64>>,
}

impl ClientStats {
    fn new(ip: String) -> Self {
        Self {
            ip,
            active_connections: AtomicU64::new(0),
            total_connections: AtomicU64::new(0),
            total_requests: AtomicU64::new(0),
            bytes_in: AtomicU64::new(0),
            bytes_out: AtomicU64::new(0),
            last_seen_ms: AtomicU64::new(now_ms()),
            targets: Mutex::new(HashMap::new()),
        }
    }

    fn touch(&self) {
        self.last_seen_ms.store(now_ms(), Ordering::Relaxed);
    }

    fn record_target(&self, target: &str) {
        let mut guard = self.targets.lock().unwrap();
        let counter = guard.entry(target.to_string()).or_insert(0);
        *counter += 1;
        self.touch();
    }

    fn snapshot(&self) -> ClientSnapshot {
        let mut top_targets: Vec<TargetStat> = {
            let guard = self.targets.lock().unwrap();
            guard
                .iter()
                .map(|(target, count)| TargetStat {
                    target: target.clone(),
                    count: *count,
                })
                .collect()
        };
        top_targets.sort_by_key(|entry| std::cmp::Reverse(entry.count));
        top_targets.truncate(5);

        ClientSnapshot {
            ip: self.ip.clone(),
            active_connections: self.active_connections.load(Ordering::Relaxed),
            total_connections: self.total_connections.load(Ordering::Relaxed),
            total_requests: self.total_requests.load(Ordering::Relaxed),
            bytes_in: self.bytes_in.load(Ordering::Relaxed),
            bytes_out: self.bytes_out.load(Ordering::Relaxed),
            last_seen_ms: self.last_seen_ms.load(Ordering::Relaxed),
            top_targets,
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn format_host_port(host: &str, port: u16) -> String {
    if host.starts_with('[') && host.contains(']') {
        format!("{host}:{port}")
    } else if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}
