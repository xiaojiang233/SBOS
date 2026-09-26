use crate::sync::SpinLock;

#[derive(Clone, Copy)]
pub struct FramebufferInfo {
    pub base: u64,
    pub size: u64,
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub pixel_format: u32,
}

struct Renderer {
    info: Option<FramebufferInfo>,
    x: usize,
    y: usize,
    escape: u8,
    params: [usize; 4],
    param_count: usize,
    private: bool,
    saved_x: usize,
    saved_y: usize,
}
static RENDERER: SpinLock<Renderer> = SpinLock::new(Renderer {
    info: None,
    x: 0,
    y: 0,
    escape: 0,
    params: [0; 4],
    param_count: 0,
    private: false,
    saved_x: 0,
    saved_y: 0,
});

pub fn init(info: FramebufferInfo) {
    if info.base == 0 || info.width == 0 || info.height == 0 || info.stride < info.width {
        return;
    }
    let mut renderer = RENDERER.lock();
    renderer.info = Some(info);
    renderer.x = 0;
    renderer.y = 0;
    renderer.escape = 0;
    renderer.params = [0; 4];
    renderer.param_count = 0;
    renderer.private = false;
    clear_locked(&mut renderer);
}

fn clear_locked(renderer: &mut Renderer) {
    let Some(info) = renderer.info else {
        return;
    };
    let pixels = info.base as *mut u32;
    let count = info
        .stride
        .saturating_mul(info.height)
        .min((info.size / 4) as usize);
    for i in 0..count {
        unsafe {
            pixels.add(i).write_volatile(0x0010_1420);
        }
    }
}

fn put_pixel(info: FramebufferInfo, x: usize, y: usize, color: u32) {
    if x >= info.width || y >= info.height {
        return;
    }
    let offset = y.saturating_mul(info.stride).saturating_add(x);
    if offset < (info.size / 4) as usize {
        unsafe {
            (info.base as *mut u32).add(offset).write_volatile(color);
        }
    }
}

fn fill_cells(info: FramebufferInfo, x0: usize, y0: usize, x1: usize, y1: usize) {
    let left = x0.saturating_mul(6).min(info.width);
    let top = y0.saturating_mul(10).min(info.height);
    let right = x1.saturating_mul(6).min(info.width);
    let bottom = y1.saturating_mul(10).min(info.height);
    for y in top..bottom {
        for x in left..right {
            put_pixel(info, x, y, 0x0010_1420);
        }
    }
}

fn parameter(renderer: &Renderer, index: usize, default: usize) -> usize {
    renderer.params.get(index).copied().filter(|value| *value != 0).unwrap_or(default)
}

fn handle_csi(renderer: &mut Renderer, byte: u8) {
    let Some(info) = renderer.info else { return };
    let max_columns = (info.width / 6).max(1);
    let max_rows = (info.height / 10).max(1);
    let first = parameter(renderer, 0, 1);
    match byte {
        b'A' => renderer.y = renderer.y.saturating_sub(first.saturating_mul(10)),
        b'B' => renderer.y = renderer.y.saturating_add(first.saturating_mul(10)).min((max_rows - 1) * 10),
        b'C' => renderer.x = renderer.x.saturating_add(first.saturating_mul(6)).min((max_columns - 1) * 6),
        b'D' => renderer.x = renderer.x.saturating_sub(first.saturating_mul(6)),
        b'H' | b'f' => {
            let row = parameter(renderer, 0, 1).saturating_sub(1).min(max_rows - 1);
            let column = parameter(renderer, 1, 1).saturating_sub(1).min(max_columns - 1);
            renderer.x = column * 6;
            renderer.y = row * 10;
        }
        b'J' => match renderer.params[0] {
            0 => fill_cells(info, renderer.x / 6, renderer.y / 10, max_columns, max_rows),
            1 => fill_cells(info, 0, 0, max_columns, renderer.y / 10 + 1),
            2 => fill_cells(info, 0, 0, max_columns, max_rows),
            _ => {}
        },
        b'K' => match renderer.params[0] {
            0 => fill_cells(info, renderer.x / 6, renderer.y / 10, max_columns, renderer.y / 10 + 1),
            1 => fill_cells(info, 0, renderer.y / 10, renderer.x / 6 + 1, renderer.y / 10 + 1),
            2 => fill_cells(info, 0, renderer.y / 10, max_columns, renderer.y / 10 + 1),
            _ => {}
        },
        b's' => {
            renderer.saved_x = renderer.x;
            renderer.saved_y = renderer.y;
        }
        b'u' => {
            renderer.x = renderer.saved_x.min((max_columns - 1) * 6);
            renderer.y = renderer.saved_y.min((max_rows - 1) * 10);
        }
        b'm' | b'h' | b'l' | b'r' => {} // SGR, private modes, and scrolling regions.
        _ => {}
    }
}

fn glyph(ch: u8) -> [u8; 7] {
    let ch = if ch.is_ascii_lowercase() { ch - 32 } else { ch };
    match ch {
        b'A' => [14, 17, 17, 31, 17, 17, 17],
        b'B' => [30, 17, 17, 30, 17, 17, 30],
        b'C' => [14, 17, 16, 16, 16, 17, 14],
        b'D' => [30, 17, 17, 17, 17, 17, 30],
        b'E' => [31, 16, 16, 30, 16, 16, 31],
        b'F' => [31, 16, 16, 30, 16, 16, 16],
        b'G' => [14, 17, 16, 23, 17, 17, 15],
        b'H' => [17, 17, 17, 31, 17, 17, 17],
        b'I' => [14, 4, 4, 4, 4, 4, 14],
        b'J' => [7, 2, 2, 2, 18, 18, 12],
        b'K' => [17, 18, 20, 24, 20, 18, 17],
        b'L' => [16, 16, 16, 16, 16, 16, 31],
        b'M' => [17, 27, 21, 21, 17, 17, 17],
        b'N' => [17, 25, 21, 19, 17, 17, 17],
        b'O' => [14, 17, 17, 17, 17, 17, 14],
        b'P' => [30, 17, 17, 30, 16, 16, 16],
        b'Q' => [14, 17, 17, 17, 21, 18, 13],
        b'R' => [30, 17, 17, 30, 20, 18, 17],
        b'S' => [15, 16, 16, 14, 1, 1, 30],
        b'T' => [31, 4, 4, 4, 4, 4, 4],
        b'U' => [17, 17, 17, 17, 17, 17, 14],
        b'V' => [17, 17, 17, 17, 17, 10, 4],
        b'W' => [17, 17, 17, 21, 21, 21, 10],
        b'X' => [17, 17, 10, 4, 10, 17, 17],
        b'Y' => [17, 17, 10, 4, 4, 4, 4],
        b'Z' => [31, 1, 2, 4, 8, 16, 31],
        b'0' => [14, 17, 19, 21, 25, 17, 14],
        b'1' => [4, 12, 4, 4, 4, 4, 14],
        b'2' => [14, 17, 1, 2, 4, 8, 31],
        b'3' => [30, 1, 1, 14, 1, 1, 30],
        b'4' => [2, 6, 10, 18, 31, 2, 2],
        b'5' => [31, 16, 16, 30, 1, 1, 30],
        b'6' => [14, 16, 16, 30, 17, 17, 14],
        b'7' => [31, 1, 2, 4, 8, 8, 8],
        b'8' => [14, 17, 17, 14, 17, 17, 14],
        b'9' => [14, 17, 17, 15, 1, 1, 14],
        b'>' => [16, 8, 4, 2, 4, 8, 16],
        b'<' => [1, 2, 4, 8, 4, 2, 1],
        b'/' => [1, 2, 2, 4, 8, 8, 16],
        b'\\' => [16, 8, 8, 4, 2, 2, 1],
        b':' => [0, 4, 4, 0, 4, 4, 0],
        b'.' => [0, 0, 0, 0, 0, 12, 12],
        b',' => [0, 0, 0, 0, 4, 4, 8],
        b'!' => [4, 4, 4, 4, 4, 0, 4],
        b'?' => [14, 17, 1, 2, 4, 0, 4],
        b'-' => [0, 0, 0, 31, 0, 0, 0],
        b'_' => [0, 0, 0, 0, 0, 0, 31],
        b'=' => [0, 31, 0, 31, 0, 0, 0],
        b'+' => [0, 4, 4, 31, 4, 4, 0],
        b'(' => [2, 4, 8, 8, 8, 4, 2],
        b')' => [8, 4, 2, 2, 2, 4, 8],
        b'[' => [14, 8, 8, 8, 8, 8, 14],
        b']' => [14, 2, 2, 2, 2, 2, 14],
        b'\'' => [4, 4, 8, 0, 0, 0, 0],
        b'"' => [10, 10, 20, 0, 0, 0, 0],
        b'#' => [10, 31, 10, 10, 31, 10, 0],
        b'%' => [17, 2, 4, 8, 17, 0, 0],
        b'&' => [12, 18, 20, 8, 21, 18, 13],
        b'*' => [0, 21, 14, 31, 14, 21, 0],
        b' ' => [0; 7],
        _ => [31, 17, 5, 9, 4, 0, 4],
    }
}

fn draw(renderer: &mut Renderer, byte: u8) {
    let Some(info) = renderer.info else {
        return;
    };
    if renderer.escape == 1 {
        if byte == b'[' {
            renderer.escape = 2;
            renderer.params = [0; 4];
            renderer.param_count = 1;
            renderer.private = false;
        } else {
            renderer.escape = 0;
        }
        return;
    }
    if renderer.escape == 2 {
        match byte {
            b'?' if renderer.param_count == 1 && renderer.params[0] == 0 => renderer.private = true,
            b'0'..=b'9' => {
                let index = renderer.param_count.saturating_sub(1).min(3);
                renderer.params[index] = renderer.params[index]
                    .saturating_mul(10)
                    .saturating_add((byte - b'0') as usize);
            }
            b';' => renderer.param_count = (renderer.param_count + 1).min(4),
            0x40..=0x7e => {
                handle_csi(renderer, byte);
                renderer.escape = 0;
                renderer.param_count = 0;
                renderer.private = false;
            }
            _ => {
                renderer.escape = 0;
                renderer.param_count = 0;
            }
        }
        return;
    }
    match byte {
        0x1b => {
            renderer.escape = 1;
            return;
        }
        b'\r' => {
            renderer.x = 0;
            return;
        }
        b'\n' => {
            renderer.x = 0;
            renderer.y += 10;
        }
        b'\t' => {
            renderer.x = (renderer.x + 32) & !31;
        }
        0x08 | 0x7f => {
            renderer.x = renderer.x.saturating_sub(6);
        }
        _ => {
            if byte < 0x20 {
                return;
            }
            let pattern = glyph(byte);
            for row in 0..7 {
                for column in 0..5 {
                    if pattern[row] & (1 << (4 - column)) != 0 {
                        put_pixel(info, renderer.x + column, renderer.y + row, 0x00e0_e8f0);
                    }
                }
            }
            renderer.x += 6;
            if renderer.x + 6 >= info.width {
                renderer.x = 0;
                renderer.y += 10;
            }
        }
    }
    if renderer.y + 8 >= info.height {
        clear_locked(renderer);
        renderer.x = 0;
        renderer.y = 0;
    }
}

pub fn write(bytes: &[u8]) {
    let mut renderer = RENDERER.lock();
    for &byte in bytes {
        draw(&mut renderer, byte);
    }
}
