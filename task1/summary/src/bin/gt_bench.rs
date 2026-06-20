//! 绿色线程调度模式对比 benchmark。
//!
//! Usage: cargo run --bin gt_bench -- [cfq|hpf|rr|all]
//!
//! x86_64 + nightly only (naked_asm)

use std::sync::{Arc, Mutex};
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode_str = args.get(1).map(|s| s.as_str()).unwrap_or("all");

    if mode_str == "all" {
        run_bench("hpf");
        run_bench("rr");
        run_bench("cfq");
    } else {
        run_bench(mode_str);
    }
}

fn run_bench(mode_str: &str) {
    use task2::green_thread::{yield_thread, Runtime, SchedulerMode};

    let mode = match mode_str {
        "cfq" => SchedulerMode::Cfq,
        "hpf" => SchedulerMode::HighestPriorityFirst,
        "rr" => SchedulerMode::RoundRobin,
        _ => { eprintln!("Unknown mode: {mode_str}"); return; }
    };

    let mode_label = match mode {
        SchedulerMode::Cfq => "CFQ (vruntime)",
        SchedulerMode::HighestPriorityFirst => "HighestPriorityFirst",
        SchedulerMode::RoundRobin => "RoundRobin",
    };

    println!("=== GT {} ===", mode_label);

    let latencies = Arc::new(Mutex::new(Vec::new()));

    let lh = Arc::clone(&latencies);
    let lm = Arc::clone(&latencies);
    let ll = Arc::clone(&latencies);

    let done_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let df = Arc::clone(&done_flag);

    std::thread::spawn(move || {
        let mut rt = Runtime::with_mode(mode);
        rt.init();

        // 每个任务：128KB checksum × 2000轮，每20轮 yield（共100个yield点）
        // 足够多的yield点才能让调度策略的差异充分体现
        let make_task = |prio: u32, lat: Arc<Mutex<Vec<(u32, u64)>>>, done: Arc<std::sync::atomic::AtomicBool>| move || {
            let start = Instant::now();
            let data = vec![0u8; 1024 * 128];
            let mut cs: u64 = 0;
            for r in 0..2000u64 {
                for (j, &b) in data.iter().enumerate() {
                    cs = cs.wrapping_add(b as u64).wrapping_add(j as u64).wrapping_mul(1103515245);
                }
                if r % 20 == 0 {
                    yield_thread();
                }
            }
            std::hint::black_box(cs);
            let us = start.elapsed().as_micros() as u64;
            lat.lock().unwrap().push((prio, us));
            if lat.lock().unwrap().len() >= 3 {
                done.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        };

        rt.spawn(30, make_task(30, ll, Arc::clone(&df)));   // 低优
        rt.spawn(10, make_task(10, lm, Arc::clone(&df)));   // 中优
        rt.spawn(2, make_task(2, lh, df));                   // 高优

        // 手动调度循环（避免 rt.run() 的 process::exit）
        loop {
            yield_thread();
            if done_flag.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
            std::hint::spin_loop();
        }
    })
    .join()
    .unwrap();

    let results = Arc::try_unwrap(latencies).unwrap().into_inner().unwrap();
    // 按完成时间排序（先完成的先打印）
    let mut sorted = results.clone();
    sorted.sort_by_key(|(_, us)| *us);

    println!("  完成顺序 (延迟ms):");
    for (i, (prio, us)) in sorted.iter().enumerate() {
        let label = match prio {
            2 => "高优(PRIO=2) ",
            10 => "中优(PRIO=10)",
            30 => "低优(PRIO=30)",
            _ => "?",
        };
        println!("    {}. {}: {:.1}ms", i + 1, label, *us as f64 / 1000.0);
    }

    // 延迟差异化：最高优先级 vs 最低优先级
    if results.len() >= 2 {
        let mut by_prio: Vec<_> = results.iter().collect();
        by_prio.sort_by_key(|(p, _)| *p);
        let high = by_prio.first().unwrap().1;
        let low = by_prio.last().unwrap().1;
        if high > 0 {
            println!("  延迟比(低优/高优): {:.2}x", low as f64 / high as f64);
        }
    }
}
