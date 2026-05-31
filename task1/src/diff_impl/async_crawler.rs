use crate::{CONCURRENCY, BenchmarkReport, CrawlResult, School, parse_file};
use std::error::Error;
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::{runtime::Builder, task::JoinSet};

const RESULTS_DIR: &str = "results/async";

pub fn async_crawler(verbose: bool) -> Result<BenchmarkReport, Box<dyn Error + Send + Sync>> {
    let overall_start = Instant::now();

    let runtime = Builder::new_multi_thread()
        .worker_threads(CONCURRENCY)
        .enable_all()
        .build()?;

    let schools = parse_file();
    fs::create_dir_all(RESULTS_DIR)?;
    let total = schools.len();
    println!("async crawler: {total} schools (concurrency={CONCURRENCY})");

    let results = Arc::new(Mutex::new(Vec::<CrawlResult>::with_capacity(total)));
    let errors = Arc::new(Mutex::new(Vec::<String>::new()));

    let results_outer = Arc::clone(&results);
    let errors_outer = Arc::clone(&errors);

    runtime.block_on(async move {
        let mut join_set = JoinSet::new();

        for school in schools {
            let results = Arc::clone(&results);
            let errors = Arc::clone(&errors);
            join_set.spawn(async move {
                match run_async_crawler(school, verbose).await {
                    Ok(r) => {
                        results.lock().unwrap().push(r);
                    }
                    Err(e) => {
                        errors.lock().unwrap().push(e);
                    }
                }
            });
        }

        while let Some(res) = join_set.join_next().await {
            if let Err(e) = res {
                eprintln!("async task panic: {:?}", e);
            }
        }

        println!("async crawler finished");
    });

    let total_elapsed = overall_start.elapsed();
    let results = Arc::try_unwrap(results_outer).unwrap().into_inner().unwrap();
    let errors = Arc::try_unwrap(errors_outer).unwrap().into_inner().unwrap();

    Ok(BenchmarkReport {
        strategy: "async (tokio)",
        total_elapsed,
        peak_rss_kb: 0,
        results,
        errors,
    })
}

async fn run_async_crawler(school: School, verbose: bool) -> Result<CrawlResult, String> {
    let task_start = Instant::now();
    let school_name = school.name.clone();
    let url = school.url.clone();

    let response = reqwest::get(&url)
        .await
        .map_err(|e| format!("❌ 无法连接到 {} 的网站: {:?}", school_name, e))?;

    let html_content = response
        .text()
        .await
        .map_err(|e| format!("❌ 解析 {} 的网页文本失败: {:?}", school_name, e))?;

    let bytes = html_content.len();
    let file_path = format!("{}/{}.txt", RESULTS_DIR, school_name);

    let mut file = File::create(&file_path)
        .await
        .map_err(|e| format!("❌ 创建文件 {} 失败: {:?}", file_path, e))?;

    file.write_all(html_content.as_bytes())
        .await
        .map_err(|e| format!("❌ 写入文件 {} 失败: {:?}", file_path, e))?;

    if verbose {
        println!("✅ saved {}", file_path);
    }

    Ok(CrawlResult {
        school,
        latency: task_start.elapsed(),
        bytes,
        output_path: file_path.into(),
    })
}
