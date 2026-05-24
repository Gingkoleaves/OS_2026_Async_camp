## Notes

### Notes for stage1

#### 向：cpu硬件对并发的支持

硬件并发指的是中断导致的，操作系统对并发的支持也是thread，编程语言并发指的是线程
异步操作系统

中断是由硬件轮询检测，因为可以直接加入fetch-exec-wb的链中；而软件总是需要额外的fetch-exec-wb链？
认为用户态中断可以极大地简化signal

#### 田：RISC-V用户态中断扩展设计和实现

提出问题：IPC需要上下文切换、数据copy、注册signal函数需要进入kernel等cost
kpti:用户内核的页表分开，防止meltdown

提出解决方法：用户态中断
用户态接受响应中断，跨核异步通知[指的是跨核的IPC，传统上需要发出核进入内核态，发出中断信号，接受核收到中断进入内核态响应；用户态中断下，不需要两次进入内核；异步通知下，不需要发送核等待 接受核返回信息]
需要考虑接收方是谁？接收方如果正在sleep，具体行为是什么？需要控制中断的登记与否[允许谁登记中断]？

riscv n扩展：RISC-V 架构中的 “用户态中断标准扩展”【引入中断委托机制,内核（S态）可以通过设置 sideleg（Supervisor Interrupt Delegation Register）寄存器，把特定的中断委托（Delegate）给 U 态。被委托的中断发生时，硬件会绕过 S 态，直接把控制权交给 utvec 指向的用户态代码]
x86也有用户态中断扩展

介绍riscv的用户它中断扩展机制
增加两个控制寄存器suist、一个uipi指令和一个用户态中断控制器
发送方中断控制寄存器：enable位表示发送方，size发送方状态表大小，ppn发送方状态表大小[表项是[valid位，sender-vec是sender登记的中断向量【即希望接收方触发的中断服务程序】，UIRS-index接受态的index]]
接收方中断控制寄存器：enable位表示接受方，index接收方状态表中本项的entry的index
用户态中断控制器：维护接收方状态表，entry为[active接受中断信号与否，mode决定64位与否，hartid硬件编号，pending记录待处理的用户态中断发送方]
uipi指令：用户程序执行uipi指令来访问用户态中断控制器，sender可以修改目标receiver的pending，receiver则可以设置active位和读取/修改pending位

在riscv硬件qemu上实现用户态中断：Chisel一种硬件描述语言、Rocket-Chip一种Soc生成器、Rocc是前者中定义的协处理器；用Rocc和Rocket-Chip实现上述需要的硬件
软件上，基于linux增加对应系统调用和库函数；用ipc-bench测试

#### 尤：软硬协同的用户态中断机制研究

n扩展寄存器：ustatus、utvec等，sideleg/sedeleg委托用户态处理某些S态中断
riscv上常见的外部中断控制器PLIC，需要把PLIC映射到用户地址用于查找具体的中断

提出一种新的中断控制器UINTC，存储enable(S,R)表示s是否能发给r，pending(s,r)是否存在s发送给r的中断等待处理，listen(c)硬件上下文监听的接收方编号【表示这个receiver是否正在运行】，sender的uiid、receiver的uiid，提供send和receive方法
需要在tcb中维护这些寄存器，另外维护一页中断缓冲区

<img width="1063" height="443" alt="image" src="https://github.com/user-attachments/assets/3122c62f-c873-4f0f-8ff6-aea0ea32a8ff" />
