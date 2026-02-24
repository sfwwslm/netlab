use std::future::Future;
use std::pin::Pin;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, Instant};

use tokio::task::JoinHandle;
use tokio::time::{Instant as TokioInstant, sleep_until};

/// 调度配置：控制并发度与生命周期。
#[derive(Clone)]
pub struct ScheduleConfig {
    pub start: Instant,
    pub duration: Duration,
    pub concurrency: usize,
    pub stop_flag: Arc<AtomicBool>,
    pub rps_limit: Option<f64>,
    pub rps_mode: RpsMode,
    pub ramp_up: Duration,
    pub total_requests_limit: Option<u64>,
    pub iterations_per_worker: Option<u64>,
}

impl ScheduleConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        start: Instant,
        duration: Duration,
        concurrency: usize,
        stop_flag: Arc<AtomicBool>,
        rps_limit: Option<f64>,
        rps_mode: RpsMode,
        ramp_up: Duration,
        total_requests_limit: Option<u64>,
        iterations_per_worker: Option<u64>,
    ) -> Self {
        Self {
            start,
            duration,
            concurrency: concurrency.max(1),
            stop_flag,
            rps_limit,
            rps_mode,
            ramp_up,
            total_requests_limit: total_requests_limit
                .and_then(|limit| if limit > 0 { Some(limit) } else { None }),
            iterations_per_worker: iterations_per_worker
                .and_then(|limit| if limit > 0 { Some(limit) } else { None }),
        }
    }
}

/// 运行时调度状态（可被多个 worker 共享）。
#[derive(Clone)]
pub struct Schedule {
    start: Instant,
    deadline: Instant,
    stop_flag: Arc<AtomicBool>,
    total_requests_limit: Option<u64>,
    request_count: Arc<AtomicU64>,
}

impl Schedule {
    pub fn new(config: &ScheduleConfig) -> Self {
        let deadline = config.start + config.duration;
        Self {
            start: config.start,
            deadline,
            stop_flag: config.stop_flag.clone(),
            total_requests_limit: config.total_requests_limit,
            request_count: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn start(&self) -> Instant {
        self.start
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    pub fn should_continue(&self) -> bool {
        Instant::now() < self.deadline
            && !self.stop_flag.load(Ordering::Relaxed)
            && !self.reached_request_limit()
    }

    pub fn try_reserve_request(&self) -> bool {
        let Some(limit) = self.total_requests_limit else {
            return true;
        };
        loop {
            let current = self.request_count.load(Ordering::Relaxed);
            if current >= limit {
                return false;
            }
            if self
                .request_count
                .compare_exchange(current, current + 1, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                return true;
            }
        }
    }

    fn reached_request_limit(&self) -> bool {
        match self.total_requests_limit {
            Some(limit) => self.request_count.load(Ordering::Relaxed) >= limit,
            None => false,
        }
    }
}

pub struct WorkerContext {
    pub worker_index: usize,
    pub schedule: Schedule,
}

pub trait Worker: Send + 'static {
    fn step(&mut self) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>>;

    fn finish(&mut self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async {})
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RpsMode {
    Global,
    PerWorker,
}

impl RpsMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::PerWorker => "perWorker",
        }
    }
}

#[derive(Clone)]
enum Pacer {
    Global {
        next_at: Arc<tokio::sync::Mutex<Instant>>,
        interval: Duration,
    },
    PerWorker {
        interval: Duration,
    },
}

impl Pacer {
    fn new(rps: f64, mode: RpsMode) -> Option<Self> {
        if !rps.is_finite() || rps <= 0.0 {
            return None;
        }
        let mut interval = Duration::from_secs_f64(1.0 / rps);
        if interval.is_zero() {
            interval = Duration::from_nanos(1);
        }
        Some(match mode {
            RpsMode::Global => Pacer::Global {
                next_at: Arc::new(tokio::sync::Mutex::new(Instant::now())),
                interval,
            },
            RpsMode::PerWorker => Pacer::PerWorker { interval },
        })
    }

    async fn wait(&self, local_next: &mut Option<Instant>) {
        match self {
            Pacer::Global { next_at, interval } => {
                let scheduled_at = {
                    let mut guard = next_at.lock().await;
                    let now = Instant::now();
                    let scheduled = if *guard <= now { now } else { *guard };
                    *guard = scheduled + *interval;
                    scheduled
                };
                let now = Instant::now();
                if scheduled_at > now {
                    sleep_until(TokioInstant::from_std(scheduled_at)).await;
                }
            }
            Pacer::PerWorker { interval } => {
                let now = Instant::now();
                let scheduled_at = match local_next {
                    Some(next) => {
                        if *next <= now {
                            now
                        } else {
                            *next
                        }
                    }
                    None => now,
                };
                *local_next = Some(scheduled_at + *interval);
                if scheduled_at > now {
                    sleep_until(TokioInstant::from_std(scheduled_at)).await;
                }
            }
        }
    }
}

/// 固定并发调度：直到到期或 stop_flag 触发。
pub async fn run_fixed_concurrency<W, F>(
    config: ScheduleConfig,
    mut make_worker: F,
) -> Vec<Result<(), String>>
where
    W: Worker,
    F: FnMut(WorkerContext) -> W + Send + 'static,
{
    let schedule = Schedule::new(&config);
    let pacer = config
        .rps_limit
        .and_then(|rps| Pacer::new(rps, config.rps_mode));
    let ramp_up = config.ramp_up;
    let total_workers = config.concurrency.max(1);
    let iterations_per_worker = config.iterations_per_worker;
    let mut handles: Vec<JoinHandle<Result<(), String>>> = Vec::with_capacity(config.concurrency);

    for worker_index in 0..config.concurrency {
        let ctx = WorkerContext {
            worker_index,
            schedule: schedule.clone(),
        };
        let mut worker = make_worker(ctx);
        let schedule = schedule.clone();
        let pacer = pacer.clone();
        let ramp_delay = if ramp_up.is_zero() || total_workers <= 1 {
            Duration::ZERO
        } else {
            let fraction = worker_index as f64 / total_workers as f64;
            Duration::from_secs_f64(ramp_up.as_secs_f64() * fraction)
        };

        handles.push(tokio::spawn(async move {
            if !ramp_delay.is_zero() {
                sleep_until(TokioInstant::from_std(schedule.start() + ramp_delay)).await;
            }
            let mut local_next = None;
            let mut iterations = 0u64;
            while schedule.should_continue() {
                if iterations_per_worker.is_some_and(|limit| iterations >= limit) {
                    break;
                }
                if let Some(pacer) = pacer.as_ref() {
                    pacer.wait(&mut local_next).await;
                    if !schedule.should_continue() {
                        break;
                    }
                }
                if !schedule.try_reserve_request() {
                    break;
                }
                worker.step().await?;
                iterations += 1;
            }
            worker.finish().await;
            Ok(())
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        match handle.await {
            Ok(result) => results.push(result),
            Err(err) => results.push(Err(format!("调度任务异常退出: {err}"))),
        }
    }
    results
}
