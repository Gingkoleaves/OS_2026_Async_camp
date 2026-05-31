use std::{
    error::Error,
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

pub mod diff_impl;

pub type AnyError = Box<dyn Error + Send + Sync>;

const STATIC_FILE: &str = "static/uni_chart.txt";
const MEMORY_SAMPLE_MS: u64 = 10;
pub const CONCURRENCY: usize = 64;
pub const PROCESS_WORKER_ENV: &str = "TASK1_PROCESS_WORKER_INDEX";
pub const USER_AGENT: &str = "task1-crawler/0.1";

#[derive(Clone, Debug)]
pub struct School {
    pub name: String,
    pub url: String,
}

#[derive(Clone, Debug)]
pub struct CrawlResult {
    pub school: School,
    pub latency: Duration,
    pub bytes: usize,
    pub output_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct BenchmarkReport {
    pub strategy: &'static str,
    pub total_elapsed: Duration,
    pub peak_rss_kb: u64,
    pub results: Vec<CrawlResult>,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct LatencyStats {
    pub min_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
    pub avg_ms: f64,
}

impl LatencyStats {
    /// Compute latency statistics from a slice of crawl results.
    pub fn from_results(results: &[CrawlResult]) -> Self {
        if results.is_empty() {
            return Self {
                min_ms: 0.0,
                p50_ms: 0.0,
                p95_ms: 0.0,
                max_ms: 0.0,
                avg_ms: 0.0,
            };
        }

        let mut ms: Vec<f64> = results
            .iter()
            .map(|r| r.latency.as_secs_f64() * 1000.0)
            .collect();
        ms.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());

        let len = ms.len();
        let min_ms = ms[0];
        let max_ms = ms[len - 1];
        let avg_ms = ms.iter().sum::<f64>() / len as f64;
        let p50_ms = percentile(&ms, 0.50);
        let p95_ms = percentile(&ms, 0.95);

        Self {
            min_ms,
            p50_ms,
            p95_ms,
            max_ms,
            avg_ms,
        }
    }
}

/// Return the value at the given percentile (0.0–1.0) from a sorted slice.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (p * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[idx]
}

/// Sample peak RSS (resident set size in KiB) while `running` is true.
/// Spawns a background thread; returns a handle whose `join()` yields the peak.
pub fn start_memory_sampler(running: Arc<AtomicBool>) -> thread::JoinHandle<u64> {
    thread::spawn(move || {
        let mut peak: u64 = 0;
        while running.load(Ordering::Relaxed) {
            if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                for line in status.lines() {
                    if line.starts_with("VmRSS:") {
                        // Format: "VmRSS:\t  12345 kB"
                        if let Some(kb_str) = line
                            .split_whitespace()
                            .nth(1)
                            .and_then(|s| s.parse::<u64>().ok())
                        {
                            peak = peak.max(kb_str);
                        }
                        break;
                    }
                }
            }
            thread::sleep(Duration::from_millis(MEMORY_SAMPLE_MS));
        }
        peak
    })
}

pub fn parse_file() -> Vec<School> {
    let content = fs::read_to_string(STATIC_FILE).expect("Failed to read static file");
    let mut schools = Vec::new();

    for line in content.lines().skip(1) {
        if let Some((name, url)) = line.split_once('\t') {
            schools.push(School {
                name: name.trim().to_string(),
                url: url.trim().to_string(),
            });
        }
    }

    schools
}
