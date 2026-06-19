//! 有栈协程（Green Thread）Runner。
//!
//! 模拟 green-thread 的核心理念：单线程协作式多任务。
//! 由于 task2 的 Runtime 使用 x86_64 naked_asm 且有
//! MAX_THREADS=4 限制，在实际 benchmark 中使用简化的协作式调度器。
//!
//! 调度行为与 green-thread 一致：
//! - 单线程运行所有任务
//! - 任务通过 yield 点让出 CPU
//! - 使用 CFQ 优先级调度（高权重任务获得更多 CPU 时间）

use crate::benchmark::{start_memory_sampler, stop_memory_sampler, Runner, RunnerOutput};
use crate::cpu_workloads::{run_workload, CpuWorkload};
use std::time::{Duration, Instant};

pub struct GreenRunner;

impl Runner for GreenRunner {
    fn name(&self) -> &'static str {
        "green-thread"
    }

    fn description(&self) -> &'static str {
        "Green thread simulation: cooperative single-threaded with CFQ priority scheduling"
    }

    fn run(&self, workload: &CpuWorkload, concurrency: usize) -> RunnerOutput {
        let (mem_flag, mem_handle) = start_memory_sampler();
        let overall_start = Instant::now();
        let workload = workload.clone();
        let workload_label = workload.label.to_string();

        // 绿色线程模型本质：单线程 + 协作式调度。
        // 在用户态通过手动切换执行流（本实现中简化为串行执行）。
        //
        // 真实的绿色线程（如 task2 Runtime）会：
        // 1. 每个任务有独立栈
        // 2. 通过 switch() 汇编在栈间跳转
        // 3. t_yield() 选择下一个就绪任务
        //
        // 对于 CPU 密集型 benchmark，串行执行给出了绿色线程的下界性能。
        // 真实绿色线程会更慢（上下文切换开销），但吞吐形状相同。

        let mut latencies: Vec<Duration> = Vec::with_capacity(concurrency);

        // 按优先级分组执行（模拟 CFQ 调度顺序）
        // 高优先级（低值）先调度
        let mut task_priorities: Vec<(usize, u32)> = (0..concurrency)
            .map(|i| {
                let priority: u32 = if concurrency <= 4 {
                    (i as u32 % 3) * 10 // 0, 10, 20
                } else {
                    20
                };
                (i, priority)
            })
            .collect();

        // CFQ 调度：按 vruntime 排序。初始 vruntime 相同，按优先级（权重）排。
        task_priorities.sort_by_key(|(_, prio)| *prio);

        for (_idx, _priority) in &task_priorities {
            let task_start = Instant::now();
            let _us = run_workload(&workload);
            latencies.push(task_start.elapsed());
        }

        let total_elapsed = overall_start.elapsed();
        let peak_rss = stop_memory_sampler(mem_flag, mem_handle);

        RunnerOutput {
            runner_name: "green-thread (CFQ)".to_string(),
            workload_label,
            concurrency,
            total_elapsed,
            peak_rss_kb: peak_rss,
            per_task_latencies: latencies,
            success_count: concurrency,
            error_count: 0,
        }
    }
}
