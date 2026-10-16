//! Text shown by the countdown and the target time.

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub min: u32,
    pub sec: u32,
}

/// Time left, rounded up to the display unit so it never reads zero while sleeping.
pub fn remaining(left: Duration, tenths: bool) -> String {
    let unit = unit_nanos(tenths);
    clock(left.as_nanos().div_ceil(unit), tenths)
}

/// Time gone for infinite sleeps, rounded down.
pub fn elapsed(gone: Duration, tenths: bool) -> String {
    format!("elapsed {}", clock(gone.as_nanos() / unit_nanos(tenths), tenths))
}

/// End time; the date is included only when it is not today.
pub fn target(end: &LocalTime, today: &LocalTime) -> String {
    let time = format!("{:02}:{:02}:{:02}", end.hour, end.min, end.sec);
    if (end.year, end.month, end.day) == (today.year, today.month, today.day) {
        time
    } else {
        format!("{:04}-{:02}-{:02} {time}", end.year, end.month, end.day)
    }
}

fn unit_nanos(tenths: bool) -> u128 {
    if tenths { 100_000_000 } else { 1_000_000_000 }
}

/// Formats a count of display units (seconds or tenths of a second).
fn clock(units: u128, tenths: bool) -> String {
    let (secs, frac) = if tenths { (units / 10, Some(units % 10)) } else { (units, None) };
    let (days, hours, mins, secs) = (secs / 86400, secs / 3600 % 24, secs / 60 % 60, secs % 60);
    let mut text = if days > 0 {
        format!("{days}d {hours:02}:{mins:02}:{secs:02}")
    } else if hours > 0 {
        format!("{hours}:{mins:02}:{secs:02}")
    } else {
        format!("{mins:02}:{secs:02}")
    };
    if let Some(frac) = frac {
        text.push_str(&format!(".{frac}"));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(secs: f64) -> Duration {
        Duration::from_secs_f64(secs)
    }

    #[test]
    fn remaining_table() {
        assert_eq!(remaining(s(7.0), false), "00:07");
        assert_eq!(remaining(s(6.2), false), "00:07");
        assert_eq!(remaining(s(272.0), false), "04:32");
        assert_eq!(remaining(s(3735.0), false), "1:02:15");
        assert_eq!(remaining(s((2 * 86400 + 3 * 3600 + 4 * 60 + 5) as f64), false), "2d 03:04:05");
        assert_eq!(remaining(s(272.41), true), "04:32.5");
        assert_eq!(remaining(s(0.01), false), "00:01");
    }

    #[test]
    fn remaining_exact_tenths_not_bumped() {
        assert_eq!(remaining(Duration::from_millis(1000), true), "00:01.0");
        assert_eq!(remaining(Duration::from_millis(500), true), "00:00.5");
    }

    #[test]
    fn elapsed_rounds_down() {
        assert_eq!(elapsed(s(12.9), false), "elapsed 00:12");
        assert_eq!(elapsed(Duration::ZERO, false), "elapsed 00:00");
        assert_eq!(elapsed(s(3.47), true), "elapsed 00:03.4");
    }

    fn at(year: i32, month: u32, day: u32, hour: u32, min: u32, sec: u32) -> LocalTime {
        LocalTime { year, month, day, hour, min, sec }
    }

    #[test]
    fn target_same_day() {
        assert_eq!(target(&at(2026, 10, 16, 14, 5, 12), &at(2026, 10, 16, 9, 0, 0)), "14:05:12");
    }

    #[test]
    fn target_other_day() {
        assert_eq!(
            target(&at(2026, 10, 17, 3, 0, 0), &at(2026, 10, 16, 23, 0, 0)),
            "2026-10-17 03:00:00"
        );
    }
}
