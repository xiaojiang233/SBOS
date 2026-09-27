use crate::arch::x86_64::gdt;
use crate::arch::x86_64::idt::TrapFrame;
use crate::sync::SpinLock;
use crate::task::thread::{Thread, ThreadState};
use alloc::sync::Arc;

const MAX_THREADS: usize = 32;
pub struct Scheduler {
    ready: [Option<Arc<Thread>>; MAX_THREADS],
    len: usize,
    cursor: usize,
    current: Option<Arc<Thread>>,
    ticks: u64,
}
impl Scheduler {
    const fn new() -> Self {
        Self {
            ready: [const { None }; MAX_THREADS],
            len: 0,
            cursor: 0,
            current: None,
            ticks: 0,
        }
    }
}
static SCHEDULER: SpinLock<Scheduler> = SpinLock::new(Scheduler::new());

pub fn initialize(initial: Arc<Thread>) -> Result<(), &'static str> {
    let mut scheduler = SCHEDULER.lock_irqsave();
    if scheduler.len == MAX_THREADS {
        return Err("scheduler thread table full");
    }
    *initial.state.lock() = ThreadState::Running;
    scheduler.ready[0] = Some(initial.clone());
    scheduler.len = 1;
    scheduler.cursor = 0;
    scheduler.current = Some(initial);
    Ok(())
}

pub fn add(thread: Arc<Thread>) -> Result<(), &'static str> {
    let mut s = SCHEDULER.lock_irqsave();
    if s.len == MAX_THREADS {
        return Err("scheduler thread table full");
    }
    let index = s.len;
    s.ready[index] = Some(thread);
    s.len += 1;
    Ok(())
}

pub fn has_capacity() -> bool {
    SCHEDULER.lock_irqsave().len < MAX_THREADS
}

pub fn current() -> Option<Arc<Thread>> {
    SCHEDULER.lock_irqsave().current.clone()
}
pub fn tick_count() -> u64 {
    SCHEDULER.lock_irqsave().ticks
}

/// Put the current thread to sleep until a WaitQueue wakes it or its deadline
/// expires. `u64::MAX` means no deadline; zero is handled by callers as a poll.
pub fn block_current(timeout_ticks: u64) -> Result<u64, &'static str> {
    let scheduler = SCHEDULER.lock_irqsave();
    let current = scheduler.current.clone().ok_or("no current thread")?;
    let deadline = if timeout_ticks == u64::MAX {
        0
    } else {
        scheduler.ticks.wrapping_add(timeout_ticks).max(1)
    };
    current.set_wake_deadline(deadline);
    *current.state.lock() = ThreadState::Blocked;
    Ok(current.tid)
}

pub fn wake_thread(tid: u64) -> bool {
    let scheduler = SCHEDULER.lock_irqsave();
    for thread in scheduler.ready[..scheduler.len].iter().flatten() {
        if thread.tid == tid && *thread.state.lock() == ThreadState::Blocked {
            thread.set_wake_deadline(0);
            *thread.state.lock() = ThreadState::Ready;
            return true;
        }
    }
    false
}

/// Round-robin selection at a user/kernel boundary. A thread without a saved
/// interrupt frame is not runnable yet, so it cannot be selected accidentally.
pub fn on_timer(frame: *mut TrapFrame) -> *mut TrapFrame {
    let mut s = SCHEDULER.lock_irqsave();
    s.ticks = s.ticks.wrapping_add(1);
    let now = s.ticks;
    for thread in s.ready[..s.len].iter().flatten() {
        let deadline = thread.wake_deadline();
        if deadline != 0
            && now.wrapping_sub(deadline) < (1u64 << 63)
            && *thread.state.lock() == ThreadState::Blocked
        {
            thread.set_wake_deadline(0);
            *thread.state.lock() = ThreadState::Ready;
        }
    }
    let Some(current) = s.current.clone() else { return schedule_ready(&mut s, frame) };
    if !frame.is_null() && unsafe { (*frame).cs & 3 == 3 } {
        current.account_user_tick();
    }
    current.remember_frame(frame);
    if *current.state.lock() == ThreadState::Blocked {
        s.current = None;
        return schedule_ready(&mut s, crate::arch::x86_64::idt::idle_frame());
    }
    if s.len < 2 {
        *current.state.lock() = ThreadState::Running;
        return frame;
    }
    for _ in 0..s.len {
        s.cursor = (s.cursor + 1) % s.len;
        let Some(candidate) = s.ready[s.cursor].clone() else {
            continue;
        };
        if candidate.tid == current.tid || *candidate.state.lock() != ThreadState::Ready {
            continue;
        }
        let saved = candidate.frame();
        if saved.is_null() { continue; }
        if *current.state.lock() == ThreadState::Running {
            *current.state.lock() = ThreadState::Ready;
        }
        *candidate.state.lock() = ThreadState::Running;
        s.current = Some(candidate.clone());
        unsafe {
            gdt::set_ring0_stack(candidate.kernel_stack_top);
        }
        if let Some(process) = crate::task::process::by_pid(candidate.process_id) {
            let _ = crate::memory::vmm::switch_root(process.address_space.root());
        }
        return saved;
    }
    if *current.state.lock() == ThreadState::Ready {
        *current.state.lock() = ThreadState::Running;
    }
    frame
}

pub fn exit_current(frame: *mut TrapFrame) -> *mut TrapFrame {
    let Some(current) = current() else { return frame };
    exit_process(current.process_id, frame)
}

/// Stop every thread belonging to an exited process, remove them from the
/// fixed scheduler table, and restore the next saved runnable context. A
/// dedicated ring-0 idle frame is returned when no thread is Ready.
pub fn exit_process(process_id: u64, frame: *mut TrapFrame) -> *mut TrapFrame {
    let mut s = SCHEDULER.lock_irqsave();
    if let Some(current) = s.current.as_ref() {
        if current.process_id == process_id {
            current.remember_frame(frame);
        }
    }
    let used_len = s.len;
    for slot in &mut s.ready[..used_len] {
        if let Some(thread) = slot.as_ref() {
            if thread.process_id == process_id {
                *thread.state.lock() = ThreadState::Exited;
            }
        }
    }
    let mut compacted = [const { None }; MAX_THREADS];
    let mut len = 0;
    for slot in &mut s.ready[..used_len] {
        if let Some(thread) = slot.take() {
            if thread.process_id != process_id && *thread.state.lock() != ThreadState::Exited {
                compacted[len] = Some(thread);
                len += 1;
            }
        }
    }
    s.ready = compacted;
    s.len = len;
    s.cursor = 0;
    s.current = None;
    let next = schedule_ready(&mut s, core::ptr::null_mut());
    if next.is_null() {
        crate::arch::x86_64::idt::idle_frame()
    } else {
        next
    }
}

/// Select a runnable saved context. When invoked by an idle timer trap, `frame`
/// remains the valid idle context if nothing has been woken yet.
fn schedule_ready(s: &mut Scheduler, frame: *mut TrapFrame) -> *mut TrapFrame {
    for index in 0..s.len {
        let Some(candidate) = s.ready[index].clone() else { continue };
        if *candidate.state.lock() != ThreadState::Ready { continue; }
        let saved = candidate.frame();
        if saved.is_null() { continue; }
        *candidate.state.lock() = ThreadState::Running;
        s.cursor = index;
        s.current = Some(candidate.clone());
        unsafe { gdt::set_ring0_stack(candidate.kernel_stack_top); }
        if let Some(process) = crate::task::process::by_pid(candidate.process_id) {
            let _ = crate::memory::vmm::switch_root(process.address_space.root());
        }
        return saved;
    }
    if s.current.is_none() {
        let kernel_root = crate::memory::vmm::kernel_root();
        if kernel_root != 0 {
            let _ = crate::memory::vmm::switch_root(kernel_root);
        }
    }
    frame
}
