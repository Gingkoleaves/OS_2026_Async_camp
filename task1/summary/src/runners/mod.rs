//! Runner 模块。
//!
//! 每种执行流模型实现 `benchmark::Runner` trait。

pub mod tokio_runner;
pub mod thread_runner;
pub mod process_runner;
pub mod green_runner;
pub mod stackless_runner;
pub mod embassy_runner;

pub use tokio_runner::TokioRunner;
pub use thread_runner::ThreadRunner;
pub use process_runner::ProcessRunner;
pub use green_runner::GreenRunner;
pub use stackless_runner::StacklessRunner;
pub use embassy_runner::EmbassyRunner;

use crate::benchmark::Runner;
use std::collections::HashMap;

/// 所有可用的 runner
pub fn all_runners() -> Vec<Box<dyn Runner>> {
    vec![
        Box::new(TokioRunner),
        Box::new(ThreadRunner),
        Box::new(ProcessRunner),
        Box::new(GreenRunner),
        Box::new(StacklessRunner),
        Box::new(EmbassyRunner),
    ]
}

/// 按名称获取 runner
pub fn get_runner(name: &str) -> Option<Box<dyn Runner>> {
    let mut map: HashMap<&str, Box<dyn Runner>> = HashMap::new();
    map.insert("tokio", Box::new(TokioRunner));
    map.insert("thread", Box::new(ThreadRunner));
    map.insert("process", Box::new(ProcessRunner));
    map.insert("green", Box::new(GreenRunner));
    map.insert("stackless", Box::new(StacklessRunner));
    map.insert("embassy", Box::new(EmbassyRunner));
    map.remove(name)
}
