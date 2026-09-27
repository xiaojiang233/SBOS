//! Polled i8042 auxiliary-port mouse driver.
//!
//! The PIT path drains bounded PS/2 packet bytes into a small event queue.
//! User processes read typed mouse events through the Native ABI; no `/dev`
//! node is created.

use crate::arch::x86_64::port::{in8, out8, wait};
use crate::sync::SpinLock;
use alloc::collections::VecDeque;

const EVENT_CAPACITY: usize = 128;
const AUX_STATUS: u8 = 0x20;
const OUTPUT_FULL: u8 = 0x01;
const INPUT_FULL: u8 = 0x02;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MouseEvent {
    pub delta_x: i16,
    pub delta_y: i16,
    pub x: i32,
    pub y: i32,
    pub buttons: u8,
    pub changed_buttons: u8,
    pub reserved: [u8; 2],
}

struct MouseState {
    initialized: bool,
    packet: [u8; 3],
    packet_length: usize,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    buttons: u8,
    queue: VecDeque<MouseEvent>,
}

impl MouseState {
    const fn new() -> Self {
        Self {
            initialized: false,
            packet: [0; 3],
            packet_length: 0,
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            buttons: 0,
            queue: VecDeque::new(),
        }
    }
}

static STATE: SpinLock<MouseState> = SpinLock::new(MouseState::new());
static WAITERS: crate::task::wait::WaitQueue = crate::task::wait::WaitQueue::new();

fn wait_input_empty() -> bool {
    for _ in 0..100_000 {
        if unsafe { in8(0x64) } & INPUT_FULL == 0 { return true; }
        core::hint::spin_loop();
    }
    false
}

fn controller_command(command: u8) -> bool {
    if !wait_input_empty() { return false; }
    unsafe { out8(0x64, command); wait(); }
    true
}

fn controller_data(value: u8) -> bool {
    if !wait_input_empty() { return false; }
    unsafe { out8(0x60, value); wait(); }
    true
}

fn read_controller_data() -> Option<u8> {
    for _ in 0..100_000 {
        let status = unsafe { in8(0x64) };
        if status & OUTPUT_FULL != 0 {
            return Some(unsafe { in8(0x60) });
        }
        core::hint::spin_loop();
    }
    None
}

fn auxiliary_command(command: u8) -> bool {
    if !controller_command(0xd4) || !controller_data(command) { return false; }
    for _ in 0..100_000 {
        let status = unsafe { in8(0x64) };
        if status & OUTPUT_FULL != 0 {
            let value = unsafe { in8(0x60) };
            if status & AUX_STATUS != 0 && value == 0xfa { return true; }
        }
        core::hint::spin_loop();
    }
    false
}

pub fn init(width: u32, height: u32) -> Result<(), &'static str> {
    if !controller_command(0xa8) { return Err("i8042 auxiliary port did not enable"); }
    if !controller_command(0x20) { return Err("i8042 configuration read failed"); }
    let mut configuration = read_controller_data().ok_or("i8042 configuration response timed out")?;
    configuration &= !0x20; // enable auxiliary clock; keep IRQ12 masked for polling
    configuration &= !0x02;
    if !controller_command(0x60) || !controller_data(configuration) {
        return Err("i8042 configuration write failed");
    }
    if !auxiliary_command(0xf6) || !auxiliary_command(0xf4) {
        return Err("PS/2 mouse did not acknowledge streaming mode");
    }
    let mut state = STATE.lock();
    state.initialized = true;
    state.packet = [0; 3];
    state.packet_length = 0;
    state.x = (width / 2) as i32;
    state.y = (height / 2) as i32;
    state.width = width.max(1) as i32;
    state.height = height.max(1) as i32;
    state.buttons = 0;
    state.queue.clear();
    Ok(())
}

fn accept_packet(packet: [u8; 3]) {
    let flags = packet[0];
    if flags & 0x08 == 0 || flags & 0xc0 != 0 { return; }
    let raw_x = packet[1] as i16 - if flags & 0x10 != 0 { 256 } else { 0 };
    let raw_y = packet[2] as i16 - if flags & 0x20 != 0 { 256 } else { 0 };
    let delta_x = raw_x;
    let delta_y = -raw_y;
    let mut state = STATE.lock();
    let buttons = flags & 0x07;
    let changed = buttons ^ state.buttons;
    let next_x = (state.x + delta_x as i32).clamp(0, state.width - 1);
    let next_y = (state.y + delta_y as i32).clamp(0, state.height - 1);
    if next_x == state.x && next_y == state.y && changed == 0 { return; }
    state.x = next_x;
    state.y = next_y;
    state.buttons = buttons;
    if state.queue.len() == EVENT_CAPACITY { state.queue.pop_front(); }
    state.queue.push_back(MouseEvent {
        delta_x,
        delta_y,
        x: next_x,
        y: next_y,
        buttons,
        changed_buttons: changed,
        reserved: [0; 2],
    });
    drop(state);
    WAITERS.wake_all();
}

/// Drain at most four complete packets per timer tick so mouse traffic cannot
/// monopolize interrupt service.
pub fn poll() {
    if !STATE.lock().initialized { return; }
    for _ in 0..12 {
        let status = unsafe { in8(0x64) };
        if status & OUTPUT_FULL == 0 || status & AUX_STATUS == 0 { break; }
        let byte = unsafe { in8(0x60) };
        let completed = {
            let mut state = STATE.lock();
            if state.packet_length == 0 && byte & 0x08 == 0 {
                None
            } else {
                let index = state.packet_length;
                state.packet[index] = byte;
                state.packet_length += 1;
                if state.packet_length == 3 {
                    state.packet_length = 0;
                    Some(state.packet)
                } else {
                    None
                }
            }
        };
        if let Some(packet) = completed { accept_packet(packet); }
    }
}

pub fn try_read() -> Option<MouseEvent> {
    STATE.lock().queue.pop_front()
}

pub fn wait_queue() -> &'static crate::task::wait::WaitQueue {
    &WAITERS
}

pub fn is_initialized() -> bool {
    STATE.lock().initialized
}
