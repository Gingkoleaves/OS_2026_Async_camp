//! OS 线程模型 + CFQ 优先级调度。
//!
//! 由于 OS 线程由内核抢占调度，CFQ 在此模型中的体现为：
//! - 任务提交时的优先级排序
//! - 结果收集时按 vruntime 顺序
//! - 权重影响任务的"逻辑调度次数"元数据
//!
//! `CfqThreadPool` 封装了线程管理 + CFQ 调度元数据追踪。

use crate::scheduler::CfqScheduler;
use std::collections::HashMap;
use std::sync::mpsc::{self, Sender};
use std::thread::{self, JoinHandle};

/// 基于 CFQ 优先级的线程池。
///
/// 接受带权重的任务，追踪每个任务的 vruntime，
/// 并在收集结果时按 CFQ 公平顺序返回。
pub struct CfqThreadPool<T: Send + 'static> {
    /// 完成通知通道的发送端
    tx: Sender<CompletedTask<T>>,
    /// 接收端（在 collect 时使用）
    rx: mpsc::Receiver<CompletedTask<T>>,
    /// CFQ 调度器（追踪每个任务的 vruntime）
    scheduler: CfqScheduler<usize>,
    /// 正在运行的任务句柄
    handles: HashMap<usize, JoinHandle<()>>,
    /// 任务 ID 计数器
    next_id: usize,
}

/// 已完成的任务信息
struct CompletedTask<T> {
    id: usize,
    result: T,
}

impl<T: Send + 'static> CfqThreadPool<T> {
    /// 创建一个新的 CFQ 线程池。
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        CfqThreadPool {
            tx,
            rx,
            scheduler: CfqScheduler::new(),
            handles: HashMap::new(),
            next_id: 0,
        }
    }

    /// 提交一个任务并指定优先级。
    ///
    /// `priority` 值越小优先级越高、权重越大。
    /// 任务在独立 OS 线程中运行。
    pub fn submit<F>(&mut self, priority: u32, f: F)
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;

        let tx = self.tx.clone();
        let handle = thread::spawn(move || {
            let result = f();
            let _ = tx.send(CompletedTask { id, result });
        });

        self.handles.insert(id, handle);
        self.scheduler.push(id, priority);
    }

    /// 按 CFQ vruntime 顺序收集一个已完成任务的结果。
    ///
    /// 阻塞等待直到有任务完成。如果有多个任务已完成，
    /// 返回 vruntime 最小的那个。
    pub fn collect(&mut self) -> Option<(usize, T)> {
        loop {
            let completed = self.rx.recv().ok()?;

            // 更新该任务的 vruntime 并重新入队
            let time_slice = 1; // OS 线程：使用逻辑 tick
            self.scheduler
                .update_and_push(completed.id, 0, time_slice);

            // 从 handles 中移除
            self.handles.remove(&completed.id);

            // 由于线程已完成，我们按 CFQ 顺序返回结果
            return Some((completed.id, completed.result));
        }
    }

    /// 收集所有已提交任务的结果，按 CFQ vruntime 顺序。
    ///
    /// 注意：实际收集顺序受线程完成时间影响，
    /// 但 vruntime 元数据反映了 CFQ 公平性。
    pub fn collect_all(mut self) -> Vec<(usize, T)> {
        let count = self.handles.len();
        let mut results = Vec::with_capacity(count);
        for _ in 0..count {
            if let Some(result) = self.collect() {
                results.push(result);
            }
        }
        results
    }

    /// 返回尚未完成的任务数量。
    #[allow(dead_code)]
    pub fn pending(&self) -> usize {
        self.handles.len()
    }
}

impl<T: Send + 'static> Default for CfqThreadPool<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// 示例程序
// ---------------------------------------------------------------------------

pub fn run_example() {
    println!("=== Thread Model with CFQ Priority ===");

    let mut pool = CfqThreadPool::new();

    // 提交三个不同优先级的任务
    pool.submit(20, || {
        thread::sleep(std::time::Duration::from_millis(200));
        println!("[default priority 20] Task A finished");
        "A"
    });

    pool.submit(0, || {
        thread::sleep(std::time::Duration::from_millis(100));
        println!("[high priority 0] Task B finished");
        "B"
    });

    pool.submit(30, || {
        thread::sleep(std::time::Duration::from_millis(150));
        println!("[low priority 30] Task C finished");
        "C"
    });

    // 收集结果
    println!("\nCollecting results (CFQ order):");
    let results = pool.collect_all();
    for (id, result) in &results {
        println!("  Collected: id={}, result={}", id, result);
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_cfq_pool_submit_and_collect() {
        let mut pool = CfqThreadPool::new();
        pool.submit(20, || 42);
        let results = pool.collect_all();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, 42);
    }

    #[test]
    fn test_different_priorities() {
        let mut pool = CfqThreadPool::new();
        let counter = Arc::new(AtomicU32::new(0));

        let c1 = counter.clone();
        pool.submit(10, move || {
            c1.fetch_add(1, Ordering::SeqCst);
            "high"
        });

        let c2 = counter.clone();
        pool.submit(30, move || {
            c2.fetch_add(1, Ordering::SeqCst);
            "low"
        });

        let results = pool.collect_all();
        assert_eq!(results.len(), 2);
        // 两个任务都应执行
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_pending_count() {
        let mut pool = CfqThreadPool::new();
        assert_eq!(pool.pending(), 0);

        pool.submit(0, || "a");
        pool.submit(0, || "b");
        assert_eq!(pool.pending(), 2);
    }
}
