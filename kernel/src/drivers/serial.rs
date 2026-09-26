use crate::arch::x86_64::port::{in8, out8};
use core::cell::UnsafeCell;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicUsize, Ordering};

const COM1: u16 = 0x3f8;
const RX_CAPACITY: usize = 512;
struct ReceiveQueue(UnsafeCell<[u8; RX_CAPACITY]>);
unsafe impl Sync for ReceiveQueue {}
static RX_BYTES: ReceiveQueue = ReceiveQueue(UnsafeCell::new([0; RX_CAPACITY]));
static RX_HEAD: AtomicUsize = AtomicUsize::new(0);
static RX_TAIL: AtomicUsize = AtomicUsize::new(0);

pub fn init() {
    unsafe {
        out8(COM1 + 1, 0x00);
        out8(COM1 + 3, 0x80);
        out8(COM1, 0x01);
        out8(COM1 + 1, 0x00);
        out8(COM1 + 3, 0x03);
        out8(COM1 + 2, 0xc7);
        out8(COM1 + 4, 0x0b);
        RX_HEAD.store(0, Ordering::Relaxed);
        RX_TAIL.store(0, Ordering::Relaxed);
        out8(COM1 + 1, 0x01); // received-data interrupt
    }
}

pub fn write_byte(byte: u8) {
    unsafe {
        for _ in 0..100_000 {
            if in8(COM1 + 5) & 0x20 != 0 {
                break;
            }
            core::hint::spin_loop();
        }
        out8(COM1, byte);
    }
}

pub fn try_read_byte() -> Option<u8> {
    let head = RX_HEAD.load(Ordering::Relaxed);
    let tail = RX_TAIL.load(Ordering::Acquire);
    if head != tail {
        let byte = unsafe { (*RX_BYTES.0.get())[head] };
        RX_HEAD.store((head + 1) % RX_CAPACITY, Ordering::Release);
        return Some(byte);
    }
    // The COM1 receive interrupt is not routed through the interrupt table yet,
    // so fall back to reading the UART directly. That keeps a serial console
    // usable (headless QEMU runs pipe input in over the serial line).
    unsafe {
        if in8(COM1 + 5) & 0x01 != 0 {
            return Some(in8(COM1));
        }
    }
    None
}

pub fn handle_irq() {
    unsafe {
        while in8(COM1 + 5) & 0x01 != 0 {
            let byte = in8(COM1);
            let tail = RX_TAIL.load(Ordering::Relaxed);
            let next = (tail + 1) % RX_CAPACITY;
            if next != RX_HEAD.load(Ordering::Acquire) {
                (*RX_BYTES.0.get())[tail] = byte;
                RX_TAIL.store(next, Ordering::Release);
            }
        }
    }
}

pub fn write(bytes: &[u8]) {
    for &byte in bytes {
        write_byte(byte);
    }
}

pub struct SerialWriter;
impl Write for SerialWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        write(value.as_bytes());
        Ok(())
    }
}

pub fn _print(args: fmt::Arguments<'_>) {
    let _ = SerialWriter.write_fmt(args);
}
