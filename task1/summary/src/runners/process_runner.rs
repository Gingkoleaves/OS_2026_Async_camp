//! fork/process Runner。
//!
//! 每个 CPU 任务运行在独立子进程中。
//! 通过 spawn 自身 binary 的 `--worker` 模式实现。

use crate::benchmark::{start_memory_sampler, stop_memory_sampler, Runner, RunnerOutput};
use crate::cpu_workloads::CpuWorkload;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub struct ProcessRunner;

impl Runner for ProcessRunner {
    fn name(&self) -> &'static str {
        "process"
    }

    fn description(&self) -> &'static str {
        "fork(2) — one child process per task"
    }

    fn run(&self, workload: &CpuWorkload, concurrency: usize) -> RunnerOutput {
        let (mem_flag, mem_handle) = start_memory_sampler();
        let overall_start = Instant::now();

        let exe = std::env::current_exe().unwrap_or_else(|_| "summary".into());
        let total_tasks = concurrency;

        let workload_label = workload.label.to_string();

        // 分批运行（每批最多 64 个并发进程）
        let batch_size = 64usize.min(concurrency);
        let mut latencies = Vec::with_capacity(total_tasks);
        let mut success_count = 0;
        let mut error_count = 0;

        for batch_start in (0..total_tasks).step_by(batch_size) {
            let batch_end = (batch_start + batch_size).min(total_tasks);
            let batch_len = batch_end - batch_start;

            let mut children = Vec::with_capacity(batch_len);
            for _ in 0..batch_len {
                match Command::new(&exe)
                    .arg("--worker")
                    .arg(&workload_label)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::inherit())
                    .spawn()
                {
                    Ok(child) => children.push(child),
                    Err(e) => {
                        eprintln!("process runner: failed to spawn worker: {}", e);
                        error_count += 1;
                    }
                }
            }

            for mut child in children {
                match child.wait() {
                    Ok(status) => {
                        let mut stdout_bytes = Vec::new();
                        if let Some(mut pipe) = child.stdout.take() {
                            let _ = pipe.read_to_end(&mut stdout_bytes);
                        }
                        let stdout_str = String::from_utf8_lossy(&stdout_bytes);

                        if status.success() {
                            // 解析 "OK <latency_us>" 格式
                            if let Some(rest) = stdout_str
                                .lines()
                                .find(|l| l.starts_with("OK "))
                                .and_then(|l| l.strip_prefix("OK "))
                            {
                                if let Ok(us) = rest.trim().parse::<u64>() {
                                    latencies.push(Duration::from_micros(us));
                                    success_count += 1;
                                } else {
                                    error_count += 1;
                                }
                            } else {
                                error_count += 1;
                            }
                        } else {
                            error_count += 1;
                        }
                    }
                    Err(_) => {
                        error_count += 1;
                    }
                }
            }
        }

        // 收集所有成批子任务延迟后计算总时间
        let total_elapsed = overall_start.elapsed();
        let peak_rss = stop_memory_sampler(mem_flag, mem_handle);

        RunnerOutput {
            runner_name: "process (fork)".to_string(),
            workload_label,
            concurrency: total_tasks,
            total_elapsed,
            peak_rss_kb: peak_rss,
            per_task_latencies: latencies,
            success_count,
            error_count,
        }
    }
}
