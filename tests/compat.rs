//! Black-box comparison with GNU sleep.

use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const DOZE: &str = env!("CARGO_BIN_EXE_doze");
const SLEEP: &str = "/usr/bin/sleep";
const GRACE: Duration = Duration::from_millis(150);

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Exited(i32),
    Running,
}

#[derive(Debug, PartialEq, Eq)]
struct Observed {
    outcome: Outcome,
    stdout: String,
    stderr: String,
}

fn observe(program: &str, args: &[&str], envs: &[(&str, &str)]) -> Observed {
    let mut cmd = Command::new(program);
    cmd.args(args).env_remove("DOZE_OPTS").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("spawn");
    let start = Instant::now();
    let mut outcome = Outcome::Running;
    while start.elapsed() < GRACE {
        if let Some(status) = child.try_wait().unwrap() {
            outcome = Outcome::Exited(status.code().unwrap_or(-1));
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    if outcome == Outcome::Running {
        child.kill().unwrap();
    }
    let out = child.wait_with_output().unwrap();
    Observed {
        outcome,
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn compare(args: &[&str], locale: &str) {
    let envs = [("LC_ALL", locale)];
    let gnu = observe(SLEEP, args, &envs);
    let mut ours = observe(DOZE, args, &envs);
    ours.stderr = ours.stderr.replace(DOZE, SLEEP).replace("doze", "sleep");
    assert_eq!(ours, gnu, "args {args:?} under {locale}");
}

const CORPUS: &[&[&str]] = &[
    &["30"], &["1."], &["00"], &["+1"], &[" 1"],
    &["1 "], &["."], &["1e"], &["0x"], &["nan"], &["s"], &["1S"], &["1ms"], &["1ss"], &["-0"], &["-1"], &[""],
    &["1e+0"], &["1e-3"], &["0x1"], &["0X1P0"], &["0x.8"], &["0x1p-4"],
    &["inf"], &["INF"], &["Inf"], &["infinity"], &["infs"], &["infm"],
    &["1e400"], &["0x1p99999"],
    &["1.5d"], &["2m"], &["1h"], &[".5s"],
    &[], &["1", "x"], &["--foo"], &["-x"], &["--", "-1"], &["0"], &["0.01", "0.01"],
];

#[test]
fn compat_corpus() {
    for locale in ["C", "en_US.UTF-8"] {
        for args in CORPUS {
            compare(args, locale);
        }
    }
}

#[test]
fn compat_help_prefix_exit_code() {
    let gnu = observe(SLEEP, &["--hel"], &[]);
    let ours = observe(DOZE, &["--hel"], &[]);
    assert_eq!(ours.outcome, gnu.outcome);
    assert_eq!(ours.outcome, Outcome::Exited(0));
}

#[test]
fn compat_pl_comma() {
    compare(&["1,5"], "pl_PL.UTF-8");
    let gnu = observe(SLEEP, &["1,5x"], &[("LC_ALL", "pl_PL.UTF-8")]);
    let ours = observe(DOZE, &["1,5x"], &[("LC_ALL", "pl_PL.UTF-8")]);
    assert_eq!(gnu.outcome, Outcome::Exited(1));
    assert_eq!(ours.outcome, Outcome::Exited(1));
}

#[test]
fn argv0_prefix() {
    let dir = std::env::temp_dir().join(format!("doze-argv0-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let link = dir.join("sleep");
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(DOZE, &link).unwrap();
    let got = observe(link.to_str().unwrap(), &["x"], &[("LC_ALL", "C")]);
    std::fs::remove_dir_all(&dir).unwrap();
    let expected = format!("sleep: invalid time interval 'x'\nTry '{} --help'", link.display());
    assert!(got.stderr.starts_with(&expected), "{got:?}");
}

fn timed_doze(args: &[&str], doze_opts: &str) -> (std::process::Output, Duration) {
    let start = Instant::now();
    let out = Command::new(DOZE)
        .args(args)
        .env("DOZE_OPTS", doze_opts)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    (out, start.elapsed())
}

#[test]
fn env_opts_silent_when_redirected() {
    let (out, took) = timed_doze(&["0.5"], "-c -t -i 0.1");
    assert!(out.status.success());
    assert!(out.stdout.is_empty() && out.stderr.is_empty(), "{out:?}");
    assert!(took >= Duration::from_millis(500));
}

#[test]
fn env_opts_tty_output_needs_stderr_tty() {
    let (out, _) = timed_doze(&["0.2"], "-c -o tty");
    assert!(out.status.success());
    assert!(out.stdout.is_empty() && out.stderr.is_empty(), "{out:?}");
}

#[test]
fn invalid_env_is_ignored() {
    let (out, took) = timed_doze(&["0.1"], "--bogus");
    assert!(out.status.success());
    assert!(out.stderr.is_empty() && out.stdout.is_empty());
    assert!(took >= Duration::from_millis(100));
}
