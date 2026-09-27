//! Small bounded-kernel wait queue. Waiters are TIDs; scheduler state and
//! deadlines remain owned by the scheduler, while resource owners choose which
//! queue to wake when their state changes.

use crate::sync::SpinLock;
use alloc::vec::Vec;

pub struct WaitQueue {
    waiters: SpinLock<Vec<u64>>,
}

impl WaitQueue {
    pub const fn new() -> Self {
        Self { waiters: SpinLock::new(Vec::new()) }
    }

    pub fn register(&self, tid: u64) {
        let mut waiters = self.waiters.lock();
        if !waiters.contains(&tid) {
            waiters.push(tid);
        }
    }

    pub fn remove(&self, tid: u64) {
        self.waiters.lock().retain(|waiter| *waiter != tid);
    }

    pub fn wake_one(&self) -> bool {
        let tid = self.waiters.lock().pop();
        tid.map(crate::task::scheduler::wake_thread).unwrap_or(false)
    }

    pub fn wake_all(&self) {
        let waiters = {
            let mut queue = self.waiters.lock();
            core::mem::take(&mut *queue)
        };
        for tid in waiters {
            crate::task::scheduler::wake_thread(tid);
        }
    }
}
