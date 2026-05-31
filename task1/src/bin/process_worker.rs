use std::env;
use std::error::Error;
use std::time::Instant;
use task1::parse_file;

const DEFAULT_RESULTS_DIR: &str = "results/process";

fn main() {
    if let Err(e) = run() {
        // Report failure to stdout so the parent can parse it
        println!("ERROR {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error + Send + Sync>> {
    let task_start = Instant::now();

    let arg = env::args().nth(1).ok_or("missing index arg")?;
    let index: usize = arg.parse()?;

    let results_dir = env::var("TASK1_RESULTS_DIR")
        .unwrap_or_else(|_| DEFAULT_RESULTS_DIR.to_string());

    let verbose = env::var("TASK1_VERBOSE")
        .map(|v| v == "1")
        .unwrap_or(true);

    let schools = parse_file();
    let school = schools
        .get(index)
        .ok_or_else(|| format!("index {index} out of bounds ({} schools)", schools.len()))?;

    std::fs::create_dir_all(&results_dir)?;

    let client = reqwest::blocking::Client::builder()
        .user_agent(task1::USER_AGENT)
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let response = client
        .get(&school.url)
        .send()
        .map_err(|e| format!("❌ failed to fetch {}: {e}", school.name))?;

    let html_content = response
        .text()
        .map_err(|e| format!("❌ failed to read response body for {}: {e}", school.name))?;

    let bytes = html_content.len();
    let output_path = format!("{}/{}.txt", results_dir, school.name);
    std::fs::write(&output_path, &html_content)
        .map_err(|e| format!("❌ failed to write {}: {e}", output_path))?;

    let latency_ms = task_start.elapsed().as_millis();

    // Verbose log → stderr
    if verbose {
        eprintln!("✅ saved {}", output_path);
    }

    // Structured result → stdout (parsed by parent)
    println!("OK {latency_ms} {bytes}");

    Ok(())
}
