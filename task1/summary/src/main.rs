//! 总结系统入口。
//!
//! 两种模式：
//! - **Benchmark 模式**（默认）：运行所有/指定 runner × workload × concurrency
//! - **Worker 模式**（`--worker`）：运行单个 workload 并输出 "OK <latency_us>"

mod benchmark;
mod cpu_workloads;
mod report;
mod runners;

use benchmark::Runner;
use cpu_workloads::{run_workload, CpuWorkload};
use runners::all_runners;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // ── Worker 模式（供 process runner 使用）──
    if args.contains(&"--worker".to_string()) {
        run_worker_mode(&args);
        return;
    }

    // ── Benchmark 模式 ──
    run_benchmark_mode(&args);
}

// ---------------------------------------------------------------------------
// Worker 模式
// ---------------------------------------------------------------------------

fn run_worker_mode(args: &[String]) {
    // 格式: summary --worker <workload_label>
    let label = args
        .iter()
        .skip_while(|a| *a != "--worker")
        .nth(1)
        .expect("Usage: summary --worker <workload_label>");

    let workload = find_workload_by_label(label)
        .unwrap_or_else(|| panic!("Unknown workload: {}", label));

    let start = Instant::now();
    let _us = run_workload(&workload);
    let latency_us = start.elapsed().as_micros() as u64;

    println!("OK {}", latency_us);
}

fn find_workload_by_label(label: &str) -> Option<CpuWorkload> {
    CpuWorkload::all_presets()
        .into_iter()
        .find(|w| w.label == label)
}

// ---------------------------------------------------------------------------
// Benchmark 模式
// ---------------------------------------------------------------------------

fn run_benchmark_mode(args: &[String]) {
    // CLI 解析
    let concurrency: usize = parse_arg(args, "--concurrency", "64");
    let runner_filter: Vec<String> = parse_multi_arg(args, "--runner");
    let workload_filter: Vec<String> = parse_multi_arg(args, "--workload");
    let quick = args.contains(&"--quick".to_string());

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           多运行时多任务性能对比 Benchmark                    ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("  concurrency: {}", concurrency);
    if quick {
        println!("  mode: quick (light workloads only)");
    }

    // 选择 runner
    let all_runners = all_runners();
    let selected_runners: Vec<&Box<dyn Runner>> = if runner_filter.is_empty() {
        all_runners.iter().collect()
    } else {
        all_runners
            .iter()
            .filter(|r| runner_filter.iter().any(|f| r.name().contains(f.as_str())))
            .collect()
    };

    // 选择 workload
    let all_workloads = if quick {
        vec![
            CpuWorkload::checksum_light(),
            CpuWorkload::prime_light(),
            CpuWorkload::matrix_light(),
            CpuWorkload::hash_light(),
        ]
    } else if workload_filter.is_empty() {
        CpuWorkload::all_presets()
    } else {
        CpuWorkload::all_presets()
            .into_iter()
            .filter(|w| workload_filter.iter().any(|f| w.label.contains(f.as_str())))
            .collect()
    };

    println!("  runners: {} ({})", selected_runners.len(),
        selected_runners.iter().map(|r| r.name()).collect::<Vec<_>>().join(", "));
    println!("  workloads: {} ({})", all_workloads.len(),
        all_workloads.iter().map(|w| w.label).collect::<Vec<_>>().join(", "));
    println!();

    // ── 运行所有 benchmark ──
    let mut all_results: Vec<benchmark::RunnerOutput> = Vec::new();

    for runner in &selected_runners {
        println!("━━━ {} ━━━", runner.name());
        for workload in &all_workloads {
            print!("  {:30} concurrency={:3} ... ", workload.label, concurrency);

            let result = runner.run(workload, concurrency);

            if result.error_count > 0 {
                print!(
                    "SKIP (errors={}/{})",
                    result.error_count,
                    result.success_count + result.error_count
                );
            } else {
                let secs = result.total_elapsed.as_secs_f64();
                let throughput = if secs > 0.0 {
                    format!("{:.1} t/s", concurrency as f64 / secs)
                } else {
                    "N/A".to_string()
                };
                print!(
                    "{:.1}s, avg={:.0}us, {}",
                    secs,
                    result
                        .per_task_latencies
                        .iter()
                        .map(|d| d.as_micros() as f64)
                        .sum::<f64>()
                        / result.per_task_latencies.len().max(1) as f64,
                    throughput
                );
            }
            println!();
            all_results.push(result);
        }
        println!();
    }

    // ── 生成报告 ──
    println!("Generating report...");
    let report_md = report::generate_report(&all_results);

    let output_path = "output/benchmark_report.md";
    std::fs::create_dir_all("output").unwrap();
    std::fs::write(output_path, &report_md).unwrap();
    println!("Report written to {}", output_path);

    // 简要摘要
    println!();
    println!("═══ 简要摘要 ═══");
    print_summary(&all_results);
}

// ---------------------------------------------------------------------------
// CLI 辅助函数
// ---------------------------------------------------------------------------

fn parse_arg<T: std::str::FromStr>(args: &[String], flag: &str, default: &str) -> T {
    let idx = args.iter().position(|a| a == flag);
    match idx {
        Some(i) => args
            .get(i + 1)
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| default.parse().ok().unwrap()),
        None => default.parse().ok().unwrap(),
    }
}

fn parse_multi_arg(args: &[String], flag: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == flag {
            if let Some(val) = args.get(i + 1) {
                if !val.starts_with("--") {
                    results.push(val.clone());
                    i += 1; // skip the value
                }
            }
        }
        i += 1;
    }
    results
}

// ---------------------------------------------------------------------------
// 摘要
// ---------------------------------------------------------------------------

fn print_summary(results: &[benchmark::RunnerOutput]) {
    // 按 workload 分组，找出最快的 runner
    use std::collections::BTreeMap;
    let mut by_wl: BTreeMap<String, Vec<&benchmark::RunnerOutput>> = BTreeMap::new();
    for r in results {
        by_wl.entry(r.workload_label.clone()).or_default().push(r);
    }

    for (wl, group) in &by_wl {
        let fastest = group
            .iter()
            .min_by_key(|r| r.total_elapsed.as_micros() as u64)
            .unwrap();

        let _slowest = group
            .iter()
            .max_by_key(|r| r.total_elapsed.as_micros() as u64)
            .unwrap();

        let most_efficient = group
            .iter()
            .min_by_key(|r| r.peak_rss_kb)
            .unwrap();

        println!(
            "  {:<30} fastest: {:<20} ({:.1}s),  most efficient: {:<20} ({:.0}KB)",
            wl,
            fastest.runner_name,
            fastest.total_elapsed.as_secs_f64(),
            most_efficient.runner_name,
            most_efficient.peak_rss_kb,
        );
    }
}
