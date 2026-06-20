//! 有栈协程（Green Thread）模型 + 三种调度策略。
//!
//! 在用户态通过 `naked_asm!` 进行寄存器级上下文切换，
//! 实现协作式多任务。每个 green thread 拥有独立的栈空间。
//!
//! 支持三种调度模式：
//! - **CFQ**：vruntime 比例公平调度（默认）
//! - **HighestPriorityFirst**：严格优先级（高优先者独占 CPU）
//! - **RoundRobin**：等权循环轮转

use crate::scheduler::{CfqScheduler, prio_to_weight};
use std::arch::naked_asm;
use std::ptr;

const DEFAULT_STACK_SIZE: usize = 1024 * 1024 * 2;
const MAX_THREADS: usize = 4;
static mut RUNTIME: usize = 0;

/// 调度模式
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SchedulerMode {
    /// vruntime 比例公平（默认）
    Cfq,
    /// 严格优先级：总是选就绪线程中 priority 最小的
    HighestPriorityFirst,
    /// 等权循环轮转
    RoundRobin,
}

pub struct Runtime {
    threads: Vec<Thread>,
    /// CFQ 调度器（仅 Cfq 模式使用）
    scheduler: CfqScheduler<usize>,
    /// 调度模式
    mode: SchedulerMode,
    /// RoundRobin 的下一个候选起始位置
    next_rr_start: usize,
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
    /// 优先级值（值越小优先级越高）
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
    /// 创建使用 CFQ 调度的 Runtime（向后兼容）。
    pub fn new() -> Self {
        Self::with_mode(SchedulerMode::Cfq)
    }

    /// 创建指定调度模式的 Runtime。
    pub fn with_mode(mode: SchedulerMode) -> Self {
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
        for i in 0..threads.len() {
            threads[i].ctx.thread_ptr = &threads[i] as *const Thread as u64;
        }

        Runtime {
            threads,
            scheduler: CfqScheduler::new(),
            mode,
            next_rr_start: 1,
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

    // ── 调度核心 ──────────────────────────────────────────

    /// 统一的上下文调度入口。根据 mode 分发到不同的选择策略。
    fn t_yield(&mut self) -> bool {
        let cur = &self.threads[self.current];
        let cur_id = cur.id;
        let cur_priority = cur.priority;
        let is_available = cur.state == State::Available;

        // CFQ 模式：将当前线程重新入队
        if self.mode == SchedulerMode::Cfq && !is_available && cur_id != 0 {
            self.scheduler
                .update_and_push(cur_id, cur_priority, 1_000_000);
        }

        // 选择下一个就绪线程
        let next_id = match self.mode {
            SchedulerMode::Cfq => self.scheduler.pop(),
            SchedulerMode::HighestPriorityFirst => self.find_highest_priority_ready(cur_id),
            SchedulerMode::RoundRobin => self.find_next_round_robin(cur_id),
        };

        match next_id {
            Some(next_id) => {
                let pos = self
                    .threads
                    .iter()
                    .position(|t| t.id == next_id && (t.state == State::Ready || t.state == State::Running));

                if let Some(pos) = pos {
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
                false // 找不到对应线程（已结束）
            }
            None => {
                // 没有就绪线程 → 切回主线程
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
                false
            }
        }
    }

    /// 严格优先级：遍历所有 Ready 线程，选 priority 值最小的。
    fn find_highest_priority_ready(&self, current_id: usize) -> Option<usize> {
        self.threads
            .iter()
            .filter(|t| {
                (t.state == State::Ready || t.state == State::Running)
                    && t.id != current_id
                    && t.id != 0 // 排除主线程（id=0）
            })
            .min_by_key(|t| t.priority)
            .map(|t| t.id)
    }

    /// 循环轮转：从上次位置开始，找下一个 Ready 线程。
    fn find_next_round_robin(&mut self, current_id: usize) -> Option<usize> {
        let len = self.threads.len();
        if len <= 1 {
            return None;
        }

        // 从 next_rr_start 开始扫描
        for offset in 0..len {
            let idx = (self.next_rr_start + offset) % len;
            let t = &self.threads[idx];
            if (t.state == State::Ready || t.state == State::Running)
                && t.id != current_id
                && t.id != 0
            {
                // 下次从下一个位置开始
                self.next_rr_start = (idx + 1) % len;
                if self.next_rr_start == 0 {
                    self.next_rr_start = 1;
                }
                return Some(t.id);
            }
        }
        None
    }

    // ── 生成绿色线程 ──────────────────────────────────────

    /// 生成一个新的 green thread，指定优先级。
    ///
    /// `priority` 值越小优先级越高。
    /// - CFQ 模式：priority 映射为 weight，线程加入 CfqScheduler
    /// - HPF 模式：priority 直接用于 `find_highest_priority_ready`
    /// - RR 模式：priority 被忽略（等权轮转）
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

            ptr::write(s_ptr.offset((size - 8) as isize) as *mut u64, guard as *const () as u64);
            ptr::write(s_ptr.offset((size - 16) as isize) as *mut u64, call as *const () as u64);
            available.ctx.rsp = s_ptr.offset((size - 16) as isize) as u64;

            let id = available.id;
            available.state = State::Ready;

            match self.mode {
                SchedulerMode::Cfq => {
                    self.scheduler.push(id, priority);
                }
                SchedulerMode::HighestPriorityFirst | SchedulerMode::RoundRobin => {
                    // 不需要显式入队——t_yield 通过扫描 Thread 状态来选任务
                    // 只需要把 state 设为 Ready 即可
                }
            }
        }
    }
}

fn call(thread: u64) {
    let thread = unsafe { &*(thread as *const Thread) };
    if let Some(f) = &thread.task {
        f();
    }
}

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

/// 上下文切换（x86_64 System V ABI: old in rdi, new in rsi）
#[unsafe(naked)]
unsafe extern "C" fn switch(_old: *mut ThreadContext, _new: *const ThreadContext) {
    naked_asm!(
        "mov     [rdi], rsp",
        "mov     [rdi + 0x08], r15",
        "mov     [rdi + 0x10], r14",
        "mov     [rdi + 0x18], r13",
        "mov     [rdi + 0x20], r12",
        "mov     [rdi + 0x28], rbx",
        "mov     [rdi + 0x30], rbp",
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

        let spawned = runtime.threads.iter().find(|t| t.state == State::Ready).unwrap();
        assert_eq!(spawned.weight, high_weight);
    }

    #[test]
    fn test_hpf_selects_highest_priority() {
        let mut rt = Runtime::with_mode(SchedulerMode::HighestPriorityFirst);
        // 手动设置线程状态来测试选择逻辑（不触发真正的上下文切换）
        rt.threads[1].state = State::Ready;
        rt.threads[1].priority = 20; // 低优先级
        rt.threads[2].state = State::Ready;
        rt.threads[2].priority = 0;  // 高优先级
        rt.threads[3].state = State::Ready;
        rt.threads[3].priority = 10; // 中优先级

        // 从 current=0 调用，应该选中 priority 最小的（thread 2，prio=0）
        let next = rt.find_highest_priority_ready(0);
        assert_eq!(next, Some(2), "HPF should select highest priority (lowest prio value)");
    }

    #[test]
    fn test_hpf_respects_state() {
        let mut rt = Runtime::with_mode(SchedulerMode::HighestPriorityFirst);
        rt.threads[1].state = State::Available; // 不可调度
        rt.threads[1].priority = 0;              // 即使优先级最高也不应被选中
        rt.threads[2].state = State::Ready;
        rt.threads[2].priority = 30;

        let next = rt.find_highest_priority_ready(0);
        assert_eq!(next, Some(2), "HPF should skip Available threads");
    }

    #[test]
    fn test_round_robin_cycles() {
        let mut rt = Runtime::with_mode(SchedulerMode::RoundRobin);
        rt.threads[1].state = State::Ready;
        rt.threads[2].state = State::Ready;
        rt.threads[3].state = State::Ready;

        let a = rt.find_next_round_robin(0).unwrap();
        let b = rt.find_next_round_robin(a).unwrap();
        let c = rt.find_next_round_robin(b).unwrap();
        let d = rt.find_next_round_robin(c).unwrap(); // 应回到第一个

        // 三个线程应该被轮流选中
        let mut ids = vec![a, b, c, d];
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 3, "RR should cycle through all 3 ready threads");
    }

    #[test]
    fn test_round_robin_skips_available() {
        let mut rt = Runtime::with_mode(SchedulerMode::RoundRobin);
        rt.threads[1].state = State::Available;
        rt.threads[2].state = State::Ready;
        rt.threads[3].state = State::Available;

        // 只有 thread 2 可调度
        let next = rt.find_next_round_robin(1);
        assert_eq!(next, Some(2));
    }

    #[test]
    fn test_cfq_mode_is_default() {
        let rt = Runtime::new();
        assert_eq!(rt.mode, SchedulerMode::Cfq);
    }

    #[test]
    fn test_hpf_mode_no_scheduler_needed() {
        let mut rt = Runtime::with_mode(SchedulerMode::HighestPriorityFirst);
        rt.init();
        // HPF 模式下 spawn 不应 panic（不依赖 CfqScheduler）
        rt.threads[1].state = State::Available;
        rt.spawn(5, || {});
        assert_eq!(rt.threads[1].state, State::Ready);
        assert_eq!(rt.threads[1].priority, 5);
    }

    #[test]
    fn test_rr_mode_no_scheduler_needed() {
        let mut rt = Runtime::with_mode(SchedulerMode::RoundRobin);
        rt.init();
        rt.threads[1].state = State::Available;
        rt.spawn(10, || {});
        assert_eq!(rt.threads[1].state, State::Ready);
        // RR 模式忽略 priority 值，但字段仍然存储
        assert_eq!(rt.threads[1].priority, 10);
    }
}
