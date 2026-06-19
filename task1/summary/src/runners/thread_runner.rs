//! std::thread 线程池 Runner。
//!
//! 每个 CPU 任务运行在独立 OS 线程上。

use crate::benchmark::{start_memory_sampler, stop_memory_sampler, Runner, RunnerOutput};
use crate::cpu_workloads::{run_workload, CpuWorkload};
use std::thread;
use std::time::Instant;

pub struct ThreadRunner;

impl Runner for ThreadRunner {
    fn name(&self) -> &'static str {
        "thread"
    }

    fn description(&self) -> &'static str {
        "std::thread thread pool — one OS thread per task"
    }

    fn run(&self, workload: &CpuWorkload, concurrency: usize) -> RunnerOutput {
        let (mem_flag, mem_handle) = start_memory_sampler();
        let overall_start = Instant::now();

        let workload = workload.clone();

        let mut handles = Vec::with_capacity(concurrency);
        for _ in 0..concurrency {
            let wl = workload.clone();
            let handle = thread::spawn(move || {
                let task_start = Instant::now();
                let _us = run_workload(&wl);
                task_start.elapsed()
            });
            handles.push(handle);
        }

        let mut latencies = Vec::with_capacity(concurrency);
        for handle in handles {
            match handle.join() {
                Ok(latency) => latencies.push(latency),
                Err(_) => { /* panic in thread */ }
            }
        }

        let total_elapsed = overall_start.elapsed();
        let peak_rss = stop_memory_sampler(mem_flag, mem_handle);

        RunnerOutput {
            runner_name: "thread (std)".to_string(),
            workload_label: workload.label.to_string(),
            concurrency,
            total_elapsed,
            peak_rss_kb: peak_rss,
            per_task_latencies: latencies,
            success_count: concurrency,
            error_count: 0,
        }
    }
}
