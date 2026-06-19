# 多运行时多任务性能对比报告

> 自动生成于 2026-06-19 22:51:22

## 目录

1. [实验设计](#实验设计)
2. [总览矩阵](#总览矩阵)
3. [按任务类型分析](#按任务类型分析)
4. [按运行时分分析](#按运行时分析)
5. [优先级调度对比](#优先级调度对比)
6. [内存开销分析](#内存开销分析)
7. [结论](#结论)

## 总览矩阵

### 总耗时对比 (ms)

| Workload | embassy (sim) | green-thread (CFQ) | process (fork) | stackless (CFQ) | thread (std) | tokio (async) |
|---|---|---|---|---|---|---|
| checksum-light | 280 | 276 | 73 | 285 | 70 | 76 |
| hash-light | 26 | 28 | 9 | 25 | 7 | 7 |
| matrix-light | 12 | 13 | 6 | 11 | 6 | 6 |
| prime-light | 4 | 5 | 5 | 4 | 3 | 3 |

### 吞吐率对比 (tasks/sec)

| Workload | embassy (sim) | green-thread (CFQ) | process (fork) | stackless (CFQ) | thread (std) | tokio (async) |
|---|---|---|---|---|---|---|
| checksum-light | 14.3 | 14.4 | 54.1 | 14.0 | 57.0 | 52.4 |
| hash-light | 151.5 | 139.1 | 435.6 | 159.9 | 563.0 | 553.5 |
| matrix-light | 326.6 | 307.0 | 581.0 | 346.6 | 642.2 | 630.5 |
| prime-light | 806.7 | 771.5 | 794.3 | 834.5 | 1029.8 | 1084.5 |

## 按任务类型分析

### checksum-light 

**checksum-light** (concurrency=4)

| Runner | Total(ms) | Avg Lat | P50 | P95 | P99 | Throughput | RSS(MB) |
|---|---|---|---|---|---|---|---|
| tokio (async) | 76 | 74.20ms | 74.97ms | 74.99ms | 74.99ms | 52.4 | 4.1 |
| thread (std) | 70 | 67.75ms | 68.14ms | 69.83ms | 69.83ms | 57.0 | 7.9 |
| process (fork) | 73 | 70.49ms | 71.28ms | 71.58ms | 71.58ms | 54.1 | 7.9 |
| green-thread (CFQ) | 276 | 69.22ms | 69.52ms | 70.16ms | 70.16ms | 14.4 | 8.9 |
| stackless (CFQ) | 285 | 71.32ms | 72.43ms | 72.82ms | 72.82ms | 14.0 | 8.9 |
| embassy (sim) | 280 | 70.08ms | 71.53ms | 71.61ms | 71.61ms | 14.3 | 8.9 |

### hash-light 

**hash-light** (concurrency=4)

| Runner | Total(ms) | Avg Lat | P50 | P95 | P99 | Throughput | RSS(MB) |
|---|---|---|---|---|---|---|---|
| tokio (async) | 7 | 6.62ms | 6.65ms | 6.97ms | 6.97ms | 553.5 | 4.8 |
| thread (std) | 7 | 6.71ms | 6.73ms | 6.77ms | 6.77ms | 563.0 | 7.9 |
| process (fork) | 9 | 7.44ms | 7.44ms | 7.47ms | 7.47ms | 435.6 | 7.9 |
| green-thread (CFQ) | 28 | 7.19ms | 7.20ms | 7.26ms | 7.26ms | 139.1 | 8.9 |
| stackless (CFQ) | 25 | 6.25ms | 6.30ms | 6.34ms | 6.34ms | 159.9 | 8.9 |
| embassy (sim) | 26 | 6.60ms | 6.59ms | 6.71ms | 6.71ms | 151.5 | 8.9 |

### matrix-light 

**matrix-light** (concurrency=4)

| Runner | Total(ms) | Avg Lat | P50 | P95 | P99 | Throughput | RSS(MB) |
|---|---|---|---|---|---|---|---|
| tokio (async) | 6 | 4.34ms | 5.19ms | 5.88ms | 5.88ms | 630.5 | 4.7 |
| thread (std) | 6 | 4.58ms | 4.76ms | 5.86ms | 5.86ms | 642.2 | 7.9 |
| process (fork) | 6 | 4.25ms | 4.62ms | 4.67ms | 4.67ms | 581.0 | 7.9 |
| green-thread (CFQ) | 13 | 3.26ms | 3.35ms | 3.43ms | 3.43ms | 307.0 | 8.9 |
| stackless (CFQ) | 11 | 2.88ms | 2.92ms | 2.93ms | 2.93ms | 346.6 | 8.9 |
| embassy (sim) | 12 | 3.06ms | 3.21ms | 3.31ms | 3.31ms | 326.6 | 8.9 |

### prime-light 

**prime-light** (concurrency=4)

| Runner | Total(ms) | Avg Lat | P50 | P95 | P99 | Throughput | RSS(MB) |
|---|---|---|---|---|---|---|---|
| tokio (async) | 3 | 3.27ms | 3.33ms | 3.33ms | 3.33ms | 1084.5 | 4.2 |
| thread (std) | 3 | 3.16ms | 3.52ms | 3.54ms | 3.54ms | 1029.8 | 7.9 |
| process (fork) | 5 | 2.86ms | 3.08ms | 3.08ms | 3.08ms | 794.3 | 7.9 |
| green-thread (CFQ) | 5 | 1.29ms | 1.20ms | 1.61ms | 1.61ms | 771.5 | 8.9 |
| stackless (CFQ) | 4 | 1.20ms | 1.20ms | 1.24ms | 1.24ms | 834.5 | 8.9 |
| embassy (sim) | 4 | 1.24ms | 1.28ms | 1.28ms | 1.28ms | 806.7 | 8.9 |

## 延迟分布详情

### tokio (async) — checksum-light

- 并发数: 4
- 平均延迟: 74.20ms
- P50: 74.97ms
- P95: 74.99ms
- P99: 74.99ms
- Min/Max: 73.39ms / 74.99ms

```
min   │███████████████████████████████████████  73.39ms
p50   │███████████████████████████████████████  74.97ms
avg   │███████████████████████████████████████  74.20ms
p95   │████████████████████████████████████████ 74.99ms
p99   │████████████████████████████████████████ 74.99ms
max   │████████████████████████████████████████ 74.99ms

```

### tokio (async) — prime-light

- 并发数: 4
- 平均延迟: 3.27ms
- P50: 3.33ms
- P95: 3.33ms
- P99: 3.33ms
- Min/Max: 3.21ms / 3.33ms

```
min   │██████████████████████████████████████   3.21ms
p50   │███████████████████████████████████████  3.33ms
avg   │███████████████████████████████████████  3.27ms
p95   │████████████████████████████████████████ 3.33ms
p99   │████████████████████████████████████████ 3.33ms
max   │████████████████████████████████████████ 3.33ms

```

### tokio (async) — matrix-light

- 并发数: 4
- 平均延迟: 4.34ms
- P50: 5.19ms
- P95: 5.88ms
- P99: 5.88ms
- Min/Max: 3.14ms / 5.88ms

```
min   │█████████████████████                    3.14ms
p50   │███████████████████████████████████      5.19ms
avg   │█████████████████████████████            4.34ms
p95   │████████████████████████████████████████ 5.88ms
p99   │████████████████████████████████████████ 5.88ms
max   │████████████████████████████████████████ 5.88ms

```

### tokio (async) — hash-light

- 并发数: 4
- 平均延迟: 6.62ms
- P50: 6.65ms
- P95: 6.97ms
- P99: 6.97ms
- Min/Max: 6.27ms / 6.97ms

```
min   │████████████████████████████████████     6.27ms
p50   │██████████████████████████████████████   6.65ms
avg   │██████████████████████████████████████   6.62ms
p95   │████████████████████████████████████████ 6.97ms
p99   │████████████████████████████████████████ 6.97ms
max   │████████████████████████████████████████ 6.97ms

```

### thread (std) — checksum-light

- 并发数: 4
- 平均延迟: 67.75ms
- P50: 68.14ms
- P95: 69.83ms
- P99: 69.83ms
- Min/Max: 65.46ms / 69.83ms

```
min   │█████████████████████████████████████    65.46ms
p50   │███████████████████████████████████████  68.14ms
avg   │██████████████████████████████████████   67.75ms
p95   │████████████████████████████████████████ 69.83ms
p99   │████████████████████████████████████████ 69.83ms
max   │████████████████████████████████████████ 69.83ms

```

### thread (std) — prime-light

- 并发数: 4
- 平均延迟: 3.16ms
- P50: 3.52ms
- P95: 3.54ms
- P99: 3.54ms
- Min/Max: 2.07ms / 3.54ms

```
min   │███████████████████████                  2.07ms
p50   │███████████████████████████████████████  3.52ms
avg   │███████████████████████████████████      3.16ms
p95   │████████████████████████████████████████ 3.54ms
p99   │████████████████████████████████████████ 3.54ms
max   │████████████████████████████████████████ 3.54ms

```

### thread (std) — matrix-light

- 并发数: 4
- 平均延迟: 4.58ms
- P50: 4.76ms
- P95: 5.86ms
- P99: 5.86ms
- Min/Max: 3.29ms / 5.86ms

```
min   │██████████████████████                   3.29ms
p50   │████████████████████████████████         4.76ms
avg   │███████████████████████████████          4.58ms
p95   │████████████████████████████████████████ 5.86ms
p99   │████████████████████████████████████████ 5.86ms
max   │████████████████████████████████████████ 5.86ms

```

### thread (std) — hash-light

- 并发数: 4
- 平均延迟: 6.71ms
- P50: 6.73ms
- P95: 6.77ms
- P99: 6.77ms
- Min/Max: 6.63ms / 6.77ms

```
min   │███████████████████████████████████████  6.63ms
p50   │███████████████████████████████████████  6.73ms
avg   │███████████████████████████████████████  6.71ms
p95   │████████████████████████████████████████ 6.77ms
p99   │████████████████████████████████████████ 6.77ms
max   │████████████████████████████████████████ 6.77ms

```

### process (fork) — checksum-light

- 并发数: 4
- 平均延迟: 70.49ms
- P50: 71.28ms
- P95: 71.58ms
- P99: 71.58ms
- Min/Max: 69.25ms / 71.58ms

```
min   │██████████████████████████████████████   69.25ms
p50   │███████████████████████████████████████  71.28ms
avg   │███████████████████████████████████████  70.49ms
p95   │████████████████████████████████████████ 71.58ms
p99   │████████████████████████████████████████ 71.58ms
max   │████████████████████████████████████████ 71.58ms

```

### process (fork) — prime-light

- 并发数: 4
- 平均延迟: 2.86ms
- P50: 3.08ms
- P95: 3.08ms
- P99: 3.08ms
- Min/Max: 2.20ms / 3.08ms

```
min   │████████████████████████████             2.20ms
p50   │███████████████████████████████████████  3.08ms
avg   │█████████████████████████████████████    2.86ms
p95   │████████████████████████████████████████ 3.08ms
p99   │████████████████████████████████████████ 3.08ms
max   │████████████████████████████████████████ 3.08ms

```

### process (fork) — matrix-light

- 并发数: 4
- 平均延迟: 4.25ms
- P50: 4.62ms
- P95: 4.67ms
- P99: 4.67ms
- Min/Max: 3.14ms / 4.67ms

```
min   │██████████████████████████               3.14ms
p50   │███████████████████████████████████████  4.62ms
avg   │████████████████████████████████████     4.25ms
p95   │████████████████████████████████████████ 4.67ms
p99   │████████████████████████████████████████ 4.67ms
max   │████████████████████████████████████████ 4.67ms

```

### process (fork) — hash-light

- 并发数: 4
- 平均延迟: 7.44ms
- P50: 7.44ms
- P95: 7.47ms
- P99: 7.47ms
- Min/Max: 7.42ms / 7.47ms

```
min   │███████████████████████████████████████  7.42ms
p50   │███████████████████████████████████████  7.44ms
avg   │███████████████████████████████████████  7.44ms
p95   │████████████████████████████████████████ 7.47ms
p99   │████████████████████████████████████████ 7.47ms
max   │████████████████████████████████████████ 7.47ms

```

### green-thread (CFQ) — checksum-light

- 并发数: 4
- 平均延迟: 69.22ms
- P50: 69.52ms
- P95: 70.16ms
- P99: 70.16ms
- Min/Max: 68.23ms / 70.16ms

```
min   │██████████████████████████████████████   68.23ms
p50   │███████████████████████████████████████  69.52ms
avg   │███████████████████████████████████████  69.22ms
p95   │████████████████████████████████████████ 70.16ms
p99   │████████████████████████████████████████ 70.16ms
max   │████████████████████████████████████████ 70.16ms

```

### green-thread (CFQ) — prime-light

- 并发数: 4
- 平均延迟: 1.29ms
- P50: 1.20ms
- P95: 1.61ms
- P99: 1.61ms
- Min/Max: 1.17ms / 1.61ms

```
min   │█████████████████████████████            1.17ms
p50   │█████████████████████████████            1.20ms
avg   │████████████████████████████████         1.29ms
p95   │████████████████████████████████████████ 1.61ms
p99   │████████████████████████████████████████ 1.61ms
max   │████████████████████████████████████████ 1.61ms

```

### green-thread (CFQ) — matrix-light

- 并发数: 4
- 平均延迟: 3.26ms
- P50: 3.35ms
- P95: 3.43ms
- P99: 3.43ms
- Min/Max: 3.09ms / 3.43ms

```
min   │███████████████████████████████████      3.09ms
p50   │██████████████████████████████████████   3.35ms
avg   │█████████████████████████████████████    3.26ms
p95   │████████████████████████████████████████ 3.43ms
p99   │████████████████████████████████████████ 3.43ms
max   │████████████████████████████████████████ 3.43ms

```

### green-thread (CFQ) — hash-light

- 并发数: 4
- 平均延迟: 7.19ms
- P50: 7.20ms
- P95: 7.26ms
- P99: 7.26ms
- Min/Max: 7.09ms / 7.26ms

```
min   │███████████████████████████████████████  7.09ms
p50   │███████████████████████████████████████  7.20ms
avg   │███████████████████████████████████████  7.19ms
p95   │████████████████████████████████████████ 7.26ms
p99   │████████████████████████████████████████ 7.26ms
max   │████████████████████████████████████████ 7.26ms

```

### stackless (CFQ) — checksum-light

- 并发数: 4
- 平均延迟: 71.32ms
- P50: 72.43ms
- P95: 72.82ms
- P99: 72.82ms
- Min/Max: 68.30ms / 72.82ms

```
min   │█████████████████████████████████████    68.30ms
p50   │███████████████████████████████████████  72.43ms
avg   │███████████████████████████████████████  71.32ms
p95   │████████████████████████████████████████ 72.82ms
p99   │████████████████████████████████████████ 72.82ms
max   │████████████████████████████████████████ 72.82ms

```

### stackless (CFQ) — prime-light

- 并发数: 4
- 平均延迟: 1.20ms
- P50: 1.20ms
- P95: 1.24ms
- P99: 1.24ms
- Min/Max: 1.16ms / 1.24ms

```
min   │█████████████████████████████████████    1.16ms
p50   │██████████████████████████████████████   1.20ms
avg   │██████████████████████████████████████   1.20ms
p95   │████████████████████████████████████████ 1.24ms
p99   │████████████████████████████████████████ 1.24ms
max   │████████████████████████████████████████ 1.24ms

```

### stackless (CFQ) — matrix-light

- 并发数: 4
- 平均延迟: 2.88ms
- P50: 2.92ms
- P95: 2.93ms
- P99: 2.93ms
- Min/Max: 2.81ms / 2.93ms

```
min   │██████████████████████████████████████   2.81ms
p50   │███████████████████████████████████████  2.92ms
avg   │███████████████████████████████████████  2.88ms
p95   │████████████████████████████████████████ 2.93ms
p99   │████████████████████████████████████████ 2.93ms
max   │████████████████████████████████████████ 2.93ms

```

### stackless (CFQ) — hash-light

- 并发数: 4
- 平均延迟: 6.25ms
- P50: 6.30ms
- P95: 6.34ms
- P99: 6.34ms
- Min/Max: 6.18ms / 6.34ms

```
min   │██████████████████████████████████████   6.18ms
p50   │███████████████████████████████████████  6.30ms
avg   │███████████████████████████████████████  6.25ms
p95   │████████████████████████████████████████ 6.34ms
p99   │████████████████████████████████████████ 6.34ms
max   │████████████████████████████████████████ 6.34ms

```

### embassy (sim) — checksum-light

- 并发数: 4
- 平均延迟: 70.08ms
- P50: 71.53ms
- P95: 71.61ms
- P99: 71.61ms
- Min/Max: 68.28ms / 71.61ms

```
min   │██████████████████████████████████████   68.28ms
p50   │███████████████████████████████████████  71.53ms
avg   │███████████████████████████████████████  70.08ms
p95   │████████████████████████████████████████ 71.61ms
p99   │████████████████████████████████████████ 71.61ms
max   │████████████████████████████████████████ 71.61ms

```

### embassy (sim) — prime-light

- 并发数: 4
- 平均延迟: 1.24ms
- P50: 1.28ms
- P95: 1.28ms
- P99: 1.28ms
- Min/Max: 1.19ms / 1.28ms

```
min   │█████████████████████████████████████    1.19ms
p50   │███████████████████████████████████████  1.28ms
avg   │██████████████████████████████████████   1.24ms
p95   │████████████████████████████████████████ 1.28ms
p99   │████████████████████████████████████████ 1.28ms
max   │████████████████████████████████████████ 1.28ms

```

### embassy (sim) — matrix-light

- 并发数: 4
- 平均延迟: 3.06ms
- P50: 3.21ms
- P95: 3.31ms
- P99: 3.31ms
- Min/Max: 2.84ms / 3.31ms

```
min   │██████████████████████████████████       2.84ms
p50   │██████████████████████████████████████   3.21ms
avg   │████████████████████████████████████     3.06ms
p95   │████████████████████████████████████████ 3.31ms
p99   │████████████████████████████████████████ 3.31ms
max   │████████████████████████████████████████ 3.31ms

```

### embassy (sim) — hash-light

- 并发数: 4
- 平均延迟: 6.60ms
- P50: 6.59ms
- P95: 6.71ms
- P99: 6.71ms
- Min/Max: 6.53ms / 6.71ms

```
min   │██████████████████████████████████████   6.53ms
p50   │███████████████████████████████████████  6.59ms
avg   │███████████████████████████████████████  6.60ms
p95   │████████████████████████████████████████ 6.71ms
p99   │████████████████████████████████████████ 6.71ms
max   │████████████████████████████████████████ 6.71ms

```

## 内存开销分析

| Runner | Avg RSS (KB) | Peak RSS (KB) | Peak RSS (MB) |
|---|---|---|---|
| embassy (sim) | 9136 | 9136 | 8.9 |
| green-thread (CFQ) | 9084 | 9088 | 8.9 |
| process (fork) | 8057 | 8060 | 7.9 |
| stackless (CFQ) | 9132 | 9132 | 8.9 |
| thread (std) | 8056 | 8056 | 7.9 |
| tokio (async) | 4544 | 4888 | 4.8 |

> 注：内存数据来自 /proc/self/status VmRSS，反映整个进程的驻留集大小。
> process (fork) runner 的内存数据包含父子进程总和。
## 原始数据

```json
[
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 74205.5,
    "latency_max_us": 74986,
    "latency_min_us": 73385,
    "latency_p50_us": 74970,
    "latency_p95_us": 74986,
    "latency_p99_us": 74986,
    "peak_rss_kb": 4168,
    "runner": "tokio (async)",
    "success": 4,
    "total_elapsed_ms": 76,
    "workload": "checksum-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 3268.5,
    "latency_max_us": 3334,
    "latency_min_us": 3205,
    "latency_p50_us": 3325,
    "latency_p95_us": 3334,
    "latency_p99_us": 3334,
    "peak_rss_kb": 4352,
    "runner": "tokio (async)",
    "success": 4,
    "total_elapsed_ms": 3,
    "workload": "prime-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 4336.0,
    "latency_max_us": 5875,
    "latency_min_us": 3138,
    "latency_p50_us": 5188,
    "latency_p95_us": 5875,
    "latency_p99_us": 5875,
    "peak_rss_kb": 4768,
    "runner": "tokio (async)",
    "success": 4,
    "total_elapsed_ms": 6,
    "workload": "matrix-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 6622.0,
    "latency_max_us": 6966,
    "latency_min_us": 6270,
    "latency_p50_us": 6646,
    "latency_p95_us": 6966,
    "latency_p99_us": 6966,
    "peak_rss_kb": 4888,
    "runner": "tokio (async)",
    "success": 4,
    "total_elapsed_ms": 7,
    "workload": "hash-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 67746.0,
    "latency_max_us": 69826,
    "latency_min_us": 65457,
    "latency_p50_us": 68137,
    "latency_p95_us": 69826,
    "latency_p99_us": 69826,
    "peak_rss_kb": 8056,
    "runner": "thread (std)",
    "success": 4,
    "total_elapsed_ms": 70,
    "workload": "checksum-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 3163.75,
    "latency_max_us": 3545,
    "latency_min_us": 2067,
    "latency_p50_us": 3525,
    "latency_p95_us": 3545,
    "latency_p99_us": 3545,
    "peak_rss_kb": 8056,
    "runner": "thread (std)",
    "success": 4,
    "total_elapsed_ms": 3,
    "workload": "prime-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 4577.75,
    "latency_max_us": 5859,
    "latency_min_us": 3294,
    "latency_p50_us": 4761,
    "latency_p95_us": 5859,
    "latency_p99_us": 5859,
    "peak_rss_kb": 8056,
    "runner": "thread (std)",
    "success": 4,
    "total_elapsed_ms": 6,
    "workload": "matrix-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 6705.25,
    "latency_max_us": 6768,
    "latency_min_us": 6631,
    "latency_p50_us": 6734,
    "latency_p95_us": 6768,
    "latency_p99_us": 6768,
    "peak_rss_kb": 8056,
    "runner": "thread (std)",
    "success": 4,
    "total_elapsed_ms": 7,
    "workload": "hash-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 70494.75,
    "latency_max_us": 71580,
    "latency_min_us": 69246,
    "latency_p50_us": 71282,
    "latency_p95_us": 71580,
    "latency_p99_us": 71580,
    "peak_rss_kb": 8056,
    "runner": "process (fork)",
    "success": 4,
    "total_elapsed_ms": 73,
    "workload": "checksum-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 2859.5,
    "latency_max_us": 3082,
    "latency_min_us": 2203,
    "latency_p50_us": 3079,
    "latency_p95_us": 3082,
    "latency_p99_us": 3082,
    "peak_rss_kb": 8056,
    "runner": "process (fork)",
    "success": 4,
    "total_elapsed_ms": 5,
    "workload": "prime-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 4252.75,
    "latency_max_us": 4674,
    "latency_min_us": 3141,
    "latency_p50_us": 4618,
    "latency_p95_us": 4674,
    "latency_p99_us": 4674,
    "peak_rss_kb": 8060,
    "runner": "process (fork)",
    "success": 4,
    "total_elapsed_ms": 6,
    "workload": "matrix-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 7442.5,
    "latency_max_us": 7472,
    "latency_min_us": 7419,
    "latency_p50_us": 7442,
    "latency_p95_us": 7472,
    "latency_p99_us": 7472,
    "peak_rss_kb": 8056,
    "runner": "process (fork)",
    "success": 4,
    "total_elapsed_ms": 9,
    "workload": "hash-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 69215.25,
    "latency_max_us": 70161,
    "latency_min_us": 68232,
    "latency_p50_us": 69521,
    "latency_p95_us": 70161,
    "latency_p99_us": 70161,
    "peak_rss_kb": 9080,
    "runner": "green-thread (CFQ)",
    "success": 4,
    "total_elapsed_ms": 276,
    "workload": "checksum-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 1294.75,
    "latency_max_us": 1615,
    "latency_min_us": 1171,
    "latency_p50_us": 1199,
    "latency_p95_us": 1615,
    "latency_p99_us": 1615,
    "peak_rss_kb": 9080,
    "runner": "green-thread (CFQ)",
    "success": 4,
    "total_elapsed_ms": 5,
    "workload": "prime-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 3256.75,
    "latency_max_us": 3434,
    "latency_min_us": 3088,
    "latency_p50_us": 3346,
    "latency_p95_us": 3434,
    "latency_p99_us": 3434,
    "peak_rss_kb": 9088,
    "runner": "green-thread (CFQ)",
    "success": 4,
    "total_elapsed_ms": 13,
    "workload": "matrix-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 7186.25,
    "latency_max_us": 7259,
    "latency_min_us": 7093,
    "latency_p50_us": 7204,
    "latency_p95_us": 7259,
    "latency_p99_us": 7259,
    "peak_rss_kb": 9088,
    "runner": "green-thread (CFQ)",
    "success": 4,
    "total_elapsed_ms": 28,
    "workload": "hash-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 71317.5,
    "latency_max_us": 72821,
    "latency_min_us": 68299,
    "latency_p50_us": 72434,
    "latency_p95_us": 72821,
    "latency_p99_us": 72821,
    "peak_rss_kb": 9132,
    "runner": "stackless (CFQ)",
    "success": 4,
    "total_elapsed_ms": 285,
    "workload": "checksum-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 1197.0,
    "latency_max_us": 1237,
    "latency_min_us": 1162,
    "latency_p50_us": 1197,
    "latency_p95_us": 1237,
    "latency_p99_us": 1237,
    "peak_rss_kb": 9132,
    "runner": "stackless (CFQ)",
    "success": 4,
    "total_elapsed_ms": 4,
    "workload": "prime-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 2883.25,
    "latency_max_us": 2926,
    "latency_min_us": 2808,
    "latency_p50_us": 2917,
    "latency_p95_us": 2926,
    "latency_p99_us": 2926,
    "peak_rss_kb": 9132,
    "runner": "stackless (CFQ)",
    "success": 4,
    "total_elapsed_ms": 11,
    "workload": "matrix-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 6250.75,
    "latency_max_us": 6337,
    "latency_min_us": 6176,
    "latency_p50_us": 6300,
    "latency_p95_us": 6337,
    "latency_p99_us": 6337,
    "peak_rss_kb": 9132,
    "runner": "stackless (CFQ)",
    "success": 4,
    "total_elapsed_ms": 25,
    "workload": "hash-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 70077.5,
    "latency_max_us": 71613,
    "latency_min_us": 68283,
    "latency_p50_us": 71528,
    "latency_p95_us": 71613,
    "latency_p99_us": 71613,
    "peak_rss_kb": 9136,
    "runner": "embassy (sim)",
    "success": 4,
    "total_elapsed_ms": 280,
    "workload": "checksum-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 1238.0,
    "latency_max_us": 1283,
    "latency_min_us": 1193,
    "latency_p50_us": 1280,
    "latency_p95_us": 1283,
    "latency_p99_us": 1283,
    "peak_rss_kb": 9136,
    "runner": "embassy (sim)",
    "success": 4,
    "total_elapsed_ms": 4,
    "workload": "prime-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 3061.5,
    "latency_max_us": 3310,
    "latency_min_us": 2840,
    "latency_p50_us": 3205,
    "latency_p95_us": 3310,
    "latency_p99_us": 3310,
    "peak_rss_kb": 9136,
    "runner": "embassy (sim)",
    "success": 4,
    "total_elapsed_ms": 12,
    "workload": "matrix-light"
  },
  {
    "concurrency": 4,
    "errors": 0,
    "latency_avg_us": 6599.75,
    "latency_max_us": 6715,
    "latency_min_us": 6530,
    "latency_p50_us": 6592,
    "latency_p95_us": 6715,
    "latency_p99_us": 6715,
    "peak_rss_kb": 9136,
    "runner": "embassy (sim)",
    "success": 4,
    "total_elapsed_ms": 26,
    "workload": "hash-light"
  }
]
```
