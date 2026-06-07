mod callback;
mod green_thread;
mod scheduler;
mod stackless_coroutiners;
mod thread;

fn main() {
    println!("=== Async Runtime Models ===\n");

    // 解析命令行参数选择模型（暂时硬编码演示）
    let args: Vec<String> = std::env::args().collect();
    let model = if args.len() > 1 {
        args[1].as_str()
    } else {
        "all"
    };

    match model {
        "thread" => thread::run_example(),
        // "callback" => callback::run_example(), // 已还原为原始版本
        "stackless" => stackless_coroutiners::run_example(),
        "green" => green_thread::run_example(),
        "all" => {
            println!("--- Thread Model ---");
            thread::run_example();
            println!("\n--- Stackless Coroutine Model ---");
            stackless_coroutiners::run_example();
            println!("\n--- Green Thread Model ---");
            green_thread::run_example();
        }
        _ => println!(
            "Unknown model: {}. Try: thread, callback, stackless, green, all",
            model
        ),
    }
}
