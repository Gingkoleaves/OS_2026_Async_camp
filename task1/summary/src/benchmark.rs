//! Benchmark harness：统一计时、内存采样、统计计算。
//!
//! 提供 `Runner` trait 和 `BenchmarkHarness`，
//! 所有运行时 Runner 实现该 trait 即可接入统一测试框架。

use crate::cpu_workloads::{run_workload, CpuWorkload};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Runner trait
// ---------------------------------------------------------------------------

/// 执行流模型 Runner。
///
/// 每个实现代表一种并发/异步模型（tokio、thread、process、green-thread 等）。
pub trait Runner {
    /// Runner 名称（如 "tokio"）
    fn name(&self) -> &'static str;

    /// Runner 描述
    fn description(&self) -> &'static str;

    /// 并发运行 `concurrency` 个 `workload` 实例，返回结果。
    fn run(&self, workload: &CpuWorkload, concurrency: usize) -> RunnerOutput;
}

// ---------------------------------------------------------------------------
// 结果类型
// ---------------------------------------------------------------------------

/// 单次 benchmark 运行的输出
#[derive(Clone, Debug)]
pub struct RunnerOutput {
    pub runner_name: String,
    pub workload_label: String,
    pub concurrency: usize,
    pub total_elapsed: Duration,
    pub peak_rss_kb: u64,
    pub per_task_latencies: Vec<Duration>,
    pub success_count: usize,
    pub error_count: usize,
}

/// 延迟统计
#[derive(Clone, Debug)]
pub struct LatencyStats {
    pub avg_us: f64,
    pub min_us: u64,
    pub max_us: u64,
    pub p50_us: u64,
    pub p95_us: u64,
    pub p99_us: u64,
}

impl LatencyStats {
    pub fn from_latencies(latencies: &[Duration]) -> Self {
        if latencies.is_empty() {
            return Self {
                avg_us: 0.0,
                min_us: 0,
                max_us: 0,
                p50_us: 0,
                p95_us: 0,
                p99_us: 0,
            };
        }

        let mut us: Vec<u64> = latencies.iter().map(|d| d.as_micros() as u64).collect();
        us.sort_unstable();

        let len = us.len();
        let avg_us = us.iter().sum::<u64>() as f64 / len as f64;

        Self {
            avg_us,
            min_us: us[0],
            max_us: us[len - 1],
            p50_us: percentile(&us, 0.50),
            p95_us: percentile(&us, 0.95),
            p99_us: percentile(&us, 0.99),
        }
    }

    pub fn format_us(us: u64) -> String {
        if us < 1000 {
            format!("{}μs", us)
        } else if us < 1_000_000 {
            format!("{:.2}ms", us as f64 / 1000.0)
        } else {
            format!("{:.2}s", us as f64 / 1_000_000.0)
        }
    }
}

fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = (p * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[idx]
}

// ---------------------------------------------------------------------------
// 内存采样
// ---------------------------------------------------------------------------

/// 启动后台内存采样线程。返回 JoinHandle 和 stop flag。
pub fn start_memory_sampler() -> (Arc<AtomicBool>, thread::JoinHandle<u64>) {
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = Arc::clone(&running);

    let handle = thread::spawn(move || {
        let mut peak: u64 = 0;
        while running_clone.load(Ordering::Relaxed) {
            if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                for line in status.lines() {
                    if line.starts_with("VmRSS:") {
                        if let Some(kb_str) = line
                            .split_whitespace()
                            .nth(1)
                            .and_then(|s| s.parse::<u64>().ok())
                        {
                            peak = peak.max(kb_str);
                        }
                        break;
                    }
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
        peak
    });

    (running, handle)
}

pub fn stop_memory_sampler(running: Arc<AtomicBool>, handle: thread::JoinHandle<u64>) -> u64 {
    running.store(false, Ordering::Relaxed);
    handle.join().unwrap_or(0)
}

// ---------------------------------------------------------------------------
// 基准运行辅助函数
// ---------------------------------------------------------------------------

/// 串联运行 workload 的辅助函数。
/// 用于同步/串行 runner：在同一线程中依次执行所有任务。
pub fn run_sequential(
    runner_name: &str,
    workload: &CpuWorkload,
    concurrency: usize,
) -> RunnerOutput {
    let (mem_flag, mem_handle) = start_memory_sampler();
    let overall_start = Instant::now();

    let mut latencies = Vec::with_capacity(concurrency);
    for _ in 0..concurrency {
        let task_start = Instant::now();
        let _us = run_workload(workload);
        latencies.push(task_start.elapsed());
    }

    let total_elapsed = overall_start.elapsed();
    let peak_rss = stop_memory_sampler(mem_flag, mem_handle);

    RunnerOutput {
        runner_name: runner_name.to_string(),
        workload_label: workload.label.to_string(),
        concurrency,
        total_elapsed,
        peak_rss_kb: peak_rss,
        per_task_latencies: latencies,
        success_count: concurrency,
        error_count: 0,
    }
}
