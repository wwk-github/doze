//! The sleep itself, optionally drawing dots or a countdown.

use std::thread;
use std::time::{Duration, Instant};

use crate::cli::{Mode, Settings};
use crate::duration::Span;
use crate::format;
use crate::locale;
use crate::output::Sink;
use crate::signals::{self, Cleanup};

const CHUNK: Duration = Duration::from_secs(86400);

/// Sleeps until `deadline`, or forever when there is none.
fn sleep_until(deadline: Option<Instant>) {
    loop {
        let wait = match deadline {
            None => CHUNK,
            Some(at) => match at.checked_duration_since(Instant::now()) {
                Some(left) if !left.is_zero() => left.min(CHUNK),
                _ => return,
            },
        };
        thread::sleep(wait);
    }
}

/// `interval * k` from `start`, or `None` when it cannot be represented.
fn tick_at(start: Instant, interval: Duration, k: u64) -> Option<Instant> {
    let nanos = interval.as_nanos().checked_mul(k as u128)?;
    let offset = Duration::new(u64::try_from(nanos / 1_000_000_000).ok()?, (nanos % 1_000_000_000) as u32);
    start.checked_add(offset)
}

pub fn run(total: Span, settings: &Settings, sink: Option<(Sink, bool)>) {
    let start = Instant::now();
    let length = match total {
        Span::Finite(d) => Some(d),
        Span::Infinite => None,
    };
    let deadline = length.and_then(|d| start.checked_add(d));
    let (Some((mut sink, inline)), Some(mode)) = (sink, settings.mode) else {
        sleep_until(deadline);
        return;
    };

    // Dots install the handler with their first output, so an interrupted
    // sleep that printed nothing leaves nothing behind.
    if inline && mode == Mode::Countdown {
        signals::install(sink.fd(), Cleanup::WipeLine);
    }
    let mut dots_started = false;
    let mut start_dots = |sink: &mut Sink| {
        if !dots_started {
            dots_started = true;
            if inline {
                signals::install(sink.fd(), Cleanup::Newline);
            }
        }
    };

    let interval = settings.interval;
    let tenths = interval < Duration::from_secs(1);
    let target = match (settings.target_time, deadline, length) {
        (true, Some(_), Some(d)) => {
            Some(format::target(&locale::now_local(d), &locale::now_local(Duration::ZERO)))
        }
        _ => None,
    };

    if let (Mode::Dots, Some(t)) = (mode, &target) {
        start_dots(&mut sink);
        sink.write(&format!("until {t}{}", if inline { " " } else { "\n" }));
    }

    let mut k: u64 = 0;
    loop {
        let Some(tick) = tick_at(start, interval, k) else { break };
        if deadline.is_some_and(|d| tick >= d) {
            break;
        }
        sleep_until(Some(tick));
        let now = Instant::now();
        match mode {
            Mode::Countdown => {
                let mut text = match deadline {
                    Some(d) => format::remaining(d.saturating_duration_since(now), tenths),
                    None => format::elapsed(now - start, tenths),
                };
                if let Some(t) = &target {
                    text.push_str(" → ");
                    text.push_str(t);
                }
                if inline {
                    sink.write(&format!("\r{text}\x1b[K"));
                } else {
                    sink.write(&format!("{text}\n"));
                }
            }
            Mode::Dots if k > 0 => {
                start_dots(&mut sink);
                sink.write(if inline { "." } else { ".\n" });
            }
            Mode::Dots => {}
        }
        // Skip ticks that are already in the past instead of firing a burst.
        let behind = ((now - start).as_nanos() / interval.as_nanos()) as u64;
        k = (k + 1).max(behind + 1);
    }

    sleep_until(deadline);
    if inline {
        match mode {
            Mode::Countdown => sink.write("\r\x1b[K"),
            Mode::Dots if dots_started => sink.write("\n"),
            Mode::Dots => {}
        }
    }
}
