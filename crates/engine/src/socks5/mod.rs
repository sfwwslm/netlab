use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

#[derive(Clone, Debug)]
pub struct Socks5Config {
    pub listen_host: String,
    pub listen_port: u16,
    pub report_interval: Duration,
    pub client_idle_ttl: Duration,
    pub enable_udp: bool,
}

#[derive(Clone, Debug)]
pub enum Socks5Event {
    Status(String),
    Snapshot(Socks5Snapshot),
}

#[derive(Clone, Debug)]
pub struct Socks5Snapshot {
    pub uptime: Duration,
    pub active_connections: u64,
    pub total_connections: u64,
    pub total_requests: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub clients: Vec<Socks5ClientSnapshot>,
}

#[derive(Clone, Debug)]
pub struct Socks5ClientSnapshot {
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

pub struct Socks5Session {
    pub events: mpsc::Receiver<Socks5Event>,
    pub handle: Socks5Handle,
}

pub struct Socks5Handle {
    stop_tx: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
    report_task: JoinHandle<()>,
    udp_task: Option<JoinHandle<()>>,
    stop_flag: Arc<AtomicBool>,
}

impl Socks5Handle {
    pub fn stop(mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        self.task.abort();
        self.report_task.abort();
        if let Some(task) = self.udp_task.take() {
            task.abort();
        }
    }
}

pub fn start(config: Socks5Config) -> Socks5Session {
    let (event_tx, event_rx) = mpsc::channel::<Socks5Event>(256);
    let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stats = Arc::new(Socks5Stats::new(config.client_idle_ttl));
    let listen_addr = format_host_port(&config.listen_host, config.listen_port);

    let udp_state = if config.enable_udp {
        Some(Arc::new(Mutex::new(UdpRelayState::default())))
    } else {
        None
    };

    let udp_task = if config.enable_udp {
        let event_tx_clone = event_tx.clone();
        let stats = stats.clone();
        let udp_state = udp_state.clone();
        let udp_listen_addr = listen_addr.clone();
        let stop_flag = stop_flag.clone();
        tokio::spawn(async move {
            run_udp_relay(udp_listen_addr, stats, udp_state, event_tx_clone, stop_flag).await;
        })
        .into()
    } else {
        None
    };

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
                .send(Socks5Event::Snapshot(snapshot))
                .await;
        }
    });

    let task = tokio::spawn(async move {
        let listener = match TcpListener::bind(&listen_addr).await {
            Ok(listener) => listener,
            Err(err) => {
                let _ = event_tx
                    .send(Socks5Event::Status(format!("socks5 listen failed: {err}")))
                    .await;
                return;
            }
        };

        let _ = event_tx
            .send(Socks5Event::Status(format!(
                "socks5 listening on {listen_addr}"
            )))
            .await;

        loop {
            tokio::select! {
                _ = &mut stop_rx => {
                    let _ = event_tx.send(Socks5Event::Status("socks5 stopped".into())).await;
                    break;
                }
                accepted = listener.accept() => {
                    match accepted {
                        Ok((stream, addr)) => {
                            let stats = stats.clone();
                            let event_tx = event_tx.clone();
                            let udp_state = udp_state.clone();
                            tokio::spawn(async move {
                                handle_tcp_client(stream, addr, stats, udp_state, event_tx).await;
                            });
                        }
                        Err(err) => {
                            let _ = event_tx
                                .send(Socks5Event::Status(format!(
                                    "socks5 accept failed: {err}"
                                )))
                                .await;
                            break;
                        }
                    }
                }
            }
        }
    });

    Socks5Session {
        events: event_rx,
        handle: Socks5Handle {
            stop_tx: Some(stop_tx),
            task,
            report_task,
            udp_task,
            stop_flag,
        },
    }
}

async fn handle_tcp_client(
    mut stream: TcpStream,
    addr: SocketAddr,
    stats: Arc<Socks5Stats>,
    udp_state: Option<Arc<Mutex<UdpRelayState>>>,
    event_tx: mpsc::Sender<Socks5Event>,
) {
    let ip = addr.ip().to_string();
    let client_stats = stats.get_or_create_client(&ip);
    stats.on_connect(&client_stats);
    let _ = event_tx
        .send(Socks5Event::Status(format!(
            "socks5 client connected: {ip}"
        )))
        .await;

    let result = handle_socks5(&mut stream, &ip, &stats, &client_stats, udp_state).await;
    if let Err(err) = result {
        let _ = event_tx
            .send(Socks5Event::Status(format!(
                "socks5 client error for {ip}: {err}"
            )))
            .await;
    }

    stats.on_disconnect(&client_stats);
    let _ = event_tx
        .send(Socks5Event::Status(format!(
            "socks5 client disconnected: {ip}"
        )))
        .await;
}

async fn handle_socks5(
    stream: &mut TcpStream,
    ip: &str,
    stats: &Arc<Socks5Stats>,
    client_stats: &Arc<Socks5ClientStats>,
    udp_state: Option<Arc<Mutex<UdpRelayState>>>,
) -> Result<(), String> {
    let mut header = [0u8; 2];
    stream
        .read_exact(&mut header)
        .await
        .map_err(|e| format!("read handshake failed: {e}"))?;
    if header[0] != 0x05 {
        return Err("unsupported socks version".into());
    }
    let method_len = header[1] as usize;
    let mut methods = vec![0u8; method_len];
    stream
        .read_exact(&mut methods)
        .await
        .map_err(|e| format!("read methods failed: {e}"))?;
    let supports_no_auth = methods.contains(&0x00);
    let response = if supports_no_auth { 0x00 } else { 0xFF };
    stream
        .write_all(&[0x05, response])
        .await
        .map_err(|e| format!("write method selection failed: {e}"))?;
    if response == 0xFF {
        return Err("no supported auth method".into());
    }

    let mut req = [0u8; 4];
    stream
        .read_exact(&mut req)
        .await
        .map_err(|e| format!("read request failed: {e}"))?;
    if req[0] != 0x05 {
        return Err("invalid request version".into());
    }
    let cmd = req[1];
    let atyp = req[3];
    let target = read_address(stream, atyp).await?;
    stats.on_request(client_stats);
    client_stats.record_target(&target);

    match cmd {
        0x01 => {
            let target_addr = resolve_target(&target).await?;
            let mut upstream = TcpStream::connect(target_addr)
                .await
                .map_err(|e| format!("connect upstream failed: {e}"))?;
            let local_addr = upstream
                .local_addr()
                .unwrap_or(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0));
            write_reply(stream, 0x00, local_addr).await?;

            let (from_client, from_server) = tokio::io::copy_bidirectional(stream, &mut upstream)
                .await
                .map_err(|e| format!("tunnel copy failed: {e}"))?;
            stats.add_bytes(Direction::In, from_client, client_stats);
            stats.add_bytes(Direction::Out, from_server, client_stats);
        }
        0x03 => {
            if udp_state.is_none() {
                write_reply(
                    stream,
                    0x07,
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
                )
                .await?;
                return Err("udp associate not enabled".into());
            }
            let bind_addr = udp_state
                .as_ref()
                .and_then(|state| state.lock().unwrap().bind_addr)
                .unwrap_or(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0));
            write_reply(stream, 0x00, bind_addr).await?;
            if let Some(state) = udp_state {
                let mut guard = state.lock().unwrap();
                guard.register_client(ip);
            }
            let mut buf = [0u8; 1];
            loop {
                let read = stream.read(&mut buf).await;
                match read {
                    Ok(0) => break,
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
        }
        _ => {
            write_reply(
                stream,
                0x07,
                SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            )
            .await?;
            return Err("unsupported command".into());
        }
    }

    Ok(())
}

async fn read_address(stream: &mut TcpStream, atyp: u8) -> Result<String, String> {
    match atyp {
        0x01 => {
            let mut addr = [0u8; 4];
            stream
                .read_exact(&mut addr)
                .await
                .map_err(|e| format!("read ipv4 failed: {e}"))?;
            let mut port_buf = [0u8; 2];
            stream
                .read_exact(&mut port_buf)
                .await
                .map_err(|e| format!("read port failed: {e}"))?;
            let port = u16::from_be_bytes(port_buf);
            Ok(format!(
                "{}.{}.{}.{}:{}",
                addr[0], addr[1], addr[2], addr[3], port
            ))
        }
        0x03 => {
            let mut len = [0u8; 1];
            stream
                .read_exact(&mut len)
                .await
                .map_err(|e| format!("read domain length failed: {e}"))?;
            let mut domain = vec![0u8; len[0] as usize];
            stream
                .read_exact(&mut domain)
                .await
                .map_err(|e| format!("read domain failed: {e}"))?;
            let mut port_buf = [0u8; 2];
            stream
                .read_exact(&mut port_buf)
                .await
                .map_err(|e| format!("read port failed: {e}"))?;
            let port = u16::from_be_bytes(port_buf);
            let host = String::from_utf8_lossy(&domain).to_string();
            Ok(format!("{host}:{port}"))
        }
        0x04 => {
            let mut addr = [0u8; 16];
            stream
                .read_exact(&mut addr)
                .await
                .map_err(|e| format!("read ipv6 failed: {e}"))?;
            let mut port_buf = [0u8; 2];
            stream
                .read_exact(&mut port_buf)
                .await
                .map_err(|e| format!("read port failed: {e}"))?;
            let port = u16::from_be_bytes(port_buf);
            let ip = Ipv6Addr::from(addr);
            Ok(format!("[{ip}]:{port}"))
        }
        _ => Err("unsupported address type".into()),
    }
}

async fn resolve_target(target: &str) -> Result<SocketAddr, String> {
    tokio::net::lookup_host(target)
        .await
        .map_err(|e| format!("resolve failed: {e}"))?
        .next()
        .ok_or_else(|| "no resolved address".to_string())
}

async fn write_reply(stream: &mut TcpStream, rep: u8, addr: SocketAddr) -> Result<(), String> {
    let mut reply = Vec::with_capacity(22);
    reply.push(0x05);
    reply.push(rep);
    reply.push(0x00);
    match addr.ip() {
        IpAddr::V4(ipv4) => {
            reply.push(0x01);
            reply.extend_from_slice(&ipv4.octets());
        }
        IpAddr::V6(ipv6) => {
            reply.push(0x04);
            reply.extend_from_slice(&ipv6.octets());
        }
    }
    reply.extend_from_slice(&addr.port().to_be_bytes());
    stream
        .write_all(&reply)
        .await
        .map_err(|e| format!("write reply failed: {e}"))
}

async fn run_udp_relay(
    listen_addr: String,
    stats: Arc<Socks5Stats>,
    state: Option<Arc<Mutex<UdpRelayState>>>,
    event_tx: mpsc::Sender<Socks5Event>,
    stop_flag: Arc<AtomicBool>,
) {
    if state.is_none() {
        return;
    }
    let state = state.unwrap();
    let socket = match UdpSocket::bind(&listen_addr).await {
        Ok(socket) => {
            {
                let mut guard = state.lock().unwrap();
                guard.bind_addr = socket.local_addr().ok();
            }
            socket
        }
        Err(err) => {
            let _ = event_tx
                .send(Socks5Event::Status(format!(
                    "socks5 udp bind failed: {err}"
                )))
                .await;
            return;
        }
    };
    let mut buf = vec![0u8; 65535];
    let mut tick = tokio::time::interval(Duration::from_millis(200));

    loop {
        tokio::select! {
            _ = tick.tick() => {
                if stop_flag.load(Ordering::Relaxed) {
                    break;
                }
                continue;
            }
            result = socket.recv_from(&mut buf) => {
                let (len, src) = match result {
                    Ok(value) => value,
                    Err(err) => {
                        let _ = event_tx
                            .send(Socks5Event::Status(format!(
                                "socks5 udp recv failed: {err}"
                            )))
                            .await;
                        break;
                    }
                };
        if len < 4 {
            continue;
        }

        let payload = &buf[..len];
        let is_client = {
            let mut guard = state.lock().unwrap();
            guard.is_client_addr(&src)
        };

        if is_client {
            if let Ok((target, data)) = parse_udp_datagram(payload).await {
                {
                    let mut guard = state.lock().unwrap();
                    guard.record_target_mapping(src, target);
                }
                if let Some(client) = stats.get_client_by_ip(&src.ip().to_string()) {
                    stats.on_request(&client);
                    stats.add_bytes(Direction::In, data.len() as u64, &client);
                    client.record_target(&format!("{target}"));
                }
                let _ = socket.send_to(data, target).await;
            }
        } else {
            let client_addr = {
                let guard = state.lock().unwrap();
                guard.client_for_target(&src)
            };
            if let Some(client_addr) = client_addr {
                let packet = build_udp_datagram(src, payload);
                let _ = socket.send_to(&packet, client_addr).await;
                if let Some(client) = stats.get_client_by_ip(&client_addr.ip().to_string()) {
                    stats.on_request(&client);
                    stats.add_bytes(Direction::Out, payload.len() as u64, &client);
                }
            }
        }
            }
        }
    }
}

async fn parse_udp_datagram(data: &[u8]) -> Result<(SocketAddr, &[u8]), String> {
    if data.len() < 4 {
        return Err("udp packet too short".into());
    }
    if data[0] != 0 || data[1] != 0 {
        return Err("invalid udp header".into());
    }
    if data[2] != 0 {
        return Err("udp fragmentation not supported".into());
    }
    let atyp = data[3];
    let mut idx = 4;
    let target = match atyp {
        0x01 => {
            if data.len() < idx + 4 + 2 {
                return Err("udp ipv4 too short".into());
            }
            let addr = Ipv4Addr::new(data[idx], data[idx + 1], data[idx + 2], data[idx + 3]);
            idx += 4;
            let port = u16::from_be_bytes([data[idx], data[idx + 1]]);
            idx += 2;
            SocketAddr::new(IpAddr::V4(addr), port)
        }
        0x03 => {
            if data.len() < idx + 1 {
                return Err("udp domain too short".into());
            }
            let len = data[idx] as usize;
            idx += 1;
            if data.len() < idx + len + 2 {
                return Err("udp domain too short".into());
            }
            let host = String::from_utf8_lossy(&data[idx..idx + len]).to_string();
            idx += len;
            let port = u16::from_be_bytes([data[idx], data[idx + 1]]);
            idx += 2;
            tokio::net::lookup_host(format!("{host}:{port}"))
                .await
                .map_err(|e| format!("udp resolve failed: {e}"))?
                .next()
                .ok_or_else(|| "udp resolve empty".to_string())?
        }
        0x04 => {
            if data.len() < idx + 16 + 2 {
                return Err("udp ipv6 too short".into());
            }
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(&data[idx..idx + 16]);
            idx += 16;
            let port = u16::from_be_bytes([data[idx], data[idx + 1]]);
            idx += 2;
            SocketAddr::new(IpAddr::V6(Ipv6Addr::from(bytes)), port)
        }
        _ => return Err("udp unsupported address type".into()),
    };
    Ok((target, &data[idx..]))
}

fn build_udp_datagram(target: SocketAddr, data: &[u8]) -> Vec<u8> {
    let mut packet = Vec::with_capacity(data.len() + 32);
    packet.extend_from_slice(&[0x00, 0x00, 0x00]);
    match target.ip() {
        IpAddr::V4(ipv4) => {
            packet.push(0x01);
            packet.extend_from_slice(&ipv4.octets());
        }
        IpAddr::V6(ipv6) => {
            packet.push(0x04);
            packet.extend_from_slice(&ipv6.octets());
        }
    }
    packet.extend_from_slice(&target.port().to_be_bytes());
    packet.extend_from_slice(data);
    packet
}

#[derive(Default)]
struct UdpRelayState {
    client_by_ip: HashMap<String, SocketAddr>,
    target_to_client: HashMap<SocketAddr, SocketAddr>,
    bind_addr: Option<SocketAddr>,
}

impl UdpRelayState {
    fn register_client(&mut self, ip: &str) {
        self.client_by_ip
            .entry(ip.to_string())
            .or_insert(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0));
    }

    fn is_client_addr(&mut self, addr: &SocketAddr) -> bool {
        if let Some(existing) = self.client_by_ip.get(&addr.ip().to_string()) {
            if existing.port() == 0 {
                self.client_by_ip.insert(addr.ip().to_string(), *addr);
                return true;
            }
            return *existing == *addr;
        }
        false
    }

    fn record_target_mapping(&mut self, client: SocketAddr, target: SocketAddr) {
        self.client_by_ip.insert(client.ip().to_string(), client);
        self.target_to_client.insert(target, client);
    }

    fn client_for_target(&self, target: &SocketAddr) -> Option<SocketAddr> {
        self.target_to_client.get(target).copied()
    }
}

#[derive(Clone, Copy)]
enum Direction {
    In,
    Out,
}

struct Socks5Stats {
    start: Instant,
    active_connections: AtomicU64,
    total_connections: AtomicU64,
    total_requests: AtomicU64,
    bytes_in: AtomicU64,
    bytes_out: AtomicU64,
    clients: Mutex<HashMap<String, Arc<Socks5ClientStats>>>,
    client_idle_ttl_ms: u64,
}

impl Socks5Stats {
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

    fn get_or_create_client(&self, ip: &str) -> Arc<Socks5ClientStats> {
        let mut guard = self.clients.lock().unwrap();
        if let Some(entry) = guard.get(ip) {
            return entry.clone();
        }
        let client = Arc::new(Socks5ClientStats::new(ip.to_string()));
        guard.insert(ip.to_string(), client.clone());
        client
    }

    fn get_client_by_ip(&self, ip: &str) -> Option<Arc<Socks5ClientStats>> {
        let guard = self.clients.lock().unwrap();
        guard.get(ip).cloned()
    }

    fn on_connect(&self, client: &Arc<Socks5ClientStats>) {
        self.total_connections.fetch_add(1, Ordering::Relaxed);
        self.active_connections.fetch_add(1, Ordering::Relaxed);
        client.total_connections.fetch_add(1, Ordering::Relaxed);
        client.active_connections.fetch_add(1, Ordering::Relaxed);
        client.touch();
    }

    fn on_disconnect(&self, client: &Arc<Socks5ClientStats>) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
        client.active_connections.fetch_sub(1, Ordering::Relaxed);
        client.touch();
    }

    fn on_request(&self, client: &Arc<Socks5ClientStats>) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        client.total_requests.fetch_add(1, Ordering::Relaxed);
        client.touch();
    }

    fn add_bytes(&self, direction: Direction, amount: u64, client: &Arc<Socks5ClientStats>) {
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

    fn snapshot(&self) -> Socks5Snapshot {
        let now = now_ms();
        let ttl_ms = self.client_idle_ttl_ms;
        let clients = {
            let mut guard = self.clients.lock().unwrap();
            if ttl_ms > 0 {
                guard.retain(|_, client| {
                    now.saturating_sub(client.last_seen_ms.load(Ordering::Relaxed)) <= ttl_ms
                });
            }
            let mut entries: Vec<Socks5ClientSnapshot> =
                guard.values().map(|client| client.snapshot()).collect();
            entries.sort_by_key(|c| std::cmp::Reverse(c.total_requests));
            entries
        };

        Socks5Snapshot {
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

struct Socks5ClientStats {
    ip: String,
    active_connections: AtomicU64,
    total_connections: AtomicU64,
    total_requests: AtomicU64,
    bytes_in: AtomicU64,
    bytes_out: AtomicU64,
    last_seen_ms: AtomicU64,
    targets: Mutex<HashMap<String, u64>>,
}

impl Socks5ClientStats {
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

    fn snapshot(&self) -> Socks5ClientSnapshot {
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

        Socks5ClientSnapshot {
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
