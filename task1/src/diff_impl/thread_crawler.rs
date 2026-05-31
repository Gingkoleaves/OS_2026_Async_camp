use crate::{CONCURRENCY, BenchmarkReport, CrawlResult, School, parse_file};
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

const RESULTS_DIR: &str = "results/thread";

pub fn thread_crawler(verbose: bool) -> Result<BenchmarkReport, Box<dyn Error + Send + Sync>> {
    let overall_start = Instant::now();

    let schools = parse_file();
    std::fs::create_dir_all(RESULTS_DIR)?;
    let total = schools.len();
    println!("thread crawler: {total} schools (concurrency={CONCURRENCY})");

    let results = Arc::new(Mutex::new(Vec::<CrawlResult>::with_capacity(total)));
    let errors = Arc::new(Mutex::new(Vec::<String>::new()));

    for batch in schools.chunks(CONCURRENCY) {
        let mut handles = Vec::with_capacity(batch.len());

        for school in batch.iter().cloned() {
            let results = Arc::clone(&results);
            let errors = Arc::clone(&errors);
            let handle = thread::spawn(move || match run_thread_crawler(school, verbose) {
                Ok(r) => {
                    results.lock().unwrap().push(r);
                }
                Err(e) => {
                    errors.lock().unwrap().push(e);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.join();
        }
    }

    let total_elapsed = overall_start.elapsed();
    let results = Arc::try_unwrap(results).unwrap().into_inner().unwrap();
    let errors = Arc::try_unwrap(errors).unwrap().into_inner().unwrap();

    println!("thread crawler finished");

    Ok(BenchmarkReport {
        strategy: "thread (std)",
        total_elapsed,
        peak_rss_kb: 0,
        results,
        errors,
    })
}

fn run_thread_crawler(school: School, verbose: bool) -> Result<CrawlResult, String> {
    let task_start = Instant::now();
    let school_name = school.name.clone();
    let url = school.url.clone();

    let response = reqwest::blocking::get(&url)
        .map_err(|e| format!("❌ 无法连接到 {} 的网站: {:?}", school_name, e))?;

    let html_content = response
        .text()
        .map_err(|e| format!("❌ 解析 {} 的网页文本失败: {:?}", school_name, e))?;

    let bytes = html_content.len();
    let output_path = format!("{}/{}.txt", RESULTS_DIR, school_name);

    std::fs::write(&output_path, &html_content)
        .map_err(|e| format!("❌ 写入 {} 的结果文件失败: {:?}", school_name, e))?;

    if verbose {
        println!(
            "✅ {} 的爬虫任务完成，结果已保存到 {}",
            school_name, output_path
        );
    }

    Ok(CrawlResult {
        school,
        latency: task_start.elapsed(),
        bytes,
        output_path: output_path.into(),
    })
}
