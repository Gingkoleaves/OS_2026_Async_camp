//! Tokio 优先级分发器 benchmark。
//!
//! 对比 "priority" (三队列批量分发) vs "default" (FIFO 提交) 两种模式
//! 在高/中/低三批优先级 CPU 任务上的完成延迟差异。
//!
//! Usage:
//!   cargo run --bin tokio_prio_bench -- [priority|default|all]

use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("all");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    if mode == "all" {
        rt.block_on(async { run_bench("priority").await });
        rt.block_on(async { run_bench("default").await });
    } else {
        rt.block_on(async { run_bench(mode).await });
    }
}

fn make_task(prio: u32, id: usize) -> impl FnOnce() -> (u32, usize, u64) {
    move || {
        let start = Instant::now();
        let data = vec![0u8; 1024 * 128];
        let mut cs: u64 = 0;
        for r in 0..2000u64 {
            for (j, &b) in data.iter().enumerate() {
                cs = cs.wrapping_add(b as u64).wrapping_add(j as u64).wrapping_mul(1103515245);
            }
        }
        std::hint::black_box(cs);
        let us = start.elapsed().as_micros() as u64;
        (prio, id, us)
    }
}

async fn run_bench(mode: &str) {
    println!("\n=== Tokio {} ===", mode);

    // 3 个任务，优先级 PRIO=2(高)/10(中)/30(低)
    let tasks: Vec<(u32, usize)> = vec![(2, 1), (10, 2), (30, 3)];

    match mode {
        "priority" => {
            // 参照 References 的"三队列批量分发"策略：按优先级从高到低分发
            let concurrency = 3;
            let sem = Arc::new(Semaphore::new(concurrency));
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

            // 高优先级先提交 (prio=2 first, then 10, then 30)
            let mut sorted = tasks.clone();
            sorted.sort_by_key(|(p, _)| *p); // 低值=高优先级先提交

            for (prio, id) in &sorted {
                let permit = sem.clone().acquire_owned().await.unwrap();
                let tx = tx.clone();
                let prio = *prio;
                let id = *id;
                tokio::task::spawn_blocking(move || {
                    let (p, i, us) = make_task(prio, id)();
                    let _ = tx.send((p, i, us));
                    drop(permit);
                });
            }

            // 收集结果
            let mut results = Vec::new();
            for _ in 0..3 {
                if let Some(r) = rx.recv().await {
                    results.push(r);
                }
            }
            print_results(mode, &results);
        }
        "default" => {
            // 默认模式：所有任务同时提交（FIFO）
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

            for (prio, id) in &tasks {
                let tx = tx.clone();
                let prio = *prio;
                let id = *id;
                tokio::task::spawn_blocking(move || {
                    let (p, i, us) = make_task(prio, id)();
                    let _ = tx.send((p, i, us));
                });
            }

            let mut results = Vec::new();
            for _ in 0..3 {
                if let Some(r) = rx.recv().await {
                    results.push(r);
                }
            }
            print_results(mode, &results);
        }
        _ => eprintln!("Unknown mode: {mode}"),
    }
}

fn print_results(mode: &str, results: &[(u32, usize, u64)]) {
    let mut sorted: Vec<_> = results.iter().collect();
    sorted.sort_by_key(|(_, _, us)| *us);

    println!("  完成顺序 (延迟ms, mode={}):", mode);
    for (i, (prio, _id, us)) in sorted.iter().enumerate() {
        let label = match prio {
            2 => "高优(PRIO=2) ",
            10 => "中优(PRIO=10)",
            30 => "低优(PRIO=30)",
            _ => "?",
        };
        println!("    {}. {}: {:.1}ms", i + 1, label, *us as f64 / 1000.0);
    }

    let mut by_prio: Vec<_> = results.iter().collect();
    by_prio.sort_by_key(|(p, _, _)| *p);
    if by_prio.len() >= 2 {
        let high = by_prio.first().unwrap().2;
        let low = by_prio.last().unwrap().2;
        if high > 0 {
            println!("  延迟比(低优/高优): {:.2}x", low as f64 / high as f64);
        }
    }
}
