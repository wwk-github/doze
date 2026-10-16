//! Locale queries through libc.

use std::ffi::CStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::format::LocalTime;

pub fn init() {
    unsafe { libc::setlocale(libc::LC_ALL, c"".as_ptr()) };
}

/// Decimal separator of the current LC_NUMERIC.
pub fn decimal_sep() -> char {
    let point = unsafe { (*libc::localeconv()).decimal_point };
    if point.is_null() {
        return '.';
    }
    let text = unsafe { CStr::from_ptr(point) }.to_string_lossy();
    text.chars().next().unwrap_or('.')
}

/// Whether the current character set is UTF-8.
pub fn is_utf8() -> bool {
    let codeset = unsafe { libc::nl_langinfo(libc::CODESET) };
    if codeset.is_null() {
        return false;
    }
    let name = unsafe { CStr::from_ptr(codeset) }.to_string_lossy().to_ascii_lowercase();
    name == "utf-8" || name == "utf8"
}

/// Local wall-clock time `offset` from now.
pub fn now_local(offset: Duration) -> LocalTime {
    let at = SystemTime::now()
        .checked_add(offset)
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    let secs = libc::time_t::try_from(at).unwrap_or(libc::time_t::MAX);
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&secs, &mut tm) };
    LocalTime {
        year: tm.tm_year + 1900,
        month: (tm.tm_mon + 1) as u32,
        day: tm.tm_mday as u32,
        hour: tm.tm_hour as u32,
        min: tm.tm_min as u32,
        sec: tm.tm_sec as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_locale_is_not_utf8() {
        unsafe { libc::setlocale(libc::LC_ALL, c"C".as_ptr()) };
        assert!(!is_utf8());
        assert_eq!(decimal_sep(), '.');
    }

    #[test]
    fn now_local_is_plausible() {
        let t = now_local(Duration::ZERO);
        assert!(t.year >= 2026 && (1..=12).contains(&t.month) && (1..=31).contains(&t.day));
        assert!(t.hour < 24 && t.min < 60 && t.sec < 61);
    }
}
