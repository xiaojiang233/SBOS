#![no_std]
#![no_main]
#![allow(dead_code)] // Stage-one kernel keeps explicit interfaces for planned native subsystems.

extern crate alloc;

use core::arch::{asm, global_asm};
use core::panic::PanicInfo;

mod arch;
mod bootinfo;
mod config;
mod device;
mod driver;
mod drivers;
mod exec;
mod fs;
mod handle;
mod interrupt;
mod io;
mod ipc;
mod memory;
#[cfg(feature = "network-stack")]
mod network;
mod object;
mod posix;
mod security;
#[cfg(feature = "fs-sbfs")]
pub mod sbfs;
mod service;
mod sync;
mod syscall;
mod task;
mod time;
pub mod user;
mod volume;

#[cfg(not(any(feature = "fs-sbfs", feature = "fs-tmpfs")))]
compile_error!("enable at least one root filesystem backend: fs-sbfs or fs-tmpfs");
#[cfg(all(feature = "network-stack", not(feature = "driver-e1000")))]
compile_error!("network-stack currently requires the driver-e1000 device adapter");

use bootinfo::{BootInfo, BOOT_MAGIC};

const POSIX_PROBE_IMAGE: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/posix-probe.elf"));
const COREUTILS_IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/coreutils.elf"));
const GREP_IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/grep.elf"));
const CLEAR_IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/clear.elf"));
const ID_IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/id.elf"));
const MV_IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/mv.elf"));
const FAULT_IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/fault.elf"));
const CHANNEL_PROBE_IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/channel-probe.elf"));
const NETWORK_PROBE_IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/network-probe.elf"));
const DESKTOP_DEMO_IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/desktop-demo.elf"));
const MOUSE_PROBE_IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/mouse-probe.elf"));
const TCP_LISTEN_PROBE_IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/tcp-listen-probe.elf"));
const XSERVER_IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/xserver.elf"));
const SELECT_PROBE_IMAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/select-probe.elf"));

/// User programs installed under /Applications next to the shell. Images that
/// are not ELF files (because the port was not built) are skipped at install
/// time, so the kernel still boots without them.
const APPLICATION_IMAGES: &[(&str, &[u8])] = &[
    ("posix-probe", POSIX_PROBE_IMAGE),
    ("basename", COREUTILS_IMAGE),
    ("cat", COREUTILS_IMAGE),
    ("cut", COREUTILS_IMAGE),
    ("date", COREUTILS_IMAGE),
    ("dirname", COREUTILS_IMAGE),
    ("env", COREUTILS_IMAGE),
    ("false", COREUTILS_IMAGE),
    ("grep", GREP_IMAGE),
    ("head", COREUTILS_IMAGE),
    ("ls", COREUTILS_IMAGE),
    ("mkdir", COREUTILS_IMAGE),
    ("printenv", COREUTILS_IMAGE),
    ("printf", COREUTILS_IMAGE),
    ("pwd", COREUTILS_IMAGE),
    ("rm", COREUTILS_IMAGE),
    ("rmdir", COREUTILS_IMAGE),
    ("seq", COREUTILS_IMAGE),
    ("sleep", COREUTILS_IMAGE),
    ("tail", COREUTILS_IMAGE),
    ("tee", COREUTILS_IMAGE),
    ("test", COREUTILS_IMAGE),
    ("tr", COREUTILS_IMAGE),
    ("true", COREUTILS_IMAGE),
    ("wc", COREUTILS_IMAGE),
    ("yes", COREUTILS_IMAGE),
    ("clear", CLEAR_IMAGE),
    ("id", ID_IMAGE),
    ("mv", MV_IMAGE),
    ("fault", FAULT_IMAGE),
    ("channel-probe", CHANNEL_PROBE_IMAGE),
    ("network-probe", NETWORK_PROBE_IMAGE),
    ("desktop-demo", DESKTOP_DEMO_IMAGE),
    ("mouse-probe", MOUSE_PROBE_IMAGE),
    ("tcp-listen-probe", TCP_LISTEN_PROBE_IMAGE),
    ("xserver", XSERVER_IMAGE),
    ("select-probe", SELECT_PROBE_IMAGE),
];

#[global_allocator]
static ALLOCATOR: memory::heap::KernelHeap = memory::heap::KernelHeap;

#[repr(align(16))]
struct Stack([u8; 64 * 1024]);
#[repr(align(16))]
struct InterruptStack([u8; 32 * 1024]);
#[no_mangle]
static mut BOOT_STACK: Stack = Stack([0; 64 * 1024]);
#[no_mangle]
static mut INTERRUPT_STACK: InterruptStack = InterruptStack([0; 32 * 1024]);

global_asm!(
    r#"
.section .text.entry,"ax"
.global _start
.type _start,@function
_start:
    cli
    lea rsp, [rip + BOOT_STACK]
    add rsp, 65536
    and rsp, -16
    xor rbp, rbp
    call kernel_main
1:
    hlt
    jmp 1b
.size _start, .-_start
"#
);

#[macro_export]
macro_rules! kprint { ($($arg:tt)*) => { $crate::drivers::serial::_print(format_args!($($arg)*)) }; }
#[macro_export]
macro_rules! kprintln { () => { $crate::kprint!("\r\n") }; ($fmt:expr) => { $crate::kprint!(concat!($fmt,"\r\n")) }; ($fmt:expr,$($arg:tt)*) => { $crate::kprint!(concat!($fmt,"\r\n"),$($arg)*) }; }

#[no_mangle]
pub extern "C" fn kernel_main(info_pointer: *const BootInfo) -> ! {
    drivers::serial::init();
    kprintln!("SBOS kernel entry");
    if info_pointer.is_null() {
        kprintln!("BootInfo pointer is null");
        panic_halt();
    }
    let info = unsafe { core::ptr::read_unaligned(info_pointer) };
    if info.magic != BOOT_MAGIC || info.version != 1 {
        kprintln!("Invalid BootInfo magic/version");
        panic_halt();
    }
    if info.shell_image == 0
        || info.shell_image_size == 0
        || info.shell_image_size > 32 * 1024 * 1024
    {
        kprintln!("Invalid shell ELF image");
        panic_halt();
    }

    let interrupt_top =
        core::ptr::addr_of!(INTERRUPT_STACK) as u64 + core::mem::size_of::<InterruptStack>() as u64;
    unsafe {
        arch::x86_64::gdt::init(interrupt_top);
        arch::x86_64::idt::init();
    }
    if let Err(error) =
        memory::pmm::init(info.memory_map, info.memory_map_size, info.descriptor_size)
    {
        kprintln!("PMM init failed: {}", error);
        panic_halt();
    }
    kprintln!("PMM: {} free 4KiB frames", memory::pmm::free_frame_count());
    if let Err(error) = unsafe { memory::vmm::init(info.framebuffer_base, info.framebuffer_size) } {
        kprintln!("VMM init failed: {}", error);
        panic_halt();
    }
    kprintln!("VMM active root={:#x}", memory::vmm::current_root());

    memory::heap::init();
    time::init();
    #[cfg(feature = "driver-framebuffer")]
    let framebuffer_base = memory::vmm::framebuffer_address(info.framebuffer_base);
    #[cfg(feature = "driver-framebuffer")]
    drivers::framebuffer::init(drivers::framebuffer::FramebufferInfo {
        base: framebuffer_base,
        size: info.framebuffer_size,
        width: info.framebuffer_width as usize,
        height: info.framebuffer_height as usize,
        stride: info.pixels_per_scan_line as usize,
        pixel_format: info.pixel_format,
    });
    drivers::console::write(b"\r\nSBOS: UEFI boot completed\r\n");

    device::init(cfg!(feature = "driver-framebuffer") && info.framebuffer_base != 0);
    #[cfg(feature = "driver-ps2-mouse")]
    {
        let (width, height, _) = drivers::framebuffer::surface_info().unwrap_or((640, 480, 0));
        match drivers::mouse::init(width, height) {
            Ok(()) => {
                device::register_input_device("PC", "PS/2 auxiliary mouse", "i8042-mouse");
                kprintln!("PS/2 mouse online (polling input events)");
            }
            Err(error) => kprintln!("PS/2 mouse unavailable: {}", error),
        }
    }
    #[cfg(feature = "driver-ata")]
    drivers::ata::init();
    #[cfg(feature = "driver-e1000")]
    match drivers::e1000::init() {
        Ok(mac) => {
            kprintln!("E1000 online: {:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
            device::register_network_device("Intel", "82540EM Gigabit Ethernet", "e1000");
            #[cfg(feature = "network-stack")]
            if let Err(error) = network::init(mac) {
                kprintln!("network stack init failed: {}", error);
            } else {
                kprintln!("IPv4 configured: 10.0.2.15/24 via 10.0.2.2 (smoltcp)");
            }
        }
        Err(error) => kprintln!("network device unavailable: {}", error),
    }

    let shell = unsafe {
        core::slice::from_raw_parts(info.shell_image as *const u8, info.shell_image_size)
    };
    let root_volume = volume::init();
    if let Err(error) = fs::vfs::init(shell, APPLICATION_IMAGES, root_volume) {
        kprintln!("root filesystem init failed: {}", error);
        panic_halt();
    }
    if let Err(error) = user::init() {
        kprintln!("identity manager init failed: {}", error);
        panic_halt();
    }
    driver::init();
    config::init();
    service::init();
    let shell_space = task::process::AddressSpace::new(memory::vmm::current_root());
    kprintln!(
        "loading shell ELF at root={:#x}, len={}",
        shell_space.root(),
        shell.len()
    );
    let shell_argv = ["/Applications/bash", "-i"];
    let terminal_rows = if info.framebuffer_height == 0 {
        25
    } else {
        (info.framebuffer_height / 10).max(1) as u16
    };
    let terminal_columns = if info.framebuffer_width == 0 {
        80
    } else {
        (info.framebuffer_width / 6).max(1) as u16
    };
    let termcap = alloc::format!(
        "TERMCAP=sbos|SBOS ANSI console:am:bs:co#{}:li#{}:cl=\\E[H\\E[2J:ce=\\E[K:cd=\\E[J:cm=\\E[%i%d;%dH:cr=\\r:do=\\E[B:ho=\\E[H:le=\\E[D:nd=\\E[C:up=\\E[A:ku=\\E[A:kd=\\E[B:kr=\\E[C:kl=\\E[D:",
        terminal_columns, terminal_rows
    );
    let shell_env = [
        "HOME=/Users/Root",
        "PWD=/Users/Root",
        "USER=root",
        "LOGNAME=root",
        "SHELL=/Applications/bash",
        "PATH=/Applications",
        "HISTFILE=",
        "TERM=sbos",
        termcap.as_str(),
    ];
    let loaded = match exec::elf::load_user(shell, &shell_space, &shell_argv, &shell_env) {
        Ok(value) => value,
        Err(error) => {
            kprintln!("shell ELF load failed: {}", error);
            panic_halt();
        }
    };
    let process = task::process::init(memory::vmm::current_root(), "/Applications/bash");
    drivers::tty::init(process.pid, terminal_rows, terminal_columns);
    let thread = task::thread::create(process.pid, interrupt_top, loaded.stack_pointer);
    process.threads.lock().push(thread.clone());
    if let Err(error) = task::scheduler::initialize(thread) {
        kprintln!("scheduler init failed: {}", error);
        panic_halt();
    }
    interrupt::init_timer();
    kprintln!("SBOS ready: entering Ring 3 shell at {:#x}", loaded.entry);
    unsafe { arch::x86_64::usermode::enter(loaded.entry, loaded.stack_pointer) }
}

#[no_mangle]
pub extern "C" fn panic_halt() -> ! {
    interrupt::disable();
    loop {
        unsafe {
            asm!("hlt", options(nomem, nostack));
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    drivers::serial::_print(format_args!("\r\nKERNEL PANIC: {}\r\n", info));
    panic_halt()
}
