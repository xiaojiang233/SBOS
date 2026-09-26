use crate::arch::x86_64::port::in8;

static mut LEFT_SHIFT: bool = false;
static mut EXTENDED: bool = false;
static mut PENDING: [u8; 4] = [0; 4];
static mut PENDING_INDEX: usize = 0;
static mut PENDING_LENGTH: usize = 0;

fn translate(scan: u8, shift: bool) -> Option<u8> {
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
    Some(if shift { upper } else { plain })
}

pub fn try_read_byte() -> Option<u8> {
    unsafe {
        if PENDING_INDEX < PENDING_LENGTH {
            let byte = PENDING[PENDING_INDEX];
            PENDING_INDEX += 1;
            if PENDING_INDEX == PENDING_LENGTH {
                PENDING_INDEX = 0;
                PENDING_LENGTH = 0;
            }
            return Some(byte);
        }
    }
    if let Some(byte) = super::serial::try_read_byte() {
        return Some(byte);
    }
    if unsafe { in8(0x64) } & 1 == 0 {
        return None;
    }
    let code = unsafe { in8(0x60) };
    if code == 0xe0 {
        unsafe { EXTENDED = true; }
        return None;
    }
    let extended = unsafe {
        let value = EXTENDED;
        EXTENDED = false;
        value
    };
    if extended {
        if code & 0x80 != 0 { return None; }
        let sequence: Option<(&[u8], usize)> = match code {
            0x48 => Some((b"\x1b[A", 3)),
            0x50 => Some((b"\x1b[B", 3)),
            0x4d => Some((b"\x1b[C", 3)),
            0x4b => Some((b"\x1b[D", 3)),
            0x47 => Some((b"\x1b[H", 3)),
            0x4f => Some((b"\x1b[F", 3)),
            0x53 => Some((b"\x1b[3~", 4)),
            0x49 => Some((b"\x1b[5~", 4)),
            0x51 => Some((b"\x1b[6~", 4)),
            _ => None,
        };
        if let Some((sequence, length)) = sequence {
            unsafe {
                PENDING[..length - 1].copy_from_slice(&sequence[1..length]);
                PENDING_INDEX = 0;
                PENDING_LENGTH = length - 1;
            }
            return Some(sequence[0]);
        }
        return None;
    }
    if code == 0x2a || code == 0x36 {
        unsafe {
            LEFT_SHIFT = true;
        }
        return None;
    }
    if code == 0xaa || code == 0xb6 {
        unsafe {
            LEFT_SHIFT = false;
        }
        return None;
    }
    if code & 0x80 != 0 {
        return None;
    }
    translate(code, unsafe { LEFT_SHIFT })
}

pub fn read_byte() -> u8 {
    loop {
        if let Some(byte) = try_read_byte() {
            return byte;
        }
        core::hint::spin_loop();
    }
}
