use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant};

/// 负载测试的统计与快照模型。
///
/// 设计目标：
/// - **请求热路径尽量轻**：计数使用原子累加，避免每次请求都争用锁。
/// - **延迟分位可选**：分位需要采样与排序，成本更高；在 `metricsMode=minimal` 下可完全跳过。
///
/// 当前实现的分位策略属于“轻量近似”：
/// - 延迟样本采用 bounded reservoir（只保留最后 N 个样本），用于 UI 展示与粗略分布观察。
/// - 未引入 HDRHistogram/t-digest 等专业数据结构（后续若要对标专业工具，建议升级）。

#[derive(Clone, Debug)]
pub struct PercentilePoint {
    pub percentile: u64,
    pub latency: Duration,
}

/// 一次快照（用于实时 UI 展示/最终结果）。
#[derive(Clone, Debug)]
pub struct MetricsSnapshot {
    pub elapsed: Duration,
    pub progress: f64,
    pub total_requests: u64,
    pub success: u64,
    pub failures: u64,
    pub avg_latency: Duration,
    pub p50: Duration,
    pub p90: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub rps: f64,
    pub throughput_bps: f64,
    pub throughput_bps_up: f64,
    pub total_bytes: u64,
    pub total_bytes_up: u64,
    pub avg_bytes_per_request: f64,
    pub avg_bytes_per_request_up: f64,
    pub status_codes: Vec<StatusCodeStat>,
    pub status_no_response: u64,
    pub status_other: u64,
    pub completion_buckets: Vec<PercentilePoint>,
    pub done: bool,
}

/// 仅包含计数/均值的轻量快照（用于极限吞吐模式）。
#[derive(Clone, Debug)]
pub struct CountersSnapshot {
    pub total_requests: u64,
    pub success: u64,
    pub failures: u64,
    pub avg_latency: Duration,
    pub byte_count: u64,
    pub byte_count_up: u64,
    pub status_codes: Vec<StatusCodeStat>,
    pub status_no_response: u64,
    pub status_other: u64,
}

#[derive(Clone, Debug)]
pub struct StatusCodeStat {
    pub code: u16,
    pub count: u64,
}

/// 负载测试统计器。
///
/// - counters：原子累加（低开销）
/// - latency_samples：Mutex 保护的 reservoir（批量 flush，避免热路径争用）
pub struct LoadMetrics {
    total_requests: AtomicU64,
    success: AtomicU64,
    failures: AtomicU64,
    total_latency_nanos: AtomicU64,
    max_latency_nanos: AtomicU64,
    byte_count: AtomicU64,
    byte_count_up: AtomicU64,
    latency_samples_nanos: Mutex<Vec<u64>>,
    max_latency_samples: usize,
    status_counters: Box<[AtomicU64]>,
    status_no_response: AtomicU64,
    status_other: AtomicU64,
}

const STATUS_BUCKETS: usize = 600;

impl LoadMetrics {
    /// 创建统计器，并设置延迟样本 reservoir 的最大容量。
    pub fn new(max_latency_samples: usize) -> Self {
        let status_counters = (0..STATUS_BUCKETS)
            .map(|_| AtomicU64::new(0))
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Self {
            total_requests: AtomicU64::new(0),
            success: AtomicU64::new(0),
            failures: AtomicU64::new(0),
            total_latency_nanos: AtomicU64::new(0),
            max_latency_nanos: AtomicU64::new(0),
            byte_count: AtomicU64::new(0),
            byte_count_up: AtomicU64::new(0),
            latency_samples_nanos: Mutex::new(Vec::new()),
            max_latency_samples: max_latency_samples.max(1),
            status_counters,
            status_no_response: AtomicU64::new(0),
            status_other: AtomicU64::new(0),
        }
    }

    /// 记录一次请求的计数类指标（热路径）。
    ///
    /// 注意：该函数不负责记录分位样本；分位样本由 worker 在本地缓冲后批量 flush。
    pub fn record_counters(
        &self,
        latency: Duration,
        bytes_down: u64,
        bytes_up: u64,
        ok: bool,
        status: Option<u16>,
    ) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        if ok {
            self.success.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failures.fetch_add(1, Ordering::Relaxed);
        }
        self.byte_count.fetch_add(bytes_down, Ordering::Relaxed);
        self.byte_count_up.fetch_add(bytes_up, Ordering::Relaxed);
        match status {
            Some(code) if (code as usize) < STATUS_BUCKETS => {
                self.status_counters[code as usize].fetch_add(1, Ordering::Relaxed);
            }
            Some(_) => {
                self.status_other.fetch_add(1, Ordering::Relaxed);
            }
            None => {
                self.status_no_response.fetch_add(1, Ordering::Relaxed);
            }
        }

        let nanos = latency.as_nanos().min(u64::MAX as u128) as u64;
        self.total_latency_nanos.fetch_add(nanos, Ordering::Relaxed);
        self.max_latency_nanos.fetch_max(nanos, Ordering::Relaxed);
    }

    /// 将 worker 本地的延迟样本批量写入共享 reservoir（有上限）。
    pub fn flush_latency_samples(&self, samples: &mut Vec<u64>) {
        if samples.is_empty() {
            return;
        }
        let mut guard = self
            .latency_samples_nanos
            .lock()
            .expect("latency sample mutex poisoned");
        guard.append(samples);
        if guard.len() > self.max_latency_samples {
            let drain_count = guard.len() - self.max_latency_samples;
            guard.drain(..drain_count);
        }
    }

    pub fn snapshot(
        &self,
        start: Instant,
        planned_duration: Duration,
        percentiles: &[u64],
        total_requests_limit: Option<u64>,
        force_done: bool,
    ) -> MetricsSnapshot {
        let elapsed = start.elapsed();
        let elapsed_secs = elapsed.as_secs_f64().max(0.001);

        let total_requests = self.total_requests.load(Ordering::Relaxed);
        let success = self.success.load(Ordering::Relaxed);
        let failures = self.failures.load(Ordering::Relaxed);
        let total_latency_nanos = self.total_latency_nanos.load(Ordering::Relaxed);
        let max_latency_nanos = self.max_latency_nanos.load(Ordering::Relaxed);
        let byte_count = self.byte_count.load(Ordering::Relaxed);
        let byte_count_up = self.byte_count_up.load(Ordering::Relaxed);
        let (status_codes, status_no_response, status_other) = self.status_snapshot();

        let avg_latency = if total_requests > 0 {
            Duration::from_nanos(total_latency_nanos / total_requests)
        } else {
            Duration::ZERO
        };

        let rps = total_requests as f64 / elapsed_secs;
        let throughput_bps = byte_count as f64 / elapsed_secs;
        let throughput_bps_up = byte_count_up as f64 / elapsed_secs;
        let avg_bytes_per_request = if total_requests > 0 {
            byte_count as f64 / total_requests as f64
        } else {
            0.0
        };
        let avg_bytes_per_request_up = if total_requests > 0 {
            byte_count_up as f64 / total_requests as f64
        } else {
            0.0
        };

        // 采样数据的排序成本较高：
        // - 锁内仅 clone，避免长时间持锁
        // - 锁外排序与计算分位
        let samples = self
            .latency_samples_nanos
            .lock()
            .expect("latency sample mutex poisoned")
            .clone();
        let completion_buckets = build_percentile_points(&samples, max_latency_nanos, percentiles);
        let p50 = percentile_from_samples(&samples, 50);
        let p90 = percentile_from_samples(&samples, 90);
        let p95 = percentile_from_samples(&samples, 95);
        let p99 = percentile_from_samples(&samples, 99);
        let time_progress = (elapsed_secs / planned_duration.as_secs_f64().max(0.001)).min(1.0);
        let request_progress = total_requests_limit
            .map(|limit| (total_requests as f64 / limit.max(1) as f64).min(1.0))
            .unwrap_or(0.0);
        let progress = if force_done {
            1.0
        } else if total_requests_limit.is_some() {
            time_progress.max(request_progress)
        } else {
            time_progress
        };
        let done = force_done
            || elapsed >= planned_duration
            || total_requests_limit
                .map(|limit| total_requests >= limit)
                .unwrap_or(false);

        MetricsSnapshot {
            elapsed,
            progress,
            total_requests,
            success,
            failures,
            avg_latency,
            p50,
            p90,
            p95,
            p99,
            rps,
            throughput_bps,
            throughput_bps_up,
            total_bytes: byte_count,
            total_bytes_up: byte_count_up,
            avg_bytes_per_request,
            avg_bytes_per_request_up,
            status_codes,
            status_no_response,
            status_other,
            completion_buckets,
            done,
        }
    }

    /// 生成仅计数/均值的轻量快照。
    ///
    /// 用于 `metricsMode=minimal`，避免排序/分位计算开销。
    pub fn counters_snapshot(&self) -> CountersSnapshot {
        let total_requests = self.total_requests.load(Ordering::Relaxed);
        let success = self.success.load(Ordering::Relaxed);
        let failures = self.failures.load(Ordering::Relaxed);
        let total_latency_nanos = self.total_latency_nanos.load(Ordering::Relaxed);
        let byte_count = self.byte_count.load(Ordering::Relaxed);
        let byte_count_up = self.byte_count_up.load(Ordering::Relaxed);
        let (status_codes, status_no_response, status_other) = self.status_snapshot();

        let avg_latency = if total_requests > 0 {
            Duration::from_nanos(total_latency_nanos / total_requests)
        } else {
            Duration::ZERO
        };

        CountersSnapshot {
            total_requests,
            success,
            failures,
            avg_latency,
            byte_count,
            byte_count_up,
            status_codes,
            status_no_response,
            status_other,
        }
    }

    fn status_snapshot(&self) -> (Vec<StatusCodeStat>, u64, u64) {
        let mut status_codes = Vec::new();
        for (code, counter) in self.status_counters.iter().enumerate() {
            let count = counter.load(Ordering::Relaxed);
            if count > 0 {
                status_codes.push(StatusCodeStat {
                    code: code as u16,
                    count,
                });
            }
        }

        let status_no_response = self.status_no_response.load(Ordering::Relaxed);
        let status_other = self.status_other.load(Ordering::Relaxed);
        (status_codes, status_no_response, status_other)
    }
}

fn build_percentile_points(
    samples: &[u64],
    max_latency_nanos: u64,
    percentiles: &[u64],
) -> Vec<PercentilePoint> {
    percentiles
        .iter()
        .map(|p| {
            let nanos = if *p == 100 {
                max_latency_nanos
            } else {
                percentile_nanos(samples, *p)
            };
            PercentilePoint {
                percentile: *p,
                latency: Duration::from_nanos(nanos),
            }
        })
        .collect()
}

fn percentile_from_samples(samples: &[u64], percentile: u64) -> Duration {
    Duration::from_nanos(percentile_nanos(samples, percentile))
}

fn percentile_nanos(samples: &[u64], percentile: u64) -> u64 {
    if samples.is_empty() {
        return 0;
    }
    let p = (percentile.min(100) as f64) / 100.0;
    let mut data = samples.to_vec();
    data.sort_unstable();
    let rank = (p * data.len() as f64).ceil() as usize;
    let idx = rank.saturating_sub(1).min(data.len() - 1);
    data[idx]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_basic() {
        let start = Instant::now();
        let metrics = LoadMetrics::new(10);
        let mut buf = Vec::new();
        for ms in [10u64, 20, 30, 40, 50] {
            metrics.record_counters(Duration::from_millis(ms), 0, 0, true, Some(200));
            buf.push(Duration::from_millis(ms).as_nanos() as u64);
        }
        metrics.flush_latency_samples(&mut buf);

        let snap = metrics.snapshot(
            start,
            Duration::from_secs(10),
            &[50, 90, 95, 99, 100],
            None,
            false,
        );
        assert_eq!(snap.total_requests, 5);
        assert_eq!(snap.p50, Duration::from_millis(30));
        assert_eq!(snap.p90, Duration::from_millis(50));
        assert_eq!(snap.p95, Duration::from_millis(50));
        assert_eq!(snap.p99, Duration::from_millis(50));
        assert_eq!(
            snap.completion_buckets.last().unwrap().latency,
            Duration::from_millis(50)
        );
    }

    #[test]
    fn latency_samples_capped() {
        let metrics = LoadMetrics::new(3);
        let mut buf = Vec::new();
        for ms in [1u64, 2, 3, 4] {
            metrics.record_counters(Duration::from_millis(ms), 0, 0, true, Some(200));
            buf.push(Duration::from_millis(ms).as_nanos() as u64);
        }
        metrics.flush_latency_samples(&mut buf);
        let samples = metrics
            .latency_samples_nanos
            .lock()
            .expect("latency sample mutex poisoned")
            .clone();
        assert_eq!(samples.len(), 3);
        assert_eq!(samples[0], Duration::from_millis(2).as_nanos() as u64);
    }

    #[test]
    fn status_and_bytes_tracked() {
        let start = Instant::now();
        let metrics = LoadMetrics::new(10);
        metrics.record_counters(Duration::from_millis(10), 128, 32, true, Some(200));
        metrics.record_counters(Duration::from_millis(12), 256, 64, false, Some(500));
        metrics.record_counters(Duration::from_millis(8), 0, 16, false, None);

        let snap = metrics.snapshot(start, Duration::from_secs(5), &[50], None, false);
        assert_eq!(snap.total_requests, 3);
        assert_eq!(snap.total_bytes, 384);
        assert_eq!(snap.total_bytes_up, 112);
        assert_eq!(snap.status_codes.len(), 2);
        assert_eq!(snap.status_no_response, 1);
        assert!(snap.avg_bytes_per_request > 0.0);
    }
}
