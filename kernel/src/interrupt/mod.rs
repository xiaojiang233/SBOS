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
            crate::task::scheduler::on_timer(frame)
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

fn exception(frame: &TrapFrame) -> ! {
    if frame.vector == 14 {
        let address: u64;
        unsafe {
            asm!("mov {}, cr2",out(reg)address,options(nomem,nostack,preserves_flags));
        }
        crate::kprintln!(
            "PAGE FAULT addr={:#x} error={:#x} rip={:#x} cs={:#x}",
            address,
            frame.error,
            frame.rip,
            frame.cs
        );
    } else {
        crate::kprintln!(
            "CPU EXCEPTION vector={} error={:#x} rip={:#x} cs={:#x}",
            frame.vector,
            frame.error,
            frame.rip,
            frame.cs
        );
    }
    crate::panic_halt()
}
