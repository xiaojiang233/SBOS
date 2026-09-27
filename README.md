# SBOS

> A Rust-first x86_64 UEFI operating-system prototype with a native object/handle API. / 一个以 Rust 编写、采用 Native Object/Handle API 的 x86_64 UEFI 操作系统原型。

## 简介

SBOS is an early-stage monolithic OS prototype. The kernel uses `#![no_std]`, boots through a Rust EFI loader, loads static ELF64 images, and starts upstream GNU Bash 5.3 as its first Ring 3 process. The kernel keeps its native object/handle API; a separate syscall ABI provides the current POSIX compatibility surface.

SBOS 当前采用模块化单体内核。文件系统只表示真实文件与目录；设备、配置、进程、服务、Volume 和 IPC 使用各自的管理器与 API，不伪装成 `/dev`、`/proc` 或 `/sys` 文件。

This is a bring-up project, not a production-ready OS. The first-stage implementation and its known limits are described below.

## 已实现

- **UEFI Loader**：从 Simple File System 读取 `kernel.elf` 和 `shell.elf`，解析 ELF64 `PT_LOAD`，取得 UEFI Memory Map、GOP Framebuffer 和 ACPI RSDP，构造 BootInfo，调用 `ExitBootServices` 并跳转到内核。
- **内核入口与架构**：`#![no_std]` Rust 内核、COM1 串口、GOP 5×7 文本渲染、panic handler、GDT、IDT、紧凑布局的 TSS 和 CPU exception/Page Fault handler。Ring 0 异常触发 kernel panic；Ring 3 异常记录 PID/TID 与 fault 信息、清理当前进程并切换回可运行线程。
- **内存**：按 UEFI Conventional Memory 管理物理页；建立 NX 页表、内核只读代码/只读数据映射、用户页与 1 GiB 内核直接映射；内核堆支持释放。
- **图形与输入接口**：UEFI GOP framebuffer 已有受 `DISPLAY` capability 控制的用户态 surface-info、矩形填充和有界 blit syscall；`desktop-demo` 可画出基础桌面面板。可选 `driver-ps2-mouse` 通过 PIT 轮询产生带坐标和按钮状态的事件，供 `INPUT` capability 用户态读取。Ring 3 的 `xserver` 已有受限 X11 11.0 核心协议子集；完整 X.Org 兼容、认证、compositor 和窗口管理器尚未实现。
- **中断与调度**：PIC/PIT timer、COM1 IRQ 接收、PS/2 键盘轮询；可选 i8042 鼠标在 PIT 驱动下产出带坐标/按键状态的输入事件。单核 Round Robin Thread 调度，带真实 Blocked/Ready 转换、WaitQueue、显式唤醒和 PIT timeout；TTY 输入由前台进程持有。可选 CMOS RTC 为统一时间服务提供 epoch，内核使用 PIT ticks 推进 monotonic 与 realtime 时钟。
- **对象与安全**：KernelObject、进程级 typed Handle Table、24-bit generation Handle、READ/WRITE/EXECUTE/MAP/WAIT/SIGNAL/DUPLICATE/TRANSFER/CONTROL 权限，以及 SecurityContext/Capabilities（含 NETWORK、DISPLAY、INPUT）。全局 Scheduler/Process/Mount 状态可用 `lock_irqsave` 精确保留并恢复 IF。
- **进程与用户态**：Process、Thread、AddressSpace、独立用户页表、Ring 3 ELF64 Loader、用户栈、自定义 `int 0x80` Native syscall ABI；Process 采用 Running/Exiting/Zombie/Reaped 状态，waitpid 后回收地址空间与线程栈，用户异常不会停掉 Bash。
- **身份管理**：独立 User/Group Manager，将 Root（UID/GID 0）和 Guest（UID/GID 1000）账户写入 SBFS 的 `/System/Accounts.db`；当前开发启动默认使用 root，账户 API 已有创建/删除入口。
- **文件和系统服务**：VFS/VNode/File/Directory、携带 MountId 的 `NodeRef`、Device Manager、块设备接口、Volume Manager、SBFS 持久化文件系统（无磁盘时回退 tmpfs）、ConfigStore、Service Manager 和双端有界 Channel。Channel 可以阻塞等待，并以 sender-retains-original 语义复制传递 Handle，保留原 rights。
- **用户程序**：GNU Bash 5.3（上游 patchlevel 20）以 Ring 3 交互 Shell 启动；GNU Coreutils 9.12 的精选单体构建提供 `basename`、`cat`、`cut`、`date`、`dirname`、`env`、`head`、`ls`、`mkdir`、`printf`、`pwd`、`rm`、`rmdir`、`seq`、`sleep`、`tail`、`tee`、`test`、`tr`、`wc` 等命令。独立 C 程序仍提供 `clear`、`id`、`mv`、`fault`、`channel-probe`、`network-probe`、`desktop-demo`、`mouse-probe`、`tcp-listen-probe`、`xserver` 和 POSIX 自检程序。
- **兼容接口**：`user/runtime` Rust API、自定义 syscall ABI、`libc/` C runtime，以及 `compat/posix` 适配层。`fork`、`execve`、`waitpid`、pipe、TTY、文件描述符和 `rename` 已有最小可运行路径。
- **网络 bring-up**：可选 PCI/e1000 驱动使用 PMM DMA 描述符环；可选 `network-stack` 使用 no_std smoltcp，已接入 Ethernet、ARP、IPv4、ICMP、UDP/TCP 协议组件和 DHCPv4 租约获取。QEMU 用户网络实测取得 `10.0.2.15/24`。Ring 3 已实现 IPv4 UDP 和 TCP stream 的基本 socket/fd API，libc `getaddrinfo` 可用 DHCP DNS 解析 IPv4 A 记录。QEMU 已验证 UDP/DNS 往返、TCP listen/accept/read/write 回显。TCP active connect、完整 socket options、IPv6 和 GNU 网络客户端兼容仍在开发。
- **图形 bring-up**：用户态通过受 `DISPLAY` capability 控制的 syscall 查询 GOP 尺寸、填充矩形和 blit 小图块；`desktop-demo` 可画基础桌面。Ring 3 `/Applications/xserver` 已实现一个可从 QEMU host-forward 连接的 X11 11.0 协议子集，支持 setup、窗口/GC 创建、映射、矩形绘制和几类查询。可选 PS/2 鼠标驱动提供用户态事件接口，Xserver 已加入 pointer motion/button 事件发送逻辑。完整 X.Org server、认证、Xlib 完整兼容、compositor、窗口管理器和键盘事件仍未实现；鼠标注入事件尚待 QEMU 验证，开发原型需显式使用 `-noauth`。

有块设备时，根 Volume 使用 SBFS，初始目录为 `/Applications`、`/System`、`/Users`、`/Shared` 和 `/Volumes`，并写入 Bash、基础用户程序与欢迎文件。没有可用磁盘时，系统使用同一初始目录布局的 tmpfs。

最近的 QEMU shell smoke 实际从 UEFI 启动并进入 Ring 3 Bash，验证了 GNU applets、SBFS 文件读写与替换、POSIX fork/pipe/waitpid/execve，以及 Shell 在子进程完成后继续交互。网络探针验证 UDP/DNS、`fork` 后 fd 共享和 `getaddrinfo`；TCP listener 探针验证外部 host-forward 连接和 echo；X11 wire-protocol 探针完成 setup、创建/映射窗口、绘制矩形和 `GetGeometry`。图形/TCP/X11 探针记录位于 `build/` 的 smoke transcript 中。

SBFS v1 使用 128-bit NodeId、Volume UUID、固定容量节点表、Allocation Bitmap、最多 8 个 inline extents、UTF-8 目录项、主体 ACL 条目和 metadata redo journal。SBFS 只管理文件与目录；设备、进程、服务和配置仍由各自的管理器负责，不提供 `/dev`、`/proc`、`/sys` 或 `/etc`。

开发启动默认以 `root` 身份进入 `/Users/Root`。身份数据库目前不含密码或登录认证；认证、会话切换及 `su`/`sudo` 用户程序属于后续阶段。`root` 是唯一管理员，普通进程凭据从父进程继承，后续可接入用户登录服务。

## 可选内核模块

驱动和根文件系统后端通过 Cargo features 选择。`qemu` 是默认参考配置；根文件系统至少要选 `fs-sbfs` 或 `fs-tmpfs`。构建脚本接受完整 feature 列表：

```powershell
./tools/build.ps1 -KernelFeatures fs-tmpfs
./tools/build.ps1 -KernelFeatures fs-sbfs,fs-tmpfs,driver-ata,driver-ps2
```

第一条构建 tmpfs-only、串口交互变体；第二条保留磁盘和键盘但不构建 PCI/e1000/network stack。`driver-e1000` 会自动启用 `driver-pci` 与 `network-stack`。feature 依赖和已知边界见 [`docs/KERNEL_FEATURES.md`](docs/KERNEL_FEATURES.md) 和 [`docs/NETWORK.md`](docs/NETWORK.md)。

## 构建与运行

需要 Rust/Cargo、`x86_64-unknown-none` 和 `x86_64-unknown-uefi` targets、clang/LLD、GNU make、POSIX shell、Python 3、x86_64 QEMU 与 OVMF 固件。GNU Bash 上游 tarball 放在 `_qemu/downloads/bash-5.3.tar.gz`；解包源码与构建目录放在被忽略的 `_qemu/src/` 下。仅启用 `network-stack` 时，内核会依赖 no_std smoltcp；PMM、VMM、调度器、VFS 和 syscall 仍由 SBOS 自行实现。

```powershell
rustup target add x86_64-unknown-none x86_64-unknown-uefi
./tools/build.ps1
./tools/run-qemu.ps1
```

`build.ps1` 先构建 libc、C 用户程序、上游 Bash 和 Coreutils，再构建内核与 EFI Loader。它生成 `build/esp`，校验 EFI PE/COFF 类型和静态 ELF64 `ET_EXEC` 入口段，并将 Bash ELF 放为 EFI Loader 读取的 `shell.elf`。QEMU 首次运行会创建 64 MiB 的 `build/sbfs.img`，之后保留该磁盘镜像以验证持久化；要重新格式化时，需在关机后自行移走或重命名该镜像。QEMU 默认用 SDL 显示 GOP framebuffer，并将串口连到启动终端；无窗口模式：

```powershell
./tools/run-qemu.ps1 -Display none
```

启动链：

```text
UEFI → SBOS EFI Loader → Rust Kernel → Memory / Interrupt / Object / Task
    → Native Syscall → VFS / Device / Config / Service / IPC / Network
    → Ring 3 GNU Bash 5.3 → C 用户程序与 POSIX libc
```

QEMU 与 OVMF 固件是本机开发依赖，不会提交到 Git。启动脚本会查找本地 `_qemu` 目录，也接受 `OVMF_CODE` 环境变量。

## Native Syscall ABI

Syscall number 放在 `rax`；六个参数依次放在 `rdi`、`rsi`、`rdx`、`r10`、`r8`、`r9`；有符号返回值从 `rax` 读取。用户态只持有不透明、进程级 Handle token，不持有内核指针。内核在复制前验证用户地址范围和页权限。

| 编号 | 调用 |
| --- | --- |
| 0–7 | Handle close、File open/read/write、Process exit/spawn、Thread create、Handle wait |
| 8–12 | Channel create/send/receive、Memory map/unmap |
| 13–19 | Console read/write、Directory open/read/change/current、System query |
| 20–30 | Config set、I/O submit、Config delete/watch/transaction、Event reset、Service lookup、TTY foreground |

Rust 包装位于 [`user/runtime/src/lib.rs`](user/runtime/src/lib.rs)，C ABI 编号与寄存器包装位于 [`user/runtime/include/sbos/native.h`](user/runtime/include/sbos/native.h)。`handle_wait` 使用 100 Hz PIT ticks 作为 timeout 单位；`0` 表示非阻塞检查，`u64::MAX` 表示无限等待。

ConfigStore watcher 可以这样验证：先运行 `config watch user/appearance`，另一次设置 `config set user/appearance/theme light`，再运行 `config wait 100`；事件已到达时 Shell 显示 `configuration changed`，否则最多等待 100 ticks 后显示超时。

## 目录结构

```text
boot/                  Rust UEFI Loader
kernel/src/arch/       x86_64 GDT、IDT、TSS、Ring 3 入口、端口 I/O
kernel/src/memory/     PMM、VMM、Kernel Heap
kernel/src/task/       Process、Thread、AddressSpace、Scheduler
kernel/src/object/     KernelObject 和对象类型
kernel/src/handle/     Handle Table 与 Access Rights
kernel/src/syscall/    Native syscall dispatcher
kernel/src/fs/         VFS、VNode、File、Directory、Mount、tmpfs
kernel/src/sbfs/       SBFS 磁盘格式、journal、bitmap、extent 与节点操作
kernel/src/user/       User/Group Manager 与 SBFS 持久化账户数据库
kernel/src/device/     Device Manager 和类型化设备接口
kernel/src/config/     ConfigStore、事务和 watch
kernel/src/service/    Service Manager
kernel/src/ipc/        双端消息 Channel
kernel/src/exec/       用户 ELF64 Loader
user/runtime/          Rust Native API 与 C 头文件
userland/              Ring 3 C utilities and POSIX probe
libc/                  freestanding C standard/POSIX runtime
ports/bash-5.3/        upstream patches and cross-build configuration
compat/posix/           早期 POSIX 文件调用适配器
tools/                  构建、产物校验、QEMU 启动与 OVMF 提取脚本
```

## 当前限制与后续方向

- PMM 和内核直接映射限于 1 GiB，目标 QEMU 配置为 512 MiB；目前没有 SMP/APIC。
- SBFS 当前使用整块原始磁盘，不解析分区表；设备驱动仅支持一个 legacy IDE ATA PIO 磁盘和 LBA28。SBFS v1 节点表最多容纳 256 个节点，单个文件最多 8 个 extents，UTF-8 文件名最多 44 字节。
- SBFS journal 只记录 metadata redo；文件数据先写盘、后提交元数据，因此突然断电时不会获得完整的数据事务保证。SBFS 节点时间戳目前仍是 CPU TSC 计数；访问时间暂不更新。统一时间服务使用 RTC epoch 加 PIT ticks 提供 realtime/monotonic API，但启动平台的 RTC 必须按 UTC 配置。自定义 ACL 是主体与访问权条目，不采用 POSIX mode 位。
- Symlink、稀疏文件、校验和、快照、压缩、加密、CoW、分区扫描和多磁盘挂载尚未实现。ConfigStore 当前仍驻留 RAM。
- 无磁盘启动时 tmpfs 单文件上限 1 MiB；这个回退文件系统不提供持久化。
- Channel 是有界消息队列，支持 payload、Handle 复制传递和 WaitQueue 唤醒；字节流 Channel、Shared Memory 传递和异步通知尚未实现。tmpfs `io_submit` 目前会立即完成。
- 设备支持目前包括 COM1、UEFI GOP、i8042 键盘/鼠标、ATA PIO 和 QEMU e1000；PCI 扫描只覆盖 legacy PCI 配置机制和 QEMU 的设备布局。用户态网络已有 IPv4 UDP/TCP 基本路径与 DNS A 前向解析，反向解析、IPv6 和通用硬件覆盖尚未实现。GOP surface、PS/2 mouse events 和受限 X11 server 已有基础实现；X11 认证、完整 client events、compositor、窗口管理器、音频和 ext2 尚未实现。
- Bash 已链接 GNU Readline 和 termcap，并使用内嵌 ANSI 终端描述；QEMU 已验证命令行中间插入。job control、NLS、多字节 locale、跨会话历史文件和完整信号语义尚未实现。
- libc 仍不完整：本地时区数据库、目录创建/权限更改、进程信号、完整 POSIX 错误语义、pthread、完整 curses 与 musl 均未实现。`clock_gettime(CLOCK_REALTIME/CLOCK_MONOTONIC)`、`time`、`gettimeofday` 和 GNU `date` 已接通 RTC/PIT 时间服务。已实现 API 之外的调用会显式报错或返回 `ENOSYS`。
- `application_sandboxed` 与 `IDENTITY_ADMIN` 目前是预留元数据/Capability，尚无独立 enforcement；实际安全判断使用已实现的 syscall capability、ACL 和 Handle-rights 检查。

GNU GPL version 3 or later，见 [`LICENSE`](LICENSE)。

## English

SBOS is an early-stage Rust-first x86_64 UEFI OS. It boots a static ELF64 kernel and launches upstream GNU Bash 5.3 as its Ring 3 shell. The native kernel API is based on typed objects and process-local handles; VFS contains files and directories only. QEMU runs SBFS over an IDE PIO block device and preserves `build/sbfs.img` between launches; systems without a disk fall back to tmpfs. SBFS is a compact v1 implementation, not a production filesystem.

Build with `rustup target add x86_64-unknown-none x86_64-unknown-uefi`, then run `./tools/build.ps1` and `./tools/run-qemu.ps1`. See “当前限制与后续方向” for incomplete subsystems; this prototype is not production-ready.
