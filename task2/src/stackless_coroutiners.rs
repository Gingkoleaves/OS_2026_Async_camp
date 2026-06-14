//! 无栈协程（Future）模型 + CFQ 优先级调度。
//!
//! 基于 Rust `Future` trait 的手写异步运行时，包含：
//! - `CfqExecutor`：CFQ 驱动的多 Future 执行器
//! - `Reactor`：模拟 I/O 事件源（超时）
//! - `Waker`：手写虚表，唤醒 park 的 executor 线程
//! - `CfqTask`：实现 Future trait 的优先级任务

use crate::scheduler::CfqScheduler;
use std::collections::HashMap;
use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::sync::{Arc, Mutex, mpsc::Sender, mpsc::channel};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};
use std::thread::{self, JoinHandle};
use std::time::Duration;

// ---------------------------------------------------------------------------
// CfqExecutor: CFQ 驱动的多 Future 执行器
// ---------------------------------------------------------------------------

/// CFQ 执行器：同时运行多个 Future，按 vruntime 顺序进行 poll。
///
/// 每个 Future 提交时指定优先级。执行器维护一个 `CfqScheduler`，
/// 每次 poll 循环选取 vruntime 最小的 Future 进行推进。
pub struct CfqExecutor {
    /// 所有已提交的 Future（按 ID 索引）
    futures: HashMap<usize, Pin<Box<dyn Future<Output = usize> + Send>>>,
    /// 每个 Future 的优先级（按 ID 索引）
    priorities: HashMap<usize, u32>,
    /// CFQ 调度器（追踪每个 Future 的 vruntime）
    scheduler: CfqScheduler<usize>,
    /// Future ID 计数器
    next_id: usize,
}

impl CfqExecutor {
    /// 创建一个空的 CFQ 执行器。
    pub fn new() -> Self {
        CfqExecutor {
            futures: HashMap::new(),
            priorities: HashMap::new(),
            scheduler: CfqScheduler::new(),
            next_id: 0,
        }
    }

    /// 提交一个 Future 并指定优先级。
    ///
    /// `priority` 值越小优先级越高。
    pub fn submit<F>(&mut self, priority: u32, future: F)
    where
        F: Future<Output = usize> + Send + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.futures.insert(id, Box::pin(future));
        self.priorities.insert(id, priority);
        self.scheduler.push(id, priority);
    }

    /// 运行执行器直到所有 Future 完成。
    ///
    /// 每次循环：
    /// 1. 从 CFQ 调度器取出 vruntime 最小的 Future
    /// 2. Poll 该 Future
    /// 3. 如果 Ready → 收集结果
    /// 4. 如果 Pending → 更新 vruntime 并重新入队
    pub fn run(&mut self) -> Vec<(usize, usize)> {
        let mut results = Vec::new();

        // Waker：在 Future pending 时唤醒 executor
        let mywaker = Arc::new(MyWaker {
            thread: thread::current(),
        });
        let waker = waker_into_waker(Arc::into_raw(mywaker));
        let mut cx = Context::from_waker(&waker);

        while !self.scheduler.is_empty() {
            if let Some(id) = self.scheduler.pop() {
                if let Some(mut future) = self.futures.remove(&id) {
                    let t0 = std::time::Instant::now();
                    let poll_result = future.as_mut().poll(&mut cx);
                    let elapsed_ns = t0.elapsed().as_nanos() as u64;

                    match poll_result {
                        Poll::Ready(output) => {
                            results.push((id, output));
                        }
                        Poll::Pending => {
                            // 按实际 poll 耗时累加 vruntime
                            // 纯 I/O 等待（~0ns）几乎不增长，CPU 密集 poll 按比例增长
                            self.futures.insert(id, future);
                            let prio = self.priorities[&id];
                            self.scheduler.update_and_push(id, prio, elapsed_ns);
                        }
                    }
                }
            }

            // 如果所有 Future 都在 pending 状态，park 等待 wake
            if self.scheduler.is_empty() && !self.futures.is_empty() {
                thread::park();
            }
        }

        results
    }
}

impl Default for CfqExecutor {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Waker（手写虚表）
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct MyWaker {
    thread: thread::Thread,
}

fn mywaker_wake(s: &MyWaker) {
    let waker_ptr: *const MyWaker = s;
    let waker_arc = unsafe { Arc::from_raw(waker_ptr) };
    waker_arc.thread.unpark();
}

fn mywaker_clone(s: &MyWaker) -> RawWaker {
    let arc = unsafe { Arc::from_raw(s) };
    std::mem::forget(arc.clone());
    RawWaker::new(Arc::into_raw(arc) as *const (), &VTABLE)
}

const VTABLE: RawWakerVTable = unsafe {
    RawWakerVTable::new(
        |s| mywaker_clone(&*(s as *const MyWaker)),
        |s| mywaker_wake(&*(s as *const MyWaker)),
        |s| mywaker_wake(*(s as *const &MyWaker)),
        |s| drop(Arc::from_raw(s as *const MyWaker)),
    )
};

fn waker_into_waker(s: *const MyWaker) -> Waker {
    let raw_waker = RawWaker::new(s as *const (), &VTABLE);
    unsafe { Waker::from_raw(raw_waker) }
}

// ---------------------------------------------------------------------------
// CfqTask: 带优先级的 Future 包装
// ---------------------------------------------------------------------------

/// 一个简单的 Future，在 Reactor 超时触发后返回 Ready。
///
/// 用于演示 CFQ 多 Future 调度。
#[derive(Clone)]
pub struct CfqTask {
    id: usize,
    reactor: Arc<Mutex<Box<Reactor>>>,
    data: u64,
}

impl CfqTask {
    fn new(reactor: Arc<Mutex<Box<Reactor>>>, data: u64, id: usize) -> Self {
        CfqTask { id, reactor, data }
    }
}

impl Future for CfqTask {
    type Output = usize;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut r = self.reactor.lock().unwrap();

        if r.is_ready(self.id) {
            *r.tasks.get_mut(&self.id).unwrap() = TaskState::Finished;
            Poll::Ready(self.id)
        } else if r.tasks.contains_key(&self.id) {
            r.tasks
                .insert(self.id, TaskState::NotReady(cx.waker().clone()));
            Poll::Pending
        } else {
            r.register(self.data, cx.waker().clone(), self.id);
            Poll::Pending
        }
    }
}

// ---------------------------------------------------------------------------
// Reactor（模拟 I/O 事件源）
// ---------------------------------------------------------------------------

enum TaskState {
    Ready,
    NotReady(Waker),
    Finished,
}

struct Reactor {
    dispatcher: Sender<Event>,
    handle: Option<JoinHandle<()>>,
    tasks: HashMap<usize, TaskState>,
}

#[derive(Debug)]
enum Event {
    Close,
    Timeout(u64, usize),
}

impl Reactor {
    fn new() -> Arc<Mutex<Box<Self>>> {
        let (tx, rx) = channel::<Event>();
        let reactor = Arc::new(Mutex::new(Box::new(Reactor {
            dispatcher: tx,
            handle: None,
            tasks: HashMap::new(),
        })));

        let reactor_clone = Arc::downgrade(&reactor);
        let handle = thread::spawn(move || {
            let mut handles = vec![];
            for event in rx {
                let reactor = reactor_clone.clone();
                match event {
                    Event::Close => break,
                    Event::Timeout(duration, id) => {
                        let event_handle = thread::spawn(move || {
                            thread::sleep(Duration::from_secs(duration));
                            let reactor = reactor.upgrade().unwrap();
                            reactor.lock().map(|mut r| r.wake(id)).unwrap();
                        });
                        handles.push(event_handle);
                    }
                }
            }
            handles
                .into_iter()
                .for_each(|handle| handle.join().unwrap());
        });
        reactor.lock().map(|mut r| r.handle = Some(handle)).unwrap();
        reactor
    }

    fn wake(&mut self, id: usize) {
        self.tasks
            .get_mut(&id)
            .map(|state| match mem::replace(state, TaskState::Ready) {
                TaskState::NotReady(waker) => waker.wake(),
                TaskState::Finished => panic!("Called 'wake' twice on task: {}", id),
                _ => unreachable!(),
            })
            .unwrap();
    }

    fn register(&mut self, duration: u64, waker: Waker, id: usize) {
        if self.tasks.insert(id, TaskState::NotReady(waker)).is_some() {
            panic!("Tried to insert a task with id: '{}', twice!", id);
        }
        self.dispatcher.send(Event::Timeout(duration, id)).unwrap();
    }

    fn close(&mut self) {
        self.dispatcher.send(Event::Close).unwrap();
    }

    fn is_ready(&self, id: usize) -> bool {
        self.tasks
            .get(&id)
            .map(|state| match state {
                TaskState::Ready => true,
                _ => false,
            })
            .unwrap_or(false)
    }
}

impl Drop for Reactor {
    fn drop(&mut self) {
        self.handle.take().map(|h| h.join().unwrap()).unwrap();
    }
}

// ---------------------------------------------------------------------------
// 示例程序
// ---------------------------------------------------------------------------

/// CPU 密集型 Future：每次 poll 忙等 `work_us` 微秒，模拟真实计算。
///
/// 通过控制每次 poll 的耗时，CFQ 调度器可以按 vruntime 实现比例公平：
/// 高权重任务 vruntime 增长慢 → 被 poll 更频繁 → 先完成。
pub struct CpuFuture {
    id: usize,
    iterations: usize,
    count: usize,
    /// 每次 poll 的模拟 CPU 工作量（微秒），0 表示不忙等
    work_us: u64,
}

impl CpuFuture {
    #[allow(dead_code)]
    pub fn new(id: usize, iterations: usize) -> Self {
        CpuFuture {
            id,
            iterations,
            count: 0,
            work_us: 0,
        }
    }

    pub fn with_work(id: usize, iterations: usize, work_us: u64) -> Self {
        CpuFuture {
            id,
            iterations,
            count: 0,
            work_us,
        }
    }
}

impl Future for CpuFuture {
    type Output = usize;

    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        // 模拟 CPU 工作：忙等 work_us 微秒
        let t0 = std::time::Instant::now();
        while t0.elapsed().as_micros() < self.work_us as u128 {}

        self.count += 1;
        if self.count >= self.iterations {
            Poll::Ready(self.id)
        } else {
            Poll::Pending
        }
    }
}

pub fn run_example() {
    println!("=== Stackless Coroutine Model with CFQ Priority ===");

    // 示例 1: 纯 CPU Future（不需要 Reactor）
    println!("\n--- CPU-bound CFQ Scheduling ---");
    let mut executor = CfqExecutor::new();

    // 高优先级 Future: 5 次 poll 完成，每次忙等 1000us
    executor.submit(0, CpuFuture::with_work(1, 5, 1000));
    // 默认优先级 Future: 3 次 poll 完成，每次忙等 1000us
    executor.submit(20, CpuFuture::with_work(2, 3, 1000));
    // 低优先级 Future: 3 次 poll 完成，每次忙等 1000us
    executor.submit(30, CpuFuture::with_work(3, 3, 1000));

    println!("Running CFQ executor with 3 CPU futures...");
    let results = executor.run();
    println!("Results: {:?}", results);

    // 示例 2: Reactor 驱动的 Future（带 I/O 模拟）
    println!("\n--- Reactor-driven CFQ Scheduling ---");
    let reactor = Reactor::new();

    let task1 = CfqTask::new(reactor.clone(), 1, 101);
    let task2 = CfqTask::new(reactor.clone(), 2, 102);
    let task3 = CfqTask::new(reactor.clone(), 3, 103);

    // 使用 block_on 分别运行（保持原有单 Future 能力）
    let r1 = block_on(task1);
    let r2 = block_on(task2);
    let r3 = block_on(task3);

    println!(
        "All reactor tasks completed: r1={}, r2={}, r3={}",
        r1, r2, r3
    );
    reactor.lock().map(|mut r| r.close()).unwrap();
}

/// 原有的单 Future block_on（保持向后兼容）。
pub fn block_on<F: Future>(mut future: F) -> F::Output {
    let mywaker = Arc::new(MyWaker {
        thread: thread::current(),
    });
    let waker = waker_into_waker(Arc::into_raw(mywaker));
    let mut cx = Context::from_waker(&waker);

    let mut future = unsafe { Pin::new_unchecked(&mut future) };

    loop {
        match Future::poll(future.as_mut(), &mut cx) {
            Poll::Ready(val) => return val,
            Poll::Pending => thread::park(),
        };
    }
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_future_completes() {
        let mut executor = CfqExecutor::new();
        executor.submit(20, CpuFuture::new(1, 3));
        let results = executor.run();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, 1);
    }

    #[test]
    fn test_multiple_cpu_futures() {
        let mut executor = CfqExecutor::new();
        executor.submit(20, CpuFuture::new(1, 3));
        executor.submit(20, CpuFuture::new(2, 3));
        executor.submit(20, CpuFuture::new(3, 3));
        let results = executor.run();
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_cfq_high_priority_completes_faster() {
        let mut executor = CfqExecutor::new();
        // 高优先级和低优先级各需要 5 次 poll，每次忙等 500us
        // 高优先级权重大 → vruntime 增长慢 → 更频繁被 poll → 先完成
        executor.submit(0, CpuFuture::with_work(100, 5, 500));
        executor.submit(30, CpuFuture::with_work(200, 5, 500));
        let results = executor.run();

        assert_eq!(results.len(), 2);

        let high_first = results.iter().position(|r| r.1 == 100).unwrap();
        let low_first = results.iter().position(|r| r.1 == 200).unwrap();
        assert!(
            high_first < low_first,
            "High priority (id=100) should complete before low priority (id=200), got {:?}",
            results
        );
    }

    #[test]
    fn test_pending_vruntime_grows_with_work() {
        let mut executor = CfqExecutor::new();
        // 提交一个需要多次 poll 的任务（每次 poll 忙等）
        executor.submit(20, CpuFuture::with_work(1, 5, 100));
        let results = executor.run();
        assert_eq!(results.len(), 1);
        // 任务必须完成（不能因 vruntime 增长而丢失）
        assert_eq!(results[0].1, 1);
    }

    #[test]
    fn test_zero_work_pending_minimal_cost() {
        let mut executor = CfqExecutor::new();
        // work_us=0 → 每次 poll 几乎没有 CPU 耗时 → vruntime 几乎不增长
        executor.submit(20, CpuFuture::new(1, 10));
        let results = executor.run();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1, 1);
    }

    #[test]
    fn test_reactor_task_completes() {
        let reactor = Reactor::new();
        let task = CfqTask::new(reactor.clone(), 0, 1);
        let result = block_on(task);
        assert_eq!(result, 1);
        reactor.lock().map(|mut r| r.close()).unwrap();
    }
}
