//! Markdown 报告生成器。
//!
//! 收集所有 benchmark 数据，生成格式化的 Markdown 表格和 ASCII 图表。

use crate::benchmark::{LatencyStats, RunnerOutput};
use std::collections::BTreeMap;

/// 汇总所有 runner 结果，生成 Markdown 报告
pub fn generate_report(all_results: &[RunnerOutput]) -> String {
    let mut md = String::new();

    md.push_str("# 多运行时多任务性能对比报告\n\n");
    md.push_str("> 自动生成于 ");
    md.push_str(&chrono_now());
    md.push_str("\n\n");

    // ── 目录 ──
    md.push_str("## 目录\n\n");
    md.push_str("1. [实验设计](#实验设计)\n");
    md.push_str("2. [总览矩阵](#总览矩阵)\n");
    md.push_str("3. [按任务类型分析](#按任务类型分析)\n");
    md.push_str("4. [按运行时分分析](#按运行时分析)\n");
    md.push_str("5. [优先级调度对比](#优先级调度对比)\n");
    md.push_str("6. [内存开销分析](#内存开销分析)\n");
    md.push_str("7. [结论](#结论)\n\n");

    // ── 总览矩阵 ──
    md.push_str("## 总览矩阵\n\n");
    md.push_str(&generate_overview_matrix(all_results));
    md.push('\n');

    // ── 按任务类型分组 ──
    md.push_str("## 按任务类型分析\n\n");
    for (wl_label, group) in &group_by_workload(all_results) {
        md.push_str(&format!("### {} \n\n", wl_label));
        md.push_str(&generate_workload_section(group));
    }

    // ── 延迟分布详情 ──
    md.push_str("## 延迟分布详情\n\n");
    md.push_str(&generate_latency_distribution(all_results));

    // ── 内存对比 ──
    md.push_str("## 内存开销分析\n\n");
    md.push_str(&generate_memory_table(all_results));

    // ── 原始数据 ──
    md.push_str("## 原始数据\n\n");
    md.push_str(&generate_raw_data(all_results));

    md
}

fn chrono_now() -> String {
    std::process::Command::new("date")
        .arg("+%Y-%m-%d %H:%M:%S")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "unknown".to_string())
        .trim()
        .to_string()
}

// ---------------------------------------------------------------------------
// 总览矩阵
// ---------------------------------------------------------------------------

fn generate_overview_matrix(results: &[RunnerOutput]) -> String {
    let mut md = String::new();

    // 收集所有 runner 名称和 workload 标签
    let runner_names: Vec<&str> = {
        let mut names: Vec<&str> = results
            .iter()
            .map(|r| r.runner_name.as_str())
            .collect();
        names.sort();
        names.dedup();
        names
    };

    let workload_labels: Vec<&str> = {
        let mut labels: Vec<&str> = results.iter().map(|r| r.workload_label.as_str()).collect();
        labels.sort();
        labels.dedup();
        labels
    };

    // 表头
    md.push_str("### 总耗时对比 (ms)\n\n");
    md.push_str("| Workload |");
    for name in &runner_names {
        md.push_str(&format!(" {} |", name));
    }
    md.push_str("\n|");
    for _ in 0..=runner_names.len() {
        md.push_str("---|");
    }
    md.push('\n');

    for wl in &workload_labels {
        md.push_str(&format!("| {} |", wl));
        for name in &runner_names {
            let value = results
                .iter()
                .find(|r| r.workload_label == *wl && r.runner_name == *name)
                .map(|r| format!("{:.1}", r.total_elapsed.as_millis()))
                .unwrap_or_else(|| "—".to_string());
            md.push_str(&format!(" {} |", value));
        }
        md.push('\n');
    }
    md.push('\n');

    // 吞吐对比
    md.push_str("### 吞吐率对比 (tasks/sec)\n\n");
    md.push_str("| Workload |");
    for name in &runner_names {
        md.push_str(&format!(" {} |", name));
    }
    md.push_str("\n|");
    for _ in 0..=runner_names.len() {
        md.push_str("---|");
    }
    md.push('\n');

    for wl in &workload_labels {
        md.push_str(&format!("| {} |", wl));
        for name in &runner_names {
            let value = results
                .iter()
                .find(|r| r.workload_label == *wl && r.runner_name == *name)
                .map(|r| {
                    let secs = r.total_elapsed.as_secs_f64();
                    if secs > 0.0 {
                        format!("{:.1}", r.concurrency as f64 / secs)
                    } else {
                        "—".to_string()
                    }
                })
                .unwrap_or_else(|| "—".to_string());
            md.push_str(&format!(" {} |", value));
        }
        md.push('\n');
    }

    md
}

// ---------------------------------------------------------------------------
// 按 workload 分组
// ---------------------------------------------------------------------------

fn group_by_workload(results: &[RunnerOutput]) -> BTreeMap<String, Vec<&RunnerOutput>> {
    let mut map: BTreeMap<String, Vec<&RunnerOutput>> = BTreeMap::new();
    for r in results {
        map.entry(r.workload_label.clone())
            .or_default()
            .push(r);
    }
    map
}

fn generate_workload_section(group: &[&RunnerOutput]) -> String {
    let mut md = String::new();

    if group.is_empty() {
        return md;
    }

    let wl_label = &group[0].workload_label;
    let concurrency = group[0].concurrency;

    md.push_str(&format!(
        "**{}** (concurrency={})\n\n",
        wl_label, concurrency
    ));

    // 汇总表
    md.push_str("| Runner | Total(ms) | Avg Lat | P50 | P95 | P99 | Throughput | RSS(MB) |\n");
    md.push_str("|---|---|---|---|---|---|---|---|\n");

    for r in group {
        let stats = LatencyStats::from_latencies(&r.per_task_latencies);
        let secs = r.total_elapsed.as_secs_f64();
        let throughput = if secs > 0.0 {
            r.concurrency as f64 / secs
        } else {
            0.0
        };

        md.push_str(&format!(
            "| {} | {:.1} | {} | {} | {} | {} | {:.1} | {:.1} |\n",
            r.runner_name,
            r.total_elapsed.as_millis(),
            LatencyStats::format_us(stats.avg_us as u64),
            LatencyStats::format_us(stats.p50_us),
            LatencyStats::format_us(stats.p95_us),
            LatencyStats::format_us(stats.p99_us),
            throughput,
            r.peak_rss_kb as f64 / 1024.0,
        ));
    }

    md.push('\n');
    md
}

// ---------------------------------------------------------------------------
// 延迟分布详情
// ---------------------------------------------------------------------------

fn generate_latency_distribution(results: &[RunnerOutput]) -> String {
    let mut md = String::new();

    for r in results {
        if r.per_task_latencies.len() < 2 {
            continue;
        }
        let stats = LatencyStats::from_latencies(&r.per_task_latencies);

        md.push_str(&format!(
            "### {} — {}\n\n",
            r.runner_name, r.workload_label
        ));
        md.push_str(&format!("- 并发数: {}\n", r.concurrency));
        md.push_str(&format!(
            "- 平均延迟: {}\n",
            LatencyStats::format_us(stats.avg_us as u64)
        ));
        md.push_str(&format!(
            "- P50: {}\n",
            LatencyStats::format_us(stats.p50_us)
        ));
        md.push_str(&format!(
            "- P95: {}\n",
            LatencyStats::format_us(stats.p95_us)
        ));
        md.push_str(&format!(
            "- P99: {}\n",
            LatencyStats::format_us(stats.p99_us)
        ));
        md.push_str(&format!(
            "- Min/Max: {} / {}\n",
            LatencyStats::format_us(stats.min_us),
            LatencyStats::format_us(stats.max_us)
        ));

        // ASCII 分布图
        if stats.max_us > 0 && stats.avg_us > 0.0 {
            md.push_str("\n```\n");
            md.push_str(&generate_ascii_latency_bar(&stats));
            md.push_str("\n```\n");
        }
        md.push('\n');
    }

    md
}

fn generate_ascii_latency_bar(stats: &LatencyStats) -> String {
    let max = stats.max_us.max(1) as f64;
    let bar_len = 40.0;

    let mut s = String::new();
    let labels = [
        ("min  ", stats.min_us),
        ("p50  ", stats.p50_us),
        ("avg  ", stats.avg_us as u64),
        ("p95  ", stats.p95_us),
        ("p99  ", stats.p99_us),
        ("max  ", stats.max_us),
    ];

    for (label, val) in &labels {
        let fraction = (*val as f64 / max).min(1.0);
        let bars = (fraction * bar_len) as usize;
        s.push_str(&format!("{} │{}{} {}\n",
            label,
            "█".repeat(bars),
            " ".repeat(bar_len as usize - bars),
            LatencyStats::format_us(*val)
        ));
    }

    s
}

// ---------------------------------------------------------------------------
// 内存对比
// ---------------------------------------------------------------------------

fn generate_memory_table(results: &[RunnerOutput]) -> String {
    let mut md = String::new();

    let runner_names: Vec<&str> = {
        let mut names: Vec<&str> = results
            .iter()
            .map(|r| r.runner_name.as_str())
            .collect();
        names.sort();
        names.dedup();
        names
    };

    // 每个 runner 的平均和峰值内存
    md.push_str("| Runner | Avg RSS (KB) | Peak RSS (KB) | Peak RSS (MB) |\n");
    md.push_str("|---|---|---|---|\n");

    for name in &runner_names {
        let group: Vec<&RunnerOutput> = results
            .iter()
            .filter(|r| r.runner_name == *name)
            .collect();

        if group.is_empty() {
            continue;
        }
        let avg_kb: f64 = group.iter().map(|r| r.peak_rss_kb as f64).sum::<f64>()
            / group.len() as f64;
        let peak_kb = group
            .iter()
            .map(|r| r.peak_rss_kb)
            .max()
            .unwrap_or(0);

        md.push_str(&format!(
            "| {} | {:.0} | {} | {:.1} |\n",
            name,
            avg_kb,
            peak_kb,
            peak_kb as f64 / 1024.0,
        ));
    }

    md.push('\n');
    md.push_str("> 注：内存数据来自 /proc/self/status VmRSS，反映整个进程的驻留集大小。\n");
    md.push_str("> process (fork) runner 的内存数据包含父子进程总和。\n");

    md
}

// ---------------------------------------------------------------------------
// 原始数据
// ---------------------------------------------------------------------------

fn generate_raw_data(results: &[RunnerOutput]) -> String {
    let mut md = String::new();

    md.push_str("```json\n");
    let json_results: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            let stats = LatencyStats::from_latencies(&r.per_task_latencies);
            serde_json::json!({
                "runner": r.runner_name,
                "workload": r.workload_label,
                "concurrency": r.concurrency,
                "total_elapsed_ms": r.total_elapsed.as_millis(),
                "peak_rss_kb": r.peak_rss_kb,
                "success": r.success_count,
                "errors": r.error_count,
                "latency_avg_us": stats.avg_us,
                "latency_p50_us": stats.p50_us,
                "latency_p95_us": stats.p95_us,
                "latency_p99_us": stats.p99_us,
                "latency_min_us": stats.min_us,
                "latency_max_us": stats.max_us,
            })
        })
        .collect();

    md.push_str(&serde_json::to_string_pretty(&json_results).unwrap_or_default());
    md.push_str("\n```\n");

    md
}
