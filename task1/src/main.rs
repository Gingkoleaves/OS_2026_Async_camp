use std::env;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use task1::diff_impl::{async_crawler::*, process_crawler::*, thread_crawler::*};
use task1::{BenchmarkReport, LatencyStats, start_memory_sampler};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let quiet = env::args().any(|a| a == "--quiet" || a == "-q");

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           Crawler Benchmark — async vs process vs thread     ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // ── Async crawler ──────────────────────────────────────────
    println!("▶▶▶ Running async crawler");
    let async_report = run_with_memory_sampling(|| async_crawler(!quiet))?;

    // ── Process crawler ────────────────────────────────────────
    println!("\n▶▶▶ Running process crawler");
    let process_report = run_with_memory_sampling(|| process_crawler(!quiet))?;

    // ── Thread crawler ─────────────────────────────────────────
    println!("\n▶▶▶ Running thread crawler");
    let thread_report = run_with_memory_sampling(|| thread_crawler(!quiet))?;

    // ── Summary table ──────────────────────────────────────────
    println!();
    println!("╔══════════════════════╦═════════════════╦═════════════════╦═════════════════╗");
    println!("║ Metric               ║ async (tokio)   ║ process (fork)  ║ thread (std)    ║");
    println!("╠══════════════════════╬═════════════════╬═════════════════╬═════════════════╣");

    print_row("Total time",
              &format_time(&async_report.total_elapsed),
              &format_time(&process_report.total_elapsed),
              &format_time(&thread_report.total_elapsed));

    let async_ok = async_report.results.len();
    let async_err = async_report.errors.len();
    let process_ok = process_report.results.len();
    let process_err = process_report.errors.len();
    let thread_ok = thread_report.results.len();
    let thread_err = thread_report.errors.len();

    print_row("Success/Total",
              &format!("{}/{}", async_ok, async_ok + async_err),
              &format!("{}/{}", process_ok, process_ok + process_err),
              &format!("{}/{}", thread_ok, thread_ok + thread_err));

    // Throughput
    let async_bytes: usize = async_report.results.iter().map(|r| r.bytes).sum();
    let process_bytes: usize = process_report.results.iter().map(|r| r.bytes).sum();
    let thread_bytes: usize = thread_report.results.iter().map(|r| r.bytes).sum();

    print_row("Throughput (KB/s)",
              &format_throughput(async_bytes, &async_report.total_elapsed),
              &format_throughput(process_bytes, &process_report.total_elapsed),
              &format_throughput(thread_bytes, &thread_report.total_elapsed));

    print_row("Peak RSS (MB)",
              &format_rss(async_report.peak_rss_kb),
              &format_rss(process_report.peak_rss_kb),
              &format_rss(thread_report.peak_rss_kb));

    // Latency distribution
    let async_stats = LatencyStats::from_results(&async_report.results);
    let process_stats = LatencyStats::from_results(&process_report.results);
    let thread_stats = LatencyStats::from_results(&thread_report.results);

    print_row("Avg latency (ms)",
              &format_latency(async_stats.avg_ms),
              &format_latency(process_stats.avg_ms),
              &format_latency(thread_stats.avg_ms));
    print_row("P50 latency (ms)",
              &format_latency(async_stats.p50_ms),
              &format_latency(process_stats.p50_ms),
              &format_latency(thread_stats.p50_ms));
    print_row("P95 latency (ms)",
              &format_latency(async_stats.p95_ms),
              &format_latency(process_stats.p95_ms),
              &format_latency(thread_stats.p95_ms));
    print_row("Min latency (ms)",
              &format_latency(async_stats.min_ms),
              &format_latency(process_stats.min_ms),
              &format_latency(thread_stats.min_ms));
    print_row("Max latency (ms)",
              &format_latency(async_stats.max_ms),
              &format_latency(process_stats.max_ms),
              &format_latency(thread_stats.max_ms));

    println!("╚══════════════════════╩═════════════════╩═════════════════╩═════════════════╝");

    Ok(())
}

/// Run a benchmark function with memory sampling in the background.
fn run_with_memory_sampling<F>(f: F) -> Result<BenchmarkReport, Box<dyn std::error::Error + Send + Sync>>
where
    F: FnOnce() -> Result<BenchmarkReport, Box<dyn std::error::Error + Send + Sync>>,
{
    let running = Arc::new(AtomicBool::new(true));
    let mem_handle = start_memory_sampler(Arc::clone(&running));

    let bench_start = Instant::now();
    let mut report = f()?;
    report.total_elapsed = bench_start.elapsed();

    // Stop memory sampler
    running.store(false, std::sync::atomic::Ordering::Relaxed);
    report.peak_rss_kb = mem_handle.join().unwrap_or(0);

    // Print individual report
    println!();
    println!("  ┌─ {} ─────────────────────────────", report.strategy);
    println!("  │ Total time:   {:>10.2} s", report.total_elapsed.as_secs_f64());
    println!("  │ Peak RSS:     {:>10.1} MB", report.peak_rss_kb as f64 / 1024.0);
    let ok = report.results.len();
    let errs = report.errors.len();
    println!("  │ Results:      {ok} ok, {errs} errors");

    if !report.results.is_empty() {
        let stats = LatencyStats::from_results(&report.results);
        let total_bytes: usize = report.results.iter().map(|r| r.bytes).sum();
        println!("  │ Throughput:   {:>10.1} KB/s",
                 total_bytes as f64 / report.total_elapsed.as_secs_f64() / 1024.0);
        println!("  │ Latency avg:  {:>10.0} ms", stats.avg_ms);
        println!("  │ Latency p50:  {:>10.0} ms", stats.p50_ms);
        println!("  │ Latency p95:  {:>10.0} ms", stats.p95_ms);
        println!("  │ Latency min:  {:>10.0} ms", stats.min_ms);
        println!("  │ Latency max:  {:>10.0} ms", stats.max_ms);
    }
    println!("  └─────────────────────────────────────────");

    Ok(report)
}

// ── formatting helpers ────────────────────────────────────────

fn format_time(d: &std::time::Duration) -> String {
    format!("{:.2} s", d.as_secs_f64())
}

fn format_rss(kb: u64) -> String {
    if kb == 0 {
        "N/A".to_string()
    } else {
        format!("{:.1} MB", kb as f64 / 1024.0)
    }
}

fn format_throughput(bytes: usize, elapsed: &std::time::Duration) -> String {
    let secs = elapsed.as_secs_f64();
    if bytes == 0 || secs == 0.0 {
        return "N/A".to_string();
    }
    format!("{:.1}", bytes as f64 / secs / 1024.0)
}

fn format_latency(ms: f64) -> String {
    if ms == 0.0 {
        "N/A".to_string()
    } else {
        format!("{:.0}", ms)
    }
}

fn print_row(metric: &str, col1: &str, col2: &str, col3: &str) {
    println!("║ {:<20} ║ {:<15} ║ {:<15} ║ {:<15} ║",
             metric, col1, col2, col3);
}
