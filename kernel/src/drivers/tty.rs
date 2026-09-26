use crate::sync::SpinLock;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

pub const IF_IGNCR: u32 = 0x0008;
pub const IF_ICRNL: u32 = 0x0100;
pub const IF_INLCR: u32 = 0x0040;
pub const IF_ISTRIP: u32 = 0x0020;
pub const OF_OPOST: u32 = 0x0001;
pub const OF_ONLCR: u32 = 0x0004;
pub const OF_OCRNL: u32 = 0x0008;
pub const OF_ONOCR: u32 = 0x0010;
pub const LF_ISIG: u32 = 0x0001;
pub const LF_ICANON: u32 = 0x0002;
pub const LF_ECHO: u32 = 0x0008;
pub const LF_ECHOE: u32 = 0x0010;
pub const LF_ECHOK: u32 = 0x0020;
pub const LF_ECHONL: u32 = 0x0040;
pub const LF_IEXTEN: u32 = 0x8000;
pub const CC_VEOF: usize = 4;
pub const CC_VTIME: usize = 5;
pub const CC_VMIN: usize = 6;
pub const CC_VERASE: usize = 2;
pub const CC_VKILL: usize = 3;
pub const NCCS: usize = 32;

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalAttributes {
    pub input_flags: u32,
    pub output_flags: u32,
    pub control_flags: u32,
    pub local_flags: u32,
    pub line: u8,
    pub control_chars: [u8; NCCS],
    pub input_speed: u32,
    pub output_speed: u32,
}

impl TerminalAttributes {
    const fn bootstrap() -> Self {
        let mut control_chars = [0; NCCS];
        control_chars[CC_VEOF] = 4;
        control_chars[CC_VTIME] = 0;
        control_chars[CC_VMIN] = 1;
        control_chars[CC_VERASE] = 127;
        control_chars[CC_VKILL] = 21;
        Self {
            input_flags: IF_ICRNL,
            output_flags: OF_OPOST | OF_ONLCR,
            control_flags: 0x0030, // CS8
            local_flags: 0,        // raw bootstrap state before a foreground process is selected
            line: 0,
            control_chars,
            input_speed: 0,
            output_speed: 0,
        }
    }

    pub const fn canonical() -> Self {
        let mut attributes = Self::bootstrap();
        attributes.local_flags = LF_ISIG | LF_ICANON | LF_ECHO | LF_ECHOE | LF_ECHOK;
        attributes
    }
}

struct Terminal {
    foreground_pid: u64,
    attributes: TerminalAttributes,
    raw_input: VecDeque<u8>,
    canonical_lines: VecDeque<Vec<u8>>,
    editing_line: Vec<u8>,
    line_offset: usize,
    rows: u16,
    columns: u16,
}

impl Terminal {
    const fn new() -> Self {
        Self {
            foreground_pid: 0,
            attributes: TerminalAttributes::bootstrap(),
            raw_input: VecDeque::new(),
            canonical_lines: VecDeque::new(),
            editing_line: Vec::new(),
            line_offset: 0,
            rows: 25,
            columns: 80,
        }
    }

    fn accept_byte(&mut self, mut byte: u8) -> ([u8; 4], usize) {
        if self.attributes.input_flags & IF_IGNCR != 0 && byte == b'\r' {
            return ([0; 4], 0);
        }
        if self.attributes.input_flags & IF_ICRNL != 0 && byte == b'\r' {
            byte = b'\n';
        } else if self.attributes.input_flags & IF_INLCR != 0 && byte == b'\n' {
            byte = b'\r';
        }
        if self.attributes.input_flags & IF_ISTRIP != 0 {
            byte &= 0x7f;
        }

        if self.attributes.local_flags & LF_ICANON == 0 {
            if self.raw_input.len() < 4096 {
                self.raw_input.push_back(byte);
            }
            return self.echo_byte(byte);
        }

        if byte == self.attributes.control_chars[CC_VERASE] || byte == 8 {
            if self.editing_line.pop().is_some()
                && self.attributes.local_flags & LF_ECHO != 0
                && self.attributes.local_flags & LF_ECHOE != 0
            {
                return (*b"\x08 \x08\0", 3);
            }
            return ([0; 4], 0);
        }
        if byte == self.attributes.control_chars[CC_VKILL] {
            let had_input = !self.editing_line.is_empty();
            self.editing_line.clear();
            if had_input && self.attributes.local_flags & LF_ECHO != 0 {
                if self.attributes.local_flags & LF_ECHOK != 0 {
                    return (*b"\r\n\0\0", 2);
                }
                return (*b"\x15\0\0\0", 1);
            }
            return ([0; 4], 0);
        }
        if byte == self.attributes.control_chars[CC_VEOF] {
            if !self.editing_line.is_empty() {
                self.canonical_lines.push_back(core::mem::take(&mut self.editing_line));
            } else {
                // An empty queued line represents EOF to the next canonical read.
                self.canonical_lines.push_back(Vec::new());
            }
            return ([0; 4], 0);
        }
        if self.editing_line.len() >= 4096 {
            return ([0; 4], 0);
        }
        self.editing_line.push(byte);
        let echo = self.echo_byte(byte);
        if byte == b'\n' {
            self.canonical_lines.push_back(core::mem::take(&mut self.editing_line));
        }
        echo
    }

    fn echo_byte(&self, byte: u8) -> ([u8; 4], usize) {
        let echo = self.attributes.local_flags & LF_ECHO != 0
            || (byte == b'\n' && self.attributes.local_flags & LF_ECHONL != 0);
        if !echo {
            return ([0; 4], 0);
        }
        if byte == b'\n' && self.attributes.output_flags & OF_ONLCR != 0 {
            return (*b"\r\n\0\0", 2);
        }
        let mut bytes = [0; 4];
        bytes[0] = byte;
        (bytes, 1)
    }

    fn ready_bytes(&self) -> usize {
        if self.attributes.local_flags & LF_ICANON != 0 {
            self.canonical_lines.front().map(Vec::len).unwrap_or(0).saturating_sub(self.line_offset)
        } else {
            self.raw_input.len()
        }
    }

    fn has_ready(&self) -> bool {
        if self.attributes.local_flags & LF_ICANON != 0 {
            !self.canonical_lines.is_empty()
        } else {
            !self.raw_input.is_empty()
        }
    }

    fn copy_ready(&mut self, output: &mut [u8]) -> Option<usize> {
        if self.attributes.local_flags & LF_ICANON != 0 {
            let line = self.canonical_lines.front()?;
            if line.is_empty() {
                self.canonical_lines.pop_front();
                self.line_offset = 0;
                return Some(0);
            }
            let count = output.len().min(line.len().saturating_sub(self.line_offset));
            output[..count].copy_from_slice(&line[self.line_offset..self.line_offset + count]);
            self.line_offset += count;
            if self.line_offset == line.len() {
                self.canonical_lines.pop_front();
                self.line_offset = 0;
            }
            Some(count)
        } else {
            if self.raw_input.is_empty() {
                return None;
            }
            let count = output.len().min(self.raw_input.len());
            for slot in output.iter_mut().take(count) {
                *slot = self.raw_input.pop_front().unwrap_or(0);
            }
            Some(count)
        }
    }
}

static TERMINAL: SpinLock<Terminal> = SpinLock::new(Terminal::new());
static FOREGROUND_PROCESS: AtomicU64 = AtomicU64::new(0);

pub fn init(foreground_pid: u64, rows: u16, columns: u16) {
    let mut terminal = TERMINAL.lock();
    terminal.foreground_pid = foreground_pid;
    terminal.attributes = TerminalAttributes::canonical();
    terminal.raw_input.clear();
    terminal.canonical_lines.clear();
    terminal.editing_line.clear();
    terminal.line_offset = 0;
    terminal.rows = rows.max(1);
    terminal.columns = columns.max(1);
    FOREGROUND_PROCESS.store(foreground_pid, Ordering::Release);
}

pub fn set_foreground(pid: u64) {
    TERMINAL.lock().foreground_pid = pid;
    FOREGROUND_PROCESS.store(pid, Ordering::Release);
}

pub fn is_foreground(pid: u64) -> bool {
    let foreground = FOREGROUND_PROCESS.load(Ordering::Acquire);
    if foreground == pid {
        return true;
    }
    let mut current = crate::task::process::by_pid(pid);
    for _ in 0..32 {
        let Some(process) = current else { return false };
        if process.parent_pid == foreground {
            return true;
        }
        if process.parent_pid == 0 {
            return false;
        }
        current = crate::task::process::by_pid(process.parent_pid);
    }
    false
}

pub fn attributes() -> TerminalAttributes {
    TERMINAL.lock().attributes
}

pub fn set_attributes(attributes: TerminalAttributes, action: u64) -> Result<(), &'static str> {
    if action > 2 {
        return Err("invalid terminal attribute action");
    }
    let mut terminal = TERMINAL.lock();
    terminal.attributes = attributes;
    if action == 2 {
        terminal.raw_input.clear();
        terminal.canonical_lines.clear();
        terminal.editing_line.clear();
        terminal.line_offset = 0;
    }
    Ok(())
}

pub fn window_size() -> (u16, u16) {
    let terminal = TERMINAL.lock();
    (terminal.rows, terminal.columns)
}

pub fn read(process_id: u64, output: &mut [u8]) -> usize {
    if output.is_empty() {
        return 0;
    }
    let start = crate::task::scheduler::tick_count();
    loop {
        let input = if is_foreground(process_id) {
            // Either the PS/2 keyboard or the serial console can drive the
            // terminal, so a headless QEMU run can type at the shell too.
            crate::drivers::keyboard::try_read_byte()
                .or_else(crate::drivers::serial::try_read_byte)
        } else {
            None
        };
        let mut echo_bytes = [0u8; 4];
        let mut echo_len = 0;
        {
            let mut terminal = TERMINAL.lock();
            if let Some(byte) = input {
                (echo_bytes, echo_len) = terminal.accept_byte(byte);
            }
            let canonical = terminal.attributes.local_flags & LF_ICANON != 0;
            let minimum = if canonical {
                1
            } else {
                (terminal.attributes.control_chars[CC_VMIN] as usize).min(output.len())
            };
            let available = terminal.ready_bytes();
            let vtime = terminal.attributes.control_chars[CC_VTIME] as u64;
            let elapsed = crate::task::scheduler::tick_count().wrapping_sub(start);
            let timed_out = vtime != 0 && elapsed >= vtime.saturating_mul(10);
            let nonblocking_empty = !canonical && minimum == 0 && vtime == 0;
            if nonblocking_empty {
                if let Some(count) = terminal.copy_ready(output) {
                    drop(terminal);
                    if echo_len != 0 {
                        crate::drivers::console::write(&echo_bytes[..echo_len]);
                    }
                    return count;
                }
            }
            if terminal.has_ready() && (canonical || available >= minimum) {
                if let Some(count) = terminal.copy_ready(output) {
                    drop(terminal);
                    if echo_len != 0 {
                        crate::drivers::console::write(&echo_bytes[..echo_len]);
                    }
                    return count;
                }
            }
            if timed_out {
                if let Some(count) = terminal.copy_ready(output) {
                    return count;
                }
            }
        }
        if echo_len != 0 {
            crate::drivers::console::write(&echo_bytes[..echo_len]);
        }
        crate::interrupt::enable();
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack));
        }
        crate::interrupt::disable();
    }
}

pub fn read_byte(process_id: u64) -> u8 {
    let mut byte = [0u8; 1];
    loop {
        if read(process_id, &mut byte) != 0 {
            return byte[0];
        }
    }
}

pub fn write(bytes: &[u8]) {
    let flags = TERMINAL.lock().attributes.output_flags;
    if flags & OF_OPOST == 0 || flags & OF_ONLCR == 0 {
        crate::drivers::console::write(bytes);
        return;
    }
    // ONLCR: expand each newline into CR LF. The translation runs straight to
    // the console instead of through a temporary buffer, so writing to the
    // terminal never depends on the kernel heap.
    let mut start = 0;
    for (index, &byte) in bytes.iter().enumerate() {
        if byte == b'\n' {
            crate::drivers::console::write(&bytes[start..index]);
            crate::drivers::console::write(b"\r\n");
            start = index + 1;
        }
    }
    if start < bytes.len() {
        crate::drivers::console::write(&bytes[start..]);
    }
}
