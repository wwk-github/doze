//! Where the display goes and whether it is shown at all.

use std::fs::File;
use std::io::Write;
use std::os::fd::{AsRawFd, RawFd};

use crate::cli::{Settings, Style, Target};

/// Observations about the environment that drive the display decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts {
    /// The chosen output is a terminal.
    pub is_tty: bool,
    /// This process is in the terminal's foreground process group.
    pub foreground: bool,
    pub stderr_is_tty: bool,
    /// `/dev/tty` could be opened (only meaningful for `Target::Tty`).
    pub tty_open_ok: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Silent,
    Show { inline: bool },
}

pub fn decide(settings: &Settings, facts: Facts) -> Decision {
    if settings.mode.is_none() {
        return Decision::Silent;
    }
    if settings.output == Target::Tty && !facts.tty_open_ok {
        return Decision::Silent;
    }
    if !settings.explicit && !(facts.is_tty && facts.foreground) {
        return Decision::Silent;
    }
    let inline = match settings.style {
        Style::Inline => true,
        Style::Lines => false,
        Style::Auto => facts.is_tty,
    };
    Decision::Show { inline }
}

pub struct Sink {
    writer: Box<dyn Write>,
    fd: RawFd,
    enabled: bool,
}

impl Sink {
    /// Writes and flushes; any error disables the sink for good.
    pub fn write(&mut self, s: &str) {
        if !self.enabled {
            return;
        }
        if self.writer.write_all(s.as_bytes()).and_then(|_| self.writer.flush()).is_err() {
            self.enabled = false;
            crate::signals::disable();
        }
    }

    pub fn fd(&self) -> RawFd {
        self.fd
    }
}

pub fn isatty(fd: RawFd) -> bool {
    unsafe { libc::isatty(fd) == 1 }
}

pub fn is_foreground(fd: RawFd) -> bool {
    let group = unsafe { libc::tcgetpgrp(fd) };
    group != -1 && group == unsafe { libc::getpgrp() }
}

/// `-o tty` taken only from DOZE_OPTS is not effective unless stderr is a
/// terminal too; the default target is used instead.
pub fn effective_target(settings: &Settings, stderr_is_tty: bool) -> Target {
    if settings.output == Target::Tty && settings.output_from_env_only && !stderr_is_tty {
        Target::Stderr
    } else {
        settings.output
    }
}

/// Opens the output chosen in `settings`, or `None` when nothing must be shown.
/// The flag is true for inline drawing.
pub fn open(settings: &Settings) -> Option<(Sink, bool)> {
    settings.mode?;
    let stderr_is_tty = isatty(2);
    let target = effective_target(settings, stderr_is_tty);
    let settings = &Settings { output: target, ..settings.clone() };
    let (writer, fd, tty_open_ok): (Box<dyn Write>, RawFd, bool) = match target {
        Target::Stderr => (Box::new(std::io::stderr()), 2, true),
        Target::Stdout => (Box::new(std::io::stdout()), 1, true),
        Target::Tty => match File::options().write(true).open("/dev/tty") {
            Ok(file) => {
                let fd = file.as_raw_fd();
                (Box::new(file), fd, true)
            }
            Err(_) => (Box::new(std::io::sink()), -1, false),
        },
    };
    let facts = Facts {
        is_tty: fd >= 0 && isatty(fd),
        foreground: fd >= 0 && is_foreground(fd),
        stderr_is_tty,
        tty_open_ok,
    };
    match decide(settings, facts) {
        Decision::Silent => None,
        Decision::Show { inline } => Some((Sink { writer, fd, enabled: true }, inline)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Mode;
    use std::time::Duration;

    fn base() -> Settings {
        Settings {
            mode: Some(Mode::Countdown),
            target_time: false,
            interval: Duration::from_secs(1),
            output: Target::Stderr,
            output_from_env_only: false,
            style: Style::Auto,
            explicit: false,
        }
    }

    const TTY: Facts = Facts { is_tty: true, foreground: true, stderr_is_tty: true, tty_open_ok: true };
    const PIPE: Facts = Facts { is_tty: false, foreground: false, stderr_is_tty: false, tty_open_ok: true };

    #[test]
    fn silent_without_mode() {
        let s = Settings { mode: None, explicit: true, ..base() };
        assert_eq!(decide(&s, TTY), Decision::Silent);
    }

    #[test]
    fn explicit_shows_on_non_tty_as_lines() {
        let s = Settings { explicit: true, ..base() };
        assert_eq!(decide(&s, PIPE), Decision::Show { inline: false });
    }

    #[test]
    fn env_only_requires_tty() {
        assert_eq!(decide(&base(), PIPE), Decision::Silent);
        assert_eq!(decide(&base(), TTY), Decision::Show { inline: true });
    }

    #[test]
    fn env_only_requires_foreground() {
        let background = Facts { foreground: false, ..TTY };
        assert_eq!(decide(&base(), background), Decision::Silent);
        let s = Settings { explicit: true, ..base() };
        assert_eq!(decide(&s, background), Decision::Show { inline: true });
    }

    #[test]
    fn env_only_tty_output_needs_stderr_tty() {
        let s = Settings { output: Target::Tty, output_from_env_only: true, ..base() };
        assert_eq!(effective_target(&s, false), Target::Stderr);
        assert_eq!(effective_target(&s, true), Target::Tty);
        let s = Settings { output_from_env_only: false, ..s };
        assert_eq!(effective_target(&s, false), Target::Tty);
    }

    #[test]
    fn tty_open_failure_is_silent() {
        let s = Settings { output: Target::Tty, explicit: true, ..base() };
        let no_tty = Facts { is_tty: false, foreground: false, tty_open_ok: false, ..PIPE };
        assert_eq!(decide(&s, no_tty), Decision::Silent);
    }

    #[test]
    fn auto_style_inline_on_tty() {
        let s = Settings { explicit: true, ..base() };
        assert_eq!(decide(&s, TTY), Decision::Show { inline: true });
        let s = Settings { style: Style::Lines, ..s };
        assert_eq!(decide(&s, TTY), Decision::Show { inline: false });
    }

    #[test]
    fn forced_inline_on_non_tty() {
        let s = Settings { explicit: true, style: Style::Inline, ..base() };
        assert_eq!(decide(&s, PIPE), Decision::Show { inline: true });
    }
}
