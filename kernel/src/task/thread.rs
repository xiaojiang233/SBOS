use crate::arch::x86_64::idt::TrapFrame;
use crate::object::{KernelObject, ObjectHeader, ObjectType};
use crate::sync::SpinLock;
use alloc::sync::Arc;
use core::any::Any;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThreadState {
    Ready,
    Running,
    Blocked,
    Exited,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CpuContext {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rbp: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub rip: u64,
    pub rsp: u64,
    pub rflags: u64,
}

pub struct Thread {
    header: ObjectHeader,
    pub tid: u64,
    pub process_id: u64,
    pub priority: u8,
    pub kernel_stack_top: u64,
    pub user_stack: AtomicU64,
    pub context: SpinLock<CpuContext>,
    pub state: SpinLock<ThreadState>,
    pub saved_frame: AtomicUsize,
    wake_deadline: AtomicU64,
    user_ticks: AtomicU64,
}
impl Thread {
    pub fn new(tid: u64, process_id: u64, kernel_stack_top: u64, user_stack: u64) -> Self {
        Self {
            header: ObjectHeader::new(ObjectType::Thread),
            tid,
            process_id,
            priority: 16,
            kernel_stack_top,
            user_stack: AtomicU64::new(user_stack),
            context: SpinLock::new(CpuContext {
                rsp: user_stack,
                ..CpuContext::default()
            }),
            state: SpinLock::new(ThreadState::Ready),
            saved_frame: AtomicUsize::new(0),
            wake_deadline: AtomicU64::new(0),
            user_ticks: AtomicU64::new(0),
        }
    }
    pub fn remember_frame(&self, frame: *mut TrapFrame) {
        if !frame.is_null() {
            let trap = unsafe { &*frame };
            *self.context.lock() = CpuContext {
                r15: trap.r15,
                r14: trap.r14,
                r13: trap.r13,
                r12: trap.r12,
                r11: trap.r11,
                r10: trap.r10,
                r9: trap.r9,
                r8: trap.r8,
                rdi: trap.rdi,
                rsi: trap.rsi,
                rbp: trap.rbp,
                rdx: trap.rdx,
                rcx: trap.rcx,
                rbx: trap.rbx,
                rax: trap.rax,
                rip: trap.rip,
                rsp: if trap.cs & 3 == 3 {
                    trap.rsp
                } else {
                    self.user_stack.load(Ordering::Acquire)
                },
                rflags: trap.rflags,
            };
        }
        self.saved_frame.store(frame as usize, Ordering::Release);
    }
    pub fn frame(&self) -> *mut TrapFrame {
        self.saved_frame.load(Ordering::Acquire) as *mut TrapFrame
    }
    pub fn account_user_tick(&self) {
        self.user_ticks.fetch_add(1, Ordering::Relaxed);
    }
    pub fn user_ticks(&self) -> u64 {
        self.user_ticks.load(Ordering::Relaxed)
    }
    pub fn set_wake_deadline(&self, deadline: u64) {
        self.wake_deadline.store(deadline, Ordering::Release);
    }
    pub fn wake_deadline(&self) -> u64 {
        self.wake_deadline.load(Ordering::Acquire)
    }
    pub fn set_user_stack(&self, address: u64) {
        self.user_stack.store(address, Ordering::Release);
    }
    pub unsafe fn seed_frame(&self, frame: TrapFrame) {
        let pointer = self.kernel_stack_top as *mut u8 as usize - core::mem::size_of::<TrapFrame>();
        (pointer as *mut TrapFrame).write(frame);
        self.remember_frame(pointer as *mut TrapFrame);
    }
}
impl KernelObject for Thread {
    fn header(&self) -> &ObjectHeader {
        &self.header
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

static NEXT_TID: AtomicU64 = AtomicU64::new(1);
pub fn new_id() -> u64 {
    NEXT_TID.fetch_add(1, Ordering::Relaxed)
}
pub fn create(process_id: u64, kernel_stack_top: u64, user_stack: u64) -> Arc<Thread> {
    Arc::new(Thread::new(
        new_id(),
        process_id,
        kernel_stack_top,
        user_stack,
    ))
}
