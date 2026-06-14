# OS_2026_Async_camp

Weekly report of oscamp in Async project stage

## 任务一：进程、线程和协程的工作原理

### 学习任务：学习相关文档和视频

实践任务：完成一种以上的爬虫程序；
在用户态写基于进程的爬虫程序；
在用户态写基于线程的爬虫程序；
在用户态写基于协程的爬虫程序；
对比基于进程、线程和协程的三种爬虫程序的性能特征（延时分布、吞吐率和内存开销）；
实践任务：写爬虫程序，依据表格“高校名称和官方网站”中的网站链接，从对应网站上下载首页的纯文本内容，并保存在本地当前目录中，命名对应文件名为学校的中文名称。
在用户态写基于进程的爬虫程序；
在用户态写基于线程的爬虫程序；
在用户态写基于协程的爬虫程序；
对比基于进程、线程和协程的三种爬虫程序的性能特征（延时分布、吞吐率和内存开销）；
参考实现：用户态协程（爬虫）（周积萍）

### 当前结果1

在task1下执行cargo run --bin task1 -- --quiet不显示细节地运行
最终结果如下：4并发
╔══════════════════════╦═════════════════╦═════════════════╦═════════════════╗
║ Metric               ║ async (tokio)   ║ process (fork)  ║ thread (std)    ║
╠══════════════════════╬═════════════════╬═════════════════╬═════════════════╣
║ Total time           ║ 3.22 s          ║ 8.79 s          ║ 6.38 s          ║
║ Success/Total        ║ 31/33           ║ 31/33           ║ 31/33           ║
║ Throughput (KB/s)    ║ 988.5           ║ 367.7           ║ 498.5           ║
║ Peak RSS (MB)        ║ 17.9 MB         ║ 13.8 MB         ║ 18.5 MB         ║
║ Avg latency (ms)     ║ 953             ║ 523             ║ 451             ║
║ P50 latency (ms)     ║ 834             ║ 368             ║ 398             ║
║ P95 latency (ms)     ║ 2262            ║ 1654            ║ 1068            ║
║ Min latency (ms)     ║ 39              ║ 38              ║ 32              ║
║ Max latency (ms)     ║ 3214            ║ 2327            ║ 1076            ║
╚══════════════════════╩═════════════════╩═════════════════╩═════════════════╝

32并发
╔══════════════════════╦═════════════════╦═════════════════╦═════════════════╗
║ Metric               ║ async (tokio)   ║ process (fork)  ║ thread (std)    ║
╠══════════════════════╬═════════════════╬═════════════════╬═════════════════╣
║ Total time           ║ 2.03 s          ║ 3.09 s          ║ 2.92 s          ║
║ Success/Total        ║ 31/33           ║ 31/33           ║ 31/33           ║
║ Throughput (KB/s)    ║ 1565.4          ║ 1046.0          ║ 1088.1          ║
║ Peak RSS (MB)        ║ 21.4 MB         ║ 16.1 MB         ║ 30.2 MB         ║
║ Avg latency (ms)     ║ 599             ║ 636             ║ 593             ║
║ P50 latency (ms)     ║ 475             ║ 527             ║ 526             ║
║ P95 latency (ms)     ║ 1449            ║ 1729            ║ 1185            ║
║ Min latency (ms)     ║ 37              ║ 30              ║ 56              ║
║ Max latency (ms)     ║ 2020            ║ 2431            ║ 2336            ║
╚══════════════════════╩═════════════════╩═════════════════╩═════════════════╝

64并发
╔══════════════════════╦═════════════════╦═════════════════╦═════════════════╗
║ Metric               ║ async (tokio)   ║ process (fork)  ║ thread (std)    ║
╠══════════════════════╬═════════════════╬═════════════════╬═════════════════╣
║ Total time           ║ 1.76 s          ║ 1.72 s          ║ 1.62 s          ║
║ Success/Total        ║ 31/33           ║ 31/33           ║ 31/33           ║
║ Throughput (KB/s)    ║ 1804.3          ║ 1875.3          ║ 1963.1          ║
║ Peak RSS (MB)        ║ 21.8 MB         ║ 15.9 MB         ║ 31.0 MB         ║
║ Avg latency (ms)     ║ 570             ║ 488             ║ 597             ║
║ P50 latency (ms)     ║ 470             ║ 418             ║ 434             ║
║ P95 latency (ms)     ║ 1524            ║ 1016            ║ 1532            ║
║ Min latency (ms)     ║ 41              ║ 34              ║ 82              ║
║ Max latency (ms)     ║ 1750            ║ 1710            ║ 1618            ║
╚══════════════════════╩═════════════════╩═════════════════╩═════════════════╝

可以补充对网络io、html解析、io和cpu密集型的分别检测

## 任务二：用户态线程和协程

### 学习任务：通过动态跟踪，分析下面程序的执行流状态变迁过程

Tokio Future: <https://tokio-zh.github.io/document/going-deeper/futures.html>

200行实现绿色线程:<https://zjp-cn.github.io/os-notes/green-thread.html>

200行实现协程序：<https://nkbai.github.io/rust/Futures_Explained_in_200_lines_of_Rust.html>

A stack-less Rust coroutine library under 100 LoC:<https://blog.aloni.org/posts/a-stack-less-rust-coroutine-100-loc/>

books-futures-explained:<https://www.infoq.com/presentations/rust-2019/>

已有参考实现
首都师范大学 王文智：轻量级的操作系统基本调度单位的设计与实现
实践任务：至少完成一个子任务；
基于动态跟踪分析的结果，更新对应文档；
选择一种执行流机制，在其中扩展优先级支持；

### 当前结果2

已完成四种并发模型的 CFQ 优先级调度实现（`task2/`）：

```rs
src/
├── scheduler.rs         ← CfqScheduler<T> 通用核心（BinaryHeap + vruntime）
├── thread.rs            ← OS 线程模型 + CfqThreadPool
├── callback.rs          ← 回调模型 + CFQ 调度器
├── green_thread.rs      ← 有栈协程 + CFQ 上下文切换
├── stackless_coroutiners.rs ← 无栈协程 + CfqExecutor
├── main.rs              ← 统一入口（--model thread|callback|green|stackless|all）
└── lib.rs               ← 库根

tests/
└── integration.rs       ← 跨模型公平性测试（10 tests）
```

**CFQ 调度核心：**

- 优先级 → 权重映射表（参考 Linux `prio_to_weight`，40 级，相邻比 ≈ 1.25x）
- `vruntime += time_slice * 1024 / weight`
- `BinaryHeap` 按 vruntime 排序，每次选取最小 vruntime 任务
- 新任务初始 vruntime = 当前 min_vruntime（防饿死）
- 相同 vruntime 时按权重排序（高权重优先）

**测试结果：** 58 tests passed (24 unit × 2 + 10 integration)，0 failures，0 warnings

> CFQ 将任务分为三个大类（Scheduling Classes）：
> Real-Time (RT, 实时)：拥有绝对最高优先级，只要它有请求，立刻处理。
> Best-Effort (BE, 尽力而为)：最常用的普通类，内部又细分为 0-7 八个优先级等级。
> Idle (空闲)：最低级，只有当系统完全没事干时才给它服务。
> 在最通用的 Best-Effort 类里，优先级高（等级 0）的任务和优先级低（等级 7）的任务，在红黑树里是平等排队的。但是！当轮到某个队列执行时，高优先级队列能够霸占磁盘更长的时间（时间片更长），而低优先级队列只能用极短的时间。

## 任务三：内核态协程

### 学习任务：通过动态跟踪，分析下面程序的执行流状态变迁过程 embassy（中文版本）

embassy<https://embassy.dev/book/>，<https://lighklife.github.io/embassy-cn/>

电子科技大学 袁子为、施诺晖：基于Rust异步机制的嵌入式操作系统调度模块embassy_preempt<https://www.yuque.com/xyong-9fuoz/hg8kgr/culbvrzfn9qu9lby#AlAhs>

刘轶凡：在星光2的S7小核上独立运行embassy<https://www.yuque.com/xyong-9fuoz/hg8kgr/orddgx677bplf6pl#vM3K0>

电子科技大学 杨长轲：嵌入式异步实时操作系统embassy_preempt<https://www.yuque.com/xyong-9fuoz/hg8kgr/orddgx677bplf6pl#ciNZh>

郑昱可：Embassy Preempt On VisionFive2Ariel OS<https://www.yuque.com/xyong-9fuoz/hg8kgr/cvbvasbkmttrf30m#TxUo2>

### 实践任务：在QEMU模拟器或星光2开发板上选择一种内核组件，利用异步机制优化性能（提高响应时效和减少内存战胜）和通用性（在多种OS中复用驱动代码

串口驱动

有线网卡驱动: 明扬：异步网卡驱动开发<https://www.yuque.com/xyong-9fuoz/hg8kgr/orddgx677bplf6pl#OIDDg>

无线网卡驱动

SD卡驱动: 余泽铖：Starry SD 卡驱动设计<https://www.yuque.com/xyong-9fuoz/hg8kgr/cvbvasbkmttrf30m#LQUxV>

NPU驱动: 周雨 ：RKNPU技术报告<https://www.yuque.com/xyong-9fuoz/hg8kgr/cvbvasbkmttrf30m#H4H9f>

调度器: 清华大学 赵方亮：基于软硬协同的任务调度和中断响应研究<https://www.yuque.com/xyong-9fuoz/hg8kgr/cxnbc2dhznprgaek#CX9e8>、北京理工大学 廖东海：ReL4-高性能异步微内核设计与实现<https://www.yuque.com/xyong-9fuoz/hg8kgr/cxnbc2dhznprgaek#-1>

系统调用

IPC

### 当前结果

看Embassy文档
