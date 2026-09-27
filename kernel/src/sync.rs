use core::arch::asm;
use core::cell::UnsafeCell;
use core::hint::spin_loop;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

/// Mutual-exclusion lock for ordinary kernel contexts. It does not mask local
/// interrupts; use `lock_irqsave` whenever an interrupt handler can access the
/// same lock to avoid self-deadlock after preempting its owner.
pub struct SpinLock<T> {
    held: AtomicBool,
    value: UnsafeCell<T>,
}
unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            held: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }
    pub fn lock(&self) -> SpinGuard<'_, T> {
        while self
            .held
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            spin_loop();
        }
        SpinGuard { lock: self }
    }
    pub fn try_lock(&self) -> Option<SpinGuard<'_, T>> {
        self.held
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| SpinGuard { lock: self })
    }

    /// Acquire this lock with local interrupts disabled, restoring the exact
    /// prior IF state after releasing the inner SpinLock.
    pub fn lock_irqsave(&self) -> IrqSpinGuard<'_, T> {
        let flags: u64;
        unsafe {
            // Keep save+CLI adjacent: an interrupt between PUSHFQ and CLI
            // returns before the lock is acquired and is therefore harmless.
            asm!("pushfq", "pop {flags}", "cli", flags = out(reg) flags);
        }
        IrqSpinGuard {
            guard: Some(self.lock()),
            restore_interrupts: flags & (1 << 9) != 0,
        }
    }
}

pub struct SpinGuard<'a, T> {
    lock: &'a SpinLock<T>,
}
impl<T> Deref for SpinGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.lock.value.get() }
    }
}
impl<T> DerefMut for SpinGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.value.get() }
    }
}
impl<T> Drop for SpinGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.held.store(false, Ordering::Release);
    }
}

pub struct IrqSpinGuard<'a, T> {
    guard: Option<SpinGuard<'a, T>>,
    restore_interrupts: bool,
}

impl<T> Deref for IrqSpinGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T { self.guard.as_ref().expect("released IRQ guard") }
}

impl<T> DerefMut for IrqSpinGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T { self.guard.as_mut().expect("released IRQ guard") }
}

impl<T> Drop for IrqSpinGuard<'_, T> {
    fn drop(&mut self) {
        drop(self.guard.take());
        if self.restore_interrupts {
            unsafe { asm!("sti", options(nomem, nostack)); }
        }
    }
}
