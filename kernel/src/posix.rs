use crate::fs::{file::FileObject, vfs, vnode::VNodeKind};
use crate::handle::{AccessRights, Handle};
use crate::memory::vmm;
use crate::object::ObjectType;
use crate::syscall::{self, UserStat};
use crate::task::{fd::{FdEntry, FdKind}, process::{self, Process}};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

const EACCES: isize = 13;
const E2BIG: isize = 7;
const EBADF: isize = 9;
const ECHILD: isize = 10;
const EEXIST: isize = 17;
const EXDEV: isize = 18;
const EBUSY: isize = 16;
const EFAULT: isize = 14;
const ESRCH: isize = 3;
const EINVAL: isize = 22;
const EIO: isize = 5;
const EISDIR: isize = 21;
const EMFILE: isize = 24;
const ENAMETOOLONG: isize = 36;
const ENOENT: isize = 2;
const ENOMEM: isize = 12;
const ENOSYS: isize = 38;
const ENOTEMPTY: isize = 39;
const ENOTDIR: isize = 20;
const ENOTTY: isize = 25;
const ENOTSUP: isize = 95;
const EPIPE: isize = 32;
const ESPIPE: isize = 29;
const EILSEQ: isize = 84;
const EINTR: isize = 4;
const ENODATA: isize = 61;
const ENETDOWN: isize = 100;
const ENOBUFS: isize = 105;
const ENOTCONN: isize = 107;
const ENOTSOCK: isize = 88;
const EMSGSIZE: isize = 90;
const EADDRINUSE: isize = 98;
const EADDRNOTAVAIL: isize = 99;
const EAFNOSUPPORT: isize = 97;
const EPROTONOSUPPORT: isize = 93;
const ECONNREFUSED: isize = 111;
const EISCONN: isize = 106;
const ETIMEDOUT: isize = 110;
const EAGAIN: isize = 11;
const EOVERFLOW: isize = 75;
const MAX_COPY: usize = 4096;
const FD_LIMIT: usize = 128;
const EXECVE_SYSCALL: u64 = 61;

const O_ACCMODE: u32 = 3;
const O_RDONLY: u32 = 0;
const O_WRONLY: u32 = 1;
const O_RDWR: u32 = 2;
const O_CLOEXEC: u32 = 0x80000;
const O_CREAT: u32 = 0x40;
const O_EXCL: u32 = 0x80;
const O_TRUNC: u32 = 0x200;
const O_APPEND: u32 = 0x400;
const O_NONBLOCK: u32 = 0x800;
const F_DUPFD: u64 = 0;
const F_GETFD: u64 = 1;
const F_SETFD: u64 = 2;
const F_GETFL: u64 = 3;
const F_SETFL: u64 = 4;
const FD_CLOEXEC: u32 = 1;
const AF_INET: u16 = 2;
const SOCK_STREAM: u32 = 1;
const SOCK_DGRAM: u32 = 2;
const SOCK_CLOEXEC: u32 = 0x80000;
const SOCK_NONBLOCK: u32 = 0x800;
const IPPROTO_UDP: u32 = 17;
const IPPROTO_TCP: u32 = 6;
const MAX_UDP_PAYLOAD: usize = 4096;

fn current() -> Result<Arc<Process>, isize> {
    process::current().ok_or(-ESRCH)
}

fn input<'a>(pointer: u64, length: usize, storage: &'a mut [u8]) -> Result<&'a [u8], isize> {
    if length > storage.len() { return Err(-EINVAL); }
    if length == 0 { return Ok(&storage[..0]); }
    vmm::copy_from_user(&mut storage[..length], pointer).map_err(|_| -EFAULT)?;
    Ok(&storage[..length])
}

#[derive(Clone, Copy)]
struct Ipv4SocketAddress { address: [u8; 4], port: u16 }

fn read_ipv4_sockaddr(pointer: u64, length: usize) -> Result<Ipv4SocketAddress, isize> {
    if length < 16 { return Err(-EINVAL); }
    let mut bytes = [0u8; 16];
    input(pointer, 16, &mut bytes)?;
    if u16::from_ne_bytes([bytes[0], bytes[1]]) != AF_INET { return Err(-EAFNOSUPPORT); }
    let port = u16::from_be_bytes([bytes[2], bytes[3]]);
    Ok(Ipv4SocketAddress { address: [bytes[4], bytes[5], bytes[6], bytes[7]], port })
}

fn write_ipv4_sockaddr(pointer: u64, length_pointer: u64, peer: Ipv4SocketAddress) -> Result<(), isize> {
    if pointer == 0 || length_pointer == 0 { return Ok(()); }
    let mut encoded_length = [0u8; 4];
    vmm::copy_from_user(&mut encoded_length, length_pointer).map_err(|_| -EFAULT)?;
    let supplied = u32::from_ne_bytes(encoded_length) as usize;
    let mut bytes = [0u8; 16];
    bytes[0..2].copy_from_slice(&AF_INET.to_ne_bytes());
    bytes[2..4].copy_from_slice(&peer.port.to_be_bytes());
    bytes[4..8].copy_from_slice(&peer.address);
    let copied = supplied.min(bytes.len());
    output(pointer, &bytes[..copied])?;
    vmm::copy_to_user(length_pointer, &(bytes.len() as u32).to_ne_bytes()).map_err(|_| -EFAULT)
}

fn output(pointer: u64, bytes: &[u8]) -> Result<(), isize> {
    if bytes.len() > MAX_COPY { return Err(-EINVAL); }
    if !vmm::is_user_range(pointer, bytes.len(), true) { return Err(-EFAULT); }
    vmm::copy_to_user(pointer, bytes).map_err(|_| -EFAULT)
}

fn path_string<'a>(pointer: u64, length: usize, storage: &'a mut [u8]) -> Result<&'a str, isize> {
    if length == 0 { return Err(-ENOENT); }
    if length > 512 { return Err(-ENAMETOOLONG); }
    core::str::from_utf8(input(pointer, length, storage)?).map_err(|_| -EILSEQ)
}

fn copy_user_c_string(pointer: u64, limit: usize) -> Result<String, isize> {
    if pointer == 0 { return Err(-EFAULT); }
    let mut bytes = Vec::new();
    for offset in 0..limit {
        let mut byte = [0u8; 1];
        vmm::copy_from_user(&mut byte, pointer.checked_add(offset as u64).ok_or(-EFAULT)?)
            .map_err(|_| -EFAULT)?;
        if byte[0] == 0 {
            return String::from_utf8(bytes).map_err(|_| -EILSEQ);
        }
        bytes.push(byte[0]);
    }
    Err(-ENAMETOOLONG)
}

fn copy_user_string_vector(pointer: u64, total_bytes: &mut usize) -> Result<Vec<String>, isize> {
    if pointer == 0 { return Ok(Vec::new()); }
    let mut values = Vec::new();
    for index in 0..32usize {
        let slot = pointer.checked_add((index * 8) as u64).ok_or(-EFAULT)?;
        let mut encoded = [0u8; 8];
        vmm::copy_from_user(&mut encoded, slot).map_err(|_| -EFAULT)?;
        let string_pointer = u64::from_le_bytes(encoded);
        if string_pointer == 0 { return Ok(values); }
        let value = copy_user_c_string(string_pointer, 1024)?;
        *total_bytes = total_bytes.checked_add(value.len() + 1).ok_or(-E2BIG)?;
        if *total_bytes > 2048 { return Err(-E2BIG); }
        values.push(value);
    }
    Err(-E2BIG)
}

pub(crate) fn execve_current(frame: &mut crate::arch::x86_64::idt::TrapFrame) -> Result<(), isize> {
    let process = current()?;
    if process.threads.lock().len() > 1 { return Err(-ENOSYS); }
    let path = copy_user_c_string(frame.rdi, 513)?;
    if path.len() > 512 { return Err(-ENAMETOOLONG); }
    let mut total_bytes = path.len() + 1;
    let mut argv = copy_user_string_vector(frame.rsi, &mut total_bytes)?;
    let envp = copy_user_string_vector(frame.rdx, &mut total_bytes)?;
    if argv.is_empty() { argv.push(path.clone()); }
    let argv_refs: Vec<&str> = argv.iter().map(String::as_str).collect();
    let envp_refs: Vec<&str> = envp.iter().map(String::as_str).collect();
    let image = vfs::executable(&path, &process.cwd()).map_err(|_| -ENOENT)?;
    let new_root = vmm::clone_kernel_root().map_err(|_| -ENOMEM)?;
    let new_space = process::AddressSpace::new(new_root);
    let loaded = match crate::exec::elf::load_user(&image, &new_space, &argv_refs, &envp_refs) {
        Ok(loaded) => loaded,
        Err(_) => {
            let _ = vmm::destroy_address_space(new_root);
            return Err(-EIO);
        }
    };

    let old_root = process.address_space.replace_with(&new_space);
    if vmm::switch_root(new_root).is_err() {
        let _ = process.address_space.replace_with(&process::AddressSpace::new(old_root));
        let _ = vmm::destroy_address_space(new_root);
        return Err(-EIO);
    }
    let _ = vmm::destroy_address_space(old_root);
    process.set_executable(&path);

    let mut to_close = [crate::task::fd::CLOSED_FD; FD_LIMIT];
    let close_count = process.fds.lock().close_on_exec(&mut to_close);
    for entry in to_close.iter().take(close_count) {
        close_entry(&process, *entry);
    }
    if let Some(thread) = process.threads.lock().first() {
        thread.set_user_stack(loaded.stack_pointer);
        thread.context.lock().rsp = loaded.stack_pointer;
    }

    frame.r15 = 0;
    frame.r14 = 0;
    frame.r13 = 0;
    frame.r12 = 0;
    frame.r11 = 0;
    frame.r10 = 0;
    frame.r9 = 0;
    frame.r8 = 0;
    frame.rdi = 0;
    frame.rsi = 0;
    frame.rbp = 0;
    frame.rdx = 0;
    frame.rcx = 0;
    frame.rbx = 0;
    frame.rax = 0;
    frame.rip = loaded.entry;
    frame.cs = crate::arch::x86_64::gdt::USER_CODE as u64;
    frame.rflags = 0x202;
    frame.rsp = loaded.stack_pointer;
    frame.ss = crate::arch::x86_64::gdt::USER_DATA as u64;
    Ok(())
}

fn file_rights(flags: u32) -> Result<(bool, bool, AccessRights), isize> {
    let (readable, writable) = match flags & O_ACCMODE {
        O_RDONLY => (true, false),
        O_WRONLY => (false, true),
        O_RDWR => (true, true),
        _ => return Err(-EINVAL),
    };
    let rights = AccessRights(
        AccessRights::DUPLICATE.0 | AccessRights::TRANSFER.0
            | (if readable { AccessRights::READ.0 } else { 0 })
            | (if writable { AccessRights::WRITE.0 } else { 0 }),
    );
    Ok((readable, writable, rights))
}

fn close_entry(process: &Arc<Process>, entry: FdEntry) {
    match entry.kind {
        FdKind::File | FdKind::PipeRead | FdKind::PipeWrite if entry.handle != 0 => {
            let _ = process.handles.lock().close(entry.handle);
        }
        FdKind::SocketUdp => {
            #[cfg(feature = "network-stack")]
            crate::network::udp_release(entry.handle);
        }
        FdKind::SocketTcp => {
            #[cfg(feature = "network-stack")]
            crate::network::tcp_release(entry.handle);
        }
        _ => {}
    }
}

fn duplicate_entry(process: &Arc<Process>, source: FdEntry) -> Result<FdEntry, isize> {
    if source.handle == 0 {
        return Ok(FdEntry { descriptor_flags: 0, ..source });
    }
    let rights = match source.kind {
        FdKind::File => file_rights(source.status_flags)?.2,
        FdKind::PipeRead => AccessRights(AccessRights::DUPLICATE.0 | AccessRights::TRANSFER.0 | AccessRights::READ.0),
        FdKind::PipeWrite => AccessRights(AccessRights::DUPLICATE.0 | AccessRights::TRANSFER.0 | AccessRights::WRITE.0),
        FdKind::SocketUdp => {
            #[cfg(feature = "network-stack")]
            crate::network::udp_retain(source.handle).map_err(|_| -EBADF)?;
            return Ok(FdEntry { descriptor_flags: 0, ..source });
        }
        FdKind::SocketTcp => {
            #[cfg(feature = "network-stack")]
            crate::network::tcp_retain(source.handle).map_err(|_| -EBADF)?;
            return Ok(FdEntry { descriptor_flags: 0, ..source });
        }
        _ => return Err(-EBADF),
    };
    let object_type = process.handles.lock().object_type(source.handle).ok_or(-EBADF)?;
    let duplicate = match object_type {
        ObjectType::File => process.handles.lock()
            .duplicate(unsafe { Handle::<FileObject>::from_raw(source.handle) }, rights)
            .map(|handle| handle.raw()),
        ObjectType::PipeReader => process.handles.lock()
            .duplicate(unsafe { Handle::<crate::ipc::pipe::PipeReader>::from_raw(source.handle) }, rights)
            .map(|handle| handle.raw()),
        ObjectType::PipeWriter => process.handles.lock()
            .duplicate(unsafe { Handle::<crate::ipc::pipe::PipeWriter>::from_raw(source.handle) }, rights)
            .map(|handle| handle.raw()),
        _ => return Err(-EBADF),
    }.map_err(|_| -EBADF)?;
    Ok(FdEntry { handle: duplicate, descriptor_flags: 0, ..source })
}

fn allocate_descriptor(process: &Arc<Process>, entry: FdEntry, minimum: usize) -> Result<usize, isize> {
    process.fds.lock().allocate(entry, minimum).map_err(|_| -EMFILE)
}

fn posix_dup(process: &Arc<Process>, source_fd: usize, minimum: usize) -> Result<isize, isize> {
    let source = process.fds.lock().get(source_fd).ok_or(-EBADF)?;
    let duplicate = duplicate_entry(process, source)?;
    match allocate_descriptor(process, duplicate, minimum) {
        Ok(fd) => Ok(fd as isize),
        Err(error) => { close_entry(process, duplicate); Err(error) }
    }
}

fn posix_dup2(process: &Arc<Process>, source_fd: usize, destination_fd: usize) -> Result<isize, isize> {
    let source = process.fds.lock().get(source_fd).ok_or(-EBADF)?;
    if destination_fd >= FD_LIMIT { return Err(-EBADF); }
    if source_fd == destination_fd { return Ok(destination_fd as isize); }
    let duplicate = duplicate_entry(process, source)?;
    let old = process.fds.lock().replace(destination_fd, duplicate).map_err(|_| -EBADF)?;
    close_entry(process, old);
    Ok(destination_fd as isize)
}

fn posix_open(process: &Arc<Process>, frame: &crate::arch::x86_64::idt::TrapFrame) -> Result<isize, isize> {
    let flags = frame.rdx as u32;
    if flags & !(O_ACCMODE | O_CLOEXEC | O_CREAT | O_EXCL | O_TRUNC | O_APPEND) != 0 {
        return Err(-ENOSYS);
    }
    if flags & O_EXCL != 0 && flags & O_CREAT == 0 { return Err(-EINVAL); }
    let (readable, writable, rights) = file_rights(flags)?;
    if readable && !process.security.capabilities.contains(crate::security::Capabilities::FILE_READ) {
        return Err(-EACCES);
    }
    if writable && !process.security.capabilities.contains(crate::security::Capabilities::FILE_WRITE) {
        return Err(-EACCES);
    }
    if flags & O_CREAT != 0
        && !process.security.capabilities.contains(crate::security::Capabilities::FILE_WRITE)
    {
        return Err(-EACCES);
    }
    let mut buffer = [0u8; 512];
    let path = path_string(frame.rdi, frame.rsi as usize, &mut buffer)?;
    let cwd = process.cwd();
    let (node, filesystem) = match vfs::lookup(path, &cwd) {
        Ok(_) if flags & O_CREAT != 0 && flags & O_EXCL != 0 => return Err(-EEXIST),
        Err(_) if flags & O_CREAT != 0 => {
            let (parent, filesystem) = vfs::parent_node(path, &cwd).map_err(|_| -ENOENT)?;
            if process.uid != 0
                && !filesystem.can_access(parent.node_id, process.uid, process.gid, crate::fs::vnode::Acl::WRITE)
            {
                return Err(-EACCES);
            }
            vfs::create_file(path, &cwd, false).map_err(|_| -EIO)?;
            vfs::lookup(path, &cwd).map_err(|_| -EIO)?
        }
        Ok(resolved) => resolved,
        Err(_) => return Err(-ENOENT),
    };
    match filesystem.kind(node.node_id) {
        Some(VNodeKind::File) => {}
        Some(VNodeKind::Directory) => return Err(-EISDIR),
        None => return Err(-ENOENT),
    }
    let access = (if readable { crate::fs::vnode::Acl::READ } else { 0 })
        | (if writable { crate::fs::vnode::Acl::WRITE } else { 0 });
    if process.uid != 0 && !filesystem.can_access(node.node_id, process.uid, process.gid, access) {
        return Err(-EACCES);
    }
    if flags & O_TRUNC != 0 {
        // O_TRUNC without write access cannot discard anything.
        if !writable { return Err(-EACCES); }
        filesystem.truncate(node.node_id).map_err(|_| -EIO)?;
    }
    let object = Arc::new(FileObject::new(node, readable, writable));
    if flags & O_APPEND != 0 {
        object.set_append(true);
    }
    let handle = process.handles.lock().insert(object, rights).map_err(|_| -EMFILE)?;
    let descriptor = FdEntry {
        handle: handle.raw(),
        kind: FdKind::File,
        status_flags: flags & (O_ACCMODE | O_APPEND),
        descriptor_flags: if flags & O_CLOEXEC != 0 { FD_CLOEXEC } else { 0 },
    };
    match allocate_descriptor(process, descriptor, 0) {
        Ok(fd) => Ok(fd as isize),
        Err(error) => {
            let _ = process.handles.lock().close(handle.raw());
            Err(error)
        }
    }
}

fn posix_unlink(process: &Arc<Process>, pointer: u64, length: usize) -> Result<isize, isize> {
    if !process.security.capabilities.contains(crate::security::Capabilities::FILE_WRITE) {
        return Err(-EACCES);
    }
    let mut buffer = [0u8; 512];
    let path = path_string(pointer, length, &mut buffer)?;
    let cwd = process.cwd();
    let (node, filesystem) = vfs::lookup(path, &cwd).map_err(|_| -ENOENT)?;
    match filesystem.kind(node.node_id) {
        Some(VNodeKind::File) => {}
        Some(VNodeKind::Directory) => return Err(-EISDIR),
        None => return Err(-ENOENT),
    }
    let (parent, parent_fs) = vfs::parent_node(path, &cwd).map_err(|_| -ENOENT)?;
    if process.uid != 0 && (node.mount_id != parent.mount_id
        || !parent_fs.can_access(parent.node_id, process.uid, process.gid, crate::fs::vnode::Acl::WRITE)) {
        return Err(-EACCES);
    }
    vfs::remove(path, &cwd).map_err(|_| -EIO)?;
    Ok(0)
}

fn posix_rename(process: &Arc<Process>, frame: &crate::arch::x86_64::idt::TrapFrame) -> Result<isize, isize> {
    if !process.security.capabilities.contains(crate::security::Capabilities::FILE_WRITE) {
        return Err(-EACCES);
    }
    let mut old_storage = [0u8; 512];
    let mut new_storage = [0u8; 512];
    let old_path = path_string(frame.rdi, frame.rsi as usize, &mut old_storage)?;
    let new_path = path_string(frame.rdx, frame.r10 as usize, &mut new_storage)?;
    let cwd = process.cwd();
    let (source, filesystem) = vfs::lookup(old_path, &cwd).map_err(|_| -ENOENT)?;
    let source_metadata = filesystem.metadata(source.node_id).ok_or(-ENOENT)?;
    if source.node_id == filesystem.resolve("/", "/").map_err(|_| -EIO)? {
        return Err(-EBUSY);
    }
    let (source_parent, source_parent_fs) = vfs::parent_node(old_path, &cwd).map_err(|_| -ENOENT)?;
    let (destination_parent, destination_parent_fs) = vfs::parent_node(new_path, &cwd).map_err(|_| -ENOENT)?;
    if source.mount_id != destination_parent.mount_id { return Err(-18); }

    if process.uid != 0 {
        let required = crate::fs::vnode::Acl::WRITE | crate::fs::vnode::Acl::EXECUTE;
        if !source_parent_fs.can_access(source_parent.node_id, process.uid, process.gid, required)
            || !destination_parent_fs.can_access(destination_parent.node_id, process.uid, process.gid, required)
        {
            return Err(-EACCES);
        }
    }

    vfs::rename(old_path, new_path, &cwd).map_err(|error| match error {
        "path not found" | "file not found" => -ENOENT,
        "not a directory" => -ENOTDIR,
        "directory is not empty" => -ENOTEMPTY,
        "rename type mismatch" if source_metadata.kind == VNodeKind::File => -EISDIR,
        "rename type mismatch" => -ENOTDIR,
        "cannot rename root" => -EBUSY,
        "directory move would create a cycle" | "invalid file name" | "invalid path"
        | "invalid SBFS file name" => -EINVAL,
        "name is too long" => -ENAMETOOLONG,
        "name already exists" => -EEXIST,
        "cross-device rename" => -EXDEV,
        _ => -EIO,
    })?;
    Ok(0)
}

fn require_terminal_fd(process: &Arc<Process>, fd: usize) -> Result<(), isize> {
    match process.fds.lock().get(fd).map(|entry| entry.kind) {
        Some(FdKind::ConsoleRead | FdKind::ConsoleWrite) => Ok(()),
        Some(FdKind::Closed) | None => Err(-EBADF),
        _ => Err(-ENOTTY),
    }
}

fn posix_tcgetattr(process: &Arc<Process>, fd: usize, pointer: u64) -> Result<isize, isize> {
    require_terminal_fd(process, fd)?;
    let attributes = crate::drivers::tty::attributes();
    let bytes = unsafe {
        core::slice::from_raw_parts(
            (&attributes as *const crate::drivers::tty::TerminalAttributes).cast::<u8>(),
            core::mem::size_of::<crate::drivers::tty::TerminalAttributes>(),
        )
    };
    output(pointer, bytes)?;
    Ok(0)
}

fn posix_tcsetattr(
    process: &Arc<Process>,
    fd: usize,
    action: u64,
    pointer: u64,
) -> Result<isize, isize> {
    require_terminal_fd(process, fd)?;
    if action > 2 { return Err(-EINVAL); }
    let mut attributes = core::mem::MaybeUninit::<crate::drivers::tty::TerminalAttributes>::uninit();
    let bytes = unsafe {
        core::slice::from_raw_parts_mut(
            attributes.as_mut_ptr().cast::<u8>(),
            core::mem::size_of::<crate::drivers::tty::TerminalAttributes>(),
        )
    };
    if !vmm::is_user_range(pointer, bytes.len(), false) { return Err(-EFAULT); }
    vmm::copy_from_user(bytes, pointer).map_err(|_| -EFAULT)?;
    let attributes = unsafe { attributes.assume_init() };
    if attributes.line != 0 { return Err(-EINVAL); }
    crate::drivers::tty::set_attributes(attributes, action).map_err(|_| -EINVAL)?;
    Ok(0)
}

fn posix_ioctl(process: &Arc<Process>, fd: usize, request: u64, argument: u64) -> Result<isize, isize> {
    const TIOCGWINSZ: u64 = 0x5413;
    const TIOCSWINSZ: u64 = 0x5414;
    require_terminal_fd(process, fd)?;
    match request {
        TIOCGWINSZ => {
            #[repr(C)]
            struct WindowSize { rows: u16, columns: u16, xpixel: u16, ypixel: u16 }
            let (rows, columns) = crate::drivers::tty::window_size();
            let size = WindowSize { rows, columns, xpixel: 0, ypixel: 0 };
            let bytes = unsafe {
                core::slice::from_raw_parts((&size as *const WindowSize).cast::<u8>(), core::mem::size_of::<WindowSize>())
            };
            output(argument, bytes)?;
            Ok(0)
        }
        TIOCSWINSZ => Err(-ENOTSUP),
        _ => Err(-ENOTTY),
    }
}

fn posix_read(process: &Arc<Process>, fd: usize, pointer: u64, requested: usize) -> Result<isize, isize> {
    let entry = process.fds.lock().get(fd).ok_or(-EBADF)?;
    if requested == 0 { return Ok(0); }
    let length = requested.min(MAX_COPY);
    let mut bytes = [0u8; MAX_COPY];
    let count = match entry.kind {
        FdKind::ConsoleRead => {
            if !vmm::is_user_range(pointer, length, true) { return Err(-EFAULT); }
            let count = crate::drivers::tty::read(process.pid, &mut bytes[..length]);
            vmm::copy_to_user(pointer, &bytes[..count]).map_err(|_| -EFAULT)?;
            return Ok(count as isize);
        }
        FdKind::File => {
            if entry.status_flags & O_ACCMODE == O_WRONLY { return Err(-EBADF); }
            let file = process.handles.lock()
                .get(unsafe { Handle::<FileObject>::from_raw(entry.handle) }, AccessRights::READ)
                .map_err(|_| -EBADF)?;
            file.read(&mut bytes[..length]).map_err(|_| -EIO)?
        }
        FdKind::PipeRead => {
            let reader = process.handles.lock()
                .get(unsafe { Handle::<crate::ipc::pipe::PipeReader>::from_raw(entry.handle) }, AccessRights::READ)
                .map_err(|_| -EBADF)?;
            let result = syscall::wait_until(reader.wait_queue(), u64::MAX, || {
                reader.try_read(&mut bytes[..length]).map(|count| count as isize)
            });
            if result < 0 { return Err(result); }
            result as usize
        }
        #[cfg(feature = "network-stack")]
        FdKind::SocketUdp => return posix_receive_from(process, fd, pointer, length, 0, 0, 0),
        #[cfg(not(feature = "network-stack"))]
        FdKind::SocketUdp => return Err(-ENOSYS),
        #[cfg(feature = "network-stack")]
        FdKind::SocketTcp => return posix_tcp_receive(process, fd, pointer, length),
        #[cfg(not(feature = "network-stack"))]
        FdKind::SocketTcp => return Err(-ENOSYS),
        FdKind::ConsoleWrite | FdKind::PipeWrite => return Err(-EBADF),
        FdKind::Closed => return Err(-EBADF),
    };
    output(pointer, &bytes[..count])?;
    Ok(count as isize)
}

fn posix_write(process: &Arc<Process>, fd: usize, pointer: u64, requested: usize) -> Result<isize, isize> {
    let entry = process.fds.lock().get(fd).ok_or(-EBADF)?;
    if requested == 0 { return Ok(0); }
    let length = requested.min(MAX_COPY);
    let mut bytes = [0u8; MAX_COPY];
    let input = input(pointer, length, &mut bytes)?;
    let count = match entry.kind {
        FdKind::ConsoleWrite => {
            crate::drivers::tty::write(input);
            input.len()
        }
        FdKind::File => {
            if entry.status_flags & O_ACCMODE == O_RDONLY { return Err(-EBADF); }
            let file = process.handles.lock()
                .get(unsafe { Handle::<FileObject>::from_raw(entry.handle) }, AccessRights::WRITE)
                .map_err(|_| -EBADF)?;
            file.write(input).map_err(|_| -EIO)?
        }
        FdKind::PipeWrite => {
            let writer = process.handles.lock()
                .get(unsafe { Handle::<crate::ipc::pipe::PipeWriter>::from_raw(entry.handle) }, AccessRights::WRITE)
                .map_err(|_| -EBADF)?;
            let result = syscall::wait_until(writer.wait_queue(), u64::MAX, || match writer.try_write(input) {
                Ok(Some(count)) => Some(count as isize),
                Ok(None) => None,
                Err(()) => Some(-EPIPE),
            });
            if result < 0 { return Err(result); }
            result as usize
        }
        #[cfg(feature = "network-stack")]
        FdKind::SocketUdp => crate::network::udp_send_connected(entry.handle, input)
            .map_err(map_udp_error)?,
        #[cfg(not(feature = "network-stack"))]
        FdKind::SocketUdp => return Err(-ENOSYS),
        #[cfg(feature = "network-stack")]
        FdKind::SocketTcp => return posix_tcp_write(process, fd, input),
        #[cfg(not(feature = "network-stack"))]
        FdKind::SocketTcp => return Err(-ENOSYS),
        FdKind::ConsoleRead | FdKind::PipeRead | FdKind::Closed => return Err(-EBADF),
    };
    Ok(count as isize)
}

fn posix_close(process: &Arc<Process>, fd: usize) -> Result<isize, isize> {
    let entry = process.fds.lock().close(fd).map_err(|_| -EBADF)?;
    close_entry(process, entry);
    Ok(0)
}

fn posix_clock_gettime(clock_id: u64, pointer: u64) -> Result<isize, isize> {
    #[repr(C)]
    struct UserTimespec { seconds: i64, nanoseconds: i64 }

    let (seconds, nanoseconds) = match clock_id {
        0 => crate::time::realtime().ok_or(-ENODATA)?,
        1 => crate::time::monotonic(),
        _ => return Err(-EINVAL),
    };
    let record = UserTimespec { seconds, nanoseconds };
    let bytes = unsafe {
        core::slice::from_raw_parts(
            (&record as *const UserTimespec).cast::<u8>(),
            core::mem::size_of::<UserTimespec>(),
        )
    };
    output(pointer, bytes)?;
    Ok(0)
}

pub fn file_sync(fd: usize) -> Result<isize, isize> {
    let process = current()?;
    let entry = process.fds.lock().get(fd).ok_or(-EBADF)?;
    if entry.kind != FdKind::File { return Err(-EINVAL); }
    let file = process.handles.lock()
        .get(unsafe { Handle::<FileObject>::from_raw(entry.handle) }, AccessRights::NONE)
        .map_err(|_| -EBADF)?;
    vfs::sync_node(file.node).map_err(|_| -EIO)?;
    Ok(0)
}

pub fn thread_sleep(timeout_ticks: u64) -> Result<isize, isize> {
    if timeout_ticks == 0 { return Ok(0); }
    let wait_queue = crate::task::wait::WaitQueue::new();
    match syscall::wait_until(&wait_queue, timeout_ticks, || None) {
        -7 => Ok(0),
        result if result < 0 => Err(-EINTR),
        _ => Ok(0),
    }
}

fn posix_pipe(process: &Arc<Process>, pointer: u64) -> Result<isize, isize> {
    if !vmm::is_user_range(pointer, 8, true) { return Err(-EFAULT); }
    let (reader, writer) = crate::ipc::pipe::PipeReader::pair();
    let read_handle = process.handles.lock()
        .insert(reader, AccessRights(AccessRights::READ.0 | AccessRights::DUPLICATE.0))
        .map_err(|_| -EMFILE)?;
    let write_handle = match process.handles.lock()
        .insert(writer, AccessRights(AccessRights::WRITE.0 | AccessRights::DUPLICATE.0))
    {
        Ok(handle) => handle,
        Err(_) => {
            let _ = process.handles.lock().close(read_handle.raw());
            return Err(-EMFILE);
        }
    };
    let read_entry = FdEntry { handle: read_handle.raw(), kind: FdKind::PipeRead, status_flags: O_RDONLY, descriptor_flags: 0 };
    let write_entry = FdEntry { handle: write_handle.raw(), kind: FdKind::PipeWrite, status_flags: O_WRONLY, descriptor_flags: 0 };
    let read_fd = match allocate_descriptor(process, read_entry, 0) {
        Ok(fd) => fd,
        Err(error) => {
            let _ = process.handles.lock().close(read_handle.raw());
            let _ = process.handles.lock().close(write_handle.raw());
            return Err(error);
        }
    };
    let write_fd = match allocate_descriptor(process, write_entry, 0) {
        Ok(fd) => fd,
        Err(error) => {
            let old = process.fds.lock().close(read_fd).unwrap();
            close_entry(process, old);
            let _ = process.handles.lock().close(write_handle.raw());
            return Err(error);
        }
    };
    let fds = [read_fd as i32, write_fd as i32];
    let bytes = unsafe { core::slice::from_raw_parts(fds.as_ptr().cast::<u8>(), 8) };
    if vmm::copy_to_user(pointer, bytes).is_err() {
        let old_read = process.fds.lock().close(read_fd).unwrap();
        let old_write = process.fds.lock().close(write_fd).unwrap();
        close_entry(process, old_read);
        close_entry(process, old_write);
        return Err(-EFAULT);
    }
    Ok(0)
}

#[cfg(feature = "network-stack")]
fn map_udp_error(error: crate::network::UdpError) -> isize {
    match error {
        crate::network::UdpError::NotReady => -ENETDOWN,
        crate::network::UdpError::NotFound => -EBADF,
        crate::network::UdpError::NoSpace => -ENOBUFS,
        crate::network::UdpError::AddressInUse => -EADDRINUSE,
        crate::network::UdpError::AddressNotAvailable => -EADDRNOTAVAIL,
        crate::network::UdpError::NotConnected => -ENOTCONN,
        crate::network::UdpError::MessageTooLarge => -EMSGSIZE,
        crate::network::UdpError::WouldBlock => -11,
        crate::network::UdpError::Invalid => -EINVAL,
    }
}

#[cfg(feature = "network-stack")]
fn map_tcp_error(error: crate::network::TcpError) -> isize {
    match error {
        crate::network::TcpError::NotReady => -ENETDOWN,
        crate::network::TcpError::NotFound => -EBADF,
        crate::network::TcpError::NoSpace => -ENOBUFS,
        crate::network::TcpError::AddressInUse => -EADDRINUSE,
        crate::network::TcpError::AddressNotAvailable => -EADDRNOTAVAIL,
        crate::network::TcpError::NotConnected => -ENOTCONN,
        crate::network::TcpError::ConnectionRefused => -ECONNREFUSED,
        crate::network::TcpError::MessageTooLarge => -EMSGSIZE,
        crate::network::TcpError::WouldBlock => -EAGAIN,
        crate::network::TcpError::Closed => -EPIPE,
        crate::network::TcpError::Invalid => -EINVAL,
    }
}

#[cfg(feature = "network-stack")]
fn posix_socket(process: &Arc<Process>, frame: &crate::arch::x86_64::idt::TrapFrame) -> Result<isize, isize> {
    if !process.security.capabilities.contains(crate::security::Capabilities::NETWORK) {
        return Err(-EACCES);
    }
    let domain = frame.rdi as u16;
    let kind = frame.rsi as u32;
    let protocol = frame.rdx as u32;
    if domain != AF_INET { return Err(-EAFNOSUPPORT); }
    let socket_kind = kind & (SOCK_STREAM | SOCK_DGRAM);
    if kind & !(SOCK_STREAM | SOCK_DGRAM | SOCK_CLOEXEC | SOCK_NONBLOCK) != 0
        || (socket_kind != SOCK_STREAM && socket_kind != SOCK_DGRAM) {
        return Err(-EPROTONOSUPPORT);
    }
    let socket = match socket_kind {
        SOCK_DGRAM if protocol == 0 || protocol == IPPROTO_UDP => crate::network::udp_open()
            .map(|id| (id, FdKind::SocketUdp)).map_err(map_udp_error)?,
        SOCK_STREAM if protocol == 0 || protocol == IPPROTO_TCP => crate::network::tcp_open()
            .map(|id| (id, FdKind::SocketTcp)).map_err(map_tcp_error)?,
        _ => return Err(-EPROTONOSUPPORT),
    };
    let entry = FdEntry {
        handle: socket.0,
        kind: socket.1,
        status_flags: if kind & SOCK_NONBLOCK != 0 { O_NONBLOCK } else { 0 },
        descriptor_flags: if kind & SOCK_CLOEXEC != 0 { FD_CLOEXEC } else { 0 },
    };
    match allocate_descriptor(process, entry, 0) {
        Ok(fd) => Ok(fd as isize),
        Err(error) => {
            match socket.1 {
                FdKind::SocketUdp => crate::network::udp_release(socket.0),
                FdKind::SocketTcp => crate::network::tcp_release(socket.0),
                _ => {}
            }
            Err(error)
        }
    }
}

#[cfg(feature = "network-stack")]
fn require_udp_socket(process: &Arc<Process>, fd: usize) -> Result<u32, isize> {
    let entry = process.fds.lock().get(fd).ok_or(-EBADF)?;
    if entry.kind != FdKind::SocketUdp { return Err(-ENOTSOCK); }
    Ok(entry.handle)
}

#[cfg(feature = "network-stack")]
fn posix_bind(process: &Arc<Process>, fd: usize, address: u64, length: usize) -> Result<isize, isize> {
    if !process.security.capabilities.contains(crate::security::Capabilities::NETWORK) {
        return Err(-EACCES);
    }
    let entry = process.fds.lock().get(fd).ok_or(-EBADF)?;
    if !matches!(entry.kind, FdKind::SocketUdp | FdKind::SocketTcp) { return Err(-ENOTSOCK); }
    let address = read_ipv4_sockaddr(address, length)?;
    match entry.kind {
        FdKind::SocketUdp => crate::network::udp_bind(entry.handle, address.address, address.port)
            .map(|_| 0).map_err(map_udp_error),
        FdKind::SocketTcp => crate::network::tcp_bind(entry.handle, address.address, address.port)
            .map(|_| 0).map_err(map_tcp_error),
        _ => Err(-ENOTSOCK),
    }
}

#[cfg(feature = "network-stack")]
fn posix_connect(process: &Arc<Process>, fd: usize, address: u64, length: usize) -> Result<isize, isize> {
    if !process.security.capabilities.contains(crate::security::Capabilities::NETWORK) {
        return Err(-EACCES);
    }
    let entry = process.fds.lock().get(fd).ok_or(-EBADF)?;
    if !matches!(entry.kind, FdKind::SocketUdp | FdKind::SocketTcp) { return Err(-ENOTSOCK); }
    let address = read_ipv4_sockaddr(address, length)?;
    let peer = crate::network::UdpPeer { address: address.address, port: address.port };
    match entry.kind {
        FdKind::SocketUdp => crate::network::udp_connect(entry.handle, peer)
            .map(|_| 0).map_err(map_udp_error),
        FdKind::SocketTcp => {
            crate::network::tcp_connect(entry.handle, peer).map_err(map_tcp_error)?;
            let result = syscall::wait_until(crate::network::tcp_wait_queue(), 1000, || {
                match crate::network::tcp_state(entry.handle) {
                    Ok(smoltcp::socket::tcp::State::Established) => Some(0),
                    Ok(smoltcp::socket::tcp::State::Closed) => Some(-ECONNREFUSED),
                    Ok(_) => None,
                    Err(error) => Some(map_tcp_error(error)),
                }
            });
            if result == -7 { return Err(-ETIMEDOUT); }
            if result < 0 { return Err(result); }
            Ok(0)
        }
        _ => Err(-ENOTSOCK),
    }
}

#[cfg(feature = "network-stack")]
fn posix_send_to(
    process: &Arc<Process>, fd: usize, pointer: u64, requested: usize, flags: u64,
    destination_pointer: u64, destination_length: usize,
) -> Result<isize, isize> {
    if flags != 0 { return Err(-ENOSYS); }
    if !process.security.capabilities.contains(crate::security::Capabilities::NETWORK) {
        return Err(-EACCES);
    }
    let entry = process.fds.lock().get(fd).ok_or(-EBADF)?;
    let length = requested.min(MAX_UDP_PAYLOAD);
    if requested > MAX_UDP_PAYLOAD { return Err(-EMSGSIZE); }
    let mut bytes = [0u8; MAX_UDP_PAYLOAD];
    let payload = input(pointer, length, &mut bytes)?;
    let sent = match entry.kind {
        FdKind::SocketUdp if destination_pointer == 0 => {
            crate::network::udp_send_connected(entry.handle, payload).map_err(map_udp_error)?
        }
        FdKind::SocketUdp => {
            let destination = read_ipv4_sockaddr(destination_pointer, destination_length)?;
            crate::network::udp_send_to(entry.handle, payload, crate::network::UdpPeer {
                address: destination.address,
                port: destination.port,
            }).map_err(map_udp_error)?
        }
        FdKind::SocketTcp if destination_pointer == 0 => return posix_tcp_write(process, fd, payload),
        FdKind::SocketTcp => return Err(-EISCONN),
        _ => return Err(-ENOTSOCK),
    };
    Ok(sent as isize)
}

#[cfg(feature = "network-stack")]
fn posix_receive_from(
    process: &Arc<Process>, fd: usize, pointer: u64, requested: usize, flags: u64,
    source_pointer: u64, source_length_pointer: u64,
) -> Result<isize, isize> {
    if flags != 0 { return Err(-ENOSYS); }
    if !process.security.capabilities.contains(crate::security::Capabilities::NETWORK) {
        return Err(-EACCES);
    }
    let entry = process.fds.lock().get(fd).ok_or(-EBADF)?;
    if entry.kind == FdKind::SocketTcp {
        let count = posix_tcp_receive(process, fd, pointer, requested)?;
        if source_pointer != 0 || source_length_pointer != 0 {
            let peer = crate::network::tcp_peer(entry.handle).map_err(map_tcp_error)?;
            write_ipv4_sockaddr(source_pointer, source_length_pointer,
                Ipv4SocketAddress { address: peer.address, port: peer.port })?;
        }
        return Ok(count);
    }
    if entry.kind != FdKind::SocketUdp { return Err(-ENOTSOCK); }
    let socket = entry.handle;
    let nonblocking = entry.status_flags & O_NONBLOCK != 0;
    if requested == 0 { return Ok(0); }
    let length = requested.min(MAX_UDP_PAYLOAD);
    if !vmm::is_user_range(pointer, length, true) { return Err(-EFAULT); }
    let mut buffer = [0u8; MAX_UDP_PAYLOAD];
    let mut peer = None;
    let result = syscall::wait_until(crate::network::udp_wait_queue(), if nonblocking { 0 } else { u64::MAX }, || {
        match crate::network::udp_receive(socket, &mut buffer[..length]) {
            Ok(Some((count, source))) => {
                peer = Some(Ipv4SocketAddress { address: source.address, port: source.port });
                Some(count as isize)
            }
            Ok(None) => None,
            Err(error) => Some(map_udp_error(error)),
        }
    });
    if result == -6 { return Err(-11); }
    if result < 0 { return Err(result); }
    let count = result as usize;
    output(pointer, &buffer[..count])?;
    if source_pointer != 0 || source_length_pointer != 0 {
        let peer = peer.ok_or(-EIO)?;
        write_ipv4_sockaddr(source_pointer, source_length_pointer, peer)?;
    }
    Ok(count as isize)
}

#[cfg(feature = "network-stack")]
fn posix_tcp_write(process: &Arc<Process>, fd: usize, bytes: &[u8]) -> Result<isize, isize> {
    let entry = process.fds.lock().get(fd).ok_or(-EBADF)?;
    if entry.kind != FdKind::SocketTcp { return Err(-ENOTSOCK); }
    if bytes.is_empty() { return Ok(0); }
    let nonblocking = entry.status_flags & O_NONBLOCK != 0;
    let result = syscall::wait_until(crate::network::tcp_wait_queue(), if nonblocking { 0 } else { u64::MAX }, || {
        match crate::network::tcp_send(entry.handle, bytes) {
            Ok(Some(count)) => Some(count as isize),
            Ok(None) => None,
            Err(crate::network::TcpError::WouldBlock) => None,
            Err(error) => Some(map_tcp_error(error)),
        }
    });
    if result == -6 { return Err(-EAGAIN); }
    if result < 0 { return Err(result); }
    Ok(result)
}

#[cfg(feature = "network-stack")]
fn posix_tcp_receive(process: &Arc<Process>, fd: usize, pointer: u64, requested: usize) -> Result<isize, isize> {
    let entry = process.fds.lock().get(fd).ok_or(-EBADF)?;
    if entry.kind != FdKind::SocketTcp { return Err(-ENOTSOCK); }
    if requested == 0 { return Ok(0); }
    let length = requested.min(MAX_COPY);
    if !vmm::is_user_range(pointer, length, true) { return Err(-EFAULT); }
    let nonblocking = entry.status_flags & O_NONBLOCK != 0;
    let mut buffer = [0u8; MAX_COPY];
    let result = syscall::wait_until(crate::network::tcp_wait_queue(), if nonblocking { 0 } else { u64::MAX }, || {
        match crate::network::tcp_receive(entry.handle, &mut buffer[..length]) {
            Ok(Some(count)) => Some(count as isize),
            Ok(None) => None,
            Err(error) => Some(map_tcp_error(error)),
        }
    });
    if result == -6 { return Err(-EAGAIN); }
    if result < 0 { return Err(result); }
    let count = result as usize;
    output(pointer, &buffer[..count])?;
    Ok(result)
}

#[cfg(feature = "network-stack")]
fn posix_listen(process: &Arc<Process>, fd: usize, backlog: usize) -> Result<isize, isize> {
    if !process.security.capabilities.contains(crate::security::Capabilities::NETWORK) { return Err(-EACCES); }
    if backlog == 0 { return Err(-EINVAL); }
    let entry = process.fds.lock().get(fd).ok_or(-EBADF)?;
    if entry.kind != FdKind::SocketTcp { return Err(-ENOTSOCK); }
    crate::network::tcp_listen(entry.handle).map(|_| 0).map_err(map_tcp_error)
}

#[cfg(feature = "network-stack")]
fn posix_accept(
    process: &Arc<Process>, fd: usize, address_pointer: u64, address_length_pointer: u64,
) -> Result<isize, isize> {
    if !process.security.capabilities.contains(crate::security::Capabilities::NETWORK) { return Err(-EACCES); }
    if (address_pointer == 0) != (address_length_pointer == 0) { return Err(-EFAULT); }
    let listener = process.fds.lock().get(fd).ok_or(-EBADF)?;
    if listener.kind != FdKind::SocketTcp { return Err(-ENOTSOCK); }
    let nonblocking = listener.status_flags & O_NONBLOCK != 0;
    let accepted = syscall::wait_until(crate::network::tcp_wait_queue(), if nonblocking { 0 } else { u64::MAX }, || {
        match crate::network::tcp_accept(listener.handle) {
            Ok(id) => Some(id as isize),
            Err(crate::network::TcpError::WouldBlock) => None,
            Err(error) => Some(map_tcp_error(error)),
        }
    });
    if accepted == -6 { return Err(-EAGAIN); }
    if accepted < 0 { return Err(accepted); }
    let accepted_id = accepted as u32;
    let result = allocate_descriptor(process, FdEntry {
        handle: accepted_id, kind: FdKind::SocketTcp,
        status_flags: 0, descriptor_flags: 0,
    }, 0);
    let new_fd = match result {
        Ok(value) => value,
        Err(error) => { crate::network::tcp_release(accepted_id); return Err(error); }
    };
    if address_pointer != 0 {
        let peer = crate::network::tcp_peer(accepted_id).map_err(map_tcp_error)?;
        if let Err(error) = write_ipv4_sockaddr(address_pointer, address_length_pointer,
            Ipv4SocketAddress { address: peer.address, port: peer.port }) {
            let old = process.fds.lock().close(new_fd).unwrap();
            close_entry(process, old);
            return Err(error);
        }
    }
    Ok(new_fd as isize)
}

#[cfg(feature = "network-stack")]
fn posix_network_config(process: &Arc<Process>, pointer: u64, length: usize) -> Result<isize, isize> {
    if !process.security.capabilities.contains(crate::security::Capabilities::NETWORK) {
        return Err(-EACCES);
    }
    let config = crate::network::config().ok_or(-ENETDOWN)?;
    let size = core::mem::size_of::<crate::network::NetworkConfig>();
    if length < size { return Err(-EINVAL); }
    if !vmm::is_user_range(pointer, size, true) { return Err(-EFAULT); }
    let bytes = unsafe {
        core::slice::from_raw_parts(
            (&config as *const crate::network::NetworkConfig).cast::<u8>(),
            size,
        )
    };
    vmm::copy_to_user(pointer, bytes).map_err(|_| -EFAULT)?;
    Ok(size as isize)
}

fn posix_fstat(process: &Arc<Process>, fd: usize, pointer: u64) -> Result<isize, isize> {
    let entry = process.fds.lock().get(fd).ok_or(-EBADF)?;
    let mut status = match entry.kind {
        FdKind::File => {
            let metadata = syscall::handle_metadata(entry.handle, process).map_err(|_| -EBADF)?;
            UserStat::from_metadata(metadata)
        }
        FdKind::ConsoleRead | FdKind::ConsoleWrite => UserStat {
            device: 1, inode: fd as u64 + 1, links: 1, mode: 0o020000 | 0o666,
            uid: process.uid, gid: process.gid, reserved: 0, special_device: 1,
            size: 0, block_size: 1, blocks: 0, access_seconds: 0, access_nanoseconds: 0,
            modify_seconds: 0, modify_nanoseconds: 0, change_seconds: 0, change_nanoseconds: 0,
        },
        FdKind::PipeRead | FdKind::PipeWrite => UserStat {
            device: 1, inode: entry.handle as u64, links: 1, mode: 0o010000 | 0o600,
            uid: process.uid, gid: process.gid, reserved: 0, special_device: 0,
            size: 0, block_size: 4096, blocks: 0, access_seconds: 0, access_nanoseconds: 0,
            modify_seconds: 0, modify_nanoseconds: 0, change_seconds: 0, change_nanoseconds: 0,
        },
        FdKind::SocketUdp => UserStat {
            device: 1, inode: entry.handle as u64, links: 1, mode: 0o140000 | 0o600,
            uid: process.uid, gid: process.gid, reserved: 0, special_device: 0,
            size: 0, block_size: 4096, blocks: 0, access_seconds: 0, access_nanoseconds: 0,
            modify_seconds: 0, modify_nanoseconds: 0, change_seconds: 0, change_nanoseconds: 0,
        },
        FdKind::SocketTcp => UserStat {
            device: 1, inode: entry.handle as u64, links: 1, mode: 0o140000 | 0o600,
            uid: process.uid, gid: process.gid, reserved: 0, special_device: 0,
            size: 0, block_size: 4096, blocks: 0, access_seconds: 0, access_nanoseconds: 0,
            modify_seconds: 0, modify_nanoseconds: 0, change_seconds: 0, change_nanoseconds: 0,
        },
        FdKind::Closed => return Err(-EBADF),
    };
    status.reserved = 0;
    syscall::copy_user_stat(pointer, status)
}

#[repr(C)]
#[derive(Clone, Copy)]
struct UserFdSet { bits: [u64; 2] }

impl UserFdSet {
    const fn empty() -> Self { Self { bits: [0; 2] } }
    fn contains(&self, fd: usize) -> bool { self.bits[fd / 64] & (1u64 << (fd % 64)) != 0 }
    fn insert(&mut self, fd: usize) { self.bits[fd / 64] |= 1u64 << (fd % 64); }
}

fn fd_read_ready(process: &Arc<Process>, fd: usize, entry: FdEntry) -> Result<bool, isize> {
    Ok(match entry.kind {
        FdKind::ConsoleRead => crate::drivers::tty::is_readable(process.pid),
        FdKind::File => entry.status_flags & O_ACCMODE != O_WRONLY,
        FdKind::PipeRead => process.handles.lock()
            .get(unsafe { Handle::<crate::ipc::pipe::PipeReader>::from_raw(entry.handle) }, AccessRights::READ)
            .map_err(|_| -EBADF)?.is_readable(),
        #[cfg(feature = "network-stack")]
        FdKind::SocketUdp => crate::network::udp_readable(entry.handle),
        #[cfg(feature = "network-stack")]
        FdKind::SocketTcp => crate::network::tcp_readable(entry.handle),
        #[cfg(not(feature = "network-stack"))]
        FdKind::SocketUdp | FdKind::SocketTcp => return Err(-ENOSYS),
        FdKind::ConsoleWrite | FdKind::PipeWrite => false,
        FdKind::Closed => { let _ = fd; return Err(-EBADF); }
    })
}

fn fd_write_ready(process: &Arc<Process>, fd: usize, entry: FdEntry) -> Result<bool, isize> {
    Ok(match entry.kind {
        FdKind::ConsoleWrite => true,
        FdKind::File => entry.status_flags & O_ACCMODE != O_RDONLY,
        FdKind::PipeWrite => process.handles.lock()
            .get(unsafe { Handle::<crate::ipc::pipe::PipeWriter>::from_raw(entry.handle) }, AccessRights::WRITE)
            .map_err(|_| -EBADF)?.is_writable(),
        #[cfg(feature = "network-stack")]
        FdKind::SocketUdp => crate::network::udp_writable(entry.handle),
        #[cfg(feature = "network-stack")]
        FdKind::SocketTcp => crate::network::tcp_writable(entry.handle),
        #[cfg(not(feature = "network-stack"))]
        FdKind::SocketUdp | FdKind::SocketTcp => return Err(-ENOSYS),
        FdKind::ConsoleRead | FdKind::PipeRead => false,
        FdKind::Closed => { let _ = fd; return Err(-EBADF); }
    })
}

fn posix_select(frame: &crate::arch::x86_64::idt::TrapFrame) -> Result<isize, isize> {
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct TimeVal { seconds: i64, microseconds: i64 }

    let process = current()?;
    let nfds = frame.rdi as usize;
    if nfds > FD_LIMIT { return Err(-EINVAL); }
    let mut requested_read = UserFdSet::empty();
    let mut requested_write = UserFdSet::empty();
    let mut requested_except = UserFdSet::empty();
    if frame.rsi != 0 {
        let bytes = unsafe { core::slice::from_raw_parts_mut((&mut requested_read as *mut UserFdSet).cast::<u8>(), 16) };
        vmm::copy_from_user(bytes, frame.rsi).map_err(|_| -EFAULT)?;
    }
    if frame.rdx != 0 {
        let bytes = unsafe { core::slice::from_raw_parts_mut((&mut requested_write as *mut UserFdSet).cast::<u8>(), 16) };
        vmm::copy_from_user(bytes, frame.rdx).map_err(|_| -EFAULT)?;
    }
    if frame.r10 != 0 {
        let bytes = unsafe { core::slice::from_raw_parts_mut((&mut requested_except as *mut UserFdSet).cast::<u8>(), 16) };
        vmm::copy_from_user(bytes, frame.r10).map_err(|_| -EFAULT)?;
    }
    let timeout = if frame.r8 == 0 {
        None
    } else {
        let mut value = TimeVal { seconds: 0, microseconds: 0 };
        let bytes = unsafe { core::slice::from_raw_parts_mut((&mut value as *mut TimeVal).cast::<u8>(), 16) };
        vmm::copy_from_user(bytes, frame.r8).map_err(|_| -EFAULT)?;
        if value.seconds < 0 || value.microseconds < 0 || value.microseconds >= 1_000_000 {
            return Err(-EINVAL);
        }
        let ticks = (value.seconds as u128)
            .checked_mul(100)
            .and_then(|whole| whole.checked_add((value.microseconds as u128 + 9_999) / 10_000))
            .ok_or(-EOVERFLOW)?;
        Some((ticks.min(u64::MAX as u128) as u64, value))
    };
    let start = crate::task::scheduler::tick_count();

    loop {
        let mut ready_read = UserFdSet::empty();
        let mut ready_write = UserFdSet::empty();
        let ready_except = UserFdSet::empty();
        let mut count = 0usize;
        for fd in 0..nfds {
            let wants_read = requested_read.contains(fd);
            let wants_write = requested_write.contains(fd);
            let wants_except = requested_except.contains(fd);
            if !(wants_read || wants_write || wants_except) { continue; }
            let entry = process.fds.lock().get(fd).ok_or(-EBADF)?;
            if wants_read && fd_read_ready(&process, fd, entry)? {
                ready_read.insert(fd);
                count += 1;
            }
            if wants_write && fd_write_ready(&process, fd, entry)? {
                ready_write.insert(fd);
                count += 1;
            }
        }

        let elapsed = crate::task::scheduler::tick_count().wrapping_sub(start);
        let expired = timeout.is_some_and(|(ticks, _)| ticks == 0 || elapsed >= ticks);
        if count != 0 || expired {
            for (pointer, value) in [
                (frame.rsi, ready_read),
                (frame.rdx, ready_write),
                (frame.r10, ready_except),
            ] {
                if pointer != 0 {
                    let bytes = unsafe { core::slice::from_raw_parts((&value as *const UserFdSet).cast::<u8>(), 16) };
                    vmm::copy_to_user(pointer, bytes).map_err(|_| -EFAULT)?;
                }
            }
            if let Some((ticks, original)) = timeout {
                let remaining = ticks.saturating_sub(elapsed);
                let value = TimeVal {
                    seconds: (remaining / 100) as i64,
                    microseconds: ((remaining % 100) * 10_000) as i64,
                };
                let bytes = unsafe { core::slice::from_raw_parts((&value as *const TimeVal).cast::<u8>(), 16) };
                if vmm::copy_to_user(frame.r8, bytes).is_err() { return Err(-EFAULT); }
                let _ = original;
            }
            return Ok(count as isize);
        }

        let tick_wait = crate::task::wait::WaitQueue::new();
        let result = syscall::wait_until(&tick_wait, 1, || None);
        if result < 0 && result != -7 { return Err(-EINTR); }
    }
}

pub fn dispatch(number: u64, frame: &crate::arch::x86_64::idt::TrapFrame) -> Result<isize, isize> {
    let process = current()?;
    match number {
        46 => posix_open(&process, frame),
        47 => posix_read(&process, frame.rdi as usize, frame.rsi, frame.rdx as usize),
        48 => posix_write(&process, frame.rdi as usize, frame.rsi, frame.rdx as usize),
        49 => posix_close(&process, frame.rdi as usize),
        50 => posix_dup(&process, frame.rdi as usize, 0),
        51 => posix_dup2(&process, frame.rdi as usize, frame.rsi as usize),
        52 => posix_pipe(&process, frame.rdi),
        53 => posix_fstat(&process, frame.rdi as usize, frame.rsi),
        54 => {
            let entry = process.fds.lock().get(frame.rdi as usize).ok_or(-EBADF)?;
            if entry.kind != FdKind::File { return Err(-ESPIPE); }
            if entry.status_flags & O_ACCMODE == O_WRONLY { /* write-only descriptors remain seekable */ }
            let file = process.handles.lock()
                .get(unsafe { Handle::<FileObject>::from_raw(entry.handle) }, AccessRights::NONE)
                .map_err(|_| -EBADF)?;
            file.seek(frame.rsi as i64, frame.rdx)
                .map(|position| position as isize)
                .map_err(|_| -EINVAL)
        }
        55 => {
            let fd = frame.rdi as usize;
            let command = frame.rsi;
            let argument = frame.rdx as u32;
            match command {
                F_GETFD => process.fds.lock().get(fd).map(|entry| entry.descriptor_flags as isize).ok_or(-EBADF),
                F_SETFD => {
                    if argument & !FD_CLOEXEC != 0 { return Err(-EINVAL); }
                    let mut fds = process.fds.lock();
                    let mut entry = fds.get(fd).ok_or(-EBADF)?;
                    entry.descriptor_flags = argument;
                    fds.replace(fd, entry).map_err(|_| -EBADF)?;
                    Ok(0)
                }
                F_GETFL => process.fds.lock().get(fd).map(|entry| entry.status_flags as isize).ok_or(-EBADF),
                F_SETFL => {
                    if argument & !(O_ACCMODE | O_NONBLOCK) != 0 { return Err(-ENOSYS); }
                    let mut fds = process.fds.lock();
                    let mut entry = fds.get(fd).ok_or(-EBADF)?;
                    if argument & O_ACCMODE != entry.status_flags & O_ACCMODE { return Err(-EINVAL); }
                    entry.status_flags = argument;
                    fds.replace(fd, entry).map_err(|_| -EBADF)?;
                    Ok(0)
                }
                F_DUPFD => posix_dup(&process, fd, argument as usize),
                _ => Err(-EINVAL),
            }
        }
        56 => {
            let entry = process.fds.lock().get(frame.rdi as usize).ok_or(-EBADF)?;
            match entry.kind {
                FdKind::ConsoleRead | FdKind::ConsoleWrite => Ok(1),
                _ => Err(-ENOTTY),
            }
        }
        57 => posix_unlink(&process, frame.rdi, frame.rsi as usize),
        58 => posix_tcgetattr(&process, frame.rdi as usize, frame.rsi),
        59 => posix_tcsetattr(&process, frame.rdi as usize, frame.rsi, frame.rdx),
        60 => posix_ioctl(&process, frame.rdi as usize, frame.rsi, frame.rdx),
        // umask(2): store the new mask and hand back the previous one.
        62 => Ok(process
            .file_mode_mask
            .swap(frame.rdi as u32 & 0o777, core::sync::atomic::Ordering::AcqRel)
            as isize),
        63 => posix_rename(&process, frame),
        68 => posix_clock_gettime(frame.rdi, frame.rsi),
        #[cfg(feature = "network-stack")]
        69 => posix_socket(&process, frame),
        #[cfg(feature = "network-stack")]
        70 => posix_bind(&process, frame.rdi as usize, frame.rsi, frame.rdx as usize),
        #[cfg(feature = "network-stack")]
        71 => posix_connect(&process, frame.rdi as usize, frame.rsi, frame.rdx as usize),
        #[cfg(feature = "network-stack")]
        72 => posix_send_to(&process, frame.rdi as usize, frame.rsi, frame.rdx as usize,
            frame.r10, frame.r8, frame.r9 as usize),
        #[cfg(feature = "network-stack")]
        73 => posix_receive_from(&process, frame.rdi as usize, frame.rsi, frame.rdx as usize,
            frame.r10, frame.r8, frame.r9),
        #[cfg(not(feature = "network-stack"))]
        69..=73 => Err(-ENOSYS),
        #[cfg(feature = "network-stack")]
        74 => posix_network_config(&process, frame.rdi, frame.rsi as usize),
        #[cfg(not(feature = "network-stack"))]
        74 => Err(-ENOSYS),
        #[cfg(feature = "network-stack")]
        79 => posix_listen(&process, frame.rdi as usize, frame.rsi as usize),
        #[cfg(feature = "network-stack")]
        80 => posix_accept(&process, frame.rdi as usize, frame.rsi, frame.rdx),
        #[cfg(not(feature = "network-stack"))]
        79..=80 => Err(-ENOSYS),
        81 => posix_select(frame),
        _ => Err(-ENOSYS),
    }
}
