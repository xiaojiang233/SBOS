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
    let mut scheduler = SCHEDULER.lock();
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
    let mut s = SCHEDULER.lock();
    if s.len == MAX_THREADS {
        return Err("scheduler thread table full");
    }
    let index = s.len;
    s.ready[index] = Some(thread);
    s.len += 1;
    Ok(())
}

pub fn has_capacity() -> bool {
    SCHEDULER.lock().len < MAX_THREADS
}

pub fn current() -> Option<Arc<Thread>> {
    SCHEDULER.lock().current.clone()
}
pub fn tick_count() -> u64 {
    SCHEDULER.lock().ticks
}

/// Round-robin selection at a user/kernel boundary. A thread without a saved
/// interrupt frame is not runnable yet, so it cannot be selected accidentally.
pub fn on_timer(frame: *mut TrapFrame) -> *mut TrapFrame {
    let mut s = SCHEDULER.lock();
    s.ticks = s.ticks.wrapping_add(1);
    let Some(current) = s.current.clone() else {
        return frame;
    };
    if !frame.is_null() && unsafe { (*frame).cs & 3 == 3 } {
        current.account_user_tick();
    }
    current.remember_frame(frame);
    if s.len < 2 {
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
        if saved.is_null() {
            continue;
        }
        *current.state.lock() = ThreadState::Ready;
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
    frame
}

pub fn exit_current(frame: *mut TrapFrame) -> *mut TrapFrame {
    let mut s = SCHEDULER.lock();
    let Some(current) = s.current.clone() else {
        return frame;
    };
    *current.state.lock() = ThreadState::Exited;
    current.remember_frame(frame);
    for _ in 0..s.len {
        s.cursor = (s.cursor + 1) % s.len;
        let Some(candidate) = s.ready[s.cursor].clone() else {
            continue;
        };
        if *candidate.state.lock() != ThreadState::Ready {
            continue;
        }
        let saved = candidate.frame();
        if saved.is_null() {
            continue;
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
    core::ptr::null_mut()
}
