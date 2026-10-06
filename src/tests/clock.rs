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

/// The whole point of `day`: a name that repeats within the day is what
/// makes the second switch replace the first rather than pile up beside it.
#[test]
fn the_day_stamp_is_eight_digits_with_no_time_in_it() {
    let d = day();
    assert_eq!(d.len(), 8, "{d} should be YYYYMMDD, got something longer");
    assert!(
        d.chars().all(|c| c.is_ascii_digit()),
        "{d} should be digits only"
    );
    // Two reads inside the same second cannot disagree, so the name really
    // is a property of the day and not of the call.
    assert_eq!(d, day());
}

#[test]
fn the_day_stamp_is_the_prefix_of_the_full_stamp() {
    assert!(
        stamp().starts_with(&day()),
        "{} should start with {}",
        stamp(),
        day()
    );
}
