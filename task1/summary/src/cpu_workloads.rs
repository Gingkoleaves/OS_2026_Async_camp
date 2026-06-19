//! CPU 密集型任务定义。
//!
//! 提供多种标准化 CPU 任务，用于跨运行时性能对比：
//! - 校验和（内存带宽敏感）
//! - 质数筛选（整数运算密集）
//! - 矩阵乘法（浮点运算密集）
//! - 哈希计算（混合运算）

use std::time::Instant;

// ---------------------------------------------------------------------------
// 任务类型
// ---------------------------------------------------------------------------

/// CPU 任务类型枚举
#[derive(Clone, Debug)]
pub enum CpuTaskType {
    /// 校验和：对 bytes 字节数据做 rounds 轮累加
    Checksum { bytes: usize, rounds: u64 },
    /// 埃拉托斯特尼筛法求 limit 以内的质数
    PrimeSieve { limit: u64 },
    /// size×size 矩阵乘法（f64）
    MatrixMul { size: usize },
    /// 对 input_size 字节做 rounds 轮 SHA256
    Hash { input_size: usize, rounds: u64 },
}

/// 负载强度
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intensity {
    Light,
    Medium,
    Heavy,
}

/// 一个可运行的 CPU 任务实例
#[derive(Clone, Debug)]
pub struct CpuWorkload {
    pub task_type: CpuTaskType,
    pub label: &'static str,
    pub intensity: Intensity,
    /// 协作式调度中的 yield 间隔（每 N 次迭代 yield 一次），None = 不 yield
    pub yield_every: Option<usize>,
}

// ---------------------------------------------------------------------------
// 预定义任务
// ---------------------------------------------------------------------------

impl CpuWorkload {
    /// 校验和 — 中等数据量
    pub fn checksum_light() -> Self {
        Self {
            task_type: CpuTaskType::Checksum {
                bytes: 1024 * 1024, // 1 MB
                rounds: 10,
            },
            label: "checksum-light",
            intensity: Intensity::Light,
            yield_every: Some(100),
        }
    }

    pub fn checksum_medium() -> Self {
        Self {
            task_type: CpuTaskType::Checksum {
                bytes: 4 * 1024 * 1024, // 4 MB
                rounds: 50,
            },
            label: "checksum-medium",
            intensity: Intensity::Medium,
            yield_every: Some(100),
        }
    }

    pub fn checksum_heavy() -> Self {
        Self {
            task_type: CpuTaskType::Checksum {
                bytes: 16 * 1024 * 1024, // 16 MB
                rounds: 100,
            },
            label: "checksum-heavy",
            intensity: Intensity::Heavy,
            yield_every: Some(100),
        }
    }

    /// 质数筛选 — 轻量
    pub fn prime_light() -> Self {
        Self {
            task_type: CpuTaskType::PrimeSieve { limit: 100_000 },
            label: "prime-light",
            intensity: Intensity::Light,
            yield_every: Some(100),
        }
    }

    pub fn prime_medium() -> Self {
        Self {
            task_type: CpuTaskType::PrimeSieve { limit: 1_000_000 },
            label: "prime-medium",
            intensity: Intensity::Medium,
            yield_every: Some(100),
        }
    }

    pub fn prime_heavy() -> Self {
        Self {
            task_type: CpuTaskType::PrimeSieve { limit: 5_000_000 },
            label: "prime-heavy",
            intensity: Intensity::Heavy,
            yield_every: Some(100),
        }
    }

    /// 矩阵乘法 — 轻量
    pub fn matrix_light() -> Self {
        Self {
            task_type: CpuTaskType::MatrixMul { size: 64 },
            label: "matrix-light",
            intensity: Intensity::Light,
            yield_every: Some(10),
        }
    }

    pub fn matrix_medium() -> Self {
        Self {
            task_type: CpuTaskType::MatrixMul { size: 256 },
            label: "matrix-medium",
            intensity: Intensity::Medium,
            yield_every: Some(10),
        }
    }

    pub fn matrix_heavy() -> Self {
        Self {
            task_type: CpuTaskType::MatrixMul { size: 512 },
            label: "matrix-heavy",
            intensity: Intensity::Heavy,
            yield_every: Some(10),
        }
    }

    /// 哈希计算 — 轻量
    pub fn hash_light() -> Self {
        Self {
            task_type: CpuTaskType::Hash {
                input_size: 1024 * 64, // 64 KB
                rounds: 50,
            },
            label: "hash-light",
            intensity: Intensity::Light,
            yield_every: Some(100),
        }
    }

    pub fn hash_medium() -> Self {
        Self {
            task_type: CpuTaskType::Hash {
                input_size: 1024 * 256, // 256 KB
                rounds: 200,
            },
            label: "hash-medium",
            intensity: Intensity::Medium,
            yield_every: Some(100),
        }
    }

    pub fn hash_heavy() -> Self {
        Self {
            task_type: CpuTaskType::Hash {
                input_size: 1024 * 1024, // 1 MB
                rounds: 500,
            },
            label: "hash-heavy",
            intensity: Intensity::Heavy,
            yield_every: Some(100),
        }
    }

    /// 饥饿测试（无 yield 点）
    pub fn checksum_no_yield() -> Self {
        Self {
            task_type: CpuTaskType::Checksum {
                bytes: 8 * 1024 * 1024, // 8 MB
                rounds: 50,
            },
            label: "checksum-no-yield",
            intensity: Intensity::Heavy,
            yield_every: None,
        }
    }

    /// 所有预设任务
    pub fn all_presets() -> Vec<CpuWorkload> {
        vec![
            Self::checksum_light(),
            Self::checksum_medium(),
            Self::checksum_heavy(),
            Self::prime_light(),
            Self::prime_medium(),
            Self::prime_heavy(),
            Self::matrix_light(),
            Self::matrix_medium(),
            Self::matrix_heavy(),
            Self::hash_light(),
            Self::hash_medium(),
            Self::hash_heavy(),
        ]
    }
}

// ---------------------------------------------------------------------------
// 任务执行函数（纯计算，无任何 I/O）
// ---------------------------------------------------------------------------

/// 执行校验和任务，返回耗时
pub fn run_checksum(bytes: usize, rounds: u64, yield_every: Option<usize>) -> u64 {
    let data = vec![0u8; bytes];
    let mut checksum: u64 = 0;
    let start = Instant::now();

    for r in 0..rounds {
        for (i, &b) in data.iter().enumerate() {
            checksum = checksum
                .wrapping_add(b as u64)
                .wrapping_add(i as u64)
                .wrapping_mul(1103515245);
        }
        // 协作式 yield 点
        if let Some(every) = yield_every {
            if r as usize % every == 0 {
                cooperative_yield();
            }
        }
    }

    // 防止编译器优化掉计算
    std::hint::black_box(checksum);
    start.elapsed().as_micros() as u64
}

/// 执行质数筛选，返回耗时
pub fn run_prime_sieve(limit: u64, yield_every: Option<usize>) -> u64 {
    let start = Instant::now();
    let limit_usize = limit as usize;
    let mut is_prime = vec![true; limit_usize + 1];
    is_prime[0] = false;
    if limit_usize >= 1 {
        is_prime[1] = false;
    }

    let sqrt_limit = (limit as f64).sqrt() as usize;
    for i in 2..=sqrt_limit {
        if is_prime[i] {
            let mut j = i * i;
            while j <= limit_usize {
                is_prime[j] = false;
                j += i;
            }
        }
        if let Some(every) = yield_every {
            if i % every == 0 {
                cooperative_yield();
            }
        }
    }

    let count = is_prime.iter().filter(|&&p| p).count();
    std::hint::black_box(count);
    start.elapsed().as_micros() as u64
}

/// 执行矩阵乘法，返回耗时
pub fn run_matrix_mul(size: usize, yield_every: Option<usize>) -> u64 {
    let start = Instant::now();
    // 创建两个 size×size 矩阵并相乘
    let a: Vec<Vec<f64>> = (0..size)
        .map(|i| (0..size).map(|j| (i + j) as f64).collect())
        .collect();
    let b: Vec<Vec<f64>> = (0..size)
        .map(|i| (0..size).map(|j| (i * j) as f64).collect())
        .collect();
    let mut c: Vec<Vec<f64>> = vec![vec![0.0; size]; size];

    for i in 0..size {
        for k in 0..size {
            let aik = a[i][k];
            for j in 0..size {
                c[i][j] += aik * b[k][j];
            }
        }
        if let Some(every) = yield_every {
            if i % every == 0 {
                cooperative_yield();
            }
        }
    }

    let sum: f64 = c.iter().flat_map(|row| row.iter()).sum();
    std::hint::black_box(sum);
    start.elapsed().as_micros() as u64
}

/// 执行哈希计算，返回耗时
pub fn run_hash(input_size: usize, rounds: u64, yield_every: Option<usize>) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let start = Instant::now();
    let data = vec![0xABu8; input_size];
    let mut final_hash: u64 = 0;

    for r in 0..rounds {
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        r.hash(&mut hasher);
        let h = hasher.finish();
        final_hash = final_hash.wrapping_add(h);
        if let Some(every) = yield_every {
            if r as usize % every == 0 {
                cooperative_yield();
            }
        }
    }

    std::hint::black_box(final_hash);
    start.elapsed().as_micros() as u64
}

/// 执行一个 CpuWorkload，返回耗时（微秒）
pub fn run_workload(wl: &CpuWorkload) -> u64 {
    match &wl.task_type {
        CpuTaskType::Checksum { bytes, rounds } => run_checksum(*bytes, *rounds, wl.yield_every),
        CpuTaskType::PrimeSieve { limit } => run_prime_sieve(*limit, wl.yield_every),
        CpuTaskType::MatrixMul { size } => run_matrix_mul(*size, wl.yield_every),
        CpuTaskType::Hash {
            input_size,
            rounds,
        } => run_hash(*input_size, *rounds, wl.yield_every),
    }
}

// ---------------------------------------------------------------------------
// 协作式 yield（被 green-thread 等运行时重写）
// ---------------------------------------------------------------------------

/// 协作式 yield 钩子。
///
/// 默认是空操作。green-thread / stackless 运行时可以设置自己的 yield 函数。
static YIELD_FN: std::sync::atomic::AtomicPtr<()> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

/// 设置自定义 yield 函数
pub fn set_yield_fn(f: fn()) {
    YIELD_FN.store(f as *mut (), std::sync::atomic::Ordering::Release);
}

/// 清除自定义 yield 函数
pub fn clear_yield_fn() {
    YIELD_FN.store(std::ptr::null_mut(), std::sync::atomic::Ordering::Release);
}

fn cooperative_yield() {
    let ptr = YIELD_FN.load(std::sync::atomic::Ordering::Acquire);
    if !ptr.is_null() {
        let f: fn() = unsafe { std::mem::transmute(ptr) };
        f();
    }
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_light() {
        let us = run_checksum(1024, 10, None);
        assert!(us > 0);
    }

    #[test]
    fn test_prime_sieve() {
        let us = run_prime_sieve(1000, None);
        assert!(us > 0);
    }

    #[test]
    fn test_matrix_mul() {
        let us = run_matrix_mul(32, None);
        assert!(us > 0);
    }

    #[test]
    fn test_hash() {
        let us = run_hash(1024, 10, None);
        assert!(us > 0);
    }

    #[test]
    fn test_all_presets_have_labels() {
        let presets = CpuWorkload::all_presets();
        assert!(!presets.is_empty());
        for p in &presets {
            assert!(!p.label.is_empty());
        }
    }
}
