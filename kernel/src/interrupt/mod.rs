use crate::arch::x86_64::idt::TrapFrame;
use crate::arch::x86_64::port::{out8, wait};
use core::arch::asm;

pub fn init_timer() {
    unsafe {
        out8(0x20, 0x11);
        wait();
        out8(0xa0, 0x11);
        wait();
        out8(0x21, 0x20);
        wait();
        out8(0xa1, 0x28);
        wait();
        out8(0x21, 0x04);
        wait();
        out8(0xa1, 0x02);
        wait();
        out8(0x21, 0x01);
        wait();
        out8(0xa1, 0x01);
        wait();
        out8(0x21, 0xee); // timer IRQ0 and COM1 receive IRQ4
        out8(0xa1, 0xff); // other legacy IRQs stay masked
        let divisor: u16 = (1_193_182 / 100) as u16;
        out8(0x43, 0x36);
        out8(0x40, divisor as u8);
        out8(0x40, (divisor >> 8) as u8);
    }
}

pub fn enable() {
    unsafe {
        asm!("sti", options(nomem, nostack));
    }
}
pub fn disable() {
    unsafe {
        asm!("cli", options(nomem, nostack));
    }
}

#[no_mangle]
pub extern "C" fn trap_dispatch(frame: *mut TrapFrame) -> *mut TrapFrame {
    if frame.is_null() {
        return frame;
    }
    let trap = unsafe { &mut *frame };
    match trap.vector {
        0..=31 => exception(trap),
        32 => {
            unsafe {
                out8(0x20, 0x20);
            }
            let next_frame = crate::task::scheduler::on_timer(frame);
            crate::task::process::reap_orphans();
            #[cfg(feature = "network-stack")]
            crate::network::poll();
            next_frame
        }
        36 => {
            crate::drivers::serial::handle_irq();
            unsafe {
                out8(0x20, 0x20);
            }
            frame
        }
        128 => crate::syscall::dispatch(frame),
        _ => frame,
    }
}

fn exception(frame: &mut TrapFrame) -> *mut TrapFrame {
    let from_user = frame.cs & 3 == 3;
    let current_thread = crate::task::scheduler::current();
    let current_process = crate::task::process::current();
    let pid = current_process.as_ref().map(|process| process.pid).unwrap_or(0);
    let tid = current_thread.as_ref().map(|thread| thread.tid).unwrap_or(0);
    if frame.vector == 14 {
        let address: u64;
        unsafe {
            asm!("mov {}, cr2",out(reg)address,options(nomem,nostack,preserves_flags));
        }
        if from_user {
            crate::kprintln!("USER PAGE FAULT pid={} tid={} addr={:#x} rip={:#x} error={:#x}",
                pid, tid, address, frame.rip, frame.error);
        } else {
            crate::kprintln!("KERNEL PAGE FAULT pid={} tid={} addr={:#x} error={:#x} rip={:#x} cs={:#x}",
                pid, tid, address, frame.error, frame.rip, frame.cs);
        }
    } else {
        crate::kprintln!("{} CPU EXCEPTION pid={} tid={} vector={} error={:#x} rip={:#x} cs={:#x}",
            if from_user { "USER" } else { "KERNEL" },
            pid, tid, frame.vector, frame.error, frame.rip, frame.cs);
    }
    if !from_user {
        crate::panic_halt();
    }

    let process_id = current_thread.as_ref().map(|thread| thread.process_id).unwrap_or(pid);
    if let Some(process) = current_process {
        let signal = match frame.vector {
            0 => 8,       // SIGFPE
            3 => 5,       // SIGTRAP
            6 => 4,       // SIGILL
            13 | 14 => 11, // SIGSEGV
            _ => 7,       // SIGBUS as the generic memory/CPU fault
        };
        process.terminate_signal(signal);
    }
    crate::task::scheduler::exit_process(process_id, frame)
}
