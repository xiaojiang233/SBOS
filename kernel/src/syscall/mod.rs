use crate::arch::x86_64::idt::TrapFrame;
use crate::config;
use crate::device;
use crate::exec::elf;
use crate::fs::{
    file::{DirectoryObject, FileObject},
    vfs,
};
use crate::handle::{AccessRights, Handle};
use crate::io::CompletionQueue;
use crate::memory::{pmm, vmm};
use crate::object::EventObject;
use crate::security::SecurityContext;
use crate::sync::SpinLock;
use crate::task::{
    process::{self, Process},
    scheduler,
    thread::{self, Thread},
};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::arch::asm;
use core::fmt::{self, Write};

const ERR_INVALID: isize = -1;
const ERR_NOT_FOUND: isize = -2;
const ERR_DENIED: isize = -3;
const ERR_NO_MEMORY: isize = -4;
const ERR_UNSUPPORTED: isize = -5;
const ERR_WOULD_BLOCK: isize = -6;
const ERR_TIMED_OUT: isize = -7;
const ERR_FAULT: isize = -8;
const ERR_NO_CHILD: isize = -9;
const ERR_BROKEN_PIPE: isize = -10;
const MAX_COPY: usize = 4096;
/// Upper bound for the packed argv buffer accepted by the process spawn call.
const MAX_ARGUMENTS: usize = 2048;
const KERNEL_STACK_PAGES: usize = 16;
const MMAP_BASE: u64 = 0x0000_6000_0000_0000;
const MMAP_END: u64 = 0x0000_7000_0000_0000;
static COMPLETIONS: SpinLock<CompletionQueue> = SpinLock::new(CompletionQueue::new());

#[derive(Clone, Copy)]
#[repr(u64)]
pub enum NativeCall {
    HandleClose = 0,
    FileOpen = 1,
    FileRead = 2,
    FileWrite = 3,
    ProcessExit = 4,
    ProcessSpawn = 5,
    ThreadCreate = 6,
    HandleWait = 7,
    ChannelCreate = 8,
    ChannelSend = 9,
    ChannelReceive = 10,
    MemoryMap = 11,
    MemoryUnmap = 12,
    ConsoleRead = 13,
    ConsoleWrite = 14,
    DirectoryOpen = 15,
    DirectoryRead = 16,
    DirectoryChange = 17,
    DirectoryCurrent = 18,
    SystemQuery = 19,
    ConfigSet = 20,
    IoSubmit = 21,
    ConfigDelete = 22,
    ConfigWatch = 23,
    ConfigBegin = 24,
    ConfigTransactionSet = 25,
    ConfigTransactionDelete = 26,
    ConfigCommit = 27,
    EventReset = 28,
    ServiceLookup = 29,
    TtySetForeground = 30,
    ProcessId = 31,
    SchedulerTicks = 32,
    StatPath = 33,
    HandleStat = 34,
    HandleDuplicate = 35,
    ProcessUid = 36,
    ProcessGid = 37,
    FileSeek = 38,
    ProcessCpuTicks = 39,
    ProcessParentId = 41,
    PipeCreate = 42,
    PipeRead = 43,
    PipeWrite = 44,
    ProcessFork = 45,
    DirectoryCreate = 64,
    DirectoryRemove = 65,
    FileSync = 66,
    ThreadSleep = 67,
    DisplayInfo = 75,
    DisplayFillRect = 76,
    DisplayBlitRect = 77,
    MouseReadEvent = 78,
    TcpListen = 79,
    TcpAccept = 80,
    PosixSelect = 81,
}

struct UserWriter<'a> {
    buffer: &'a mut [u8],
    length: usize,
}
impl Write for UserWriter<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let n = s.len().min(self.buffer.len().saturating_sub(self.length));
        self.buffer[self.length..self.length + n].copy_from_slice(&s.as_bytes()[..n]);
        self.length += n;
        if n < s.len() {
            return Err(fmt::Error);
        }
        Ok(())
    }
}

fn copy_in<'a>(pointer: u64, length: usize, buffer: &'a mut [u8]) -> Result<&'a [u8], isize> {
    if length > buffer.len() {
        return Err(ERR_INVALID);
    }
    if length == 0 {
        return Ok(&buffer[..0]);
    }
    vmm::copy_from_user(&mut buffer[..length], pointer).map_err(|_| ERR_FAULT)?;
    Ok(&buffer[..length])
}
fn copy_string(pointer: u64, length: usize, buffer: &mut [u8]) -> Result<&str, isize> {
    let bytes = copy_in(pointer, length, buffer)?;
    core::str::from_utf8(bytes).map_err(|_| ERR_INVALID)
}
fn check_out(pointer: u64, length: usize) -> Result<(), isize> {
    if length > MAX_COPY {
        Err(ERR_INVALID)
    } else if !vmm::is_user_range(pointer, length, true) {
        Err(ERR_FAULT)
    } else {
        Ok(())
    }
}
fn current() -> Result<Arc<Process>, isize> {
    process::current().ok_or(ERR_DENIED)
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct UserStat {
    pub device: u64,
    pub inode: u64,
    pub links: u64,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub reserved: u32,
    pub special_device: u64,
    pub size: i64,
    pub block_size: i64,
    pub blocks: i64,
    pub access_seconds: i64,
    pub access_nanoseconds: i64,
    pub modify_seconds: i64,
    pub modify_nanoseconds: i64,
    pub change_seconds: i64,
    pub change_nanoseconds: i64,
}

impl UserStat {
    pub(crate) fn from_metadata(metadata: crate::fs::vnode::VNodeMetadata) -> Self {
        const REGULAR: u32 = 0o100000;
        const DIRECTORY: u32 = 0o040000;
        let (kind, permissions, links) = match metadata.kind {
            crate::fs::vnode::VNodeKind::File => (
                REGULAR,
                if metadata.executable { 0o755 } else { 0o644 },
                1,
            ),
            crate::fs::vnode::VNodeKind::Directory => (DIRECTORY, 0o755, 2),
        };
        Self {
            device: 1,
            inode: metadata.id,
            links,
            mode: kind | permissions,
            uid: metadata.uid,
            gid: metadata.gid,
            reserved: 0,
            special_device: 0,
            size: metadata.size as i64,
            block_size: pmm::PAGE_SIZE as i64,
            blocks: metadata.size.div_ceil(512) as i64,
            access_seconds: 0,
            access_nanoseconds: 0,
            modify_seconds: 0,
            modify_nanoseconds: 0,
            change_seconds: 0,
            change_nanoseconds: 0,
        }
    }
}

pub(crate) fn copy_user_stat(pointer: u64, record: UserStat) -> Result<isize, isize> {
    check_out(pointer, core::mem::size_of::<UserStat>())?;
    let bytes = unsafe {
        core::slice::from_raw_parts(
            (&record as *const UserStat).cast::<u8>(),
            core::mem::size_of::<UserStat>(),
        )
    };
    vmm::copy_to_user(pointer, bytes).map_err(|_| ERR_FAULT)?;
    Ok(0)
}

fn copy_stat_to_user(
    pointer: u64,
    metadata: crate::fs::vnode::VNodeMetadata,
) -> Result<isize, isize> {
    copy_user_stat(pointer, UserStat::from_metadata(metadata))
}

pub(crate) fn handle_metadata(raw: u32, process: &Arc<Process>) -> Result<crate::fs::vnode::VNodeMetadata, isize> {
    let object_type = process.handles.lock().object_type(raw).ok_or(ERR_NOT_FOUND)?;
    let node = match object_type {
        crate::object::ObjectType::File => {
            let handle = unsafe { Handle::<FileObject>::from_raw(raw) };
            process
                .handles
                .lock()
                .get(handle, AccessRights::NONE)
                .map_err(|_| ERR_DENIED)?
                .node
        }
        crate::object::ObjectType::Directory => {
            let handle = unsafe { Handle::<DirectoryObject>::from_raw(raw) };
            process
                .handles
                .lock()
                .get(handle, AccessRights::NONE)
                .map_err(|_| ERR_DENIED)?
                .node
        }
        _ => return Err(ERR_INVALID),
    };
    vfs::metadata_ref(node).ok_or(ERR_NOT_FOUND)
}

/// Wait at a kernel scheduling boundary. Timer interrupts can switch to another
/// ready thread while this one sleeps; zero polls once and `u64::MAX` waits forever.
pub(crate) fn wait_until(
    wait_queue: &crate::task::wait::WaitQueue,
    timeout_ticks: u64,
    mut ready: impl FnMut() -> Option<isize>,
) -> isize {
    if let Some(result) = ready() {
        return result;
    }
    if timeout_ticks == 0 {
        return ERR_WOULD_BLOCK;
    }
    let start = scheduler::tick_count();
    let Some(thread) = scheduler::current() else { return ERR_INVALID };
    let tid = thread.tid;
    loop {
        let elapsed = scheduler::tick_count().wrapping_sub(start);
        if timeout_ticks != u64::MAX && elapsed >= timeout_ticks {
            wait_queue.remove(tid);
            return ERR_TIMED_OUT;
        }
        wait_queue.register(tid);
        if let Some(result) = ready() {
            wait_queue.remove(tid);
            return result;
        }
        let remaining = if timeout_ticks == u64::MAX {
            u64::MAX
        } else {
            timeout_ticks - elapsed
        };
        if scheduler::block_current(remaining).is_err() {
            wait_queue.remove(tid);
            return ERR_INVALID;
        }
        crate::interrupt::enable();
        unsafe {
            asm!("hlt", options(nomem, nostack));
        }
        crate::interrupt::disable();
        if let Some(result) = ready() {
            wait_queue.remove(tid);
            return result;
        }
    }
}

fn spawn(path: &str, arguments: &[u8], parent: &Arc<Process>) -> Result<u32, isize> {
    if !parent
        .security
        .capabilities
        .contains(crate::security::Capabilities::PROCESS_CONTROL)
    {
        return Err(ERR_DENIED);
    }
    // The caller passes the argument vector as NUL separated strings. Without
    // one the program still gets argv[0] so it can find out its own name.
    let mut argv = alloc::vec::Vec::new();
    for argument in arguments.split(|byte| *byte == 0) {
        if argument.is_empty() {
            continue;
        }
        argv.push(core::str::from_utf8(argument).map_err(|_| ERR_INVALID)?);
    }
    if argv.is_empty() {
        argv.push(path);
    }
    let image = vfs::executable(path, &parent.cwd()).map_err(|_| ERR_NOT_FOUND)?;
    let root = vmm::clone_kernel_root().map_err(|_| ERR_NO_MEMORY)?;
    let security = if parent.uid == crate::user::ROOT_UID {
        SecurityContext::root()
    } else if path.ends_with("bash") {
        SecurityContext::interactive_shell()
    } else {
        SecurityContext::restricted()
    };
    let child = process::create(root, path, security, parent.pid);
    let loaded = elf::load_user(&image, &child.address_space, &argv, &[])
        .map_err(|_| ERR_INVALID)?;
    let stack_base = pmm::allocate_contiguous_pages(KERNEL_STACK_PAGES).ok_or(ERR_NO_MEMORY)?;
    let stack_top = stack_base + KERNEL_STACK_PAGES as u64 * pmm::PAGE_SIZE;
    let thread = thread::create(child.pid, stack_top, loaded.stack_pointer);
    let frame = TrapFrame {
        r15: 0,
        r14: 0,
        r13: 0,
        r12: 0,
        r11: 0,
        r10: 0,
        r9: 0,
        r8: 0,
        rdi: 0,
        rsi: 0,
        rbp: 0,
        rdx: 0,
        rcx: 0,
        rbx: 0,
        rax: 0,
        vector: 0,
        error: 0,
        rip: loaded.entry,
        cs: crate::arch::x86_64::gdt::USER_CODE as u64,
        rflags: 0x202,
        rsp: loaded.stack_pointer,
        ss: crate::arch::x86_64::gdt::USER_DATA as u64,
    };
    unsafe {
        thread.seed_frame(frame);
    }
    child.threads.lock().push(thread.clone());
    let parent_terminal = crate::drivers::tty::attributes();
    child.set_terminal_restore(parent_terminal);
    let _ = crate::drivers::tty::set_attributes(crate::drivers::tty::TerminalAttributes::canonical(), 0);
    crate::drivers::tty::set_foreground(child.pid);
    if scheduler::add(thread).is_err() {
        let _ = crate::drivers::tty::set_attributes(parent_terminal, 0);
        crate::drivers::tty::set_foreground(parent.pid);
        return Err(ERR_NO_MEMORY);
    }
    let handle = parent
        .handles
        .lock()
        .insert(
            child,
            AccessRights(
                AccessRights::WAIT.0 | AccessRights::CONTROL.0 | AccessRights::DUPLICATE.0,
            ),
        )
        .map_err(|_| ERR_NO_MEMORY)?;
    Ok(handle.raw())
}

fn create_user_thread(entry: u64, argument: u64, process: &Arc<Process>) -> Result<u32, isize> {
    if !vmm::is_user_executable(entry) {
        return Err(ERR_DENIED);
    }
    let tid = thread::new_id();
    let stack_top = vmm::USER_STACK_TOP - 0x20_0000 - (tid % 128) * 0x20_000;
    let mut top_frame = 0;
    for page in (stack_top - 2 * pmm::PAGE_SIZE..stack_top).step_by(pmm::PAGE_SIZE as usize) {
        let physical = pmm::allocate_frame().ok_or(ERR_NO_MEMORY)?;
        if page == stack_top - pmm::PAGE_SIZE {
            top_frame = physical;
        }
        unsafe {
            core::ptr::write_bytes(physical as *mut u8, 0, pmm::PAGE_SIZE as usize);
        }
        vmm::map_page_in_root(
            process.address_space.root(),
            page,
            physical,
            vmm::PageFlags::USER_DATA,
        )
        .map_err(|_| ERR_NO_MEMORY)?;
    }
    let user_stack = stack_top - 8;
    unsafe {
        ((top_frame + pmm::PAGE_SIZE - 8) as *mut u64).write(0);
    }
    let kernel_base = pmm::allocate_contiguous_pages(KERNEL_STACK_PAGES).ok_or(ERR_NO_MEMORY)?;
    let kernel_top = kernel_base + KERNEL_STACK_PAGES as u64 * pmm::PAGE_SIZE;
    let thread = Arc::new(Thread::new(tid, process.pid, kernel_top, user_stack));
    unsafe {
        thread.seed_frame(TrapFrame {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            r11: 0,
            r10: 0,
            r9: 0,
            r8: 0,
            rdi: argument,
            rsi: 0,
            rbp: 0,
            rdx: 0,
            rcx: 0,
            rbx: 0,
            rax: 0,
            vector: 0,
            error: 0,
            rip: entry,
            cs: crate::arch::x86_64::gdt::USER_CODE as u64,
            rflags: 0x202,
            rsp: user_stack,
            ss: crate::arch::x86_64::gdt::USER_DATA as u64,
        });
    }
    process.threads.lock().push(thread.clone());
    scheduler::add(thread.clone()).map_err(|_| ERR_NO_MEMORY)?;
    process
        .handles
        .lock()
        .insert(
            thread,
            AccessRights(
                AccessRights::WAIT.0 | AccessRights::CONTROL.0 | AccessRights::DUPLICATE.0,
            ),
        )
        .map(|h| h.raw())
        .map_err(|_| ERR_NO_MEMORY)
}

fn query(kind: u64, input: &[u8], output: &mut [u8]) -> Result<usize, isize> {
    let mut writer = UserWriter {
        buffer: output,
        length: 0,
    };
    match kind {
        1 => {
            if !current()?
                .security
                .capabilities
                .contains(crate::security::Capabilities::PROCESS_CONTROL)
            {
                return Err(ERR_DENIED);
            }
            for p in process::all() {
                let state = if p.has_exited() { "exited" } else { "running" };
                let _ = writeln!(writer, "pid={} {} {}", p.pid, state, p.executable());
            }
        }
        2 => {
            if !current()?
                .security
                .capabilities
                .contains(crate::security::Capabilities::DEVICE_QUERY)
            {
                return Err(ERR_DENIED);
            }
            for d in device::list() {
                let _ = writeln!(writer, "{:?} {} [{}]", d.class, d.model, d.driver);
            }
        }
        3 => {
            for v in crate::volume::list() {
                let _ = writeln!(writer, "{} ({}) {}", v.name, v.filesystem, v.mount_point);
            }
            for (path, volume) in vfs::mount_list() {
                let _ = writeln!(writer, "mount {} -> {}", path, volume);
            }
        }
        4 => {
            if !current()?
                .security
                .capabilities
                .contains(crate::security::Capabilities::DEVICE_QUERY)
            {
                return Err(ERR_DENIED);
            }
            for s in crate::service::list() {
                let _ = writeln!(writer, "{} {:?}", s.name, s.state);
            }
        }
        5 => {
            if !current()?
                .security
                .capabilities
                .contains(crate::security::Capabilities::CONFIG_READ)
            {
                return Err(ERR_DENIED);
            }
            let key = core::str::from_utf8(input).map_err(|_| ERR_INVALID)?;
            if let Some(value) = config::get(key) {
                let _ = writeln!(writer, "{}", value);
            } else {
                return Err(ERR_NOT_FOUND);
            }
        }
        _ => return Err(ERR_INVALID),
    }
    Ok(writer.length)
}

fn handle_call(frame: &mut TrapFrame) -> Option<isize> {
    let number = frame.rax;
    let a0 = frame.rdi;
    let a1 = frame.rsi;
    let a2 = frame.rdx;
    let a3 = frame.r10;
    let a4 = frame.r8;
    let a5 = frame.r9;
    let result: Result<isize, isize> = (|| {
        Ok(match number {
            0 => {
                let process = current()?;
                process
                    .handles
                    .lock()
                    .close(a0 as u32)
                    .map(|_| 0)
                    .map_err(|_| ERR_NOT_FOUND)?;
                0
            }
            1 => {
                let process = current()?;
                if a1 == 0 || a1 as usize > 512 || a2 == 0 || a2 & !3 != 0 {
                    return Err(ERR_INVALID);
                }
                if !process
                    .security
                    .capabilities
                    .contains(crate::security::Capabilities::FILE_READ)
                {
                    return Err(ERR_DENIED);
                }
                let mut path = [0u8; 512];
                let path = copy_string(a0, a1 as usize, &mut path)?;
                let cwd = process.cwd();
                let (node, filesystem) = vfs::lookup(path, &cwd).map_err(|_| ERR_NOT_FOUND)?;
                if filesystem.kind(node.node_id) != Some(crate::fs::vnode::VNodeKind::File) {
                    return Err(ERR_INVALID);
                }
                let readable = a2 & 1 != 0;
                let writable = a2 & 2 != 0;
                if writable
                    && !process
                        .security
                        .capabilities
                        .contains(crate::security::Capabilities::FILE_WRITE)
                {
                    return Err(ERR_DENIED);
                }
                let access = (if readable { crate::fs::vnode::Acl::READ } else { 0 })
                    | (if writable { crate::fs::vnode::Acl::WRITE } else { 0 });
                if process.uid != 0 && !filesystem.can_access(node.node_id, process.uid, process.gid, access) {
                    return Err(ERR_DENIED);
                }
                let file = Arc::new(FileObject::new(node, readable, writable));
                let rights = AccessRights(
                    AccessRights::DUPLICATE.0
                        | AccessRights::TRANSFER.0
                        | (if readable { AccessRights::READ.0 } else { 0 })
                        | (if writable { AccessRights::WRITE.0 } else { 0 }),
                );
                let handle = process
                    .handles
                    .lock()
                    .insert(file, rights)
                    .map(|h| h.raw() as isize)
                    .map_err(|_| ERR_NO_MEMORY)?;
                handle
            }
            2 => {
                let process = current()?;
                let len = (a2 as usize).min(MAX_COPY);
                if a2 as usize > MAX_COPY {
                    return Err(ERR_INVALID);
                }
                check_out(a1, len)?;
                let handle = unsafe { Handle::<FileObject>::from_raw(a0 as u32) };
                let file = process
                    .handles
                    .lock()
                    .get(handle, AccessRights::READ)
                    .map_err(|_| ERR_DENIED)?;
                let mut bytes = [0u8; MAX_COPY];
                let count = file.read(&mut bytes[..len]).map_err(|_| ERR_INVALID)?;
                vmm::copy_to_user(a1, &bytes[..count as usize]).map_err(|_| ERR_INVALID)?;
                count as isize
            }
            3 => {
                let process = current()?;
                let len = a2 as usize;
                if len > MAX_COPY {
                    return Err(ERR_INVALID);
                }
                let mut bytes = [0u8; MAX_COPY];
                copy_in(a1, len, &mut bytes)?;
                let handle = unsafe { Handle::<FileObject>::from_raw(a0 as u32) };
                let file = process
                    .handles
                    .lock()
                    .get(handle, AccessRights::WRITE)
                    .map_err(|_| ERR_DENIED)?;
                file.write(&bytes[..len]).map_err(|_| ERR_INVALID)? as isize
            }
            4 => {
                let process = current()?;
                let status = a0 as u32 as i32;
                process.terminate(status);
                0
            }
            5 => {
                let process = current()?;
                let mut path = [0u8; 512];
                let path = copy_string(a0, a1 as usize, &mut path)?;
                let mut argument_storage = [0u8; MAX_ARGUMENTS];
                let arguments = if a3 == 0 {
                    &argument_storage[..0]
                } else {
                    copy_in(a2, a3 as usize, &mut argument_storage)?
                };
                spawn(path, arguments, &process)? as isize
            }
            6 => {
                let process = current()?;
                create_user_thread(a0, a1, &process)? as isize
            }
            7 => {
                let process = current()?;
                let raw = a0 as u32;
                let object_type = { process.handles.lock().object_type(raw) };
                let value = match object_type {
                    Some(crate::object::ObjectType::Process) => {
                        let h = unsafe { Handle::<Process>::from_raw(raw) };
                        let target = process
                            .handles
                            .lock()
                            .get(h, AccessRights::WAIT)
                            .map_err(|_| ERR_DENIED)?;
                        wait_until(target.exit_wait_queue(), a1, || {
                            target.has_exited().then(|| {
                                target
                                    .exit_status
                                    .load(core::sync::atomic::Ordering::Acquire)
                                    as isize
                            })
                        })
                    }
                    Some(crate::object::ObjectType::Event) => {
                        let h = unsafe { Handle::<EventObject>::from_raw(raw) };
                        let event = process
                            .handles
                            .lock()
                            .get(h, AccessRights::WAIT)
                            .map_err(|_| ERR_DENIED)?;
                        wait_until(event.wait_queue(), a1, || event.is_signaled().then_some(0))
                    }
                    _ => return Err(ERR_INVALID),
                };
                value
            }
            8 => {
                let process = current()?;
                if !vmm::is_user_range(a0, 8, true) {
                    return Err(ERR_INVALID);
                }
                let (left, right) = crate::ipc::ChannelEndpoint::pair();
                let l = process
                    .handles
                    .lock()
                    .insert(
                        left,
                        AccessRights(
                            AccessRights::READ.0
                                | AccessRights::WRITE.0
                                | AccessRights::WAIT.0
                                | AccessRights::DUPLICATE.0
                                | AccessRights::TRANSFER.0,
                        ),
                    )
                    .map_err(|_| ERR_NO_MEMORY)?;
                let r = process
                    .handles
                    .lock()
                    .insert(
                        right,
                        AccessRights(
                            AccessRights::READ.0
                                | AccessRights::WRITE.0
                                | AccessRights::WAIT.0
                                | AccessRights::DUPLICATE.0
                                | AccessRights::TRANSFER.0,
                        ),
                    )
                    .map_err(|_| ERR_NO_MEMORY)?;
                let values = [l.raw(), r.raw()];
                vmm::copy_to_user(a0, unsafe {
                    core::slice::from_raw_parts(values.as_ptr().cast::<u8>(), 8)
                })
                .map_err(|_| ERR_INVALID)?;
                0
            }
            9 => {
                let process = current()?;
                let len = a2 as usize;
                let transfer_count = a4 as usize;
                if len > 256 || transfer_count > 8 {
                    return Err(ERR_INVALID);
                }
                let mut bytes = [0u8; 256];
                copy_in(a1, len, &mut bytes)?;
                let mut raw_handles = [0u32; 8];
                if transfer_count != 0 {
                    let handle_bytes = transfer_count * core::mem::size_of::<u32>();
                    copy_in(a3, handle_bytes, unsafe {
                        core::slice::from_raw_parts_mut(raw_handles.as_mut_ptr().cast::<u8>(), handle_bytes)
                    })?;
                }
                let handles = process.handles.lock();
                let mut transfers = Vec::new();
                transfers.try_reserve_exact(transfer_count).map_err(|_| ERR_NO_MEMORY)?;
                for raw in raw_handles.iter().take(transfer_count) {
                    transfers.push(handles.export_transfer(*raw).map_err(|_| ERR_DENIED)?);
                }
                drop(handles);
                let h = unsafe { Handle::<crate::ipc::ChannelEndpoint>::from_raw(a0 as u32) };
                let channel = process
                    .handles
                    .lock()
                    .get(h, AccessRights::WRITE)
                    .map_err(|_| ERR_DENIED)?;
                wait_until(channel.writable_wait_queue(), u64::MAX, || {
                    match channel.send_with_transfers(&bytes[..len], &transfers) {
                        Ok(()) => Some(len as isize),
                        Err("channel queue is full") => None,
                        Err(_) => Some(ERR_INVALID),
                    }
                })
            }
            10 => {
                let process = current()?;
                let len = (a2 as usize).min(256);
                let handle_capacity = (a5 as usize).min(8);
                if a5 as usize > 8 { return Err(ERR_INVALID); }
                check_out(a1, len)?;
                check_out(a3, 8)?;
                if handle_capacity != 0 {
                    check_out(a4, handle_capacity * core::mem::size_of::<u32>())?;
                }
                let h = unsafe { Handle::<crate::ipc::ChannelEndpoint>::from_raw(a0 as u32) };
                let channel = process
                    .handles
                    .lock()
                    .get(h, AccessRights::READ)
                    .map_err(|_| ERR_DENIED)?;
                let mut bytes = [0u8; 256];
                let mut received_handles = [0u32; 8];
                let mut received_handle_count = 0usize;
                let count = wait_until(channel.readable_wait_queue(), u64::MAX, || {
                    let mut table = process.handles.lock();
                    match channel.receive_with_transfers(
                        &mut bytes[..len],
                        &mut table,
                        &mut received_handles[..handle_capacity],
                    ) {
                        Ok((count, handle_count)) => {
                            received_handle_count = handle_count;
                            Some(count as isize)
                        }
                        Err("channel has no message") => None,
                        Err(_) => Some(ERR_INVALID),
                    }
                });
                if count < 0 { return Err(count); }
                if vmm::copy_to_user(a1, &bytes[..count as usize]).is_err()
                    || (received_handle_count != 0 && vmm::copy_to_user(a4, unsafe {
                        core::slice::from_raw_parts(
                            received_handles.as_ptr().cast::<u8>(),
                            received_handle_count * core::mem::size_of::<u32>(),
                        )
                    }).is_err())
                {
                    let mut handles = process.handles.lock();
                    for raw in received_handles.iter().take(received_handle_count) {
                        let _ = handles.close(*raw);
                    }
                    return Err(ERR_INVALID);
                }
                vmm::copy_to_user(a3, &(received_handle_count as u64).to_ne_bytes())
                    .map_err(|_| ERR_INVALID)?;
                count
            }
            11 => {
                let process = current()?;
                let length = a1 as usize;
                if length == 0 || length > 16 * 1024 * 1024 || a2 & !7 != 0 || a2 & 1 == 0 {
                    return Err(ERR_INVALID);
                }
                if a2 & 2 != 0 && a2 & 4 != 0 {
                    return Err(ERR_DENIED);
                }
                let pages = (length as u64 + pmm::PAGE_SIZE - 1) / pmm::PAGE_SIZE;
                let rounded = pages * pmm::PAGE_SIZE;
                let start = process
                    .address_space
                    .mapping_start(a0, rounded, MMAP_BASE, MMAP_END)
                    .map_err(|_| ERR_INVALID)?;
                let flags = vmm::PageFlags {
                    user: true,
                    writable: a2 & 2 != 0,
                    executable: a2 & 4 != 0,
                    cache_disable: false,
                };
                let mut mapped = 0u64;
                for offset in (0..rounded).step_by(pmm::PAGE_SIZE as usize) {
                    let Some(physical) = pmm::allocate_frame() else {
                        rollback(process.address_space.root(), start, mapped);
                        return Err(ERR_NO_MEMORY);
                    };
                    unsafe {
                        core::ptr::write_bytes(physical as *mut u8, 0, pmm::PAGE_SIZE as usize);
                    }
                    if vmm::map_page_in_root(
                        process.address_space.root(),
                        start + offset,
                        physical,
                        flags,
                    )
                    .is_err()
                    {
                        let _ = pmm::free_frame(physical);
                        rollback(process.address_space.root(), start, mapped);
                        return Err(ERR_INVALID);
                    }
                    mapped += pmm::PAGE_SIZE;
                }
                process.address_space.record(start, mapped, a2 as u8);
                start as isize
            }
            12 => {
                let process = current()?;
                let length = a1 as usize;
                if length == 0 || length > 16 * 1024 * 1024 || a0 % pmm::PAGE_SIZE != 0 {
                    return Err(ERR_INVALID);
                }
                let pages = (length as u64 + pmm::PAGE_SIZE - 1) / pmm::PAGE_SIZE;
                let rounded = pages * pmm::PAGE_SIZE;
                for offset in (0..rounded).step_by(pmm::PAGE_SIZE as usize) {
                    let physical = vmm::unmap_page_in_root(process.address_space.root(), a0 + offset)
                        .map_err(|_| ERR_INVALID)?;
                    let _ = pmm::free_frame(physical);
                }
                process.address_space.forget(a0, rounded);
                0
            }
            13 => {
                let len = (a1 as usize).min(MAX_COPY);
                if a1 as usize > MAX_COPY {
                    return Err(ERR_INVALID);
                }
                if len == 0 { return Ok(0); }
                check_out(a0, len)?;
                let process = current()?;
                let mut bytes = [0u8; MAX_COPY];
                let count = crate::drivers::tty::read(process.pid, &mut bytes[..len]);
                vmm::copy_to_user(a0, &bytes[..count]).map_err(|_| ERR_INVALID)?;
                count as isize
            }
            14 => {
                let len = a1 as usize;
                if len > MAX_COPY {
                    return Err(ERR_INVALID);
                }
                let mut bytes = [0u8; MAX_COPY];
                copy_in(a0, len, &mut bytes)?;
                crate::drivers::console::write(&bytes[..len]);
                len as isize
            }
            15 => {
                let process = current()?;
                let mut path = [0u8; 512];
                let path = copy_string(a0, a1 as usize, &mut path)?;
                let cwd = process.cwd();
                let (node, filesystem) = vfs::lookup(path, &cwd).map_err(|_| ERR_NOT_FOUND)?;
                if filesystem.kind(node.node_id) != Some(crate::fs::vnode::VNodeKind::Directory) {
                    return Err(ERR_INVALID);
                }
                let directory = Arc::new(DirectoryObject::new(node));
                let handle = process
                    .handles
                    .lock()
                    .insert(directory, AccessRights(AccessRights::FILE_READ.0 | AccessRights::TRANSFER.0))
                    .map(|h| h.raw() as isize)
                    .map_err(|_| ERR_NO_MEMORY)?;
                handle
            }
            16 => {
                let process = current()?;
                let len = (a2 as usize).min(MAX_COPY);
                if a2 as usize > MAX_COPY {
                    return Err(ERR_INVALID);
                }
                check_out(a1, len)?;
                let h = unsafe { Handle::<DirectoryObject>::from_raw(a0 as u32) };
                let dir = process
                    .handles
                    .lock()
                    .get(h, AccessRights::READ)
                    .map_err(|_| ERR_DENIED)?;
                let mut bytes = [0u8; MAX_COPY];
                let count = vfs::read_directory(dir.node, &mut bytes[..len])
                    .map_err(|_| ERR_INVALID)?;
                vmm::copy_to_user(a1, &bytes[..count]).map_err(|_| ERR_INVALID)?;
                count as isize
            }
            17 => {
                let process = current()?;
                let mut path = [0u8; 512];
                let path = copy_string(a0, a1 as usize, &mut path)?;
                let cwd = process.cwd();
                let target = vfs::change_directory(&cwd, path).map_err(|_| ERR_NOT_FOUND)?;
                process.chdir(target);
                0
            }
            18 => {
                let len = (a1 as usize).min(512);
                check_out(a0, len)?;
                let process = current()?;
                let cwd = process.cwd();
                let bytes = cwd.as_bytes();
                let count = bytes.len().min(len);
                vmm::copy_to_user(a0, &bytes[..count]).map_err(|_| ERR_INVALID)?;
                count as isize
            }
            19 => {
                let kind = a0;
                let input_len = a2 as usize;
                if input_len > 256 {
                    return Err(ERR_INVALID);
                }
                let mut input = [0u8; 256];
                let input = copy_in(a1, input_len, &mut input)?;
                let cap = (a4 as usize).min(MAX_COPY);
                if a4 as usize > MAX_COPY {
                    return Err(ERR_INVALID);
                }
                check_out(a3, cap)?;
                let mut output = [0u8; MAX_COPY];
                let count = query(kind, input, &mut output[..cap])?;
                vmm::copy_to_user(a3, &output[..count]).map_err(|_| ERR_INVALID)?;
                count as isize
            }
            20 => {
                let process = current()?;
                if !process
                    .security
                    .capabilities
                    .contains(crate::security::Capabilities::CONFIG_WRITE)
                {
                    return Err(ERR_DENIED);
                }
                let mut key = [0u8; 256];
                let key = copy_string(a0, a1 as usize, &mut key)?;
                let mut value = [0u8; 1024];
                let value = copy_string(a2, a3 as usize, &mut value)?;
                config::set(key, value)
                    .map(|_| 0)
                    .map_err(|_| ERR_INVALID)?
            }
            21 => io_submit(a0)?,
            22 => {
                let process = current()?;
                if !process
                    .security
                    .capabilities
                    .contains(crate::security::Capabilities::CONFIG_WRITE)
                {
                    return Err(ERR_DENIED);
                }
                let mut key = [0u8; 256];
                let key = copy_string(a0, a1 as usize, &mut key)?;
                config::delete(key)
                    .map(|generation| generation as isize)
                    .map_err(|_| ERR_INVALID)?
            }
            23 => {
                let process = current()?;
                if !process
                    .security
                    .capabilities
                    .contains(crate::security::Capabilities::CONFIG_READ)
                {
                    return Err(ERR_DENIED);
                }
                let mut prefix = [0u8; 256];
                let prefix = copy_string(a0, a1 as usize, &mut prefix)?;
                let event = Arc::new(EventObject::new());
                config::watch(prefix, &event);
                let raw = process
                    .handles
                    .lock()
                    .insert(
                        event,
                        AccessRights(
                            AccessRights::WAIT.0
                                | AccessRights::SIGNAL.0
                                | AccessRights::DUPLICATE.0
                                | AccessRights::TRANSFER.0,
                        ),
                    )
                    .map(|handle| handle.raw() as isize)
                    .map_err(|_| ERR_NO_MEMORY)?;
                raw
            }
            24 => {
                let process = current()?;
                if !process
                    .security
                    .capabilities
                    .contains(crate::security::Capabilities::CONFIG_WRITE)
                {
                    return Err(ERR_DENIED);
                }
                let transaction = Arc::new(config::ConfigTransactionObject::new());
                let raw = process
                    .handles
                    .lock()
                    .insert(
                        transaction,
                        AccessRights(AccessRights::CONTROL.0 | AccessRights::DUPLICATE.0),
                    )
                    .map(|handle| handle.raw() as isize)
                    .map_err(|_| ERR_NO_MEMORY)?;
                raw
            }
            25 => {
                let process = current()?;
                if !process
                    .security
                    .capabilities
                    .contains(crate::security::Capabilities::CONFIG_WRITE)
                {
                    return Err(ERR_DENIED);
                }
                let mut key = [0u8; 256];
                let key = copy_string(a1, a2 as usize, &mut key)?;
                let mut value = [0u8; 1024];
                let value = copy_string(a3, a4 as usize, &mut value)?;
                let handle =
                    unsafe { Handle::<config::ConfigTransactionObject>::from_raw(a0 as u32) };
                let transaction = process
                    .handles
                    .lock()
                    .get(handle, AccessRights::CONTROL)
                    .map_err(|_| ERR_DENIED)?;
                transaction
                    .set(key, value)
                    .map(|_| 0)
                    .map_err(|_| ERR_INVALID)?
            }
            26 => {
                let process = current()?;
                if !process
                    .security
                    .capabilities
                    .contains(crate::security::Capabilities::CONFIG_WRITE)
                {
                    return Err(ERR_DENIED);
                }
                let mut key = [0u8; 256];
                let key = copy_string(a1, a2 as usize, &mut key)?;
                let handle =
                    unsafe { Handle::<config::ConfigTransactionObject>::from_raw(a0 as u32) };
                let transaction = process
                    .handles
                    .lock()
                    .get(handle, AccessRights::CONTROL)
                    .map_err(|_| ERR_DENIED)?;
                transaction
                    .delete(key)
                    .map(|_| 0)
                    .map_err(|_| ERR_INVALID)?
            }
            27 => {
                let process = current()?;
                if !process
                    .security
                    .capabilities
                    .contains(crate::security::Capabilities::CONFIG_WRITE)
                {
                    return Err(ERR_DENIED);
                }
                let handle =
                    unsafe { Handle::<config::ConfigTransactionObject>::from_raw(a0 as u32) };
                let transaction = process
                    .handles
                    .lock()
                    .get(handle, AccessRights::CONTROL)
                    .map_err(|_| ERR_DENIED)?;
                transaction
                    .commit()
                    .map(|generation| generation as isize)
                    .map_err(|_| ERR_INVALID)?
            }
            28 => {
                let process = current()?;
                let handle = unsafe { Handle::<EventObject>::from_raw(a0 as u32) };
                process
                    .handles
                    .lock()
                    .get(handle, AccessRights::SIGNAL)
                    .map_err(|_| ERR_DENIED)?
                    .reset();
                0
            }
            29 => {
                let process = current()?;
                if !process
                    .security
                    .capabilities
                    .contains(crate::security::Capabilities::DEVICE_QUERY)
                {
                    return Err(ERR_DENIED);
                }
                let mut name = [0u8; 128];
                let name = copy_string(a0, a1 as usize, &mut name)?;
                let service = crate::service::lookup(name).ok_or(ERR_NOT_FOUND)?;
                let raw = process
                    .handles
                    .lock()
                    .insert(
                        service,
                        AccessRights(AccessRights::CONTROL.0 | AccessRights::WAIT.0),
                    )
                    .map(|handle| handle.raw() as isize)
                    .map_err(|_| ERR_NO_MEMORY)?;
                raw
            }
            30 => {
                let process = current()?;
                if !process
                    .security
                    .capabilities
                    .contains(crate::security::Capabilities::PROCESS_CONTROL)
                {
                    return Err(ERR_DENIED);
                }
                crate::drivers::tty::set_foreground(process.pid);
                0
            }
            31 => current()?.pid as isize,
            32 => scheduler::tick_count() as isize,
            33 => {
                let process = current()?;
                if a1 == 0 || a1 as usize > 512 {
                    return Err(ERR_INVALID);
                }
                let mut path_buffer = [0u8; 512];
                let path = copy_string(a0, a1 as usize, &mut path_buffer)?;
                let metadata = vfs::metadata(path, &process.cwd()).map_err(|_| ERR_NOT_FOUND)?;
                copy_stat_to_user(a2, metadata)?
            }
            34 => {
                let process = current()?;
                let metadata = handle_metadata(a0 as u32, &process)?;
                copy_stat_to_user(a1, metadata)?
            }
            35 => {
                let process = current()?;
                if a1 > u16::MAX as u64 {
                    return Err(ERR_INVALID);
                }
                let rights = AccessRights(a1 as u16);
                let raw = a0 as u32;
                let object_type = { process.handles.lock().object_type(raw) };
                match object_type {
                    Some(crate::object::ObjectType::File) => {
                        let handle = unsafe { Handle::<FileObject>::from_raw(raw) };
                        process
                            .handles
                            .lock()
                            .duplicate(handle, rights)
                            .map(|duplicate| duplicate.raw() as isize)
                            .map_err(|_| ERR_DENIED)?
                    }
                    Some(crate::object::ObjectType::Directory) => {
                        let handle = unsafe { Handle::<DirectoryObject>::from_raw(raw) };
                        process
                            .handles
                            .lock()
                            .duplicate(handle, rights)
                            .map(|duplicate| duplicate.raw() as isize)
                            .map_err(|_| ERR_DENIED)?
                    }
                    Some(crate::object::ObjectType::PipeReader) => {
                        let handle = unsafe { Handle::<crate::ipc::pipe::PipeReader>::from_raw(raw) };
                        process.handles.lock().duplicate(handle, rights)
                            .map(|duplicate| duplicate.raw() as isize)
                            .map_err(|_| ERR_DENIED)?
                    }
                    Some(crate::object::ObjectType::PipeWriter) => {
                        let handle = unsafe { Handle::<crate::ipc::pipe::PipeWriter>::from_raw(raw) };
                        process.handles.lock().duplicate(handle, rights)
                            .map(|duplicate| duplicate.raw() as isize)
                            .map_err(|_| ERR_DENIED)?
                    }
                    _ => return Err(ERR_INVALID),
                }
            }
            36 => current()?.uid as isize,
            37 => current()?.gid as isize,
            38 => {
                let process = current()?;
                let handle = unsafe { Handle::<FileObject>::from_raw(a0 as u32) };
                let file = process
                    .handles
                    .lock()
                    .get(handle, AccessRights::NONE)
                    .map_err(|_| ERR_DENIED)?;
                file.seek(a1 as i64, a2).map(|position| position as isize).map_err(|_| ERR_INVALID)?
            }
            39 => current()?.user_ticks() as isize,
            41 => current()?.parent_pid as isize,
            42 => {
                let process = current()?;
                check_out(a0, 8)?;
                let (reader, writer) = crate::ipc::pipe::PipeReader::pair();
                let reader_handle = process.handles.lock()
                    .insert(reader, AccessRights(AccessRights::READ.0 | AccessRights::DUPLICATE.0 | AccessRights::TRANSFER.0))
                    .map_err(|_| ERR_NO_MEMORY)?;
                let writer_handle = match process.handles.lock()
                    .insert(writer, AccessRights(AccessRights::WRITE.0 | AccessRights::DUPLICATE.0 | AccessRights::TRANSFER.0))
                {
                    Ok(handle) => handle,
                    Err(_) => {
                        let _ = process.handles.lock().close(reader_handle.raw());
                        return Err(ERR_NO_MEMORY);
                    }
                };
                let pair = [reader_handle.raw(), writer_handle.raw()];
                let bytes = unsafe {
                    core::slice::from_raw_parts(pair.as_ptr().cast::<u8>(), core::mem::size_of_val(&pair))
                };
                if vmm::copy_to_user(a0, bytes).is_err() {
                    let mut handles = process.handles.lock();
                    let _ = handles.close(reader_handle.raw());
                    let _ = handles.close(writer_handle.raw());
                    return Err(ERR_FAULT);
                }
                0
            }
            43 => {
                let process = current()?;
                let length = a2 as usize;
                if length > MAX_COPY { return Err(ERR_INVALID); }
                check_out(a1, length)?;
                let handle = unsafe { Handle::<crate::ipc::pipe::PipeReader>::from_raw(a0 as u32) };
                let reader = process.handles.lock().get(handle, AccessRights::READ)
                    .map_err(|_| ERR_DENIED)?;
                let mut bytes = [0u8; MAX_COPY];
                let count = wait_until(reader.wait_queue(), u64::MAX, || {
                    reader.try_read(&mut bytes[..length]).map(|count| count as isize)
                });
                if count < 0 { return Err(count); }
                vmm::copy_to_user(a1, &bytes[..count as usize]).map_err(|_| ERR_FAULT)?;
                count
            }
            44 => {
                let process = current()?;
                let length = a2 as usize;
                if length > MAX_COPY { return Err(ERR_INVALID); }
                let mut bytes = [0u8; MAX_COPY];
                let input = copy_in(a1, length, &mut bytes)?;
                let handle = unsafe { Handle::<crate::ipc::pipe::PipeWriter>::from_raw(a0 as u32) };
                let writer = process.handles.lock().get(handle, AccessRights::WRITE)
                    .map_err(|_| ERR_DENIED)?;
                wait_until(writer.wait_queue(), u64::MAX, || match writer.try_write(input) {
                    Ok(Some(count)) => Some(count as isize),
                    Ok(None) => None,
                    Err(()) => Some(ERR_BROKEN_PIPE),
                })
            }
            45 => {
                let parent = current()?;
                if !scheduler::has_capacity() {
                    return Err(ERR_NO_MEMORY);
                }
                let kernel_base = pmm::allocate_contiguous_pages(KERNEL_STACK_PAGES).ok_or(ERR_NO_MEMORY)?;
                let root = match vmm::clone_address_space(parent.address_space.root()) {
                    Ok(root) => root,
                    Err(_) => {
                        for page in 0..KERNEL_STACK_PAGES {
                            let _ = pmm::free_frame(kernel_base + page as u64 * pmm::PAGE_SIZE);
                        }
                        return Err(ERR_NO_MEMORY);
                    }
                };
                let child = process::fork_process(&parent, root);
                let thread = thread::create(
                    child.pid,
                    kernel_base + KERNEL_STACK_PAGES as u64 * pmm::PAGE_SIZE,
                    frame.rsp,
                );
                let mut child_frame = *frame;
                child_frame.rax = 0;
                unsafe { thread.seed_frame(child_frame); }
                child.threads.lock().push(thread.clone());
                if scheduler::add(thread).is_err() {
                    child.terminate(127);
                    return Err(ERR_NO_MEMORY);
                }
                child.pid as isize
            }
            64 => {
                let process = current()?;
                if !process.security.capabilities.contains(crate::security::Capabilities::FILE_WRITE) {
                    return Err(ERR_DENIED);
                }
                let mut storage = [0u8; 512];
                let path = copy_string(a0, a1 as usize, &mut storage)?;
                let cwd = process.cwd();
                let (parent, filesystem) = vfs::parent_node(path, &cwd).map_err(|_| ERR_NOT_FOUND)?;
                if process.uid != 0
                    && !filesystem.can_access(
                        parent.node_id,
                        process.uid,
                        process.gid,
                        crate::fs::vnode::Acl::WRITE | crate::fs::vnode::Acl::EXECUTE,
                    )
                {
                    return Err(ERR_DENIED);
                }
                vfs::create_directory(path, &cwd).map_err(|_| ERR_INVALID)?;
                0
            }
            65 => {
                let process = current()?;
                if !process.security.capabilities.contains(crate::security::Capabilities::FILE_WRITE) {
                    return Err(ERR_DENIED);
                }
                let mut storage = [0u8; 512];
                let path = copy_string(a0, a1 as usize, &mut storage)?;
                let cwd = process.cwd();
                let (node, filesystem) = vfs::lookup(path, &cwd).map_err(|_| ERR_NOT_FOUND)?;
                if filesystem.kind(node.node_id) != Some(crate::fs::vnode::VNodeKind::Directory) {
                    return Err(ERR_INVALID);
                }
                if node.node_id == filesystem.resolve("/", "/").map_err(|_| ERR_INVALID)? {
                    return Err(ERR_DENIED);
                }
                let (parent, parent_fs) = vfs::parent_node(path, &cwd).map_err(|_| ERR_NOT_FOUND)?;
                if process.uid != 0
                    && (node.mount_id != parent.mount_id
                        || !parent_fs.can_access(
                            parent.node_id,
                            process.uid,
                            process.gid,
                            crate::fs::vnode::Acl::WRITE | crate::fs::vnode::Acl::EXECUTE,
                        ))
                {
                    return Err(ERR_DENIED);
                }
                vfs::remove(path, &cwd).map_err(|_| ERR_INVALID)?;
                0
            }
            66 => crate::posix::file_sync(a0 as usize)?,
            67 => {
                if a0 > i64::MAX as u64 || a1 >= 1_000_000_000 { return Err(-22); }
                let duration = (a0 as u128) * 100
                    + ((a1 as u128 + 9_999_999) / 10_000_000);
                if duration > i64::MAX as u128 { return Err(-75); }
                crate::posix::thread_sleep(duration as u64)?
            }
            75 => {
                let process = current()?;
                if !process.security.capabilities.contains(crate::security::Capabilities::DISPLAY) {
                    return Err(ERR_DENIED);
                }
                #[repr(C)]
                struct DisplayInfo { width: u32, height: u32, pixel_format: u32, reserved: u32 }
                let (width, height, pixel_format) = crate::drivers::framebuffer::surface_info()
                    .ok_or(ERR_UNSUPPORTED)?;
                let info = DisplayInfo { width, height, pixel_format, reserved: 0 };
                let bytes = unsafe {
                    core::slice::from_raw_parts((&info as *const DisplayInfo).cast::<u8>(), core::mem::size_of::<DisplayInfo>())
                };
                check_out(a0, bytes.len())?;
                vmm::copy_to_user(a0, bytes).map_err(|_| ERR_FAULT)?;
                bytes.len() as isize
            }
            76 => {
                let process = current()?;
                if !process.security.capabilities.contains(crate::security::Capabilities::DISPLAY) {
                    return Err(ERR_DENIED);
                }
                if !crate::drivers::framebuffer::fill_rect(a0 as u32, a1 as u32, a2 as u32, a3 as u32, a4 as u32) {
                    return Err(ERR_UNSUPPORTED);
                }
                0
            }
            77 => {
                let process = current()?;
                if !process.security.capabilities.contains(crate::security::Capabilities::DISPLAY) {
                    return Err(ERR_DENIED);
                }
                let width = a2 as usize;
                let height = a3 as usize;
                let stride = a5 as usize;
                let count = stride.checked_mul(height).ok_or(ERR_INVALID)?;
                if stride < width || count > MAX_COPY / core::mem::size_of::<u32>() {
                    return Err(ERR_INVALID);
                }
                let mut pixels = [0u32; MAX_COPY / core::mem::size_of::<u32>()];
                let byte_count = count * core::mem::size_of::<u32>();
                if byte_count != 0 {
                    copy_in(a4, byte_count, unsafe {
                        core::slice::from_raw_parts_mut(pixels.as_mut_ptr().cast::<u8>(), byte_count)
                    })?;
                }
                if !crate::drivers::framebuffer::blit_rect(
                    a0 as u32, a1 as u32, width as u32, height as u32, stride, &pixels[..count],
                ) {
                    return Err(ERR_UNSUPPORTED);
                }
                count as isize
            }
            78 => {
                #[cfg(feature = "driver-ps2-mouse")]
                {
                    let process = current()?;
                    if !process.security.capabilities.contains(crate::security::Capabilities::INPUT) {
                        return Err(ERR_DENIED);
                    }
                    if a1 & !1 != 0 { return Err(ERR_INVALID); }
                    let mut event = None;
                    let result = wait_until(
                        crate::drivers::mouse::wait_queue(),
                        if a1 & 1 != 0 { 0 } else { u64::MAX },
                        || crate::drivers::mouse::try_read().map(|value| { event = Some(value); 0 }),
                    );
                    if result < 0 { return Err(result); }
                    let event = event.ok_or(ERR_INVALID)?;
                    let bytes = unsafe {
                        core::slice::from_raw_parts(
                            (&event as *const crate::drivers::mouse::MouseEvent).cast::<u8>(),
                            core::mem::size_of::<crate::drivers::mouse::MouseEvent>(),
                        )
                    };
                    check_out(a0, bytes.len())?;
                    vmm::copy_to_user(a0, bytes).map_err(|_| ERR_FAULT)?;
                    0
                }
                #[cfg(not(feature = "driver-ps2-mouse"))]
                { return Err(ERR_UNSUPPORTED); }
            }
            40 => {
                let parent = current()?;
                let requested_pid = a0 as i64;
                let options = a2;
                if options & !1 != 0 || requested_pid == 0 || requested_pid < -1 {
                    return Err(ERR_INVALID);
                }
                if a1 != 0 {
                    check_out(a1, core::mem::size_of::<i32>())?;
                }
                let mut candidates = process::children_of(parent.pid);
                if requested_pid > 0 {
                    candidates.retain(|child| child.pid == requested_pid as u64);
                }
                if candidates.is_empty() {
                    return Err(ERR_NO_CHILD);
                }
                let mut child_status = 0u32;
                let completed_pid = if options & 1 != 0 {
                    candidates.iter().find_map(|child| {
                        child.reap_exit_status().map(|status| {
                            process::remove_reaped(child.pid);
                            child_status = encode_wait_status(status, child.exit_signal());
                            child.pid as isize
                        })
                    }).unwrap_or(0)
                } else {
                    wait_until(parent.child_wait_queue(), u64::MAX, || {
                        candidates.iter().find_map(|child| {
                            child.reap_exit_status().map(|status| {
                                process::remove_reaped(child.pid);
                                child_status = encode_wait_status(status, child.exit_signal());
                                child.pid as isize
                            })
                        })
                    })
                };
                if completed_pid < 0 {
                    return Err(completed_pid);
                }
                if completed_pid != 0 && a1 != 0 {
                    vmm::copy_to_user(a1, unsafe {
                        core::slice::from_raw_parts(
                            (&child_status as *const u32).cast::<u8>(),
                            core::mem::size_of::<u32>(),
                        )
                    }).map_err(|_| ERR_FAULT)?;
                }
                completed_pid
            }
            46..=63 | 68..=74 | 79..=81 => crate::posix::dispatch(number, frame)?,
            _ => return Err(ERR_UNSUPPORTED),
        })
    })();
    match result {
        Ok(value) => Some(value),
        Err(error) => Some(error),
    }
}

fn encode_wait_status(exit_code: i32, signal: i32) -> u32 {
    if signal > 0 {
        signal as u32 & 0x7f
    } else {
        (exit_code as u32 & 0xff) << 8
    }
}

fn rollback(root: u64, start: u64, length: u64) {
    for offset in (0..length).step_by(pmm::PAGE_SIZE as usize) {
        if let Ok(physical) = vmm::unmap_page_in_root(root, start + offset) {
            let _ = pmm::free_frame(physical);
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct UserIoRequest {
    handle: u32,
    operation: u16,
    flags: u16,
    buffer: u64,
    length: u64,
    offset: u64,
    completion_key: u64,
    result: i64,
    completed: u32,
    reserved: u32,
}
fn io_submit(pointer: u64) -> Result<isize, isize> {
    if !vmm::is_user_range(pointer, core::mem::size_of::<UserIoRequest>(), true) {
        return Err(ERR_INVALID);
    }
    let process = current()?;
    let mut request = unsafe { core::ptr::read_unaligned(pointer as *const UserIoRequest) };
    let length = request.length as usize;
    if length > MAX_COPY {
        return Err(ERR_INVALID);
    }
    let handle = unsafe { Handle::<FileObject>::from_raw(request.handle) };
    let file = process
        .handles
        .lock()
        .get(
            handle,
            if request.operation == 0 {
                AccessRights::READ
            } else if request.operation == 1 {
                AccessRights::WRITE
            } else {
                return Err(ERR_INVALID);
            },
        )
        .map_err(|_| ERR_DENIED)?;
    let result = if request.operation == 0 {
        if !vmm::is_user_range(request.buffer, length, true) {
            return Err(ERR_INVALID);
        }
        let mut buffer = [0u8; MAX_COPY];
        let offset = usize::try_from(request.offset).map_err(|_| ERR_INVALID)?;
        let count = file
            .read_at(offset, &mut buffer[..length])
            .map_err(|_| ERR_INVALID)?;
        vmm::copy_to_user(request.buffer, &buffer[..count]).map_err(|_| ERR_INVALID)?;
        count as i64
    } else {
        if !vmm::is_user_range(request.buffer, length, false) {
            return Err(ERR_INVALID);
        }
        let mut buffer = [0u8; MAX_COPY];
        vmm::copy_from_user(&mut buffer[..length], request.buffer).map_err(|_| ERR_INVALID)?;
        let offset = usize::try_from(request.offset).map_err(|_| ERR_INVALID)?;
        file.write_at(offset, &buffer[..length])
            .map_err(|_| ERR_INVALID)? as i64
    };
    request.result = result;
    request.completed = COMPLETIONS
        .lock()
        .post(request.completion_key, result, result.max(0) as u64)
        .sequence as u32;
    unsafe {
        core::ptr::write_unaligned(pointer as *mut UserIoRequest, request);
    }
    Ok(0)
}

pub fn dispatch(frame: *mut TrapFrame) -> *mut TrapFrame {
    if frame.is_null() {
        return frame;
    }
    let trap = unsafe { &mut *frame };
    if trap.rax == 61 {
        return match crate::posix::execve_current(trap) {
            Ok(()) => frame,
            Err(error) => {
                trap.rax = error as u64;
                frame
            }
        };
    }
    let exited = trap.rax == NativeCall::ProcessExit as u64;
    let result = handle_call(trap).unwrap_or(ERR_INVALID);
    if exited {
        let next = scheduler::exit_current(frame);
        if next.is_null() {
            crate::panic_halt();
        }
        next
    } else {
        trap.rax = result as u64;
        frame
    }
}
