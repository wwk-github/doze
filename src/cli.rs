//! Command-line and DOZE_OPTS parsing with GNU getopt style diagnostics.

use std::time::Duration;

use crate::duration::{Span, parse_interval, sum};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Dots,
    Countdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Stderr,
    Stdout,
    Tty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Auto,
    Inline,
    Lines,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// `None` means silent.
    pub mode: Option<Mode>,
    pub target_time: bool,
    pub interval: Duration,
    pub output: Target,
    /// `-o` came only from DOZE_OPTS.
    pub output_from_env_only: bool,
    pub style: Style,
    /// `-d`, `-c` or `-t` was given on the command line.
    pub explicit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Sleep { total: Span, settings: Settings, warning: Option<String> },
    Help,
    Version,
}

/// Options collected from one source.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Partial {
    mode: Option<Mode>,
    target_time: bool,
    interval: Option<Duration>,
    output: Option<Target>,
    style: Option<Style>,
    quiet: bool,
    mode_flag: bool,
}

enum OptError {
    InvalidShort(char),
    UnrecognizedLong(String),
    AmbiguousLong(String),
    MissingShort(char),
    MissingLong(&'static str),
    UnexpectedValue(&'static str),
    BadInterval(String),
    BadChoice { option: &'static str, value: String, valid: &'static [&'static str] },
    NotAnOption(String),
    NotAllowed(&'static str),
}

impl OptError {
    /// Diagnostics that GNU getopt prints itself, prefixed with argv[0] as given.
    fn from_getopt(&self) -> bool {
        matches!(
            self,
            OptError::InvalidShort(_)
                | OptError::UnrecognizedLong(_)
                | OptError::AmbiguousLong(_)
                | OptError::MissingShort(_)
                | OptError::MissingLong(_)
                | OptError::UnexpectedValue(_)
        )
    }

    /// First line of the diagnostic without the program prefix, plus any extra lines.
    fn render(&self, utf8: bool) -> String {
        let q = |s: &str| quote(s, utf8);
        match self {
            OptError::InvalidShort(c) => format!("invalid option -- '{c}'"),
            OptError::UnrecognizedLong(name) => format!("unrecognized option '--{name}'"),
            OptError::AmbiguousLong(name) => format!("option '--{name}' is ambiguous"),
            OptError::MissingShort(c) => format!("option requires an argument -- '{c}'"),
            OptError::MissingLong(name) => format!("option '--{name}' requires an argument"),
            OptError::UnexpectedValue(name) => format!("option '--{name}' doesn't allow an argument"),
            OptError::BadInterval(v) => format!("invalid time interval {}", q(v)),
            OptError::BadChoice { option, value, valid } => {
                let mut text = format!(
                    "invalid argument {} for {}\nValid arguments are:",
                    q(value),
                    q(&format!("--{option}"))
                );
                for v in valid.iter() {
                    text.push_str(&format!("\n  - {}", q(v)));
                }
                text
            }
            OptError::NotAnOption(v) => format!("unexpected argument {}", q(v)),
            OptError::NotAllowed(name) => format!("option '--{name}' is not allowed here"),
        }
    }
}

fn quote(s: &str, utf8: bool) -> String {
    if utf8 { format!("‘{s}’") } else { format!("'{s}'") }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Opt {
    Dots,
    Countdown,
    Target,
    Interval,
    Output,
    Style,
    Quiet,
    Help,
    Version,
}

const LONG: &[(&str, Opt)] = &[
    ("dots", Opt::Dots),
    ("countdown", Opt::Countdown),
    ("target", Opt::Target),
    ("interval", Opt::Interval),
    ("output", Opt::Output),
    ("style", Opt::Style),
    ("quiet", Opt::Quiet),
    ("help", Opt::Help),
    ("version", Opt::Version),
];

fn short(c: char) -> Option<Opt> {
    Some(match c {
        'd' => Opt::Dots,
        'c' => Opt::Countdown,
        't' => Opt::Target,
        'i' => Opt::Interval,
        'o' => Opt::Output,
        'q' => Opt::Quiet,
        _ => return None,
    })
}

fn long_name(opt: Opt) -> &'static str {
    LONG.iter().find(|(_, o)| *o == opt).map(|(n, _)| *n).unwrap_or("")
}

fn takes_value(opt: Opt) -> bool {
    matches!(opt, Opt::Interval | Opt::Output | Opt::Style)
}

fn lookup_long(name: &str) -> Result<Opt, OptError> {
    if let Some((_, opt)) = LONG.iter().find(|(n, _)| *n == name) {
        return Ok(*opt);
    }
    let matches: Vec<Opt> = LONG.iter().filter(|(n, _)| n.starts_with(name)).map(|(_, o)| *o).collect();
    match matches.as_slice() {
        [opt] => Ok(*opt),
        [] => Err(OptError::UnrecognizedLong(name.to_string())),
        _ => Err(OptError::AmbiguousLong(name.to_string())),
    }
}

/// Result of scanning one source of arguments.
struct Scan {
    partial: Partial,
    operands: Vec<String>,
    request: Option<Action>,
}

fn apply(partial: &mut Partial, opt: Opt, value: Option<&str>, decimal_sep: char) -> Result<(), OptError> {
    match opt {
        Opt::Dots => {
            partial.mode = Some(Mode::Dots);
            partial.mode_flag = true;
        }
        Opt::Countdown => {
            partial.mode = Some(Mode::Countdown);
            partial.mode_flag = true;
        }
        Opt::Target => {
            partial.target_time = true;
            partial.mode_flag = true;
        }
        Opt::Quiet => partial.quiet = true,
        Opt::Interval => {
            let v = value.unwrap_or_default();
            match parse_interval(v, decimal_sep) {
                Some(Span::Finite(d)) if !d.is_zero() => partial.interval = Some(d),
                _ => return Err(OptError::BadInterval(v.to_string())),
            }
        }
        Opt::Output => {
            let v = value.unwrap_or_default();
            partial.output = Some(match v {
                "stderr" => Target::Stderr,
                "stdout" => Target::Stdout,
                "tty" => Target::Tty,
                _ => {
                    return Err(OptError::BadChoice {
                        option: "output",
                        value: v.to_string(),
                        valid: &["stderr", "stdout", "tty"],
                    });
                }
            });
        }
        Opt::Style => {
            let v = value.unwrap_or_default();
            partial.style = Some(match v {
                "auto" => Style::Auto,
                "inline" => Style::Inline,
                "lines" => Style::Lines,
                _ => {
                    return Err(OptError::BadChoice {
                        option: "style",
                        value: v.to_string(),
                        valid: &["auto", "inline", "lines"],
                    });
                }
            });
        }
        Opt::Help | Opt::Version => {}
    }
    Ok(())
}

/// Walks the arguments like GNU getopt_long with permutation.
fn scan(args: &[String], decimal_sep: char) -> Result<Scan, OptError> {
    let mut out = Scan { partial: Partial::default(), operands: Vec::new(), request: None };
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        i += 1;
        if arg == "--" {
            out.operands.extend(args[i..].iter().cloned());
            break;
        }
        if let Some(body) = arg.strip_prefix("--") {
            let (name, attached) = match body.split_once('=') {
                Some((n, v)) => (n, Some(v)),
                None => (body, None),
            };
            let opt = lookup_long(name)?;
            let value = if takes_value(opt) {
                match attached {
                    Some(v) => Some(v.to_string()),
                    None if i < args.len() => {
                        i += 1;
                        Some(args[i - 1].clone())
                    }
                    None => return Err(OptError::MissingLong(long_name(opt))),
                }
            } else if attached.is_some() {
                return Err(OptError::UnexpectedValue(long_name(opt)));
            } else {
                None
            };
            match opt {
                Opt::Help => return Ok(Scan { request: Some(Action::Help), ..out }),
                Opt::Version => return Ok(Scan { request: Some(Action::Version), ..out }),
                _ => apply(&mut out.partial, opt, value.as_deref(), decimal_sep)?,
            }
            continue;
        }
        if arg.len() > 1 && arg.starts_with('-') {
            let chars: Vec<char> = arg[1..].chars().collect();
            let mut k = 0;
            while k < chars.len() {
                let c = chars[k];
                k += 1;
                let opt = short(c).ok_or(OptError::InvalidShort(c))?;
                if takes_value(opt) {
                    let value = if k < chars.len() {
                        chars[k..].iter().collect::<String>()
                    } else if i < args.len() {
                        i += 1;
                        args[i - 1].clone()
                    } else {
                        return Err(OptError::MissingShort(c));
                    };
                    apply(&mut out.partial, opt, Some(&value), decimal_sep)?;
                    break;
                }
                apply(&mut out.partial, opt, None, decimal_sep)?;
            }
            continue;
        }
        out.operands.push(arg.clone());
    }
    Ok(out)
}

/// `prog` is the basename used as message prefix; `invoked` is argv[0] as given,
/// which GNU uses in the "Try ... --help" hint.
pub fn parse(
    prog: &str,
    invoked: &str,
    args: &[String],
    env_opts: Option<&str>,
    decimal_sep: char,
    utf8: bool,
) -> Result<Action, String> {
    let fail = |e: OptError| {
        let prefix = if e.from_getopt() { invoked } else { prog };
        format!("{prefix}: {}\nTry '{invoked} --help' for more information.\n", e.render(utf8))
    };
    let argv = scan(args, decimal_sep).map_err(fail)?;
    if let Some(request) = argv.request {
        return Ok(request);
    }
    if argv.operands.is_empty() {
        return Err(format!("{prog}: missing operand\nTry '{invoked} --help' for more information.\n"));
    }
    let mut spans = Vec::with_capacity(argv.operands.len());
    for operand in &argv.operands {
        match parse_interval(operand, decimal_sep) {
            Some(span) => spans.push(span),
            None => return Err(fail(OptError::BadInterval(operand.clone()))),
        }
    }

    let (env, warning) = match env_opts.map(|e| parse_env_inner(e, decimal_sep)) {
        None => (Partial::default(), None),
        Some(Ok(p)) => (p, None),
        Some(Err(e)) => (Partial::default(), Some(e.render(utf8))),
    };
    let cmd = argv.partial;

    let env_mode = if env.quiet { None } else { env.mode };
    let env_target = env.target_time && !env.quiet;
    let target_time = cmd.target_time || env_target;
    let mut mode = cmd.mode.or(env_mode);
    if target_time && mode.is_none() {
        mode = Some(Mode::Countdown);
    }
    if cmd.quiet {
        mode = None;
    }

    let settings = Settings {
        mode,
        target_time,
        interval: cmd.interval.or(env.interval).unwrap_or(Duration::from_secs(1)),
        output: cmd.output.or(env.output).unwrap_or(Target::Stderr),
        output_from_env_only: cmd.output.is_none() && env.output.is_some(),
        style: cmd.style.or(env.style).unwrap_or(Style::Auto),
        explicit: cmd.mode_flag,
    };
    Ok(Action::Sleep { total: sum(&spans), settings, warning })
}

fn parse_env_inner(env_opts: &str, decimal_sep: char) -> Result<Partial, OptError> {
    let tokens: Vec<String> = env_opts.split_whitespace().map(str::to_string).collect();
    let found = scan(&tokens, decimal_sep)?;
    match found.request {
        Some(Action::Help) => return Err(OptError::NotAllowed("help")),
        Some(_) => return Err(OptError::NotAllowed("version")),
        None => {}
    }
    if let Some(operand) = found.operands.first() {
        return Err(OptError::NotAnOption(operand.clone()));
    }
    Ok(found.partial)
}

/// Parses DOZE_OPTS alone; the error is a one-line reason without prefix.
#[cfg(test)]
pub fn parse_env(env_opts: &str, decimal_sep: char) -> Result<Partial, String> {
    parse_env_inner(env_opts, decimal_sep).map_err(|e| e.render(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn run(list: &[&str], env: Option<&str>) -> Result<Action, String> {
        parse("doze", "doze", &args(list), env, '.', true)
    }

    fn settings(list: &[&str], env: Option<&str>) -> Settings {
        match run(list, env) {
            Ok(Action::Sleep { settings, .. }) => settings,
            other => panic!("expected sleep, got {other:?}"),
        }
    }

    const TRY: &str = "Try 'doze --help' for more information.\n";

    #[test]
    fn defaults_are_silent() {
        let got = run(&["5"], None).unwrap();
        assert_eq!(
            got,
            Action::Sleep {
                total: Span::Finite(Duration::from_secs(5)),
                settings: Settings {
                    mode: None,
                    target_time: false,
                    interval: Duration::from_secs(1),
                    output: Target::Stderr,
                    output_from_env_only: false,
                    style: Style::Auto,
                    explicit: false,
                },
                warning: None,
            }
        );
    }

    #[test]
    fn durations_are_summed() {
        match run(&["1", "2m"], None).unwrap() {
            Action::Sleep { total, .. } => assert_eq!(total, Span::Finite(Duration::from_secs(121))),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn dots_and_countdown_last_wins() {
        assert_eq!(settings(&["-d", "-c", "1"], None).mode, Some(Mode::Countdown));
        assert_eq!(settings(&["-c", "1", "--dots"], None).mode, Some(Mode::Dots));
        assert_eq!(settings(&["-d", "1"], Some("-c")).mode, Some(Mode::Dots));
        assert_eq!(settings(&["1"], Some("-d -c")).mode, Some(Mode::Countdown));
    }

    #[test]
    fn target_implies_countdown_after_merge() {
        let s = settings(&["-t", "30"], Some("--dots"));
        assert_eq!(s.mode, Some(Mode::Dots));
        assert!(s.target_time);
        let s = settings(&["-t", "30"], None);
        assert_eq!(s.mode, Some(Mode::Countdown));
        assert!(s.target_time);
    }

    #[test]
    fn quiet_on_argv_always_wins() {
        assert_eq!(settings(&["-q", "-c", "5"], None).mode, None);
        assert_eq!(settings(&["-c", "5", "-q"], None).mode, None);
        assert_eq!(settings(&["-t", "5", "-q"], Some("-d")).mode, None);
    }

    #[test]
    fn quiet_in_env_clears_env_mode_only() {
        assert_eq!(settings(&["-d", "1"], Some("-c -q")).mode, Some(Mode::Dots));
        assert_eq!(settings(&["1"], Some("-q -c")).mode, None);
        assert!(!settings(&["1"], Some("-t -q")).target_time);
    }

    #[test]
    fn explicit_only_for_mode_flags() {
        let s = settings(&["-i", "2", "1"], Some("-c"));
        assert!(!s.explicit);
        assert_eq!(s.mode, Some(Mode::Countdown));
        assert_eq!(s.interval, Duration::from_secs(2));
        assert!(settings(&["-c", "1"], None).explicit);
        assert!(settings(&["-d", "1"], None).explicit);
        assert!(settings(&["-t", "1"], None).explicit);
        assert!(!settings(&["-o", "stdout", "--style", "lines", "1"], Some("-c")).explicit);
    }

    #[test]
    fn env_with_duration_is_ignored() {
        match run(&["1"], Some("-c 5")).unwrap() {
            Action::Sleep { settings, warning, total } => {
                assert_eq!(settings.mode, None);
                assert!(warning.is_some());
                assert_eq!(total, Span::Finite(Duration::from_secs(1)));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn env_invalid_sets_warning() {
        for env in ["--bogus", "-c --help", "--version", "-i 0", "-o nowhere", "-x"] {
            match run(&["1"], Some(env)).unwrap() {
                Action::Sleep { settings, warning, .. } => {
                    assert_eq!(settings.mode, None, "{env}");
                    assert!(warning.is_some(), "{env}");
                }
                other => panic!("{env}: {other:?}"),
            }
        }
        assert_eq!(
            parse_env("--bogus", '.'),
            Err("unrecognized option '--bogus'".to_string())
        );
    }

    #[test]
    fn env_empty_is_fine() {
        match run(&["1"], Some("   ")).unwrap() {
            Action::Sleep { warning, .. } => assert_eq!(warning, None),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn long_prefix_help() {
        assert_eq!(run(&["--hel"], None), Ok(Action::Help));
        assert_eq!(run(&["--help", "x"], None), Ok(Action::Help));
        assert_eq!(run(&["--vers"], None), Ok(Action::Version));
        assert_eq!(settings(&["--count", "1"], None).mode, Some(Mode::Countdown));
    }

    #[test]
    fn bundled_short() {
        let s = settings(&["-ct", "1"], None);
        assert_eq!(s.mode, Some(Mode::Countdown));
        assert!(s.target_time);
        let s = settings(&["-ci5s", "1"], None);
        assert_eq!(s.interval, Duration::from_secs(5));
    }

    #[test]
    fn attached_values() {
        assert_eq!(settings(&["-i5s", "1"], None).interval, Duration::from_secs(5));
        assert_eq!(settings(&["--interval=2m", "1"], None).interval, Duration::from_secs(120));
        assert_eq!(settings(&["--interval", "0.5", "1"], None).interval, Duration::from_millis(500));
        assert_eq!(settings(&["-o", "tty", "1"], None).output, Target::Tty);
        assert_eq!(settings(&["--output=stdout", "1"], None).output, Target::Stdout);
        assert_eq!(settings(&["--style=lines", "1"], None).style, Style::Lines);
        assert_eq!(settings(&["--sty", "inline", "1"], None).style, Style::Inline);
    }

    #[test]
    fn options_between_operands() {
        match run(&["1", "-c", "2"], None).unwrap() {
            Action::Sleep { total, settings, .. } => {
                assert_eq!(total, Span::Finite(Duration::from_secs(3)));
                assert_eq!(settings.mode, Some(Mode::Countdown));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn double_dash() {
        assert_eq!(
            run(&["--", "-1"], None),
            Err(format!("doze: invalid time interval ‘-1’\n{TRY}"))
        );
        assert_eq!(
            run(&["--", "-c"], None),
            Err(format!("doze: invalid time interval ‘-c’\n{TRY}"))
        );
    }

    #[test]
    fn missing_operand_text() {
        assert_eq!(run(&[], None), Err(format!("doze: missing operand\n{TRY}")));
        assert_eq!(run(&["-c"], None), Err(format!("doze: missing operand\n{TRY}")));
    }

    #[test]
    fn invalid_interval_text_utf8_and_c() {
        assert_eq!(run(&["x"], None), Err(format!("doze: invalid time interval ‘x’\n{TRY}")));
        assert_eq!(
            parse("sleep", "/usr/bin/sleep", &args(&["1", "x", "y"]), None, '.', false),
            Err("sleep: invalid time interval 'x'\nTry '/usr/bin/sleep --help' for more information.\n".to_string())
        );
    }

    #[test]
    fn getopt_errors_use_full_invocation() {
        let full = |list: &[&str]| parse("sleep", "/usr/bin/sleep", &args(list), None, '.', true);
        let hint = "Try '/usr/bin/sleep --help' for more information.\n";
        assert_eq!(full(&["-x"]), Err(format!("/usr/bin/sleep: invalid option -- 'x'\n{hint}")));
        assert_eq!(full(&["--foo"]), Err(format!("/usr/bin/sleep: unrecognized option '--foo'\n{hint}")));
        assert_eq!(full(&[]), Err(format!("sleep: missing operand\n{hint}")));
        assert_eq!(full(&["-i", "0", "1"]), Err(format!("sleep: invalid time interval ‘0’\n{hint}")));
    }

    #[test]
    fn invalid_short_option_text() {
        assert_eq!(run(&["-1"], None), Err(format!("doze: invalid option -- '1'\n{TRY}")));
        assert_eq!(run(&["x", "-z"], None), Err(format!("doze: invalid option -- 'z'\n{TRY}")));
        assert_eq!(
            run(&["-i"], None),
            Err(format!("doze: option requires an argument -- 'i'\n{TRY}"))
        );
    }

    #[test]
    fn unrecognized_long_text() {
        assert_eq!(run(&["--foo"], None), Err(format!("doze: unrecognized option '--foo'\n{TRY}")));
        assert_eq!(
            run(&["--interval"], None),
            Err(format!("doze: option '--interval' requires an argument\n{TRY}"))
        );
        assert_eq!(
            run(&["--dots=1", "1"], None),
            Err(format!("doze: option '--dots' doesn't allow an argument\n{TRY}"))
        );
        assert_eq!(
            run(&["-o", "x", "1"], None),
            Err(format!(
                "doze: invalid argument ‘x’ for ‘--output’\nValid arguments are:\n  - ‘stderr’\n  - ‘stdout’\n  - ‘tty’\n{TRY}"
            ))
        );
    }

    #[test]
    fn interval_must_be_positive() {
        for bad in ["0", "inf", "x", "-1"] {
            assert_eq!(
                run(&["-i", bad, "1"], None),
                Err(format!("doze: invalid time interval ‘{bad}’\n{TRY}")),
                "{bad}"
            );
        }
    }

    #[test]
    fn interval_accepts_locale_comma() {
        let got = parse("doze", "doze", &args(&["-i", "0,5", "1,5"]), None, ',', true).unwrap();
        match got {
            Action::Sleep { total, settings, .. } => {
                assert_eq!(total, Span::Finite(Duration::from_millis(1500)));
                assert_eq!(settings.interval, Duration::from_millis(500));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn output_tty_from_env_only_flag() {
        assert!(settings(&["1"], Some("-c -o tty")).output_from_env_only);
        assert!(!settings(&["-o", "tty", "1"], Some("-c -o tty")).output_from_env_only);
        assert!(!settings(&["1"], Some("-c")).output_from_env_only);
    }
}
