//! CMOS real-time clock reader for the legacy x86 RTC.
//!
//! The hardware register access is kept in the driver boundary. The rest of
//! the kernel consumes UTC seconds through `crate::time`.

use crate::arch::x86_64::port::{in8, out8};
use crate::sync::SpinLock;

static CMOS_LOCK: SpinLock<()> = SpinLock::new(());

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Snapshot {
    second: u8,
    minute: u8,
    hour: u8,
    day: u8,
    month: u8,
    year: u8,
    century: u8,
    status_b: u8,
}

unsafe fn read_register(register: u8) -> u8 {
    out8(0x70, register & 0x7f);
    in8(0x71)
}

unsafe fn wait_for_update_end() -> bool {
    // Bound the wait so a broken or absent RTC cannot hang early boot.
    for _ in 0..100_000 {
        if read_register(0x0a) & 0x80 == 0 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

unsafe fn snapshot() -> Option<Snapshot> {
    if !wait_for_update_end() {
        return None;
    }
    Some(Snapshot {
        second: read_register(0x00),
        minute: read_register(0x02),
        hour: read_register(0x04),
        day: read_register(0x07),
        month: read_register(0x08),
        year: read_register(0x09),
        century: read_register(0x32),
        status_b: read_register(0x0b),
    })
}

fn decode(value: u8, binary: bool) -> Option<u8> {
    if binary {
        Some(value)
    } else {
        let low = value & 0x0f;
        let high = value >> 4;
        if low > 9 || high > 9 {
            None
        } else {
            Some(high * 10 + low)
        }
    }
}

fn leap_year(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let adjusted_year = year - i64::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let year_of_era = adjusted_year - era * 400;
    let adjusted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn convert(snapshot: Snapshot) -> Option<i64> {
    let binary = snapshot.status_b & 0x04 != 0;
    let hour_12 = snapshot.status_b & 0x02 == 0;
    let second = decode(snapshot.second, binary)?;
    let minute = decode(snapshot.minute, binary)?;
    let mut hour_raw = snapshot.hour;
    let is_pm = hour_raw & 0x80 != 0;
    hour_raw &= 0x7f;
    let mut hour = decode(hour_raw, binary)?;
    let day = decode(snapshot.day, binary)?;
    let month = decode(snapshot.month, binary)?;
    let year_low = decode(snapshot.year, binary)? as i64;
    let century = decode(snapshot.century, binary).filter(|v| (19..=99).contains(v));
    let year = century.map_or_else(
        || if year_low >= 70 { 1900 + year_low } else { 2000 + year_low },
        |century| century as i64 * 100 + year_low,
    );

    if hour_12 {
        if !(1..=12).contains(&hour) {
            return None;
        }
        hour %= 12;
        if is_pm {
            hour += 12;
        }
    }
    if second > 59 || minute > 59 || hour > 23 || day == 0 || month == 0 || month > 12 {
        return None;
    }
    let month_lengths = [31, if leap_year(year) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    if day > month_lengths[month as usize - 1] {
        return None;
    }

    Some(
        days_from_civil(year, month as i64, day as i64) * 86_400
            + hour as i64 * 3_600
            + minute as i64 * 60
            + second as i64,
    )
}

/// Read and validate the RTC as Unix UTC seconds. QEMU's reference machine
/// configuration uses UTC; platforms configured to store local RTC time must
/// supply a timezone policy above this hardware driver.
pub fn unix_seconds() -> Option<i64> {
    let _guard = CMOS_LOCK.lock_irqsave();
    for _ in 0..4 {
        let first = unsafe { snapshot()? };
        let second = unsafe { snapshot()? };
        if first == second {
            return convert(first);
        }
    }
    None
}
