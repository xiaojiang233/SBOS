use crate::handle::HandleTable;
use crate::object::{KernelObject, ObjectHeader, ObjectType};
use crate::security::SecurityContext;
use crate::sync::SpinLock;
use crate::task::thread::Thread;
use crate::task::fd::FdTable;
use alloc::string::String;
use alloc::sync::Arc;
use core::any::Any;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, Ordering};

pub struct AddressSpace {
    root: AtomicU64,
    regions: SpinLock<alloc::vec::Vec<(u64, u64, u8)>>,
    mmap_cursor: AtomicU64,
}
impl AddressSpace {
    pub fn new(root: u64) -> Self {
        Self {
            root: AtomicU64::new(root),
            regions: SpinLock::new(alloc::vec::Vec::new()),
            mmap_cursor: AtomicU64::new(0),
        }
    }
    pub fn record(&self, start: u64, length: u64, flags: u8) {
        self.regions.lock().push((start, length, flags));
    }
    pub fn fork_with_root(&self, root: u64) -> Self {
        Self {
            root: AtomicU64::new(root),
            regions: SpinLock::new(self.regions.lock().clone()),
            mmap_cursor: AtomicU64::new(self.mmap_cursor.load(Ordering::Acquire)),
        }
    }
    pub fn root(&self) -> u64 {
        self.root.load(Ordering::Acquire)
    }
    pub fn replace_with(&self, next: &AddressSpace) -> u64 {
        let previous = self.root.swap(next.root(), Ordering::AcqRel);
        *self.regions.lock() = next.regions.lock().clone();
        self.mmap_cursor.store(next.mmap_cursor.load(Ordering::Acquire), Ordering::Release);
        previous
    }
    pub fn mapping_start(
        &self,
        requested: u64,
        length: u64,
        base: u64,
        limit: u64,
    ) -> Result<u64, &'static str> {
        if length == 0 || length % 4096 != 0 {
            return Err("invalid mapping size");
        }
        let regions = self.regions.lock();
        let overlaps = |start: u64| {
            regions.iter().any(|(other, size, _)| {
                start < other.saturating_add(*size) && *other < start.saturating_add(length)
            })
        };
        if requested != 0 {
            if requested % 4096 != 0
                || requested < base
                || requested.saturating_add(length) > limit
                || overlaps(requested)
            {
                return Err("mapping overlaps or is outside the user mmap range");
            }
            return Ok(requested);
        }
        let mut candidate = self.mmap_cursor.load(Ordering::Acquire).max(base);
        loop {
            if candidate.saturating_add(length) > limit {
                return Err("user mmap range exhausted");
            }
            if !overlaps(candidate) {
                self.mmap_cursor
                    .store(candidate + length + 4096, Ordering::Release);
                return Ok(candidate);
            }
            candidate = candidate.saturating_add(4096);
        }
    }
    pub fn forget(&self, start: u64, length: u64) {
        self.regions
            .lock()
            .retain(|(address, size, _)| *address != start || *size != length);
    }
}

pub struct Process {
    header: ObjectHeader,
    pub pid: u64,
    pub parent_pid: u64,
    pub address_space: AddressSpace,
    pub handles: SpinLock<HandleTable>,
    pub fds: SpinLock<FdTable>,
    pub security: SecurityContext,
    pub uid: u32,
    pub gid: u32,
    pub current_directory: SpinLock<String>,
    pub threads: SpinLock<alloc::vec::Vec<Arc<Thread>>>,
    pub exit_status: AtomicI32,
    exited: AtomicBool,
    reaped: AtomicBool,
    pub executable: SpinLock<String>,
    /// Process file creation mask, as reported by umask(2). SBFS takes new file
    /// permissions from the parent ACL, so this value records what programs ask
    /// for without further restricting file creation.
    pub file_mode_mask: AtomicU32,
    terminal_restore: SpinLock<Option<crate::drivers::tty::TerminalAttributes>>,
}
impl Process {
    pub fn new(
        pid: u64,
        parent_pid: u64,
        executable: &str,
        address_space: AddressSpace,
        security: SecurityContext,
        uid: u32,
        gid: u32,
    ) -> Self {
        Self {
            header: ObjectHeader::new(ObjectType::Process),
            pid,
            parent_pid,
            address_space,
            handles: SpinLock::new(HandleTable::new()),
            fds: SpinLock::new(FdTable::new()),
            security,
            uid,
            gid,
            current_directory: SpinLock::new(String::from("/")),
            threads: SpinLock::new(alloc::vec::Vec::new()),
            exit_status: AtomicI32::new(i32::MIN),
            exited: AtomicBool::new(false),
            reaped: AtomicBool::new(false),
            executable: SpinLock::new(String::from(executable)),
            file_mode_mask: AtomicU32::new(0o022),
            terminal_restore: SpinLock::new(None),
        }
    }
    pub fn cwd(&self) -> String {
        self.current_directory.lock().clone()
    }
    pub fn executable(&self) -> String {
        self.executable.lock().clone()
    }
    pub fn set_executable(&self, path: &str) {
        *self.executable.lock() = String::from(path);
    }
    pub fn set_terminal_restore(&self, attributes: crate::drivers::tty::TerminalAttributes) {
        *self.terminal_restore.lock() = Some(attributes);
    }
    pub fn chdir(&self, path: String) {
        *self.current_directory.lock() = path;
    }
    pub fn terminate(&self, status: i32) {
        self.exit_status.store(status, Ordering::Release);
        self.exited.store(true, Ordering::Release);
        if crate::drivers::tty::is_foreground(self.pid) {
            if let Some(attributes) = self.terminal_restore.lock().take() {
                let _ = crate::drivers::tty::set_attributes(attributes, 0);
            }
            crate::drivers::tty::set_foreground(self.parent_pid);
        }
        self.fds.lock().clear();
        self.handles.lock().clear();
    }
    pub fn has_exited(&self) -> bool {
        self.exited.load(Ordering::Acquire)
    }
    pub fn has_been_reaped(&self) -> bool {
        self.reaped.load(Ordering::Acquire)
    }
    pub fn reap_exit_status(&self) -> Option<i32> {
        if !self.has_exited() {
            return None;
        }
        self.reaped
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| self.exit_status.load(Ordering::Acquire))
    }
    pub fn user_ticks(&self) -> u64 {
        self.threads
            .lock()
            .iter()
            .fold(0u64, |sum, thread| sum.wrapping_add(thread.user_ticks()))
    }
}
impl KernelObject for Process {
    fn header(&self) -> &ObjectHeader {
        &self.header
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

static NEXT_PID: AtomicU64 = AtomicU64::new(1);
static CURRENT: SpinLock<Option<Arc<Process>>> = SpinLock::new(None);
static PROCESSES: SpinLock<alloc::vec::Vec<Arc<Process>>> = SpinLock::new(alloc::vec::Vec::new());
pub fn init(root: u64, executable: &str) -> Arc<Process> {
    PROCESSES.lock().clear();
    let (uid, gid) = crate::user::default_credentials();
    let process = create_with_identity(root, executable, SecurityContext::for_uid(uid), 0, uid, gid);
    if let Some(account) = crate::user::account_by_uid(uid) {
        process.chdir(account.home);
    }
    *CURRENT.lock() = Some(process.clone());
    process
}
pub fn create(
    root: u64,
    executable: &str,
    security: SecurityContext,
    parent_pid: u64,
) -> Arc<Process> {
    let (uid, gid) = if parent_pid != 0 {
        by_pid(parent_pid)
            .map(|parent| (parent.uid, parent.gid))
            .unwrap_or_else(crate::user::default_credentials)
    } else {
        crate::user::default_credentials()
    };
    create_with_identity(root, executable, security, parent_pid, uid, gid)
}

pub fn create_with_identity(
    root: u64,
    executable: &str,
    security: SecurityContext,
    parent_pid: u64,
    uid: u32,
    gid: u32,
) -> Arc<Process> {
    let process = Arc::new(Process::new(
        NEXT_PID.fetch_add(1, Ordering::Relaxed),
        parent_pid,
        executable,
        AddressSpace::new(root),
        security,
        uid,
        gid,
    ));
    PROCESSES.lock().push(process.clone());
    process
}
pub fn fork_process(parent: &Arc<Process>, root: u64) -> Arc<Process> {
    let process = Arc::new(Process::new(
        NEXT_PID.fetch_add(1, Ordering::Relaxed),
        parent.pid,
        &parent.executable(),
        parent.address_space.fork_with_root(root),
        parent.security,
        parent.uid,
        parent.gid,
    ));
    process.chdir(parent.cwd());
    let inherited_handles = parent.handles.lock().clone();
    *process.handles.lock() = inherited_handles;
    *process.fds.lock() = parent.fds.lock().clone();
    PROCESSES.lock().push(process.clone());
    process
}
pub fn by_pid(pid: u64) -> Option<Arc<Process>> {
    PROCESSES.lock().iter().find(|p| p.pid == pid).cloned()
}
pub fn current() -> Option<Arc<Process>> {
    let pid = super::scheduler::current().map(|t| t.process_id);
    if let Some(pid) = pid {
        return by_pid(pid);
    }
    CURRENT.lock().as_ref().cloned()
}
pub fn pid() -> u64 {
    current().map(|p| p.pid).unwrap_or(0)
}
pub fn all() -> alloc::vec::Vec<Arc<Process>> {
    PROCESSES.lock().clone()
}
pub fn children_of(parent_pid: u64) -> alloc::vec::Vec<Arc<Process>> {
    PROCESSES
        .lock()
        .iter()
        .filter(|process| process.parent_pid == parent_pid && !process.has_been_reaped())
        .cloned()
        .collect()
}
