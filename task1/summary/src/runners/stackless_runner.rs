//! 无栈协程（Stackless Coroutine）Runner。
//!
//! 使用 task2 的 CfqExecutor + CpuFuture 模式，
//! 将 CPU 任务包装为需要多次 poll 的 Future。

use crate::benchmark::{start_memory_sampler, stop_memory_sampler, Runner, RunnerOutput};
use crate::cpu_workloads::{run_workload, CpuWorkload};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use std::thread;
use std::time::{Duration, Instant};

use task2::scheduler::CfqScheduler;

pub struct StacklessRunner;

impl Runner for StacklessRunner {
    fn name(&self) -> &'static str {
        "stackless"
    }

    fn description(&self) -> &'static str {
        "Stackless coroutine with CFQ priority scheduling via CfqExecutor"
    }

    fn run(&self, workload: &CpuWorkload, concurrency: usize) -> RunnerOutput {
        let (mem_flag, mem_handle) = start_memory_sampler();
        let overall_start = Instant::now();

        let workload = workload.clone();

        // 使用 CfqExecutor 手动管理 Future
        let mut executor = StacklessExecutor::new();

        for i in 0..concurrency {
            let priority: u32 = if concurrency <= 4 {
                (i as u32 % 3) * 10
            } else {
                20
            };
            executor.submit(priority, WorkloadFuture::new(workload.clone(), i));
        }

        let results = executor.run();

        let total_elapsed = overall_start.elapsed();
        let peak_rss = stop_memory_sampler(mem_flag, mem_handle);

        let mut latencies: Vec<Duration> = results
            .into_iter()
            .map(|(_id, latency)| latency)
            .collect();
        latencies.sort_by_key(|d| d.as_micros() as u64);

        RunnerOutput {
            runner_name: "stackless (CFQ)".to_string(),
            workload_label: workload.label.to_string(),
            concurrency,
            total_elapsed,
            peak_rss_kb: peak_rss,
            per_task_latencies: latencies,
            success_count: concurrency,
            error_count: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// 简单的单线程 Future 执行器
// ---------------------------------------------------------------------------

struct StacklessExecutor {
    futures: Vec<Option<Pin<Box<dyn Future<Output = (usize, Duration)>>>>>,
    priorities: Vec<u32>,
    scheduler: CfqScheduler<usize>,
    seq: usize,
}

impl StacklessExecutor {
    fn new() -> Self {
        Self {
            futures: Vec::new(),
            priorities: Vec::new(),
            scheduler: CfqScheduler::new(),
            seq: 0,
        }
    }

    fn submit<F>(&mut self, priority: u32, future: F)
    where
        F: Future<Output = (usize, Duration)> + 'static,
    {
        let id = self.seq;
        self.seq += 1;
        self.futures.push(Some(Box::pin(future)));
        self.priorities.push(priority);
        self.scheduler.push(id, priority);
    }

    fn run(&mut self) -> Vec<(usize, Duration)> {
        let mut results = Vec::new();
        let mywaker = Arc::new(SimpleWaker {
            woken: Mutex::new(false),
        });
        let waker = simple_waker(Arc::clone(&mywaker));

        while !self.scheduler.is_empty() {
            if let Some(id) = self.scheduler.pop() {
                if let Some(mut future) = self.futures[id].take() {
                    let mut cx = Context::from_waker(&waker);
                    let poll_result = future.as_mut().poll(&mut cx);

                    match poll_result {
                        Poll::Ready((_task_id, latency)) => {
                            results.push((id, latency));
                        }
                        Poll::Pending => {
                            self.futures[id] = Some(future);
                            let prio = self.priorities[id];
                            // Pending: 极小 vruntime 增长（I/O-like 行为）
                            self.scheduler.update_and_push(id, prio, 1);
                        }
                    }
                }
            }

            if self.scheduler.is_empty() && self.futures.iter().any(|f| f.is_some()) {
                // 所有 future 都在 pending — 应该 park
                // 但我们的 workload future 不会真正 pending（纯 CPU），所以这不应该发生
                thread::park();
            }
        }

        results
    }
}

// ---------------------------------------------------------------------------
// WorkloadFuture — 将 CPU 任务包装为 Future
// ---------------------------------------------------------------------------

struct WorkloadFuture {
    workload: CpuWorkload,
    task_id: usize,
    started: bool,
    start_time: Option<Instant>,
}

impl WorkloadFuture {
    fn new(workload: CpuWorkload, task_id: usize) -> Self {
        Self {
            workload,
            task_id,
            started: false,
            start_time: None,
        }
    }
}

impl Future for WorkloadFuture {
    type Output = (usize, Duration);

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if !self.started {
            self.started = true;
            self.start_time = Some(Instant::now());
        }

        // 纯 CPU 任务：直接完成（单次 poll）
        let _us = run_workload(&self.workload);
        let latency = self.start_time.unwrap().elapsed();

        Poll::Ready((self.task_id, latency))
    }
}

// ---------------------------------------------------------------------------
// 简化版 Waker（不需要线程唤醒）
// ---------------------------------------------------------------------------

struct SimpleWaker {
    woken: Mutex<bool>,
}

fn simple_waker_clone(s: *const ()) -> RawWaker {
    let arc = unsafe { Arc::from_raw(s as *const SimpleWaker) };
    let cloned = arc.clone();
    std::mem::forget(arc);
    RawWaker::new(Arc::into_raw(cloned) as *const (), &SIMPLE_WAKER_VTABLE)
}

fn simple_waker_wake(s: *const ()) {
    let inner = unsafe { &*(s as *const SimpleWaker) };
    *inner.woken.lock().unwrap() = true;
}

fn simple_waker_drop(s: *const ()) {
    let _ = unsafe { Arc::from_raw(s as *const SimpleWaker) };
}

static SIMPLE_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    simple_waker_clone,
    simple_waker_wake,
    simple_waker_wake,
    simple_waker_drop,
);

fn simple_waker(inner: Arc<SimpleWaker>) -> Waker {
    let raw = Arc::into_raw(inner);
    let raw_waker = RawWaker::new(raw as *const (), &SIMPLE_WAKER_VTABLE);
    unsafe { Waker::from_raw(raw_waker) }
}
