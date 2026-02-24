use std::collections::VecDeque;
use std::error::Error;
use std::fs;
use std::io::{self, Stdout};
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::{Parser, ValueEnum};
use crossterm::{
    event::{self, Event as CEvent, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use engine::conn::ConnectionMode;
use engine::metrics::{MetricsSnapshot, StatusCodeStat};
use engine::net_debug::{
    self, DebugConfig as NetDebugConfig, DebugEvent as NetDebugEvent, IpVersion, Protocol, Role,
    SendPlan,
};
use engine::proxy::{
    self, ClientSnapshot as ProxyClientSnapshot, ProxyConfig, ProxyEvent, ProxySnapshot,
};
use engine::scenario::load_test::{
    LoadTestConfig, LoadTestEvent, MetricsMode, ResponseMode, spawn,
};
use engine::scheduler::RpsMode;
use engine::socks5::{self, Socks5ClientSnapshot, Socks5Config, Socks5Event, Socks5Snapshot};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Table, Wrap},
};
use shadow_rs::{formatcp, shadow};
use tokio::sync::mpsc;

shadow!(build);

const VERSION_INFO: &str = formatcp!(
    r#"{}
commit_hash: {}
build_time: {}
build_env: {},{}"#,
    build::PKG_VERSION,
    build::SHORT_COMMIT,
    build::BUILD_TIME,
    build::RUST_VERSION,
    build::RUST_CHANNEL
);

#[derive(Parser, Debug)]
#[command(name = "netlab-cli", version = VERSION_INFO, about = "netlab CLI load tester (ratatui + clap)")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    Bench(RunArgs),
    #[command(name = "net-debug")]
    NetDebug(NetDebugArgs),
    Proxy(ProxyArgs),
    Socks5(Socks5Args),
}

#[derive(Parser, Debug)]
struct RunArgs {
    /// Target URL (http/https)
    url: String,
    /// HTTP method
    #[arg(long, default_value = "GET")]
    method: String,
    /// Concurrent worker count
    #[arg(long, default_value_t = 30)]
    concurrency: usize,
    /// Ramp-up duration (seconds)
    #[arg(long, default_value_t = 0)]
    ramp_up: u64,
    /// Max iterations per worker
    #[arg(long)]
    iterations_per_worker: Option<u64>,
    /// Total request limit
    #[arg(long)]
    total_requests_limit: Option<u64>,
    /// Total duration (seconds)
    #[arg(long, default_value_t = 10)]
    duration: u64,
    /// Per-request timeout (ms)
    #[arg(long, default_value_t = 10000)]
    timeout_ms: u64,
    /// Request body (string)
    #[arg(long)]
    body: Option<String>,
    /// Extra headers (repeatable), format: Key:Value
    #[arg(short = 'H', long = "header")]
    headers: Vec<String>,
    /// Response handling mode
    #[arg(long, value_enum, default_value_t = ResponseModeArg::CountBytes)]
    response_mode: ResponseModeArg,
    /// Metrics mode
    #[arg(long, value_enum, default_value_t = MetricsModeArg::Full)]
    metrics_mode: MetricsModeArg,
    /// Connection mode
    #[arg(long, value_enum, default_value_t = ConnectionModeArg::KeepAlive)]
    connection_mode: ConnectionModeArg,
    /// RPS limit (global or per worker)
    #[arg(long)]
    rps: Option<f64>,
    /// RPS scheduling mode
    #[arg(long, value_enum, default_value_t = RpsModeArg::Global)]
    rps_mode: RpsModeArg,
    /// Allow insecure certs (https only)
    #[arg(long)]
    allow_insecure_certs: bool,
    /// Metrics report interval (ms)
    #[arg(long, default_value_t = 500)]
    report_interval_ms: u64,
    /// Max latency samples to keep
    #[arg(long, default_value_t = 10_000)]
    max_latency_samples: usize,
}

#[derive(Parser, Debug)]
struct NetDebugArgs {
    /// Destination host or IP (client mode only)
    #[arg(value_name = "DESTINATION")]
    destination: Option<String>,
    /// Run as server and listen on a local port
    #[arg(short = 's', long = "server")]
    server: bool,
    /// Use TCP (default: UDP)
    #[arg(long = "tcp")]
    tcp: bool,
    /// Use IPv6
    #[arg(short = '6', long = "ipv6")]
    ipv6: bool,
    /// Listen or target port
    #[arg(short = 'p', long = "port", default_value_t = 8080)]
    port: u16,
    /// Buffer size (bytes)
    #[arg(long = "buf", default_value_t = 2048)]
    buffer_size: usize,
    /// Quiet mode (suppress sent logs)
    #[arg(short = 'q', long = "quiet")]
    quiet: bool,
    /// Repeat sending until exit
    #[arg(short = 'r', long = "repeat")]
    repeat: bool,
    /// Ignore interval and batch size, send as fast as possible
    #[arg(long = "max-speed")]
    max_speed: bool,
    /// Batch size per interval
    #[arg(long = "batch-size", default_value_t = 1)]
    batch_size: usize,
    /// Inline data to send
    #[arg(long = "data")]
    data: Option<String>,
    /// Load payloads from file or directory
    #[arg(long = "input")]
    input: Option<String>,
    /// Interval between batches (ms)
    #[arg(short = 'i', long = "interval", default_value_t = 1000)]
    interval_ms: u64,
}

#[derive(Parser, Debug)]
struct ProxyArgs {
    /// Listen host
    #[arg(long, default_value = "0.0.0.0")]
    listen_host: String,
    /// Listen port
    #[arg(short = 'p', long = "port", default_value_t = 8899)]
    listen_port: u16,
    /// Stats report interval (ms)
    #[arg(long, default_value_t = 500)]
    report_interval_ms: u64,
}

#[derive(Parser, Debug)]
struct Socks5Args {
    /// Listen host
    #[arg(long, default_value = "0.0.0.0")]
    listen_host: String,
    /// Listen port
    #[arg(short = 'p', long = "port", default_value_t = 1080)]
    listen_port: u16,
    /// Enable UDP relay (UDP ASSOCIATE)
    #[arg(long = "udp")]
    enable_udp: bool,
    /// Stats report interval (ms)
    #[arg(long, default_value_t = 500)]
    report_interval_ms: u64,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum ResponseModeArg {
    CountBytes,
    DiscardBody,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum MetricsModeArg {
    Full,
    Minimal,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum ConnectionModeArg {
    KeepAlive,
    NewConnection,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum RpsModeArg {
    Global,
    PerWorker,
}

#[derive(Debug)]
enum InputEvent {
    Key(KeyEvent),
}

#[derive(Default)]
struct AppState {
    snapshot: Option<MetricsSnapshot>,
    logs: VecDeque<String>,
    status: String,
    done: bool,
    target: String,
    method: String,
    concurrency: usize,
    duration: Duration,
    timeout: Duration,
}

#[derive(Default)]
struct NetDebugState {
    logs: VecDeque<String>,
    status: String,
    role: String,
    protocol: String,
    ip_version: String,
    local_addr: String,
    remote_addr: Option<String>,
    buffer_size: usize,
    interval: Duration,
    batch_size: usize,
    repeat: bool,
    max_speed: bool,
    quiet: bool,
}

#[derive(Default)]
struct ProxyState {
    logs: VecDeque<String>,
    status: String,
    listen_addr: String,
    snapshot: Option<ProxySnapshot>,
    status_history: VecDeque<String>,
    client_offset: usize,
    client_page_size: usize,
    peak_active: u64,
}

#[derive(Default)]
struct Socks5State {
    logs: VecDeque<String>,
    status: String,
    listen_addr: String,
    snapshot: Option<Socks5Snapshot>,
    status_history: VecDeque<String>,
    client_offset: usize,
    client_page_size: usize,
    peak_active: u64,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    match cli.command {
        Commands::Bench(args) => rt.block_on(run_tui(args)),
        Commands::NetDebug(args) => rt.block_on(run_net_debug_tui(args)),
        Commands::Proxy(args) => rt.block_on(run_proxy_tui(args)),
        Commands::Socks5(args) => rt.block_on(run_socks5_tui(args)),
    }
}

async fn run_tui(args: RunArgs) -> Result<(), Box<dyn Error>> {
    let config = build_config(&args)?;
    let run = spawn(config).map_err(io::Error::other)?;
    let engine::scenario::load_test::LoadTestRun {
        task,
        stop_flag,
        events,
    } = run;

    let (input_tx, mut input_rx) = mpsc::unbounded_channel::<InputEvent>();
    std::thread::spawn(move || {
        loop {
            if let Ok(true) = event::poll(Duration::from_millis(100))
                && let Ok(CEvent::Key(key)) = event::read()
                && input_tx.send(InputEvent::Key(key)).is_err()
            {
                break;
            }
        }
    });

    let mut terminal = setup_terminal()?;
    let mut state = AppState {
        target: args.url.clone(),
        method: args.method.clone(),
        concurrency: args.concurrency,
        duration: Duration::from_secs(args.duration.max(1)),
        timeout: Duration::from_millis(args.timeout_ms.max(100)),
        ..AppState::default()
    };
    state.status = "Running".to_string();
    let mut stop_requested_at: Option<Instant> = None;

    let result = run_app(
        &mut terminal,
        &mut state,
        events,
        &mut input_rx,
        stop_flag.clone(),
        &mut stop_requested_at,
    )
    .await;

    restore_terminal(&mut terminal)?;
    result?;

    stop_flag.store(true, Ordering::Relaxed);
    let _ = tokio::time::timeout(Duration::from_secs(2), task).await;

    Ok(())
}

async fn run_net_debug_tui(args: NetDebugArgs) -> Result<(), Box<dyn Error>> {
    let (config, mut state) = build_net_debug_config(&args)?;
    let session = net_debug::start(config);
    let mut events_rx = session.events;
    let handle = session.handle;

    let (input_tx, mut input_rx) = mpsc::unbounded_channel::<InputEvent>();
    std::thread::spawn(move || {
        loop {
            if let Ok(true) = event::poll(Duration::from_millis(100))
                && let Ok(CEvent::Key(key)) = event::read()
                && input_tx.send(InputEvent::Key(key)).is_err()
            {
                break;
            }
        }
    });

    let mut terminal = setup_terminal()?;
    state.status = "Running".to_string();

    let tick_rate = Duration::from_millis(200);
    let mut stopped = false;

    loop {
        terminal.draw(|f| draw_net_debug_ui(f, &state))?;

        tokio::select! {
            maybe_event = events_rx.recv() => {
                match maybe_event {
                    Some(event) => handle_net_debug_event(&mut state, event),
                    None => {
                        if !stopped {
                            state.status = "Stopped".to_string();
                            stopped = true;
                        }
                    }
                }
            }
            maybe_input = input_rx.recv() => {
                if let Some(InputEvent::Key(key)) = maybe_input
                    && handle_net_debug_key(&mut state, key)
                {
                    break;
                }
            }
            _ = tokio::time::sleep(tick_rate) => {}
        }
    }

    handle.stop();
    restore_terminal(&mut terminal)?;
    Ok(())
}

async fn run_proxy_tui(args: ProxyArgs) -> Result<(), Box<dyn Error>> {
    let config = ProxyConfig {
        listen_host: args.listen_host.clone(),
        listen_port: args.listen_port,
        report_interval: Duration::from_millis(args.report_interval_ms.max(100)),
        client_idle_ttl: Duration::from_secs(300),
    };
    let session = proxy::start(config);
    let mut events_rx = session.events;
    let handle = session.handle;

    let (input_tx, mut input_rx) = mpsc::unbounded_channel::<InputEvent>();
    std::thread::spawn(move || {
        loop {
            if let Ok(true) = event::poll(Duration::from_millis(100))
                && let Ok(CEvent::Key(key)) = event::read()
                && input_tx.send(InputEvent::Key(key)).is_err()
            {
                break;
            }
        }
    });

    let mut terminal = setup_terminal()?;
    let listen_addr = format_host_port(&args.listen_host, args.listen_port);
    let mut state = ProxyState {
        status: "Starting".to_string(),
        listen_addr,
        ..ProxyState::default()
    };

    let tick_rate = Duration::from_millis(200);
    let mut stopped = false;

    loop {
        let size = terminal.size()?;
        state.client_page_size = proxy_client_page_size(size);
        terminal.draw(|f| draw_proxy_ui(f, &state))?;

        tokio::select! {
            maybe_event = events_rx.recv() => {
                match maybe_event {
                    Some(event) => handle_proxy_event(&mut state, event),
                    None => {
                        if !stopped {
                            state.status = "Stopped".to_string();
                            stopped = true;
                        }
                    }
                }
            }
            maybe_input = input_rx.recv() => {
                if let Some(InputEvent::Key(key)) = maybe_input
                    && handle_proxy_key(&mut state, key)
                {
                    break;
                }
            }
            _ = tokio::time::sleep(tick_rate) => {}
        }
    }

    handle.stop();
    restore_terminal(&mut terminal)?;
    Ok(())
}

async fn run_socks5_tui(args: Socks5Args) -> Result<(), Box<dyn Error>> {
    let config = Socks5Config {
        listen_host: args.listen_host.clone(),
        listen_port: args.listen_port,
        report_interval: Duration::from_millis(args.report_interval_ms.max(100)),
        client_idle_ttl: Duration::from_secs(300),
        enable_udp: args.enable_udp,
    };
    let session = socks5::start(config);
    let mut events_rx = session.events;
    let handle = session.handle;

    let (input_tx, mut input_rx) = mpsc::unbounded_channel::<InputEvent>();
    std::thread::spawn(move || {
        loop {
            if let Ok(true) = event::poll(Duration::from_millis(100))
                && let Ok(CEvent::Key(key)) = event::read()
                && input_tx.send(InputEvent::Key(key)).is_err()
            {
                break;
            }
        }
    });

    let mut terminal = setup_terminal()?;
    let listen_addr = format_host_port(&args.listen_host, args.listen_port);
    let mut state = Socks5State {
        status: "Starting".to_string(),
        listen_addr,
        ..Socks5State::default()
    };

    let tick_rate = Duration::from_millis(200);
    let mut stopped = false;

    loop {
        let size = terminal.size()?;
        state.client_page_size = proxy_client_page_size(size);
        terminal.draw(|f| draw_socks5_ui(f, &state))?;

        tokio::select! {
            maybe_event = events_rx.recv() => {
                match maybe_event {
                    Some(event) => handle_socks5_event(&mut state, event),
                    None => {
                        if !stopped {
                            state.status = "Stopped".to_string();
                            stopped = true;
                        }
                    }
                }
            }
            maybe_input = input_rx.recv() => {
                if let Some(InputEvent::Key(key)) = maybe_input
                    && handle_socks5_key(&mut state, key)
                {
                    break;
                }
            }
            _ = tokio::time::sleep(tick_rate) => {}
        }
    }

    handle.stop();
    restore_terminal(&mut terminal)?;
    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    state: &mut AppState,
    mut events_rx: mpsc::UnboundedReceiver<LoadTestEvent>,
    input_rx: &mut mpsc::UnboundedReceiver<InputEvent>,
    stop_flag: Arc<AtomicBool>,
    stop_requested_at: &mut Option<Instant>,
) -> Result<(), Box<dyn Error>> {
    let tick_rate = Duration::from_millis(200);
    let mut events_closed = false;

    loop {
        terminal.draw(|f| draw_ui(f, state))?;

        tokio::select! {
            maybe_event = events_rx.recv() => {
                match maybe_event {
                    Some(LoadTestEvent::Metrics(snapshot)) => {
                        state.done = snapshot.done;
                        state.snapshot = Some(*snapshot);
                        if state.done {
                            state.status = "Done".to_string();
                        }
                    }
                    Some(LoadTestEvent::Log(message)) => {
                        push_log(state, message);
                    }
                    None => {
                        events_closed = true;
                        if state.done {
                            state.status = "Done".to_string();
                        } else {
                            state.status = "Stopped".to_string();
                        }
                    }
                }
            }
            maybe_input = input_rx.recv() => {
                if let Some(InputEvent::Key(key)) = maybe_input
                    && handle_key(state, key, &stop_flag, stop_requested_at)
                {
                    break;
                }
            }
            _ = tokio::time::sleep(tick_rate) => {}
        }

        if let Some(started) = stop_requested_at
            && started.elapsed() > Duration::from_secs(2)
        {
            break;
        }

        if events_closed && state.done {
            // Finished: keep rendering until user exits.
            continue;
        }
    }

    stop_flag.store(true, Ordering::Relaxed);
    Ok(())
}

fn handle_key(
    state: &mut AppState,
    key: KeyEvent,
    stop_flag: &Arc<AtomicBool>,
    stop_requested_at: &mut Option<Instant>,
) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            stop_flag.store(true, Ordering::Relaxed);
            *stop_requested_at = Some(Instant::now());
            state.status = "Stopping".to_string();
            true
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            stop_flag.store(true, Ordering::Relaxed);
            *stop_requested_at = Some(Instant::now());
            state.status = "Stopping".to_string();
            true
        }
        _ => false,
    }
}

fn push_log(state: &mut AppState, message: String) {
    if state.logs.len() >= 8 {
        state.logs.pop_front();
    }
    state.logs.push_back(message);
}

fn handle_net_debug_key(_state: &mut NetDebugState, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => true,
        _ => false,
    }
}

fn handle_proxy_key(state: &mut ProxyState, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => true,
        KeyCode::Up => {
            if state.client_offset > 0 {
                state.client_offset -= 1;
            }
            false
        }
        KeyCode::Down => {
            let max_offset = state.max_client_offset();
            state.client_offset = (state.client_offset + 1).min(max_offset);
            false
        }
        KeyCode::PageUp => {
            let step = state.client_page_size.max(1);
            state.client_offset = state.client_offset.saturating_sub(step);
            false
        }
        KeyCode::PageDown => {
            let step = state.client_page_size.max(1);
            let max_offset = state.max_client_offset();
            state.client_offset = (state.client_offset + step).min(max_offset);
            false
        }
        _ => false,
    }
}

fn handle_socks5_key(state: &mut Socks5State, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => true,
        KeyCode::Up => {
            if state.client_offset > 0 {
                state.client_offset -= 1;
            }
            false
        }
        KeyCode::Down => {
            let max_offset = state.max_client_offset();
            state.client_offset = (state.client_offset + 1).min(max_offset);
            false
        }
        KeyCode::PageUp => {
            let step = state.client_page_size.max(1);
            state.client_offset = state.client_offset.saturating_sub(step);
            false
        }
        KeyCode::PageDown => {
            let step = state.client_page_size.max(1);
            let max_offset = state.max_client_offset();
            state.client_offset = (state.client_offset + step).min(max_offset);
            false
        }
        _ => false,
    }
}

fn handle_net_debug_event(state: &mut NetDebugState, event: NetDebugEvent) {
    match event {
        NetDebugEvent::Status(message) => {
            state.status = message.clone();
            push_net_debug_log(state, format!("[STATUS] {}", normalize_log(&message)));
        }
        NetDebugEvent::Sent(message) => {
            if !state.quiet {
                push_net_debug_log(state, format!("[SENT] {}", normalize_log(&message)));
            }
        }
        NetDebugEvent::Received { content, source } => {
            let prefix = source
                .map(|addr| format!("[RECV] {addr}: "))
                .unwrap_or_else(|| "[RECV] ".to_string());
            push_net_debug_log(state, format!("{}{}", prefix, normalize_log(&content)));
        }
    }
}

fn push_net_debug_log(state: &mut NetDebugState, message: String) {
    if state.logs.len() >= 200 {
        state.logs.pop_front();
    }
    state.logs.push_back(message);
}

fn handle_proxy_event(state: &mut ProxyState, event: ProxyEvent) {
    match event {
        ProxyEvent::Status(message) => {
            state.status = message.clone();
            push_proxy_log(state, format!("[STATUS] {}", message));
            push_proxy_status(state, message);
        }
        ProxyEvent::Snapshot(snapshot) => {
            state.peak_active = state.peak_active.max(snapshot.active_connections);
            state.snapshot = Some(snapshot);
            let max_offset = state.max_client_offset();
            if state.client_offset > max_offset {
                state.client_offset = max_offset;
            }
        }
    }
}

fn push_proxy_log(state: &mut ProxyState, message: String) {
    if state.logs.len() >= 200 {
        state.logs.pop_front();
    }
    state.logs.push_back(message);
}

fn push_proxy_status(state: &mut ProxyState, message: String) {
    if state.status_history.len() >= 5 {
        state.status_history.pop_back();
    }
    state.status_history.push_front(message);
}

fn handle_socks5_event(state: &mut Socks5State, event: Socks5Event) {
    match event {
        Socks5Event::Status(message) => {
            state.status = message.clone();
            push_socks5_log(state, format!("[STATUS] {}", message));
            push_socks5_status(state, message);
        }
        Socks5Event::Snapshot(snapshot) => {
            state.peak_active = state.peak_active.max(snapshot.active_connections);
            state.snapshot = Some(snapshot);
            let max_offset = state.max_client_offset();
            if state.client_offset > max_offset {
                state.client_offset = max_offset;
            }
        }
    }
}

fn push_socks5_log(state: &mut Socks5State, message: String) {
    if state.logs.len() >= 200 {
        state.logs.pop_front();
    }
    state.logs.push_back(message);
}

fn push_socks5_status(state: &mut Socks5State, message: String) {
    if state.status_history.len() >= 5 {
        state.status_history.pop_back();
    }
    state.status_history.push_front(message);
}

fn draw_ui(f: &mut Frame, state: &AppState) {
    let size = f.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(13),
            Constraint::Length(8),
            Constraint::Min(6),
        ])
        .split(size);

    draw_header(f, chunks[0], state);
    draw_metrics(f, chunks[1], state);
    draw_status_codes(f, chunks[2], state);
    draw_logs(f, chunks[3], state);
}

fn draw_net_debug_ui(f: &mut Frame, state: &NetDebugState) {
    let size = f.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(10),
            Constraint::Min(6),
        ])
        .split(size);

    let header = Paragraph::new("Network Debug")
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);
    f.render_widget(header, chunks[0]);

    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[1]);

    let label_style = Style::default().fg(Color::Cyan);
    let value_style = Style::default().fg(Color::Yellow);

    let label = |text: &str| Cell::from(text.to_string()).style(label_style);
    let value = |text: String| Cell::from(text).style(value_style);

    let rows = vec![
        Row::new(vec![
            label("Role"),
            value(state.role.clone()),
            label("Protocol"),
            value(state.protocol.clone()),
        ]),
        Row::new(vec![
            label("IP"),
            value(state.ip_version.clone()),
            label("Local"),
            value(state.local_addr.clone()),
        ]),
        Row::new(vec![
            label("Buffer"),
            value(format!("{}", state.buffer_size)),
            label("Interval"),
            value(fmt_duration(state.interval)),
        ]),
        Row::new(vec![
            label("Batch"),
            value(state.batch_size.to_string()),
            label("Repeat"),
            value(if state.repeat { "yes" } else { "no" }.to_string()),
        ]),
        Row::new(vec![
            label("Max Speed"),
            value(if state.max_speed { "yes" } else { "no" }.to_string()),
            label("Quiet"),
            value(if state.quiet { "yes" } else { "no" }.to_string()),
        ]),
    ];

    let table = Table::new(
        rows,
        [
            Constraint::Length(10),
            Constraint::Length(18),
            Constraint::Length(10),
            Constraint::Min(10),
        ],
    )
    .block(Block::default().title("Session").borders(Borders::ALL))
    .column_spacing(1);
    f.render_widget(table, middle[0]);

    let mut target_lines = vec![
        format!("Status: {}", state.status),
        format!("Local: {}", state.local_addr),
    ];
    if let Some(remote) = &state.remote_addr {
        target_lines.push(format!("Remote: {}", remote));
    } else {
        target_lines.push("Remote: -".to_string());
    }
    let target = Paragraph::new(target_lines.join("\n"))
        .block(Block::default().title("Target").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(target, middle[1]);

    let text = if state.logs.is_empty() {
        "Press q/Esc to exit".to_string()
    } else {
        let max_lines = chunks[2].height.saturating_sub(2) as usize;
        tail_lines(&state.logs, max_lines).join("\n")
    };
    let logs = Paragraph::new(text)
        .block(Block::default().title("Logs").borders(Borders::ALL))
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left);
    f.render_widget(logs, chunks[2]);
}

fn draw_proxy_ui(f: &mut Frame, state: &ProxyState) {
    let size = f.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(8),
        ])
        .split(size);

    let header = Paragraph::new("HTTP/HTTPS Proxy")
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);
    f.render_widget(header, chunks[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(chunks[1]);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(10), Constraint::Min(3)])
        .split(body[0]);

    let empty = ProxySnapshot {
        uptime: Duration::ZERO,
        active_connections: 0,
        total_connections: 0,
        total_requests: 0,
        bytes_in: 0,
        bytes_out: 0,
        clients: Vec::new(),
    };
    let snapshot = state.snapshot.as_ref().unwrap_or(&empty);

    let label_style = Style::default().fg(Color::Cyan);
    let value_style = Style::default().fg(Color::Yellow);

    let label = |text: &str| Cell::from(text.to_string()).style(label_style);
    let value = |text: String| Cell::from(text).style(value_style);

    let stats_rows = vec![
        Row::new(vec![label("Status"), value(state.status.clone())]),
        Row::new(vec![label("Listen"), value(state.listen_addr.clone())]),
        Row::new(vec![label("Uptime"), value(fmt_uptime(snapshot.uptime))]),
        Row::new(vec![
            label("Active Conn"),
            value(snapshot.active_connections.to_string()),
        ]),
        Row::new(vec![
            label("Peak Active"),
            value(state.peak_active.to_string()),
        ]),
        Row::new(vec![
            label("Total Conn"),
            value(snapshot.total_connections.to_string()),
        ]),
        Row::new(vec![
            label("Requests"),
            value(snapshot.total_requests.to_string()),
        ]),
        Row::new(vec![label("Bytes In"), value(fmt_bytes(snapshot.bytes_in))]),
        Row::new(vec![
            label("Bytes Out"),
            value(fmt_bytes(snapshot.bytes_out)),
        ]),
    ];

    let stats_table = Table::new(stats_rows, [Constraint::Length(14), Constraint::Min(10)])
        .block(Block::default().title("Stats").borders(Borders::ALL))
        .column_spacing(1);
    f.render_widget(stats_table, left[0]);

    let status_lines = if state.status_history.is_empty() {
        vec!["No status yet".to_string()]
    } else {
        state.status_history.iter().cloned().collect::<Vec<_>>()
    };
    let status_panel = Paragraph::new(status_lines.join("\n"))
        .block(Block::default().title("Status").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(status_panel, left[1]);

    let max_rows = state.client_page_size.max(1);
    let offset = state.client_offset.min(state.max_client_offset());
    let client_rows = if snapshot.clients.is_empty() {
        vec![Row::new(vec![
            Cell::from("No clients"),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
        ])]
    } else {
        snapshot
            .clients
            .iter()
            .skip(offset)
            .take(max_rows)
            .map(|client| proxy_client_row(client))
            .collect()
    };

    let client_table = Table::new(
        client_rows,
        [
            Constraint::Length(18),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Min(12),
        ],
    )
    .block(Block::default().title("Clients").borders(Borders::ALL))
    .header(Row::new(vec![
        Cell::from("IP"),
        Cell::from("Act"),
        Cell::from("Tot"),
        Cell::from("Req"),
        Cell::from("In"),
        Cell::from("Out"),
        Cell::from("Seen"),
        Cell::from("Top"),
    ]))
    .column_spacing(1);
    f.render_widget(client_table, body[1]);

    let text = if state.logs.is_empty() {
        "Press q/Esc to exit".to_string()
    } else {
        let max_lines = chunks[2].height.saturating_sub(2) as usize;
        tail_lines(&state.logs, max_lines).join("\n")
    };
    let logs = Paragraph::new(text)
        .block(Block::default().title("Logs").borders(Borders::ALL))
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left);
    f.render_widget(logs, chunks[2]);
}

fn draw_socks5_ui(f: &mut Frame, state: &Socks5State) {
    let size = f.size();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(8),
        ])
        .split(size);

    let header = Paragraph::new("SOCKS5 Proxy")
        .block(Block::default().borders(Borders::ALL))
        .alignment(Alignment::Center);
    f.render_widget(header, chunks[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(chunks[1]);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(11), Constraint::Min(3)])
        .split(body[0]);

    let empty = Socks5Snapshot {
        uptime: Duration::ZERO,
        active_connections: 0,
        total_connections: 0,
        total_requests: 0,
        bytes_in: 0,
        bytes_out: 0,
        clients: Vec::new(),
    };
    let snapshot = state.snapshot.as_ref().unwrap_or(&empty);

    let label_style = Style::default().fg(Color::Cyan);
    let value_style = Style::default().fg(Color::Yellow);

    let label = |text: &str| Cell::from(text.to_string()).style(label_style);
    let value = |text: String| Cell::from(text).style(value_style);

    let stats_rows = vec![
        Row::new(vec![label("Status"), value(state.status.clone())]),
        Row::new(vec![label("Listen"), value(state.listen_addr.clone())]),
        Row::new(vec![label("Uptime"), value(fmt_uptime(snapshot.uptime))]),
        Row::new(vec![
            label("Active Conn"),
            value(snapshot.active_connections.to_string()),
        ]),
        Row::new(vec![
            label("Peak Active"),
            value(state.peak_active.to_string()),
        ]),
        Row::new(vec![
            label("Total Conn"),
            value(snapshot.total_connections.to_string()),
        ]),
        Row::new(vec![
            label("Requests"),
            value(snapshot.total_requests.to_string()),
        ]),
        Row::new(vec![label("Bytes In"), value(fmt_bytes(snapshot.bytes_in))]),
        Row::new(vec![
            label("Bytes Out"),
            value(fmt_bytes(snapshot.bytes_out)),
        ]),
    ];

    let stats_table = Table::new(stats_rows, [Constraint::Length(14), Constraint::Min(10)])
        .block(Block::default().title("Stats").borders(Borders::ALL))
        .column_spacing(1);
    f.render_widget(stats_table, left[0]);

    let status_lines = if state.status_history.is_empty() {
        vec!["No status yet".to_string()]
    } else {
        state.status_history.iter().cloned().collect::<Vec<_>>()
    };
    let status_panel = Paragraph::new(status_lines.join("\n"))
        .block(Block::default().title("Status").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(status_panel, left[1]);

    let max_rows = state.client_page_size.max(1);
    let offset = state.client_offset.min(state.max_client_offset());
    let client_rows = if snapshot.clients.is_empty() {
        vec![Row::new(vec![
            Cell::from("No clients"),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
        ])]
    } else {
        snapshot
            .clients
            .iter()
            .skip(offset)
            .take(max_rows)
            .map(|client| socks5_client_row(client))
            .collect()
    };

    let client_table = Table::new(
        client_rows,
        [
            Constraint::Length(18),
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Min(12),
        ],
    )
    .block(Block::default().title("Clients").borders(Borders::ALL))
    .header(Row::new(vec![
        Cell::from("IP"),
        Cell::from("Act"),
        Cell::from("Tot"),
        Cell::from("Req"),
        Cell::from("In"),
        Cell::from("Out"),
        Cell::from("Seen"),
        Cell::from("Top"),
    ]))
    .column_spacing(1);
    f.render_widget(client_table, body[1]);

    let text = if state.logs.is_empty() {
        "Press q/Esc to exit".to_string()
    } else {
        let max_lines = chunks[2].height.saturating_sub(2) as usize;
        tail_lines(&state.logs, max_lines).join("\n")
    };
    let logs = Paragraph::new(text)
        .block(Block::default().title("Logs").borders(Borders::ALL))
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left);
    f.render_widget(logs, chunks[2]);
}

fn draw_header(f: &mut Frame, area: Rect, state: &AppState) {
    let progress = state.snapshot.as_ref().map(|s| s.progress).unwrap_or(0.0);
    let gauge = Gauge::default()
        .block(Block::default().title("Progress").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Green))
        .ratio(progress.clamp(0.0, 1.0));
    f.render_widget(gauge, area);
}

fn draw_metrics(f: &mut Frame, area: Rect, state: &AppState) {
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let target_line = Paragraph::new(format!("{}  |  Status: {}", state.target, state.status))
        .block(Block::default().title("Target").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(target_line, sections[0]);

    let mut rows: Vec<Row> = Vec::new();
    let empty = MetricsSnapshot {
        elapsed: Duration::ZERO,
        progress: 0.0,
        total_requests: 0,
        success: 0,
        failures: 0,
        avg_latency: Duration::ZERO,
        p50: Duration::ZERO,
        p90: Duration::ZERO,
        p95: Duration::ZERO,
        p99: Duration::ZERO,
        rps: 0.0,
        throughput_bps: 0.0,
        throughput_bps_up: 0.0,
        total_bytes: 0,
        total_bytes_up: 0,
        avg_bytes_per_request: 0.0,
        avg_bytes_per_request_up: 0.0,
        status_codes: Vec::new(),
        status_no_response: 0,
        status_other: 0,
        completion_buckets: Vec::new(),
        done: false,
    };
    let snapshot = state.snapshot.as_ref().unwrap_or(&empty);

    let label_style = Style::default().fg(Color::Cyan);
    let value_style = Style::default().fg(Color::Yellow);
    let subtle_style = Style::default().fg(Color::DarkGray);

    let label = |text: &str| Cell::from(text.to_string()).style(label_style);
    let value = |text: String| Cell::from(text).style(value_style);
    let subtle = |text: String| Cell::from(text).style(subtle_style);

    rows.push(Row::new(vec![
        label("Method"),
        subtle(state.method.clone()),
        label("Concurrency"),
        subtle(state.concurrency.to_string()),
    ]));
    rows.push(Row::new(vec![
        label("Duration"),
        subtle(fmt_duration(state.duration)),
        label("Timeout"),
        subtle(fmt_duration(state.timeout)),
    ]));
    rows.push(Row::new(vec![
        label("Elapsed"),
        value(fmt_duration(snapshot.elapsed)),
        label("Progress"),
        value(format!("{:.1}%", snapshot.progress * 100.0)),
    ]));
    rows.push(Row::new(vec![
        label("Requests"),
        value(snapshot.total_requests.to_string()),
        label("RPS"),
        value(format!("{:.2}", snapshot.rps)),
    ]));
    rows.push(Row::new(vec![
        label("Success"),
        value(snapshot.success.to_string()),
        label("Failures"),
        value(snapshot.failures.to_string()),
    ]));
    rows.push(Row::new(vec![
        label("Avg Latency"),
        value(fmt_latency(snapshot.avg_latency)),
        label("Avg Req Bytes"),
        value(format!(
            "{} / {}",
            fmt_bytes(snapshot.avg_bytes_per_request as u64),
            fmt_bytes(snapshot.avg_bytes_per_request_up as u64)
        )),
    ]));
    rows.push(Row::new(vec![
        label("P50/P90"),
        value(format!(
            "{} / {}",
            fmt_latency(snapshot.p50),
            fmt_latency(snapshot.p90)
        )),
        label("P95/P99"),
        value(format!(
            "{} / {}",
            fmt_latency(snapshot.p95),
            fmt_latency(snapshot.p99)
        )),
    ]));
    rows.push(Row::new(vec![
        label("Total Bytes"),
        value(format!(
            "{} / {}",
            fmt_bytes(snapshot.total_bytes),
            fmt_bytes(snapshot.total_bytes_up)
        )),
        label("Throughput"),
        value(format!(
            "{} / {}",
            fmt_bytes_per_sec(snapshot.throughput_bps),
            fmt_bytes_per_sec(snapshot.throughput_bps_up)
        )),
    ]));

    let table = Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Length(18),
            Constraint::Length(14),
            Constraint::Min(10),
        ],
    )
    .block(Block::default().title("Metrics").borders(Borders::ALL))
    .column_spacing(1);
    f.render_widget(table, sections[1]);
}

fn draw_status_codes(f: &mut Frame, area: Rect, state: &AppState) {
    let mut lines = Vec::new();
    if let Some(snapshot) = state.snapshot.as_ref() {
        let mut codes: Vec<StatusCodeStat> = snapshot.status_codes.clone();
        codes.sort_by_key(|stat| stat.code);
        for stat in codes {
            lines.push(format!("{}: {}", stat.code, stat.count));
        }
        lines.push(format!("No Response: {}", snapshot.status_no_response));
        lines.push(format!("Other: {}", snapshot.status_other));
    } else {
        lines.push("No data".to_string());
    }

    let paragraph = Paragraph::new(lines.join("\n"))
        .block(Block::default().title("Status Codes").borders(Borders::ALL))
        .wrap(Wrap { trim: true });
    f.render_widget(paragraph, area);
}

fn draw_logs(f: &mut Frame, area: Rect, state: &AppState) {
    let text = if state.logs.is_empty() {
        if state.done {
            "Done. Press q/Esc to exit".to_string()
        } else {
            "Press q/Esc to stop".to_string()
        }
    } else {
        state.logs.iter().cloned().collect::<Vec<_>>().join("\n")
    };

    let paragraph = Paragraph::new(text)
        .block(Block::default().title("Logs").borders(Borders::ALL))
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left);
    f.render_widget(paragraph, area);
}

fn proxy_client_row(client: &ProxyClientSnapshot) -> Row<'_> {
    Row::new(vec![
        Cell::from(client.ip.clone()),
        Cell::from(client.active_connections.to_string()),
        Cell::from(client.total_connections.to_string()),
        Cell::from(client.total_requests.to_string()),
        Cell::from(fmt_bytes(client.bytes_in)),
        Cell::from(fmt_bytes(client.bytes_out)),
        Cell::from(fmt_age_ms(client.last_seen_ms)),
        Cell::from(format_top_targets(&client.top_targets)),
    ])
}

fn socks5_client_row(client: &Socks5ClientSnapshot) -> Row<'_> {
    Row::new(vec![
        Cell::from(client.ip.clone()),
        Cell::from(client.active_connections.to_string()),
        Cell::from(client.total_connections.to_string()),
        Cell::from(client.total_requests.to_string()),
        Cell::from(fmt_bytes(client.bytes_in)),
        Cell::from(fmt_bytes(client.bytes_out)),
        Cell::from(fmt_age_ms(client.last_seen_ms)),
        Cell::from(format_top_targets_socks(&client.top_targets)),
    ])
}

fn build_net_debug_config(
    args: &NetDebugArgs,
) -> Result<(NetDebugConfig, NetDebugState), Box<dyn Error>> {
    let protocol = if args.tcp {
        Protocol::Tcp
    } else {
        Protocol::Udp
    };
    let role = if args.server {
        Role::Server
    } else {
        Role::Client
    };
    let ip_version = if args.ipv6 {
        IpVersion::Ipv6
    } else {
        IpVersion::Ipv4
    };

    let local_host = match ip_version {
        IpVersion::Ipv6 => "::",
        IpVersion::Ipv4 => "0.0.0.0",
    };

    let (remote_host, remote_port) = if role == Role::Client {
        let host = args
            .destination
            .as_ref()
            .ok_or("DESTINATION is required in client mode")?
            .clone();
        (host, args.port)
    } else {
        ("".to_string(), 0)
    };

    let local_port = if role == Role::Server { args.port } else { 0 };
    let interval = Duration::from_millis(args.interval_ms.max(1));
    let batch_size = args.batch_size.max(1);

    let should_send = !args.server || args.repeat || args.max_speed;
    let send_plan = if should_send {
        let payloads = load_payloads(args)?;
        Some(SendPlan {
            payloads,
            interval,
            batch_size,
            repeat: args.repeat,
            max_speed: args.max_speed,
        })
    } else {
        None
    };

    let config = NetDebugConfig {
        protocol,
        role,
        ip_version,
        host: local_host.to_string(),
        local_port,
        remote_host: remote_host.clone(),
        remote_port,
        buffer_size: args.buffer_size.max(1),
        send_plan,
    };

    let local_addr = format_host_port(local_host, local_port);
    let remote_addr = if role == Role::Client {
        Some(format_host_port(&remote_host, remote_port))
    } else {
        None
    };

    let state = NetDebugState {
        status: "Ready".to_string(),
        role: if args.server { "Server" } else { "Client" }.to_string(),
        protocol: if args.tcp { "TCP" } else { "UDP" }.to_string(),
        ip_version: if args.ipv6 { "IPv6" } else { "IPv4" }.to_string(),
        local_addr,
        remote_addr,
        buffer_size: args.buffer_size.max(1),
        interval,
        batch_size,
        repeat: args.repeat,
        max_speed: args.max_speed,
        quiet: args.quiet,
        ..NetDebugState::default()
    };

    Ok((config, state))
}

fn load_payloads(args: &NetDebugArgs) -> Result<Vec<Vec<u8>>, Box<dyn Error>> {
    if let Some(input) = &args.input {
        let path = Path::new(input);
        let metadata = fs::metadata(path)?;
        if metadata.is_dir() {
            let mut entries: Vec<_> = fs::read_dir(path)?.filter_map(Result::ok).collect();
            entries.sort_by_key(|entry| entry.path());
            let mut payloads = Vec::new();
            for entry in entries {
                let entry_path = entry.path();
                if entry.file_type()?.is_file() {
                    payloads.push(fs::read(&entry_path)?);
                }
            }
            if payloads.is_empty() {
                return Err(format!("input directory has no files: {}", path.display()).into());
            }
            Ok(payloads)
        } else {
            Ok(vec![fs::read(path)?])
        }
    } else if let Some(data) = &args.data {
        Ok(vec![data.clone().into_bytes()])
    } else {
        Err("missing data: use --data or --input".into())
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

fn normalize_log(text: &str) -> String {
    text.replace('\r', "\\r").replace('\n', "\\n")
}

fn tail_lines(logs: &VecDeque<String>, max_lines: usize) -> Vec<String> {
    if max_lines == 0 {
        return Vec::new();
    }
    let len = logs.len();
    let start = len.saturating_sub(max_lines);
    logs.iter().skip(start).cloned().collect()
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>, Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<(), Box<dyn Error>> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn build_config(args: &RunArgs) -> Result<LoadTestConfig, Box<dyn Error>> {
    let mut config = LoadTestConfig::new(
        args.url.clone(),
        args.method.clone(),
        args.concurrency,
        Duration::from_secs(args.duration.max(1)),
        Duration::from_millis(args.timeout_ms.max(100)),
    );

    config.body = args.body.as_ref().map(|s| s.as_bytes().to_vec());
    config.headers = parse_headers(&args.headers)?;
    config.response_mode = match args.response_mode {
        ResponseModeArg::CountBytes => ResponseMode::CountBytes,
        ResponseModeArg::DiscardBody => ResponseMode::DiscardBody,
    };
    config.metrics_mode = match args.metrics_mode {
        MetricsModeArg::Full => MetricsMode::Full,
        MetricsModeArg::Minimal => MetricsMode::Minimal,
    };
    config.connection_mode = match args.connection_mode {
        ConnectionModeArg::KeepAlive => ConnectionMode::KeepAlive,
        ConnectionModeArg::NewConnection => ConnectionMode::NewConnection,
    };
    config.rps_limit = args.rps;
    config.rps_mode = match args.rps_mode {
        RpsModeArg::Global => RpsMode::Global,
        RpsModeArg::PerWorker => RpsMode::PerWorker,
    };
    config.allow_insecure_certs = args.allow_insecure_certs;
    config.report_interval = Duration::from_millis(args.report_interval_ms.max(100));
    config.max_latency_samples = args.max_latency_samples.max(1);
    config.ramp_up = Duration::from_secs(args.ramp_up);
    config.iterations_per_worker = args.iterations_per_worker;
    config.total_requests_limit = args.total_requests_limit;

    Ok(config)
}

fn parse_headers(headers: &[String]) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let mut parsed = Vec::new();
    for raw in headers {
        if let Some((k, v)) = raw.split_once(':') {
            parsed.push((k.trim().to_string(), v.trim().to_string()));
        } else {
            return Err(format!("invalid header format: {raw}").into());
        }
    }
    Ok(parsed)
}

fn fmt_latency(duration: Duration) -> String {
    if duration.is_zero() {
        "-".to_string()
    } else {
        format!("{:.2} ms", duration.as_secs_f64() * 1000.0)
    }
}

fn fmt_duration(duration: Duration) -> String {
    let secs = duration.as_secs_f64();
    if secs < 60.0 {
        format!("{:.2} s", secs)
    } else {
        let mins = secs / 60.0;
        format!("{:.2} min", mins)
    }
}

fn fmt_uptime(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{:.1}m", secs as f64 / 60.0)
    } else {
        format!("{:.1}h", secs as f64 / 3600.0)
    }
}

fn fmt_age_ms(last_seen_ms: u64) -> String {
    if last_seen_ms == 0 {
        return "-".to_string();
    }
    let now = now_ms();
    let age = now.saturating_sub(last_seen_ms);
    if age < 1000 {
        format!("{age} ms")
    } else {
        format!("{:.1} s", age as f64 / 1000.0)
    }
}

fn format_top_targets(targets: &[engine::proxy::TargetStat]) -> String {
    if targets.is_empty() {
        return "-".to_string();
    }
    targets
        .iter()
        .take(2)
        .map(|entry| format!("{}x{}", entry.target, entry.count))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_top_targets_socks(targets: &[engine::socks5::TargetStat]) -> String {
    if targets.is_empty() {
        return "-".to_string();
    }
    targets
        .iter()
        .take(2)
        .map(|entry| format!("{}x{}", entry.target, entry.count))
        .collect::<Vec<_>>()
        .join(", ")
}

fn fmt_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.2} MB", b / MB)
    } else if b >= KB {
        format!("{:.2} KB", b / KB)
    } else {
        format!("{bytes} B")
    }
}

fn fmt_bytes_per_sec(bps: f64) -> String {
    format!("{}/s", fmt_bytes(bps.max(0.0) as u64))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl ProxyState {
    fn max_client_offset(&self) -> usize {
        let len = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.clients.len())
            .unwrap_or(0);
        let page = self.client_page_size.max(1);
        len.saturating_sub(page)
    }
}

impl Socks5State {
    fn max_client_offset(&self) -> usize {
        let len = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.clients.len())
            .unwrap_or(0);
        let page = self.client_page_size.max(1);
        len.saturating_sub(page)
    }
}

fn proxy_client_page_size(size: Rect) -> usize {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(8),
        ])
        .split(size);
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(chunks[1]);
    let clients_area = body[1];
    clients_area.height.saturating_sub(3) as usize
}
