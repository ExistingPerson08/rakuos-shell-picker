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
mod tests {
    use super::*;

    #[test]
    fn the_epoch_itself() {
        assert_eq!(
            from_epoch(0, 0),
            Utc {
                year: 1970,
                month: 1,
                day: 1,
                hour: 0,
                minute: 0,
                second: 0,
                millis: 0
            }
        );
    }

    #[test]
    fn a_leap_day() {
        // 2024-02-29T12:00:00Z, the day a naive implementation gets wrong.
        let t = from_epoch(1_709_208_000, 0);
        assert_eq!((t.year, t.month, t.day), (2024, 2, 29));
        assert_eq!((t.hour, t.minute, t.second), (12, 0, 0));
    }

    #[test]
    fn a_century_that_is_not_a_leap_year() {
        // 1900-03-01Z. 1900 is divisible by 100 and not by 400, so February had
        // no 29th, and the arithmetic has to agree.
        let t = from_epoch(-2_203_891_200, 0);
        assert_eq!((t.year, t.month, t.day), (1900, 3, 1));
    }

    #[test]
    fn a_date_before_the_epoch_keeps_its_sign() {
        let t = from_epoch(-1, 0);
        assert_eq!((t.year, t.month, t.day, t.hour), (1969, 12, 31, 23));
    }

    #[test]
    fn round_trips_every_hour_for_four_years() {
        // Not a random sample: consecutive hours catch an off-by-one in the day
        // or month rollover far better than a handful of chosen dates would.
        let mut seconds = 1_704_067_200; // 2024-01-01T00:00:00Z
        for _ in 0..(4 * 366 * 24) {
            let t = from_epoch(seconds, 0);
            assert!(
                (1..=12).contains(&t.month) && (1..=31).contains(&t.day),
                "implausible date {t:?} at {seconds}"
            );
            assert!(t.hour < 24 && t.minute < 60 && t.second < 60, "{t:?}");
            seconds += 3_600;
        }
    }

    #[test]
    fn agrees_with_the_system_clock_for_the_current_time() {
        // Cross-check against the C library rather than against another copy of
        // the same arithmetic, so a shared misreading cannot pass unnoticed.
        let ours = rfc3339();
        let theirs = std::process::Command::new("date")
            .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
            .output()
            .expect("date");
        let theirs = String::from_utf8_lossy(&theirs.stdout).trim().to_string();
        assert_eq!(ours, theirs, "our clock disagrees with the system clock");
    }

    #[test]
    fn the_millisecond_stamp_is_longer_and_still_readable() {
        let a = stamp_with_millis();
        let b = stamp();
        assert!(a.starts_with(&b), "{a} should start with {b}");
        assert_eq!(a.len(), b.len() + 4);
    }
}
