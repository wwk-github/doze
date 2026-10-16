use std::io::{BufRead, BufReader};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const DOZE: &str = env!("CARGO_BIN_EXE_doze");

fn doze(args: &[&str]) -> (Output, Duration) {
    let start = Instant::now();
    let out = Command::new(DOZE)
        .args(args)
        .env_remove("DOZE_OPTS")
        .stdin(Stdio::null())
        .output()
        .expect("run doze");
    (out, start.elapsed())
}

fn stdout_lines(out: &Output) -> Vec<String> {
    String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect()
}

#[test]
fn countdown_lines_explicit() {
    let (out, _) = doze(&["-c", "--style", "lines", "-o", "stdout", "2.2"]);
    assert!(out.status.success());
    assert_eq!(stdout_lines(&out), ["00:03", "00:02", "00:01"]);
    assert!(out.stderr.is_empty());
}

#[test]
fn countdown_tenths() {
    let (out, _) = doze(&["-c", "-i", "0.5", "--style", "lines", "-o", "stdout", "1"]);
    assert_eq!(stdout_lines(&out), ["00:01.0", "00:00.5"]);
}

#[test]
fn countdown_with_target() {
    let (out, _) = doze(&["-c", "-t", "--style", "lines", "-o", "stdout", "0.5"]);
    let lines = stdout_lines(&out);
    assert_eq!(lines.len(), 1);
    let (left, end) = lines[0].split_once(" → ").expect("target separator");
    assert_eq!(left, "00:01");
    assert!(end.len() == 8 || end.len() == 19, "{end:?}");
}

#[test]
fn dots_lines_with_target() {
    let (out, _) = doze(&["-d", "-t", "-i", "0.3", "--style", "lines", "-o", "stdout", "1"]);
    let lines = stdout_lines(&out);
    assert!(lines[0].starts_with("until "), "{lines:?}");
    assert_eq!(&lines[1..], [".", ".", "."]);
}

#[test]
fn dots_inline_ends_with_newline() {
    let (out, _) = doze(&["-d", "-i", "0.3", "--style", "inline", "-o", "stdout", "1"]);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "...\n");
}

#[test]
fn countdown_inline_clears() {
    let (out, _) = doze(&["-c", "--style", "inline", "-o", "stdout", "1.5"]);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("\r00:02\x1b[K"), "{text:?}");
    assert!(text.ends_with("\r\x1b[K"), "{text:?}");
}

#[test]
fn display_goes_to_stderr_by_default() {
    let (out, _) = doze(&["-c", "--style", "lines", "0.5"]);
    assert!(out.stdout.is_empty());
    assert_eq!(String::from_utf8_lossy(&out.stderr), "00:01\n");
}

#[test]
fn total_time_matches_with_tiny_interval() {
    let (out, took) = doze(&["-c", "-i", "0.01", "--style", "lines", "-o", "stdout", "2"]);
    assert!(out.status.success());
    assert!(took >= Duration::from_secs(2) && took <= Duration::from_millis(2150), "{took:?}");
    assert!(stdout_lines(&out).len() <= 201);
}

#[test]
fn silent_total_time() {
    let (out, took) = doze(&["0.3", "0.2"]);
    assert!(out.status.success());
    assert!(out.stdout.is_empty() && out.stderr.is_empty());
    assert!(took >= Duration::from_millis(500) && took <= Duration::from_millis(650), "{took:?}");
}

#[test]
fn infinite_never_overflows() {
    let mut child = Command::new(DOZE)
        .args(["-c", "--style", "lines", "-o", "stdout", "infinity"])
        .env_remove("DOZE_OPTS")
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn");
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
    child.kill().unwrap();
    child.wait().unwrap();
    assert_eq!(line, "elapsed 00:00\n");
}

#[test]
fn help_and_version() {
    let (out, _) = doze(&["--help"]);
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with(&format!("Usage: {DOZE} ")), "{text}");
    assert!(text.contains("DOZE_OPTS"));
    let (out, _) = doze(&["--version"]);
    assert_eq!(String::from_utf8_lossy(&out.stdout), "doze 0.1.0\n");
}

#[test]
fn epipe_keeps_sleeping() {
    let start = Instant::now();
    let mut child = Command::new(DOZE)
        .args(["-c", "--style", "lines", "-o", "stdout", "-i", "0.1", "1"])
        .env_remove("DOZE_OPTS")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let mut line = String::new();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    reader.read_line(&mut line).unwrap();
    drop(reader);
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{out:?}");
    assert!(out.stderr.is_empty(), "{out:?}");
    assert!(start.elapsed() >= Duration::from_millis(900));
}

fn interrupt(args: &[&str], after: Duration, signal: i32) -> (std::process::ExitStatus, String, String) {
    let child = Command::new(DOZE)
        .args(args)
        .env_remove("DOZE_OPTS")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    std::thread::sleep(after);
    unsafe { libc::kill(child.id() as i32, signal) };
    let out = child.wait_with_output().unwrap();
    (
        out.status,
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn sigint_reraises_and_cleans() {
    use std::os::unix::process::ExitStatusExt;
    let (status, stdout, _) = interrupt(
        &["-c", "--style", "inline", "-o", "stdout", "5"],
        Duration::from_millis(300),
        libc::SIGINT,
    );
    assert_eq!(status.signal(), Some(libc::SIGINT));
    assert!(stdout.starts_with("\r00:05\x1b[K"), "{stdout:?}");
    assert!(stdout.ends_with("\r\x1b[K"), "{stdout:?}");
}

#[test]
fn sigterm_dots_inline_keeps_dots() {
    use std::os::unix::process::ExitStatusExt;
    let (status, stdout, _) = interrupt(
        &["-d", "-i", "0.1", "--style", "inline", "-o", "stdout", "5"],
        Duration::from_millis(350),
        libc::SIGTERM,
    );
    assert_eq!(status.signal(), Some(libc::SIGTERM));
    assert!(stdout.len() >= 3 && stdout.ends_with(".\n"), "{stdout:?}");
    assert!(stdout.trim_end_matches('\n').chars().all(|c| c == '.'), "{stdout:?}");
}

#[test]
fn sighup_countdown_inline_cleans() {
    use std::os::unix::process::ExitStatusExt;
    let (status, stdout, _) = interrupt(
        &["-c", "--style", "inline", "-o", "stdout", "5"],
        Duration::from_millis(300),
        libc::SIGHUP,
    );
    assert_eq!(status.signal(), Some(libc::SIGHUP));
    assert!(stdout.ends_with("\r\x1b[K"), "{stdout:?}");
}

#[test]
fn silent_sigint_is_plain() {
    use std::os::unix::process::ExitStatusExt;
    let (status, stdout, stderr) = interrupt(&["5"], Duration::from_millis(200), libc::SIGINT);
    assert_eq!(status.signal(), Some(libc::SIGINT));
    assert!(stdout.is_empty() && stderr.is_empty());
}

#[test]
fn ignored_signals_stay_ignored() {
    use std::os::unix::process::CommandExt;
    let start = Instant::now();
    let mut cmd = Command::new(DOZE);
    cmd.args(["-c", "--style", "inline", "-o", "stdout", "0.6"])
        .env_remove("DOZE_OPTS")
        .stdout(Stdio::piped());
    unsafe {
        cmd.pre_exec(|| {
            libc::signal(libc::SIGINT, libc::SIG_IGN);
            libc::signal(libc::SIGHUP, libc::SIG_IGN);
            Ok(())
        });
    }
    let child = cmd.spawn().expect("spawn");
    std::thread::sleep(Duration::from_millis(200));
    unsafe {
        libc::kill(child.id() as i32, libc::SIGINT);
        libc::kill(child.id() as i32, libc::SIGHUP);
    }
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{:?}", out.status);
    assert!(start.elapsed() >= Duration::from_millis(600));
}

#[test]
fn dots_inline_shorter_than_interval_prints_nothing() {
    let (out, _) = doze(&["-d", "--style", "inline", "-o", "stdout", "0.5"]);
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout), "");
}

#[test]
fn explicit_mode_falls_back_to_stderr_when_env_tty_unusable() {
    let out = Command::new(DOZE)
        .args(["-c", "--style", "lines", "0.5"])
        .env("DOZE_OPTS", "-o tty")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stderr), "00:01\n");
}

/// Runs doze with stderr on a fresh pty that is not its controlling terminal,
/// so stderr is a tty but doze is not in its foreground process group.
fn stderr_on_foreign_pty(args: &[&str], doze_opts: &str) -> String {
    use std::os::fd::FromRawFd;
    let (mut master, mut slave) = (-1, -1);
    let rc = unsafe {
        libc::openpty(&mut master, &mut slave, std::ptr::null_mut(), std::ptr::null(), std::ptr::null())
    };
    assert_eq!(rc, 0);
    let status = Command::new(DOZE)
        .args(args)
        .env("DOZE_OPTS", doze_opts)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(unsafe { Stdio::from_raw_fd(slave) })
        .status()
        .unwrap();
    assert!(status.success());
    unsafe { libc::fcntl(master, libc::F_SETFL, libc::O_NONBLOCK) };
    let mut buf = [0u8; 4096];
    let n = unsafe { libc::read(master, buf.as_mut_ptr().cast(), buf.len()) };
    unsafe { libc::close(master) };
    String::from_utf8_lossy(&buf[..n.max(0) as usize]).into_owned()
}

#[test]
fn env_warning_needs_foreground() {
    assert_eq!(stderr_on_foreign_pty(&["0.1"], "--bogus"), "");
}

#[test]
fn env_display_needs_foreground() {
    assert_eq!(stderr_on_foreign_pty(&["0.3"], "-c"), "");
}
