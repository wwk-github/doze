//! Tidying the inline display when SIGINT, SIGTERM or SIGHUP arrives.

use std::os::fd::RawFd;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cleanup {
    /// Countdown: wipe the line.
    WipeLine,
    /// Dots: end the line so the dots stay visible.
    Newline,
}

static FD: AtomicI32 = AtomicI32::new(-1);
static KIND: AtomicU8 = AtomicU8::new(0);
static ENABLED: AtomicBool = AtomicBool::new(false);

const SIGNALS: [libc::c_int; 3] = [libc::SIGINT, libc::SIGTERM, libc::SIGHUP];

extern "C" fn handle(sig: libc::c_int) {
    // Only async-signal-safe calls from here on.
    if ENABLED.load(Ordering::SeqCst) {
        let bytes: &[u8] = if KIND.load(Ordering::SeqCst) == 0 { b"\r\x1b[K" } else { b"\n" };
        unsafe { libc::write(FD.load(Ordering::SeqCst), bytes.as_ptr().cast(), bytes.len()) };
    }
    unsafe {
        libc::signal(sig, libc::SIG_DFL);
        libc::raise(sig);
    }
}

/// Installs the handlers; the signal is re-raised after cleanup so the exit
/// status matches an interrupted sleep.
pub fn install(fd: RawFd, cleanup: Cleanup) {
    FD.store(fd, Ordering::SeqCst);
    KIND.store(if cleanup == Cleanup::WipeLine { 0 } else { 1 }, Ordering::SeqCst);
    ENABLED.store(true, Ordering::SeqCst);
    for sig in SIGNALS {
        unsafe {
            // A signal the parent ignores (nohup, trap '') must stay ignored, as in sleep.
            let mut old: libc::sigaction = std::mem::zeroed();
            libc::sigaction(sig, std::ptr::null(), &mut old);
            if old.sa_sigaction == libc::SIG_IGN {
                continue;
            }
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = handle as extern "C" fn(libc::c_int) as libc::sighandler_t;
            action.sa_flags = 0;
            libc::sigemptyset(&mut action.sa_mask);
            libc::sigaction(sig, &action, std::ptr::null_mut());
        }
    }
}

/// Stops the handler from writing, e.g. after the output failed.
pub fn disable() {
    ENABLED.store(false, Ordering::SeqCst);
}
