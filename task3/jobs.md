# 进一步工作

## 阅读embassy源代码和embassy-preempt代码

[X]

## 为embassy添加qemu上的动态跟踪功能并跑通

[X]原本已经支持功能

## 为embassy—preempt添加qemu上的动态跟踪功能并跑通

[X]原本已经支持功能

## 为preempt提供同优先级robin功能

[X],使用riscv-64gc成功运行

cargo build --target riscv64gc-unknown-none-elf --bin rr_test --features qemu-virt

qemu-system-riscv64 \
-machine virt \
-cpu rv64 \
-smp 1 \
-m 128M \
-bios none \
-nographic \
-serial mon:stdio \
-kernel target/riscv64gc-unknown-none-elf/debug/rr_test

## 为preempt提供mutex

[X] 运行方式见下

OS_MUTEX_EN=1 cargo build --bin mutex_test --target riscv64gc-unknown-none-elf --features qemu-virt

qemu-system-riscv64 \
-machine virt \
-cpu rv64 \
-smp 1 \
-m 128M \
-bios none \
-nographic \
-serial mon:stdio \
-kernel target/riscv64gc-unknown-none-elf/debug/mutex_test
