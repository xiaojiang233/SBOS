//! Kernel time service: PIT-backed monotonic time plus an RTC wall-clock base.

use core::sync::atomic::{AtomicI64, Ordering};

static REALTIME_BASE: AtomicI64 = AtomicI64::new(i64::MIN);

pub fn init() {
    #[cfg(feature = "driver-rtc")]
    if let Some(seconds) = crate::drivers::rtc::unix_seconds() {
        REALTIME_BASE.store(seconds, Ordering::Release);
        crate::kprintln!("RTC UTC: {} seconds since Unix epoch", seconds);
        return;
    }
    crate::kprintln!("RTC unavailable; CLOCK_REALTIME will report ENODATA");
}

pub fn monotonic() -> (i64, i64) {
    let ticks = crate::task::scheduler::tick_count();
    ((ticks / 100) as i64, ((ticks % 100) * 10_000_000) as i64)
}

pub fn realtime() -> Option<(i64, i64)> {
    let base = REALTIME_BASE.load(Ordering::Acquire);
    if base == i64::MIN {
        return None;
    }
    let (elapsed_seconds, nanoseconds) = monotonic();
    Some((base.saturating_add(elapsed_seconds), nanoseconds))
}
