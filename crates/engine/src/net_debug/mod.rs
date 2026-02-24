use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    Tcp,
    Udp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Client,
    Server,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IpVersion {
    Ipv4,
    Ipv6,
}

#[derive(Clone, Debug)]
pub struct DebugConfig {
    pub protocol: Protocol,
    pub role: Role,
    pub ip_version: IpVersion,
    pub host: String,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
    pub buffer_size: usize,
    pub send_plan: Option<SendPlan>,
}

#[derive(Clone, Debug)]
pub enum DebugEvent {
    Status(String),
    Sent(String),
    Received {
        content: String,
        source: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub struct SendRequest {
    pub data: Vec<u8>,
    pub target: Option<SocketAddr>,
}

#[derive(Clone, Debug)]
pub struct SendPlan {
    pub payloads: Vec<Vec<u8>>,
    pub interval: Duration,
    pub batch_size: usize,
    pub repeat: bool,
    pub max_speed: bool,
}

pub struct DebugHandle {
    sender: mpsc::Sender<SendRequest>,
    stop_tx: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
    send_task: Option<JoinHandle<()>>,
    stop_flag: Arc<AtomicBool>,
    default_target: Option<SocketAddr>,
}

impl DebugHandle {
    pub fn sender(&self) -> mpsc::Sender<SendRequest> {
        self.sender.clone()
    }

    pub async fn send(&self, data: Vec<u8>, target: Option<SocketAddr>) -> Result<(), String> {
        self.sender
            .send(SendRequest { data, target })
            .await
            .map_err(|e| format!("send data failed: {e}"))
    }

    pub fn start_auto_send(
        &mut self,
        plan: SendPlan,
        target: Option<SocketAddr>,
    ) -> Result<(), String> {
        if plan.payloads.is_empty() {
            return Err("payload is empty".into());
        }
        self.stop_auto_send();
        self.send_task = spawn_sender(
            Some(plan),
            self.sender.clone(),
            self.stop_flag.clone(),
            self.default_target,
            target,
        );
        Ok(())
    }

    pub fn stop_auto_send(&mut self) {
        if let Some(task) = self.send_task.take() {
            task.abort();
        }
    }

    pub fn stop(mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        self.task.abort();
        if let Some(task) = self.send_task.take() {
            task.abort();
        }
    }
}

pub struct DebugSession {
    pub events: mpsc::Receiver<DebugEvent>,
    pub handle: DebugHandle,
}

pub fn start(config: DebugConfig) -> DebugSession {
    let (event_tx, event_rx) = mpsc::channel::<DebugEvent>(128);
    let (send_tx, mut send_rx) = mpsc::channel::<SendRequest>(32);
    let (stop_tx, mut stop_rx) = oneshot::channel::<()>();
    let stop_flag = Arc::new(AtomicBool::new(false));
    let buffer_size = config.buffer_size.max(1);
    let default_target = default_target_addr(&config);
    let send_task = spawn_sender(
        config.send_plan.clone(),
        send_tx.clone(),
        stop_flag.clone(),
        default_target,
        None,
    );

    let task = tokio::spawn(async move {
        async fn emit(event_tx: &mpsc::Sender<DebugEvent>, event: DebugEvent) {
            let _ = event_tx.send(event).await;
        }

        match (config.protocol, config.role) {
            (Protocol::Tcp, Role::Client) => {
                let remote_addr = format_host_port(&config.remote_host, config.remote_port);
                match TcpStream::connect(&remote_addr).await {
                    Ok(stream) => {
                        let peer_addr = stream
                            .peer_addr()
                            .map(|addr| addr.to_string())
                            .unwrap_or_else(|_| remote_addr);
                        emit(
                            &event_tx,
                            DebugEvent::Status(format!("成功连接到 {peer_addr}")),
                        )
                        .await;

                        let (mut reader, mut writer) = stream.into_split();
                        loop {
                            let mut buf = vec![0; buffer_size];
                            tokio::select! {
                                _ = &mut stop_rx => break,
                                Some(request) = send_rx.recv() => {
                                    if writer.write_all(&request.data).await.is_err() {
                                        break;
                                    }
                                    emit(
                                        &event_tx,
                                        DebugEvent::Sent(String::from_utf8_lossy(&request.data).to_string()),
                                    ).await;
                                }
                                result = reader.read(&mut buf) => {
                                    match result {
                                        Ok(n) if n > 0 => {
                                            emit(
                                                &event_tx,
                                                DebugEvent::Received {
                                                    content: String::from_utf8_lossy(&buf[..n]).to_string(),
                                                    source: Some(peer_addr.clone()),
                                                },
                                            ).await;
                                        }
                                        _ => break,
                                    }
                                }
                            }
                        }
                        emit(&event_tx, DebugEvent::Status("连接已断开".into())).await;
                    }
                    Err(e) => {
                        emit(&event_tx, DebugEvent::Status(format!("连接失败: {e}"))).await;
                    }
                }
            }
            (Protocol::Tcp, Role::Server) => {
                let local_addr = format_host_port(&config.host, config.local_port);
                match TcpListener::bind(&local_addr).await {
                    Ok(listener) => {
                        emit(
                            &event_tx,
                            DebugEvent::Status(format!("正在监听 {local_addr}")),
                        )
                        .await;
                        'accept: loop {
                            tokio::select! {
                                _ = &mut stop_rx => break 'accept,
                                accepted = listener.accept() => {
                                    match accepted {
                                        Ok((socket, addr)) => {
                                            let peer_addr = addr.to_string();
                                            emit(
                                                &event_tx,
                                                DebugEvent::Status(format!("接受来自 {peer_addr} 的连接")),
                                            )
                                            .await;
                                            let (mut reader, mut writer) = socket.into_split();
                                            loop {
                                                let mut buf = vec![0; buffer_size];
                                                tokio::select! {
                                                    _ = &mut stop_rx => break 'accept,
                                                    Some(request) = send_rx.recv() => {
                                                        if writer.write_all(&request.data).await.is_err() { break; }
                                                        emit(
                                                            &event_tx,
                                                            DebugEvent::Sent(String::from_utf8_lossy(&request.data).to_string()),
                                                        ).await;
                                                    }
                                                    result = reader.read(&mut buf) => {
                                                        match result {
                                                            Ok(n) if n > 0 => {
                                                                emit(
                                                                    &event_tx,
                                                                    DebugEvent::Received {
                                                                        content: String::from_utf8_lossy(&buf[..n]).to_string(),
                                                                        source: Some(peer_addr.clone()),
                                                                    },
                                                                ).await;
                                                            }
                                                            _ => break,
                                                        }
                                                    }
                                                }
                                            }
                                            emit(
                                                &event_tx,
                                                DebugEvent::Status(format!(
                                                    "与 {peer_addr} 的连接已断开"
                                                )),
                                            )
                                            .await;
                                        }
                                        Err(e) => {
                                            emit(
                                                &event_tx,
                                                DebugEvent::Status(format!("接受连接失败: {e}")),
                                            )
                                            .await;
                                            break 'accept;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        emit(&event_tx, DebugEvent::Status(format!("监听失败: {e}"))).await;
                    }
                }
            }
            (Protocol::Udp, _) => {
                let local_addr = match config.role {
                    Role::Server => format_host_port(&config.host, config.local_port),
                    Role::Client => match (config.local_port, config.ip_version) {
                        (0, IpVersion::Ipv6) => "[::]:0".to_string(),
                        (0, IpVersion::Ipv4) => "0.0.0.0:0".to_string(),
                        (_, IpVersion::Ipv6) => format!("[::]:{}", config.local_port),
                        (_, IpVersion::Ipv4) => format!("0.0.0.0:{}", config.local_port),
                    },
                };

                match UdpSocket::bind(&local_addr).await {
                    Ok(socket) => {
                        let bound_addr = socket
                            .local_addr()
                            .map(|addr| addr.to_string())
                            .unwrap_or(local_addr);
                        emit(
                            &event_tx,
                            DebugEvent::Status(format!("UDP Socket 已绑定到 {bound_addr}")),
                        )
                        .await;

                        let socket = Arc::new(socket);
                        let mut last_client_addr: Option<SocketAddr> = None;

                        loop {
                            let mut buf = vec![0; buffer_size];
                            tokio::select! {
                                _ = &mut stop_rx => break,
                                Some(request) = send_rx.recv() => {
                                    let target = request.target.or(last_client_addr);
                                    if let Some(target_addr) = target {
                                        if socket.send_to(&request.data, target_addr).await.is_err() { break; }
                                        emit(
                                            &event_tx,
                                            DebugEvent::Sent(String::from_utf8_lossy(&request.data).to_string()),
                                        ).await;
                                    } else {
                                        emit(
                                            &event_tx,
                                            DebugEvent::Status("发送失败：无目标地址".into()),
                                        ).await;
                                    }
                                }
                                result = socket.recv_from(&mut buf) => {
                                    if let Ok((len, src)) = result {
                                        last_client_addr = Some(src);
                                        emit(
                                            &event_tx,
                                            DebugEvent::Received {
                                                content: String::from_utf8_lossy(&buf[..len]).to_string(),
                                                source: Some(src.to_string()),
                                            },
                                        ).await;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        emit(
                            &event_tx,
                            DebugEvent::Status(format!("绑定 UDP Socket 失败: {e}")),
                        )
                        .await;
                    }
                }
            }
        }
    });

    DebugSession {
        events: event_rx,
        handle: DebugHandle {
            sender: send_tx,
            stop_tx: Some(stop_tx),
            task,
            send_task,
            stop_flag,
            default_target,
        },
    }
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

fn default_target_addr(config: &DebugConfig) -> Option<SocketAddr> {
    if config.protocol != Protocol::Udp || config.role != Role::Client {
        return None;
    }
    let addr = format_host_port(&config.remote_host, config.remote_port);
    addr.parse().ok()
}

fn spawn_sender(
    plan: Option<SendPlan>,
    sender: mpsc::Sender<SendRequest>,
    stop_flag: Arc<AtomicBool>,
    default_target: Option<SocketAddr>,
    override_target: Option<SocketAddr>,
) -> Option<JoinHandle<()>> {
    let plan = plan?;
    if plan.payloads.is_empty() {
        return None;
    }

    Some(tokio::spawn(async move {
        let mut index = 0usize;
        let repeat = plan.repeat || plan.max_speed;
        let batch_size = if plan.max_speed {
            1
        } else {
            plan.batch_size.max(1)
        };
        let interval = plan.interval;

        loop {
            if stop_flag.load(Ordering::Relaxed) {
                break;
            }
            for _ in 0..batch_size {
                if stop_flag.load(Ordering::Relaxed) {
                    return;
                }
                let payload = plan.payloads[index].clone();
                index = (index + 1) % plan.payloads.len();
                let target = override_target.or(default_target);
                if sender
                    .send(SendRequest {
                        data: payload,
                        target,
                    })
                    .await
                    .is_err()
                {
                    return;
                }
            }

            if !repeat {
                break;
            }
            if !plan.max_speed {
                tokio::time::sleep(interval).await;
            }
        }
    }))
}
