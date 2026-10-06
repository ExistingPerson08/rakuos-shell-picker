//! UTC timestamps, computed without GLib.
//!
//! This exists because the obvious way to spell a UTC timestamp in a GTK
//! application is `glib::DateTime::now_utc()`, and using it off the main thread
//! froze the window: GLib and GObject may only be touched from the thread that
//! owns the main context, and a switch runs on a worker thread. The failure was
//! not a panic, it was a window that stopped responding and could not be closed,
//! which is a far worse way to find out.
//!
//! So the clock here is `std::time` and integer arithmetic. It has no GTK
//! dependency, which means it is safe to call from any thread, and it is
//! testable without a display, which the GLib version was not.

use std::time::{SystemTime, UNIX_EPOCH};

/// A broken-down UTC timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Utc {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub millis: u32,
}

/// Split seconds since the Unix epoch into a UTC date and time.
///
/// The calendar conversion is the standard one: shift the epoch to 0000-03-01 so
/// leap days land at the end of the year, then count 400-year eras, 100-year
/// centuries, 4-year years and the day within the year. Proleptic Gregorian, so
/// it agrees with `date` for every input rather than only for dates after 1970.
pub fn from_epoch(seconds: i64, millis: u32) -> Utc {
    let days = seconds.div_euclid(86_400);
    let rem = seconds.rem_euclid(86_400);

    // Days from 0000-03-01 to the Unix epoch.
    const SHIFT: i64 = 719_468;
    let z = days + SHIFT;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    if month <= 2 {
        year += 1;
    }

    Utc {
        year,
        month: month as u32,
        day: day as u32,
        hour: (rem / 3_600) as u32,
        minute: (rem % 3_600 / 60) as u32,
        second: (rem % 60) as u32,
        millis,
    }
}

fn now() -> Utc {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => from_epoch(d.as_secs() as i64, d.subsec_millis()),
        // Before 1970. Not reachable in practice, but the fallback is a real
        // date rather than a panic, because a wrong clock should not be the
        // thing that stops somebody recovering their configuration.
        Err(e) => {
            let d = e.duration();
            from_epoch(-(d.as_secs() as i64), 0)
        }
    }
}

/// `20261003`, for a backup directory name that holds one per day.
///
/// The clock deliberately stops at the date. A second switch on the same day
/// produces the same name, so it replaces the copy taken earlier instead of
/// adding another directory: a day of changing your mind leaves one backup, not
/// one per click. Hours and minutes would defeat that by making every name
/// unique again, which is what the three-directory day was.
///
/// Only the backup uses this. Rollback copies keep a full timestamp because
/// several of them are written in one switch and they must not collide.
pub fn day() -> String {
    let t = now();
    format!("{:04}{:02}{:02}", t.year, t.month, t.day)
}

/// `20261003-023944`, for a backup directory name.
pub fn stamp() -> String {
    let t = now();
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        t.year, t.month, t.day, t.hour, t.minute, t.second
    )
}

/// `20261003-023944-127`, for a backup directory name.
///
/// The milliseconds are not decoration. Two switches inside one second used to
/// produce the same directory name, and the second one was refused because the
/// directory already existed, which reads to the user as a broken backup rather
/// than as the collision it was.
pub fn stamp_with_millis() -> String {
    let t = now();
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}-{:03}",
        t.year, t.month, t.day, t.hour, t.minute, t.second, t.millis
    )
}

/// `2026-10-03T02:39:44Z`, for the log.
pub fn rfc3339() -> String {
    let t = now();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        t.year, t.month, t.day, t.hour, t.minute, t.second
    )
}

#[cfg(test)]
#[path = "tests/clock.rs"]
mod tests;
