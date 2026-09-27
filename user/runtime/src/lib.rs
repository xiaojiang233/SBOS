#![no_std]

//! Small Rust runtime for the SBOS Native System Call ABI.
//! Handles are opaque process-local values; they are never kernel pointers.

use core::marker::PhantomData;

pub mod syscall {
    pub const HANDLE_CLOSE: u64 = 0;
    pub const FILE_OPEN: u64 = 1;
    pub const FILE_READ: u64 = 2;
    pub const FILE_WRITE: u64 = 3;
    pub const PROCESS_EXIT: u64 = 4;
    pub const PROCESS_SPAWN: u64 = 5;
    pub const THREAD_CREATE: u64 = 6;
    pub const HANDLE_WAIT: u64 = 7;
    pub const CHANNEL_CREATE: u64 = 8;
    pub const CHANNEL_SEND: u64 = 9;
    pub const CHANNEL_RECEIVE: u64 = 10;
    pub const MEMORY_MAP: u64 = 11;
    pub const MEMORY_UNMAP: u64 = 12;
    pub const CONSOLE_READ: u64 = 13;
    pub const CONSOLE_WRITE: u64 = 14;
    pub const DIRECTORY_OPEN: u64 = 15;
    pub const DIRECTORY_READ: u64 = 16;
    pub const DIRECTORY_CHANGE: u64 = 17;
    pub const DIRECTORY_CURRENT: u64 = 18;
    pub const SYSTEM_QUERY: u64 = 19;
    pub const CONFIG_SET: u64 = 20;
    pub const IO_SUBMIT: u64 = 21;
    pub const CONFIG_DELETE: u64 = 22;
    pub const CONFIG_WATCH: u64 = 23;
    pub const CONFIG_BEGIN: u64 = 24;
    pub const CONFIG_TRANSACTION_SET: u64 = 25;
    pub const CONFIG_TRANSACTION_DELETE: u64 = 26;
    pub const CONFIG_COMMIT: u64 = 27;
    pub const EVENT_RESET: u64 = 28;
    pub const SERVICE_LOOKUP: u64 = 29;
    pub const TTY_SET_FOREGROUND: u64 = 30;
    pub const PROCESS_ID: u64 = 31;
    pub const SCHEDULER_TICKS: u64 = 32;
    pub const STAT_PATH: u64 = 33;
    pub const HANDLE_STAT: u64 = 34;
    pub const HANDLE_DUPLICATE: u64 = 35;
    pub const PROCESS_UID: u64 = 36;
    pub const PROCESS_GID: u64 = 37;
    pub const FILE_SEEK: u64 = 38;
    pub const PROCESS_CPU_TICKS: u64 = 39;
    pub const PROCESS_WAITPID: u64 = 40;
    pub const PROCESS_PARENT_ID: u64 = 41;
    pub const PIPE_CREATE: u64 = 42;
    pub const PIPE_READ: u64 = 43;
    pub const PIPE_WRITE: u64 = 44;
    pub const PROCESS_FORK: u64 = 45;
    pub const POSIX_OPEN: u64 = 46;
    pub const POSIX_READ: u64 = 47;
    pub const POSIX_WRITE: u64 = 48;
    pub const POSIX_CLOSE: u64 = 49;
    pub const POSIX_DUP: u64 = 50;
    pub const POSIX_DUP2: u64 = 51;
    pub const POSIX_PIPE: u64 = 52;
    pub const POSIX_FSTAT: u64 = 53;
    pub const POSIX_LSEEK: u64 = 54;
    pub const POSIX_FCNTL: u64 = 55;
    pub const POSIX_ISATTY: u64 = 56;
    pub const POSIX_UNLINK: u64 = 57;
    pub const POSIX_TTY_GET: u64 = 58;
    pub const POSIX_TTY_SET: u64 = 59;
    pub const POSIX_IOCTL: u64 = 60;
    pub const POSIX_EXECVE: u64 = 61;
    pub const DIRECTORY_CREATE: u64 = 64;
    pub const DIRECTORY_REMOVE: u64 = 65;
    pub const FILE_SYNC: u64 = 66;
    pub const THREAD_SLEEP: u64 = 67;
    pub const DISPLAY_INFO: u64 = 75;
    pub const DISPLAY_FILL_RECT: u64 = 76;
    pub const DISPLAY_BLIT_RECT: u64 = 77;
    pub const MOUSE_READ_EVENT: u64 = 78;
    pub const POSIX_LISTEN: u64 = 79;
    pub const POSIX_ACCEPT: u64 = 80;
    pub const KEYBOARD_READ_EVENT: u64 = 82;
    pub const KEYBOARD_CLAIM_INPUT: u64 = 83;
}

pub mod query {
    pub const PROCESSES: u64 = 1;
    pub const DEVICES: u64 = 2;
    pub const VOLUMES: u64 = 3;
    pub const SERVICES: u64 = 4;
    pub const CONFIG_GET: u64 = 5;
}

pub struct FileObject;
pub struct DirectoryObject;
pub struct ProcessObject;
pub struct ThreadObject;
pub struct ChannelObject;
pub struct EventObject;
pub struct ConfigTransactionObject;
pub struct ServiceObject;

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisplayInfo {
    pub width: u32,
    pub height: u32,
    pub pixel_format: u32,
    pub reserved: u32,
}

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

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct KeyEvent {
    pub keycode: u8,
    pub pressed: u8,
    pub modifiers: u16,
    pub reserved: [u8; 4],
}

pub fn keyboard_read_event(event: &mut KeyEvent, nonblocking: bool) -> isize {
    unsafe {
        raw_syscall(
            syscall::KEYBOARD_READ_EVENT,
            event as *mut KeyEvent as u64,
            nonblocking as u64,
            0, 0, 0, 0,
        )
    }
}

pub fn keyboard_claim_input(claim: bool) -> bool {
    unsafe {
        raw_syscall(
            syscall::KEYBOARD_CLAIM_INPUT,
            claim as u64,
            0,
            0,
            0,
            0,
            0,
        ) == 0
    }
}

pub fn mouse_read_event(event: &mut MouseEvent, nonblocking: bool) -> isize {
    unsafe {
        raw_syscall(
            syscall::MOUSE_READ_EVENT,
            event as *mut MouseEvent as u64,
            nonblocking as u64,
            0, 0, 0, 0,
        )
    }
}

pub fn display_info() -> Option<DisplayInfo> {
    let mut info = DisplayInfo { width: 0, height: 0, pixel_format: 0, reserved: 0 };
    let result = unsafe {
        raw_syscall(
            syscall::DISPLAY_INFO,
            (&mut info as *mut DisplayInfo) as u64,
            0, 0, 0, 0, 0,
        )
    };
    if result == core::mem::size_of::<DisplayInfo>() as isize { Some(info) } else { None }
}

pub fn display_fill_rect(x: u32, y: u32, width: u32, height: u32, rgb: u32) -> bool {
    unsafe {
        raw_syscall(syscall::DISPLAY_FILL_RECT,
                    x as u64, y as u64, width as u64, height as u64, rgb as u64, 0) == 0
    }
}

pub fn display_blit_rect(x: u32, y: u32, width: u32, height: u32,
                         pixels: &[u32], stride_pixels: u32) -> bool {
    if stride_pixels < width ||
       (stride_pixels as usize).checked_mul(height as usize) != Some(pixels.len()) {
        return false;
    }
    let result = unsafe {
        raw_syscall(
            syscall::DISPLAY_BLIT_RECT, x as u64, y as u64,
            width as u64, height as u64, pixels.as_ptr() as u64,
            stride_pixels as u64,
        )
    };
    result == pixels.len() as isize
}

#[repr(transparent)]
#[derive(Eq, PartialEq, Debug)]
pub struct Handle<T> {
    raw: u32,
    _type: PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    /// Construct a typed wrapper around a handle returned by the kernel.
    /// The token remains process-local and must not be guessed or reused after close.
    pub const unsafe fn from_raw(raw: u32) -> Self {
        Self {
            raw,
            _type: PhantomData,
        }
    }
    pub const fn raw(self) -> u32 {
        self.raw
    }
}
impl<T> Copy for Handle<T> {}
impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

/// Invoke the x86_64 SBOS ABI: number in rax, arguments in rdi/rsi/rdx/r10/r8/r9.
/// The kernel validates every pointer before accessing user memory.
#[inline(always)]
pub unsafe fn raw_syscall(
    number: u64,
    a0: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
) -> isize {
    let result: u64;
    core::arch::asm!(
        "int 0x80",
        inlateout("rax") number => result,
        in("rdi") a0,
        in("rsi") a1,
        in("rdx") a2,
        in("r10") a3,
        in("r8") a4,
        in("r9") a5,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack)
    );
    result as isize
}

fn call(number: u64, a0: u64, a1: u64, a2: u64) -> isize {
    unsafe { raw_syscall(number, a0, a1, a2, 0, 0, 0) }
}

pub fn console_write(bytes: &[u8]) -> usize {
    call(
        syscall::CONSOLE_WRITE,
        bytes.as_ptr() as u64,
        bytes.len() as u64,
        0,
    )
    .max(0) as usize
}

pub fn console_read(bytes: &mut [u8]) -> usize {
    call(
        syscall::CONSOLE_READ,
        bytes.as_mut_ptr() as u64,
        bytes.len() as u64,
        0,
    )
    .max(0) as usize
}

pub fn file_open(path: &str, rights: u64) -> Option<Handle<FileObject>> {
    let raw = call(
        syscall::FILE_OPEN,
        path.as_ptr() as u64,
        path.len() as u64,
        rights,
    );
    if raw < 0 {
        None
    } else {
        Some(unsafe { Handle::from_raw(raw as u32) })
    }
}

pub fn file_read(handle: Handle<FileObject>, out: &mut [u8]) -> isize {
    call(
        syscall::FILE_READ,
        handle.raw as u64,
        out.as_mut_ptr() as u64,
        out.len() as u64,
    )
}

pub fn file_write(handle: Handle<FileObject>, bytes: &[u8]) -> isize {
    call(
        syscall::FILE_WRITE,
        handle.raw as u64,
        bytes.as_ptr() as u64,
        bytes.len() as u64,
    )
}

pub fn directory_open(path: &str) -> Option<Handle<DirectoryObject>> {
    let raw = call(
        syscall::DIRECTORY_OPEN,
        path.as_ptr() as u64,
        path.len() as u64,
        0,
    );
    if raw < 0 {
        None
    } else {
        Some(unsafe { Handle::from_raw(raw as u32) })
    }
}

pub fn directory_read(handle: Handle<DirectoryObject>, out: &mut [u8]) -> isize {
    call(
        syscall::DIRECTORY_READ,
        handle.raw as u64,
        out.as_mut_ptr() as u64,
        out.len() as u64,
    )
}

pub fn directory_change(path: &str) -> bool {
    call(
        syscall::DIRECTORY_CHANGE,
        path.as_ptr() as u64,
        path.len() as u64,
        0,
    ) == 0
}

pub fn directory_current(out: &mut [u8]) -> usize {
    call(
        syscall::DIRECTORY_CURRENT,
        out.as_mut_ptr() as u64,
        out.len() as u64,
        0,
    )
    .max(0) as usize
}

pub fn handle_close<T>(handle: Handle<T>) -> bool {
    call(syscall::HANDLE_CLOSE, handle.raw as u64, 0, 0) == 0
}

pub fn process_exit(status: i32) -> ! {
    let _ = call(syscall::PROCESS_EXIT, status as u32 as u64, 0, 0);
    loop {
        core::hint::spin_loop();
    }
}

pub fn process_uid() -> u32 {
    call(syscall::PROCESS_UID, 0, 0, 0).max(0) as u32
}

pub fn process_gid() -> u32 {
    call(syscall::PROCESS_GID, 0, 0, 0).max(0) as u32
}

pub fn process_spawn(path: &str) -> Option<Handle<ProcessObject>> {
    let raw = call(
        syscall::PROCESS_SPAWN,
        path.as_ptr() as u64,
        path.len() as u64,
        0,
    );
    if raw < 0 {
        None
    } else {
        Some(unsafe { Handle::from_raw(raw as u32) })
    }
}

/// Start `path` with an explicit argument vector.
///
/// The arguments travel to the kernel as one NUL separated buffer, which the
/// kernel splits back into argv. Pass the program name as the first element,
/// the way execve expects it.
pub fn process_spawn_with_args(path: &str, arguments: &[&str]) -> Option<Handle<ProcessObject>> {
    const CAPACITY: usize = 2048;
    let mut buffer = [0u8; CAPACITY];
    let mut length = 0usize;
    for argument in arguments {
        let bytes = argument.as_bytes();
        if length + bytes.len() + 1 > CAPACITY {
            return None;
        }
        buffer[length..length + bytes.len()].copy_from_slice(bytes);
        length += bytes.len();
        buffer[length] = 0;
        length += 1;
    }
    let raw = unsafe {
        raw_syscall(
            syscall::PROCESS_SPAWN,
            path.as_ptr() as u64,
            path.len() as u64,
            buffer.as_ptr() as u64,
            length as u64,
            0,
            0,
        )
    };
    if raw < 0 {
        None
    } else {
        Some(unsafe { Handle::from_raw(raw as u32) })
    }
}

pub fn thread_create(entry: usize, argument: usize) -> Option<Handle<ThreadObject>> {
    let raw = call(syscall::THREAD_CREATE, entry as u64, argument as u64, 0);
    if raw < 0 {
        None
    } else {
        Some(unsafe { Handle::from_raw(raw as u32) })
    }
}

pub fn handle_wait<T>(handle: Handle<T>, timeout_ticks: u64) -> isize {
    call(syscall::HANDLE_WAIT, handle.raw as u64, timeout_ticks, 0)
}

pub fn channel_create() -> Option<(Handle<ChannelObject>, Handle<ChannelObject>)> {
    let mut pair = [0u32; 2];
    let result = unsafe {
        raw_syscall(
            syscall::CHANNEL_CREATE,
            pair.as_mut_ptr() as u64,
            0,
            0,
            0,
            0,
            0,
        )
    };
    if result < 0 {
        None
    } else {
        Some((unsafe { Handle::from_raw(pair[0]) }, unsafe {
            Handle::from_raw(pair[1])
        }))
    }
}

pub fn channel_send(handle: Handle<ChannelObject>, bytes: &[u8]) -> isize {
    call(
        syscall::CHANNEL_SEND,
        handle.raw as u64,
        bytes.as_ptr() as u64,
        bytes.len() as u64,
    )
}

/// Send a bounded message with duplicated object handles. Each source handle
/// must carry TRANSFER; the receiver gets a new process-local token with the
/// same rights, and the sender keeps its original token.
pub fn channel_send_with_handles(
    handle: Handle<ChannelObject>,
    bytes: &[u8],
    transferred: &[u32],
) -> isize {
    unsafe {
        raw_syscall(
            syscall::CHANNEL_SEND,
            handle.raw as u64,
            bytes.as_ptr() as u64,
            bytes.len() as u64,
            transferred.as_ptr() as u64,
            transferred.len() as u64,
            0,
        ) as isize
    }
}

pub fn channel_receive(handle: Handle<ChannelObject>, out: &mut [u8]) -> isize {
    let mut transferred = [0u32; 8];
    let (result, count) = channel_receive_with_handles(handle, out, &mut transferred);
    if count != 0 {
        for raw in transferred.iter().take(count) {
            let _ = call(syscall::HANDLE_CLOSE, *raw as u64, 0, 0);
        }
        -1
    } else {
        result
    }
}

pub fn channel_receive_with_handles(
    handle: Handle<ChannelObject>,
    out: &mut [u8],
    transferred: &mut [u32],
) -> (isize, usize) {
    let mut count = 0u64;
    let result = unsafe {
        raw_syscall(
            syscall::CHANNEL_RECEIVE,
            handle.raw as u64,
            out.as_mut_ptr() as u64,
            out.len() as u64,
            (&mut count as *mut u64) as u64,
            transferred.as_mut_ptr() as u64,
            transferred.len() as u64,
        ) as isize
    };
    (result, count as usize)
}

pub fn memory_map(address: usize, length: usize, permissions: u64) -> Option<usize> {
    let raw = call(
        syscall::MEMORY_MAP,
        address as u64,
        length as u64,
        permissions,
    );
    if raw < 0 {
        None
    } else {
        Some(raw as usize)
    }
}

pub fn memory_unmap(address: usize, length: usize) -> bool {
    call(syscall::MEMORY_UNMAP, address as u64, length as u64, 0) == 0
}

pub fn system_query(kind: u64, input: &[u8], output: &mut [u8]) -> isize {
    unsafe {
        raw_syscall(
            syscall::SYSTEM_QUERY,
            kind,
            input.as_ptr() as u64,
            input.len() as u64,
            output.as_mut_ptr() as u64,
            output.len() as u64,
            0,
        )
    }
}

pub fn config_set(key: &str, value: &str) -> bool {
    let Some(transaction) = config_begin() else {
        return false;
    };
    let committed =
        config_transaction_set(transaction, key, value) && config_commit(transaction) >= 0;
    let _ = handle_close(transaction);
    committed
}

pub fn config_delete(key: &str) -> bool {
    let Some(transaction) = config_begin() else {
        return false;
    };
    let committed = config_transaction_delete(transaction, key) && config_commit(transaction) >= 0;
    let _ = handle_close(transaction);
    committed
}
pub fn config_watch(prefix: &str) -> Option<Handle<EventObject>> {
    let raw = call(
        syscall::CONFIG_WATCH,
        prefix.as_ptr() as u64,
        prefix.len() as u64,
        0,
    );
    if raw < 0 {
        None
    } else {
        Some(unsafe { Handle::from_raw(raw as u32) })
    }
}
pub fn event_reset(event: Handle<EventObject>) -> bool {
    call(syscall::EVENT_RESET, event.raw as u64, 0, 0) == 0
}
pub fn config_begin() -> Option<Handle<ConfigTransactionObject>> {
    let raw = call(syscall::CONFIG_BEGIN, 0, 0, 0);
    if raw < 0 {
        None
    } else {
        Some(unsafe { Handle::from_raw(raw as u32) })
    }
}
pub fn config_transaction_set(
    transaction: Handle<ConfigTransactionObject>,
    key: &str,
    value: &str,
) -> bool {
    unsafe {
        raw_syscall(
            syscall::CONFIG_TRANSACTION_SET,
            transaction.raw as u64,
            key.as_ptr() as u64,
            key.len() as u64,
            value.as_ptr() as u64,
            value.len() as u64,
            0,
        ) == 0
    }
}
pub fn config_transaction_delete(transaction: Handle<ConfigTransactionObject>, key: &str) -> bool {
    unsafe {
        raw_syscall(
            syscall::CONFIG_TRANSACTION_DELETE,
            transaction.raw as u64,
            key.as_ptr() as u64,
            key.len() as u64,
            0,
            0,
            0,
        ) == 0
    }
}
pub fn config_commit(transaction: Handle<ConfigTransactionObject>) -> isize {
    call(syscall::CONFIG_COMMIT, transaction.raw as u64, 0, 0)
}
pub fn service_lookup(name: &str) -> Option<Handle<ServiceObject>> {
    let raw = call(
        syscall::SERVICE_LOOKUP,
        name.as_ptr() as u64,
        name.len() as u64,
        0,
    );
    if raw < 0 {
        None
    } else {
        Some(unsafe { Handle::from_raw(raw as u32) })
    }
}

pub fn tty_set_foreground() -> bool {
    call(syscall::TTY_SET_FOREGROUND, 0, 0, 0) == 0
}

/// Queue a file operation for completion-driven I/O. The request layout is ABI version 1.
pub fn io_submit(request: &mut IoRequest) -> isize {
    call(
        syscall::IO_SUBMIT,
        request as *mut IoRequest as u64,
        core::mem::size_of::<IoRequest>() as u64,
        0,
    )
}

#[repr(C)]
pub struct IoRequest {
    pub handle: u32,
    pub operation: u16,
    pub flags: u16,
    pub buffer: u64,
    pub length: u64,
    pub offset: u64,
    pub completion_key: u64,
    pub result: i64,
    pub completed: u32,
    pub reserved: u32,
}
