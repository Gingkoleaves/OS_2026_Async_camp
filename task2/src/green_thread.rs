//! 有栈协程（Green Thread）模型 + CFQ 优先级调度。
//!
//! 在用户态通过 `naked_asm!` 进行寄存器级上下文切换，
//! 实现协作式多任务。每个 green thread 拥有独立的栈空间。
//!
//! CFQ 集成点：
//! - 每个 thread 拥有 CFQ 权重
//! - `t_yield()` 时按 vruntime 选择下一个运行的 thread
//! - 高权重 thread 获得更多 CPU 时间片

use crate::scheduler::{CfqScheduler, prio_to_weight};
use std::arch::naked_asm;
use std::ptr;

const DEFAULT_STACK_SIZE: usize = 1024 * 1024 * 2;
const MAX_THREADS: usize = 4;
static mut RUNTIME: usize = 0;

pub struct Runtime {
    threads: Vec<Thread>,
    /// CFQ 调度器：追踪就绪线程的 ID 顺序
    scheduler: CfqScheduler<usize>,
    current: usize,
}

#[derive(PartialEq, Eq, Debug)]
enum State {
    Available,
    Running,
    Ready,
}

struct Thread {
    id: usize,
    stack: Vec<u8>,
    ctx: ThreadContext,
    state: State,
    task: Option<Box<dyn Fn()>>,
    /// CFQ 权重（由 priority 映射得出）
    weight: u32,
    /// CFQ 优先级值（用于 vruntime 更新）
    priority: u32,
}

#[derive(Debug, Default)]
#[repr(C)]
struct ThreadContext {
    rsp: u64,
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbx: u64,
    rbp: u64,
    thread_ptr: u64,
}

impl Thread {
    fn new(id: usize) -> Self {
        Thread {
            id,
            stack: vec![0_u8; DEFAULT_STACK_SIZE],
            ctx: ThreadContext::default(),
            state: State::Available,
            task: None,
            weight: prio_to_weight(20),
            priority: 20,
        }
    }
}

impl Runtime {
    pub fn new() -> Self {
        let base_thread = Thread {
            id: 0,
            stack: vec![0_u8; DEFAULT_STACK_SIZE],
            ctx: ThreadContext::default(),
            state: State::Running,
            task: None,
            weight: prio_to_weight(20),
            priority: 20,
        };

        let mut threads = vec![base_thread];
        let mut available_threads: Vec<Thread> = (1..MAX_THREADS).map(|i| Thread::new(i)).collect();
        threads.append(&mut available_threads);
        // 在 Vec 完全填充后再设置指针，避免 realloc 导致悬垂
        for i in 0..threads.len() {
            threads[i].ctx.thread_ptr = &threads[i] as *const Thread as u64;
        }

        Runtime {
            threads,
            scheduler: CfqScheduler::new(),
            current: 0,
        }
    }

    pub fn init(&self) {
        unsafe {
            let r_ptr: *const Runtime = self;
            RUNTIME = r_ptr as usize;
        }
    }

    pub fn run(&mut self) -> ! {
        while self.t_yield() {}
        std::process::exit(0);
    }

    /// CFQ 驱动的上下文调度。
    fn t_yield(&mut self) -> bool {
        let cur = &self.threads[self.current];
        let cur_id = cur.id;
        let cur_priority = cur.priority;
        let is_available = cur.state == State::Available;

        // 仍在运行的线程：更新 vruntime 并入队
        if !is_available && cur_id != 0 {
            self.scheduler
                .update_and_push(cur_id, cur_priority, 1_000_000);
        }

        // 循环找下一个可用线程
        loop {
            match self.scheduler.pop() {
                Some(next_id) => {
                    let pos = self
                        .threads
                        .iter()
                        .position(|t| t.id == next_id && t.state == State::Ready)
                        .or_else(|| {
                            self.threads
                                .iter()
                                .position(|t| t.id == next_id && t.state == State::Running)
                        });

                    if let Some(pos) = pos {
                        // 只有自己在调度器中 → 继续执行当前线程
                        if pos == self.current {
                            self.threads[self.current].state = State::Running;
                            return false;
                        }

                        if !is_available {
                            self.threads[self.current].state = State::Ready;
                        }
                        self.threads[pos].state = State::Running;
                        let old_pos = self.current;
                        self.current = pos;

                        unsafe {
                            switch(
                                &mut self.threads[old_pos].ctx,
                                &self.threads[pos].ctx,
                            );
                        }
                        return true;
                    }
                    // 状态不一致（线程已结束），继续
                }
                None => {
                    // 调度器为空 → 切回主线程或退出
                    if self.current != 0
                        && (self.threads[0].state == State::Running
                            || self.threads[0].state == State::Ready)
                    {
                        self.threads[0].state = State::Running;
                        let old_pos = self.current;
                        self.current = 0;
                        unsafe {
                            switch(
                                &mut self.threads[old_pos].ctx,
                                &self.threads[0].ctx,
                            );
                        }
                    }
                    if !is_available {
                        self.threads[self.current].state = State::Running;
                    }
                    return false;
                }
            }
        }
    }

    /// 生成一个新的 green thread，指定优先级。
    ///
    /// `priority` 值越小优先级越高。
    pub fn spawn<F: Fn() + 'static>(&mut self, priority: u32, f: F) {
        unsafe {
            let available = self
                .threads
                .iter_mut()
                .find(|t| t.state == State::Available)
                .expect("no available thread.");

            let size = available.stack.len();
            let s_ptr = available.stack.as_mut_ptr();
            available.task = Some(Box::new(f));
            available.weight = prio_to_weight(priority);
            available.priority = priority;
            available.ctx.thread_ptr = available as *const Thread as u64;

            // 设置栈：返回地址 = guard, 然后 call 的地址在下方
            ptr::write(s_ptr.offset((size - 8) as isize) as *mut u64, guard as *const () as u64);
            ptr::write(s_ptr.offset((size - 16) as isize) as *mut u64, call as *const () as u64);
            available.ctx.rsp = s_ptr.offset((size - 16) as isize) as u64;

            let id = available.id;
            available.state = State::Ready;

            // 将新线程加入 CFQ 调度器
            self.scheduler.push(id, priority);
        }
    }
}

fn call(thread: u64) {
    let thread = unsafe { &*(thread as *const Thread) };
    if let Some(f) = &thread.task {
        f();
    }
}

/// Green thread 完成时的入口点。
///
/// 通过 `ret` 从 `call` 跳转至此。使用 `#[unsafe(naked)]` 避免栈帧问题，
/// 通过 `call` 指令正确建立栈帧后再进入 Rust 代码。
#[unsafe(naked)]
unsafe extern "C" fn guard() {
    naked_asm!(
        "call {}",
        sym guard_impl,
    );
}

fn guard_impl() {
    let rt_ptr = unsafe { RUNTIME as *mut Runtime };
    let rt = unsafe { &mut *rt_ptr };
    if rt.current != 0 {
        rt.threads[rt.current].state = State::Available;
        rt.t_yield();
    }
}

pub fn yield_thread() {
    unsafe {
        let rt_ptr = RUNTIME as *mut Runtime;
        (*rt_ptr).t_yield();
    };
}

/// 上下文切换：保存当前寄存器到 `old`，从 `new` 恢复。
///
/// 遵循 x86_64 System V ABI：`old` 在 rdi，`new` 在 rsi。
#[unsafe(naked)]
unsafe extern "C" fn switch(_old: *mut ThreadContext, _new: *const ThreadContext) {
    naked_asm!(
        // 保存当前上下文到 old (rdi)
        "mov     [rdi], rsp",
        "mov     [rdi + 0x08], r15",
        "mov     [rdi + 0x10], r14",
        "mov     [rdi + 0x18], r13",
        "mov     [rdi + 0x20], r12",
        "mov     [rdi + 0x28], rbx",
        "mov     [rdi + 0x30], rbp",
        // 恢复新上下文从 new (rsi)
        "mov     rsp, [rsi]",
        "mov     r15, [rsi + 0x08]",
        "mov     r14, [rsi + 0x10]",
        "mov     r13, [rsi + 0x18]",
        "mov     r12, [rsi + 0x20]",
        "mov     rbx, [rsi + 0x28]",
        "mov     rbp, [rsi + 0x30]",
        "mov     rdi, [rsi + 0x38]",
        "ret",
    );
}

// ---------------------------------------------------------------------------
// 示例程序
// ---------------------------------------------------------------------------

pub fn run_example() {
    println!("=== Green Thread Model with CFQ Priority ===");

    let mut runtime = Runtime::new();
    runtime.init();

    // 使用接近的优先级 (19/20/21)，权重比 ~1.25x，产生可见的交错调度
    // prio 19: weight=1277, delta per yield = 1M*1024/1277 ≈ 801,880
    // prio 20: weight=1024, delta per yield = 1M*1024/1024 = 1,000,000
    // prio 21: weight=820,  delta per yield = 1M*1024/820  ≈ 1,248,780
    runtime.spawn(21, || {
        for i in 0..5 {
            println!("  [prio 21, w=820]  iter {}", i);
            yield_thread();
        }
    });
    runtime.spawn(19, || {
        for i in 0..5 {
            println!("  [prio 19, w=1277] iter {}", i);
            yield_thread();
        }
    });
    runtime.spawn(20, || {
        for i in 0..5 {
            println!("  [prio 20, w=1024] iter {}", i);
            yield_thread();
        }
    });

    runtime.run();
}

// ---------------------------------------------------------------------------
// 单元测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_weight_initialization() {
        let runtime = Runtime::new();
        // 检查默认线程的权重
        assert_eq!(runtime.threads[0].weight, prio_to_weight(20));
    }

    #[test]
    fn test_spawn_sets_weight() {
        let mut runtime = Runtime::new();
        runtime.init();

        let high_weight = prio_to_weight(0);
        runtime.spawn(0, || {
            println!("high priority task");
            yield_thread();
        });

        // 找到刚生成的线程
        let spawned = runtime.threads.iter().find(|t| t.state == State::Ready).unwrap();
        assert_eq!(spawned.weight, high_weight);
    }
}
