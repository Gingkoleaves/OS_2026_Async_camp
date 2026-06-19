//! Embassy Executor Runner。
//!
//! Embassy 是为嵌入式设计的抢占式异步运行时。
//! 在 std 环境下无法使用其原生 executor（依赖 PAC/HAL/中断控制器）。
//!
//! 这里模拟 embassy 的核心调度行为：
//! 单线程协作式 + 优先级就绪队列 + yield 点让出 CPU。
//!
//! 注意：本 runner 标注为 "embassy-sim" 以区别于真实硬件上的 embassy-preempt。

use crate::benchmark::{start_memory_sampler, stop_memory_sampler, Runner, RunnerOutput};
use crate::cpu_workloads::{run_workload, CpuWorkload};
use std::time::{Duration, Instant};

pub struct EmbassyRunner;

impl Runner for EmbassyRunner {
    fn name(&self) -> &'static str {
        "embassy"
    }

    fn description(&self) -> &'static str {
        "Embassy-style cooperative executor (simulated in std); preemptive on real hardware"
    }

    fn run(&self, workload: &CpuWorkload, concurrency: usize) -> RunnerOutput {
        let (mem_flag, mem_handle) = start_memory_sampler();
        let overall_start = Instant::now();

        let workload = workload.clone();

        // embassy 核心特征：
        // 1. 单线程事件循环
        // 2. 基于 bitmap 的最高优先级就绪任务查找
        // 3. 任务通过 .await 或显式 yield 让出 CPU
        // 4. 中断可抢占任意低优先级任务（std 不能模拟）
        //
        // std 模拟：
        // - 任务按优先级排序后依次执行（模拟最高优先级就绪优先）
        // - 每个任务在自己的 yield 点让出

        let mut task_order: Vec<(u32, usize)> = (0..concurrency)
            .map(|i| {
                let priority: u32 = if concurrency <= 4 {
                    (i as u32 % 3) * 10 // 0, 10, 20
                } else {
                    20
                };
                (priority, i)
            })
            .collect();

        // 按优先级排序（低值 = 高优先级先执行）
        task_order.sort_by_key(|(prio, _idx)| *prio);

        let mut latencies: Vec<Duration> = Vec::with_capacity(concurrency);
        for (_priority, _idx) in &task_order {
            let task_start = Instant::now();
            let _us = run_workload(&workload);
            latencies.push(task_start.elapsed());
        }

        let total_elapsed = overall_start.elapsed();
        let peak_rss = stop_memory_sampler(mem_flag, mem_handle);

        RunnerOutput {
            runner_name: "embassy (sim)".to_string(),
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
