//! Tokio 多线程异步运行时 Runner。
//!
//! 使用 `tokio::spawn` 并发执行 CPU 任务。
//! 注意：CPU 密集型任务会阻塞 tokio worker 线程，
//! 因此使用 `tokio::task::spawn_blocking` 隔离。

use crate::benchmark::{start_memory_sampler, stop_memory_sampler, Runner, RunnerOutput};
use crate::cpu_workloads::{run_workload, CpuWorkload};
use std::time::Instant;

pub struct TokioRunner;

impl Runner for TokioRunner {
    fn name(&self) -> &'static str {
        "tokio"
    }

    fn description(&self) -> &'static str {
        "Tokio multi-thread async runtime with spawn_blocking for CPU tasks"
    }

    fn run(&self, workload: &CpuWorkload, concurrency: usize) -> RunnerOutput {
        let (mem_flag, mem_handle) = start_memory_sampler();
        let overall_start = Instant::now();

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(num_cpus::get().min(concurrency))
            .enable_all()
            .build()
            .expect("Failed to build tokio runtime");

        let workload_label = workload.label.to_string();

        let result = runtime.block_on(async move {
            let mut handles = Vec::with_capacity(concurrency);

            for _ in 0..concurrency {
                let wl = workload.clone();
                let handle = tokio::task::spawn_blocking(move || {
                    let task_start = Instant::now();
                    let _us = run_workload(&wl);
                    task_start.elapsed()
                });
                handles.push(handle);
            }

            let mut latencies = Vec::with_capacity(concurrency);
            for handle in handles {
                match handle.await {
                    Ok(latency) => latencies.push(latency),
                    Err(e) => eprintln!("tokio task panicked: {}", e),
                }
            }
            latencies
        });

        let total_elapsed = overall_start.elapsed();
        let peak_rss = stop_memory_sampler(mem_flag, mem_handle);

        RunnerOutput {
            runner_name: "tokio (async)".to_string(),
            workload_label,
            concurrency,
            total_elapsed,
            peak_rss_kb: peak_rss,
            per_task_latencies: result,
            success_count: concurrency,
            error_count: 0,
        }
    }
}
