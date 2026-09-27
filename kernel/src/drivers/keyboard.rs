//! Polled PS/2 set-1 keyboard driver.
//!
//! The driver publishes two independent views of input: translated bytes for
//! the bootstrap TTY and physical key transitions for graphical clients. The
//! byte queue and event queue are bounded so a stalled consumer cannot grow
//! kernel memory without limit.

use crate::arch::x86_64::port::in8;
use crate::sync::SpinLock;
use alloc::collections::VecDeque;

const QUEUE_CAPACITY: usize = 256;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KeyEvent {
    /// X11-compatible keycode (PC set-1 scan code + 8 for ordinary keys).
    pub keycode: u8,
    /// 1 for press, 0 for release.
    pub pressed: u8,
    /// X11 core modifier mask before this transition: Shift, Lock, Control, Mod1.
    pub modifiers: u16,
    pub reserved: [u8; 4],
}

struct KeyboardState {
    left_shift: bool,
    right_shift: bool,
    left_control: bool,
    right_control: bool,
    left_alt: bool,
    right_alt: bool,
    caps_lock: bool,
    extended: bool,
    pause_bytes: u8,
    input_owner: u64,
    bytes: VecDeque<u8>,
    events: VecDeque<KeyEvent>,
}

impl KeyboardState {
    const fn new() -> Self {
        Self {
            left_shift: false,
            right_shift: false,
            left_control: false,
            right_control: false,
            left_alt: false,
            right_alt: false,
            caps_lock: false,
            extended: false,
            pause_bytes: 0,
            input_owner: 0,
            bytes: VecDeque::new(),
            events: VecDeque::new(),
        }
    }

    fn modifiers(&self) -> u16 {
        (if self.left_shift || self.right_shift {
            1 << 0
        } else {
            0
        }) | (if self.caps_lock { 1 << 1 } else { 0 })
            | (if self.left_control || self.right_control {
                1 << 2
            } else {
                0
            })
            | (if self.left_alt || self.right_alt {
                1 << 3
            } else {
                0
            })
    }
}

static STATE: SpinLock<KeyboardState> = SpinLock::new(KeyboardState::new());
static WAITERS: crate::task::wait::WaitQueue = crate::task::wait::WaitQueue::new();

fn translate(scan: u8, shift: bool, caps_lock: bool) -> Option<u8> {
    let (plain, upper) = match scan {
        0x02 => (b'1', b'!'),
        0x03 => (b'2', b'@'),
        0x04 => (b'3', b'#'),
        0x05 => (b'4', b'$'),
        0x06 => (b'5', b'%'),
        0x07 => (b'6', b'^'),
        0x08 => (b'7', b'&'),
        0x09 => (b'8', b'*'),
        0x0a => (b'9', b'('),
        0x0b => (b'0', b')'),
        0x0c => (b'-', b'_'),
        0x0d => (b'=', b'+'),
        0x0e => (8, 8),
        0x0f => (b'\t', b'\t'),
        0x10 => (b'q', b'Q'),
        0x11 => (b'w', b'W'),
        0x12 => (b'e', b'E'),
        0x13 => (b'r', b'R'),
        0x14 => (b't', b'T'),
        0x15 => (b'y', b'Y'),
        0x16 => (b'u', b'U'),
        0x17 => (b'i', b'I'),
        0x18 => (b'o', b'O'),
        0x19 => (b'p', b'P'),
        0x1a => (b'[', b'{'),
        0x1b => (b']', b'}'),
        0x1c => (b'\n', b'\n'),
        0x1e => (b'a', b'A'),
        0x1f => (b's', b'S'),
        0x20 => (b'd', b'D'),
        0x21 => (b'f', b'F'),
        0x22 => (b'g', b'G'),
        0x23 => (b'h', b'H'),
        0x24 => (b'j', b'J'),
        0x25 => (b'k', b'K'),
        0x26 => (b'l', b'L'),
        0x27 => (b';', b':'),
        0x28 => (b'\'', b'"'),
        0x29 => (b'`', b'~'),
        0x2b => (b'\\', b'|'),
        0x2c => (b'z', b'Z'),
        0x2d => (b'x', b'X'),
        0x2e => (b'c', b'C'),
        0x2f => (b'v', b'V'),
        0x30 => (b'b', b'B'),
        0x31 => (b'n', b'N'),
        0x32 => (b'm', b'M'),
        0x33 => (b',', b'<'),
        0x34 => (b'.', b'>'),
        0x35 => (b'/', b'?'),
        0x39 => (b' ', b' '),
        _ => return None,
    };
    let letter = plain.is_ascii_lowercase();
    Some(if shift ^ (caps_lock && letter) {
        upper
    } else {
        plain
    })
}

fn x_keycode(scan: u8, extended: bool) -> Option<u8> {
    if extended {
        return Some(match scan {
            0x1c => 104, // keypad Enter
            0x1d => 105, // right Control
            0x35 => 106, // keypad Divide
            0x38 => 108, // right Alt
            0x47 => 110, // Home
            0x48 => 111, // Up
            0x49 => 112, // Page Up
            0x4b => 113, // Left
            0x4d => 114, // Right
            0x4f => 115, // End
            0x50 => 116, // Down
            0x51 => 117, // Page Down
            0x52 => 118, // Insert
            0x53 => 119, // Delete
            _ => return None,
        });
    }
    if (1..=0x58).contains(&scan) {
        Some(scan + 8)
    } else {
        None
    }
}

fn queue_byte(state: &mut KeyboardState, byte: u8) {
    if state.bytes.len() == QUEUE_CAPACITY {
        state.bytes.pop_front();
    }
    state.bytes.push_back(byte);
}

fn queue_key_event(state: &mut KeyboardState, keycode: u8, pressed: bool, modifiers: u16) {
    if state.events.len() == QUEUE_CAPACITY {
        state.events.pop_front();
    }
    state.events.push_back(KeyEvent {
        keycode,
        pressed: pressed as u8,
        modifiers,
        reserved: [0; 4],
    });
}

fn handle_scancode(code: u8) {
    let mut state = STATE.lock();
    if state.pause_bytes != 0 {
        state.pause_bytes -= 1;
        return;
    }
    if code == 0xe0 {
        state.extended = true;
        return;
    }
    if code == 0xe1 {
        // Discard the five remaining bytes in Pause/Break's set-1 sequence.
        state.extended = false;
        state.pause_bytes = 5;
        return;
    }
    let extended = core::mem::replace(&mut state.extended, false);
    let pressed = code & 0x80 == 0;
    let scan = code & 0x7f;
    let modifiers_before = state.modifiers();
    match (extended, scan) {
        (false, 0x2a) => state.left_shift = pressed,
        (false, 0x36) => state.right_shift = pressed,
        (false, 0x1d) => state.left_control = pressed,
        (true, 0x1d) => state.right_control = pressed,
        (false, 0x38) => state.left_alt = pressed,
        (true, 0x38) => state.right_alt = pressed,
        (false, 0x3a) if pressed => state.caps_lock = !state.caps_lock,
        _ => {}
    }

    if let Some(keycode) = x_keycode(scan, extended) {
        queue_key_event(&mut state, keycode, pressed, modifiers_before);
    }
    if !pressed {
        return;
    }
    if extended {
        if state.input_owner == 0 {
            let sequence: &[u8] = match scan {
                0x48 => b"\x1b[A",
                0x50 => b"\x1b[B",
                0x4d => b"\x1b[C",
                0x4b => b"\x1b[D",
                0x47 => b"\x1b[H",
                0x4f => b"\x1b[F",
                0x53 => b"\x1b[3~",
                0x49 => b"\x1b[5~",
                0x51 => b"\x1b[6~",
                _ => &[],
            };
            for byte in sequence {
                queue_byte(&mut state, *byte);
            }
        }
        return;
    }
    if state.input_owner == 0 {
        if let Some(byte) = translate(scan, state.left_shift || state.right_shift, state.caps_lock)
        {
            queue_byte(&mut state, byte);
        }
    }
}

/// Drain a bounded amount of PS/2 keyboard input from the i8042 output buffer.
/// Auxiliary-port bytes are left for the mouse driver.
pub fn poll() {
    let mut received = false;
    for _ in 0..32 {
        let status = unsafe { in8(0x64) };
        if status & 1 == 0 || status & 0x20 != 0 {
            break;
        }
        handle_scancode(unsafe { in8(0x60) });
        received = true;
    }
    if received {
        WAITERS.wake_all();
    }
}

pub fn try_read_byte() -> Option<u8> {
    STATE.lock().bytes.pop_front()
}

pub fn try_read_event() -> Option<KeyEvent> {
    STATE.lock().events.pop_front()
}

/// Claim raw key events exclusively from the TTY byte stream.
pub fn claim_input(process_id: u64) -> bool {
    let mut state = STATE.lock();
    if state.input_owner != 0 && state.input_owner != process_id {
        return false;
    }
    state.input_owner = process_id;
    state.bytes.clear();
    state.events.clear();
    true
}

/// Release a process's input lease when it exits or explicitly gives it up.
pub fn release_input(process_id: u64) {
    let mut state = STATE.lock();
    if state.input_owner == process_id {
        state.input_owner = 0;
        state.bytes.clear();
        state.events.clear();
    }
}

pub fn wait_queue() -> &'static crate::task::wait::WaitQueue {
    &WAITERS
}

pub fn read_byte() -> u8 {
    loop {
        if let Some(byte) = try_read_byte() {
            return byte;
        }
        core::hint::spin_loop();
    }
}
