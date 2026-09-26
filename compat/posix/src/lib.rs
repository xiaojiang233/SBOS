#![no_std]
//! Small POSIX file-descriptor adapter over SBOS process-local handles.
//!
//! This crate implements the synchronous descriptor subset used by the first C
//! runtime. Values returned here are POSIX descriptors (0..127), never native
//! kernel handle tokens. Errors are returned as negative POSIX errno values;
//! the C library translates them to `-1` and stores `errno`.

use core::ffi::c_char;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

const FD_COUNT: usize = 128;
const FIRST_FILE_FD: usize = 3;
const MAX_IO: usize = 4096;

pub const EACCES: i32 = 13;
pub const EBADF: i32 = 9;
pub const EINVAL: i32 = 22;
pub const EMFILE: i32 = 24;
pub const ENOENT: i32 = 2;
pub const ENOMEM: i32 = 12;
pub const ENOSYS: i32 = 38;
pub const EAGAIN: i32 = 11;
pub const ETIMEDOUT: i32 = 110;
pub const EFAULT: i32 = 14;
pub const ECHILD: i32 = 10;

static HANDLES: [AtomicU32; FD_COUNT] = [const { AtomicU32::new(0) }; FD_COUNT];
static STANDARD_OPEN: [AtomicBool; 3] = [
    AtomicBool::new(true),
    AtomicBool::new(true),
    AtomicBool::new(true),
];

fn translate_error(value: isize) -> isize {
    match value {
        -1 => -(EINVAL as isize),
        -2 => -(ENOENT as isize),
        -3 => -(EACCES as isize),
        -4 => -(ENOMEM as isize),
        -5 => -(ENOSYS as isize),
        -6 => -(EAGAIN as isize),
        -7 => -(ETIMEDOUT as isize),
        -8 => -(EFAULT as isize),
        -9 => -(ECHILD as isize),
        other => other,
    }
}

/// Open an existing regular file. `rights` uses bit 0 for read and bit 1 for
/// write, matching `O_RDONLY`, `O_WRONLY`, and `O_RDWR` after conversion.
#[no_mangle]
pub unsafe extern "C" fn sbos_posix_open(path: *const c_char, rights: u64) -> isize {
    if path.is_null() || rights == 0 || rights & !3 != 0 {
        return -(EINVAL as isize);
    }
    let mut len = 0usize;
    while len < 512 && *path.add(len) != 0 {
        len += 1;
    }
    if len == 512 {
        return -(EINVAL as isize);
    }
    let bytes = core::slice::from_raw_parts(path.cast::<u8>(), len);
    let Some(path) = core::str::from_utf8(bytes).ok() else {
        return -(EINVAL as isize);
    };
    let Some(handle) = sbos_runtime::file_open(path, rights) else {
        // FILE_OPEN currently uses the kernel's -1..-5 status set. Reissue the
        // syscall here so POSIX callers retain the specific native error.
        let result = sbos_runtime::raw_syscall(
            sbos_runtime::syscall::FILE_OPEN,
            path.as_ptr() as u64,
            path.len() as u64,
            rights,
            0,
            0,
            0,
        );
        return translate_error(result);
    };
    let raw = handle.raw();
    for (fd, slot) in HANDLES.iter().enumerate().skip(FIRST_FILE_FD) {
        if slot
            .compare_exchange(0, raw, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return fd as isize;
        }
    }
    let _ = sbos_runtime::handle_close(handle);
    -(EMFILE as isize)
}

#[no_mangle]
pub unsafe extern "C" fn sbos_posix_read(fd: i32, buffer: *mut u8, length: usize) -> isize {
    if buffer.is_null() && length != 0 {
        return -(EINVAL as isize);
    }
    if fd < 0 || fd as usize >= FD_COUNT {
        return -(EBADF as isize);
    }
    if fd < FIRST_FILE_FD as i32 {
        if !STANDARD_OPEN[fd as usize].load(Ordering::Acquire) {
            return -(EBADF as isize);
        }
        if fd != 0 {
            return -(EBADF as isize);
        }
        return sbos_runtime::raw_syscall(
            sbos_runtime::syscall::CONSOLE_READ,
            buffer as u64,
            length.min(MAX_IO) as u64,
            0,
            0,
            0,
            0,
        );
    }
    let raw = HANDLES[fd as usize].load(Ordering::Acquire);
    if raw == 0 {
        return -(EBADF as isize);
    }
    translate_error(sbos_runtime::raw_syscall(
        sbos_runtime::syscall::FILE_READ,
        raw as u64,
        buffer as u64,
        length.min(MAX_IO) as u64,
        0,
        0,
        0,
    ))
}

#[no_mangle]
pub unsafe extern "C" fn sbos_posix_write(fd: i32, buffer: *const u8, length: usize) -> isize {
    if buffer.is_null() && length != 0 {
        return -(EINVAL as isize);
    }
    if fd < 0 || fd as usize >= FD_COUNT {
        return -(EBADF as isize);
    }
    if fd < FIRST_FILE_FD as i32 {
        if !STANDARD_OPEN[fd as usize].load(Ordering::Acquire) {
            return -(EBADF as isize);
        }
        if fd == 0 {
            return -(EBADF as isize);
        }
        return sbos_runtime::raw_syscall(
            sbos_runtime::syscall::CONSOLE_WRITE,
            buffer as u64,
            length.min(MAX_IO) as u64,
            0,
            0,
            0,
            0,
        );
    }
    let raw = HANDLES[fd as usize].load(Ordering::Acquire);
    if raw == 0 {
        return -(EBADF as isize);
    }
    translate_error(sbos_runtime::raw_syscall(
        sbos_runtime::syscall::FILE_WRITE,
        raw as u64,
        buffer as u64,
        length.min(MAX_IO) as u64,
        0,
        0,
        0,
    ))
}

#[no_mangle]
pub extern "C" fn sbos_posix_close(fd: i32) -> isize {
    if fd < 0 || fd as usize >= FD_COUNT {
        return -(EBADF as isize);
    }
    if fd < FIRST_FILE_FD as i32 {
        return if STANDARD_OPEN[fd as usize].swap(false, Ordering::AcqRel) {
            0
        } else {
            -(EBADF as isize)
        };
    }
    let slot = &HANDLES[fd as usize];
    let raw = slot.load(Ordering::Acquire);
    if raw == 0 {
        return -(EBADF as isize);
    }
    let result = unsafe {
        sbos_runtime::raw_syscall(
            sbos_runtime::syscall::HANDLE_CLOSE,
            raw as u64,
            0,
            0,
            0,
            0,
            0,
        )
    };
    if result < 0 {
        return translate_error(result);
    }
    let _ = slot.compare_exchange(raw, 0, Ordering::AcqRel, Ordering::Acquire);
    0
}

#[no_mangle]
pub extern "C" fn sbos_posix_getpid() -> isize {
    unsafe {
        sbos_runtime::raw_syscall(
            sbos_runtime::syscall::PROCESS_ID,
            0,
            0,
            0,
            0,
            0,
            0,
        )
    }
}

/// Process creation is not emulated: SBOS does not yet implement fork's
/// address-space snapshot and child-return semantics.
#[no_mangle]
pub extern "C" fn sbos_posix_fork() -> isize {
    -(ENOSYS as isize)
}
