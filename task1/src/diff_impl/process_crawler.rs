use crate::{CONCURRENCY, BenchmarkReport, CrawlResult, School, parse_file};
use std::error::Error;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const RESULTS_DIR: &str = "results/process";
const RESULTS_DIR_ENV: &str = "TASK1_RESULTS_DIR";
const VERBOSE_ENV: &str = "TASK1_VERBOSE";

/// Orchestrator: spawns child processes in batches of `CONCURRENCY`.
/// Each child runs the `process_worker` binary, which prints a structured
/// result line to stdout (`OK <latency_ms> <bytes>`) on success, or
/// `ERROR <message>` on failure. The orchestrator parses stdout to
/// collect per-child latency and throughput data.
pub fn process_crawler(verbose: bool) -> Result<BenchmarkReport, Box<dyn Error + Send + Sync>> {
    let overall_start = Instant::now();

    let schools = parse_file();
    std::fs::create_dir_all(RESULTS_DIR)?;
    let total = schools.len();
    println!("process crawler: {total} schools (concurrency={CONCURRENCY})");

    let exe = std::env::current_exe()?;
    let worker_path = exe.with_file_name("process_worker");

    let mut results = Vec::with_capacity(total);
    let mut errors = Vec::new();
    let mut ok = 0usize;
    let mut failed = 0usize;

    let dispatch_start = Instant::now();
    let mut total_spawned = 0usize;
    for batch in schools.chunks(CONCURRENCY) {
        let batch_schools: Vec<&School> = batch.iter().collect();
        let mut children = Vec::with_capacity(batch.len());

        for (i, school) in batch_schools.iter().enumerate() {
            let global_idx = total_spawned + i;
            match Command::new(&worker_path)
                .arg(global_idx.to_string())
                .env(RESULTS_DIR_ENV, RESULTS_DIR)
                .env(VERBOSE_ENV, if verbose { "1" } else { "0" })
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
            {
                Ok(child) => children.push((global_idx, (*school).clone(), child)),
                Err(e) => {
                    let msg = format!("❌ failed to spawn worker {global_idx}: {e}");
                    if verbose { eprintln!("{msg}"); }
                    errors.push(msg);
                    failed += 1;
                }
            }
        }
        total_spawned += batch.len();

        // Wait for this batch and parse stdout from each child
        for (idx, school, mut child) in children {
            match child.wait() {
                Ok(status) => {
                    let mut stdout_bytes = Vec::new();
                    if let Some(mut pipe) = child.stdout.take() {
                        let _ = pipe.read_to_end(&mut stdout_bytes);
                    }
                    let stdout_str = String::from_utf8_lossy(&stdout_bytes);

                    if status.success() {
                        if let Some(result) = parse_worker_stdout(&stdout_str, &school) {
                            results.push(result);
                            ok += 1;
                        } else {
                            let msg = format!(
                                "❌ worker {idx}: bad stdout format: {}",
                                stdout_str.trim()
                            );
                            if verbose { eprintln!("{msg}"); }
                            errors.push(msg);
                            failed += 1;
                        }
                    } else {
                        let err_detail = stdout_str
                            .lines()
                            .find(|l| l.starts_with("ERROR "))
                            .map(|l| l.strip_prefix("ERROR ").unwrap_or(l))
                            .unwrap_or("unknown error");
                        let msg = format!("❌ worker {idx}: {err_detail}");
                        if verbose { eprintln!("{msg}"); }
                        errors.push(msg);
                        failed += 1;
                    }
                }
                Err(e) => {
                    let msg = format!("❌ failed to wait on worker {idx}: {e}");
                    if verbose { eprintln!("{msg}"); }
                    errors.push(msg);
                    failed += 1;
                }
            }
        }
    }

    let dispatch_elapsed = dispatch_start.elapsed();
    println!(
        "batched {total} workers in {:.2}s — {ok}/{total} ok, {failed} failed",
        dispatch_elapsed.as_secs_f64()
    );
    println!("process crawler finished");

    let total_elapsed = overall_start.elapsed();

    Ok(BenchmarkReport {
        strategy: "process (fork)",
        total_elapsed,
        peak_rss_kb: 0,
        results,
        errors,
    })
}

fn parse_worker_stdout(stdout: &str, school: &School) -> Option<CrawlResult> {
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("OK ") {
            let mut parts = rest.split_whitespace();
            let latency_ms: u64 = parts.next()?.parse().ok()?;
            let bytes: usize = parts.next()?.parse().ok()?;
            return Some(CrawlResult {
                school: school.clone(),
                latency: Duration::from_millis(latency_ms),
                bytes,
                output_path: format!("{}/{}.txt", RESULTS_DIR, school.name).into(),
            });
        }
    }
    None
}
