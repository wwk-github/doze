mod cli;
mod duration;
mod format;
mod locale;
mod output;
mod run;
mod signals;

use std::path::Path;
use std::process::exit;

use cli::Action;

fn help(prog: &str) -> String {
    format!(
        "\
Usage: {prog} [OPTION]... NUMBER[SUFFIX]...
  or:  {prog} OPTION
Pause for NUMBER seconds, where NUMBER is an integer or floating-point.
SUFFIX may be 's','m','h', or 'd', for seconds, minutes, hours, days.
With multiple arguments, pause for the sum of their values.
Without display options it behaves exactly like sleep and prints nothing.

  -d, --dots               print a dot every interval
  -c, --countdown          show the time left, refreshed every interval
  -t, --target             also show when the sleep ends (implies -c without -d)
  -i, --interval=NUMBER[SUFFIX]
                           dot or refresh interval (default 1s)
  -o, --output=WHERE       stderr (default), stdout or tty
      --style=STYLE        auto (default), inline or lines
  -q, --quiet              show nothing, even if DOZE_OPTS asks for it
      --help               display this help and exit
      --version            output version information and exit

DOZE_OPTS may hold default options, e.g. DOZE_OPTS='--countdown -t'.
Command-line options override it. Options taken only from DOZE_OPTS are
shown only on a terminal held by a foreground job.
"
    )
}

fn main() {
    locale::init();
    let mut argv = std::env::args_os();
    let invoked = argv.next().map_or_else(|| "doze".to_string(), |a| a.to_string_lossy().into_owned());
    let prog = Path::new(&invoked)
        .file_name()
        .map_or_else(|| invoked.clone(), |n| n.to_string_lossy().into_owned());
    let args: Vec<String> = argv.map(|a| a.to_string_lossy().into_owned()).collect();
    let env_opts = std::env::var("DOZE_OPTS").ok();

    match cli::parse(&prog, &invoked, &args, env_opts.as_deref(), locale::decimal_sep(), locale::is_utf8()) {
        Err(text) => {
            eprint!("{text}");
            exit(1);
        }
        Ok(Action::Help) => print!("{}", help(&invoked)),
        Ok(Action::Version) => println!("doze {}", env!("CARGO_PKG_VERSION")),
        Ok(Action::Sleep { total, settings, warning }) => {
            if let Some(reason) = warning {
                if output::isatty(2) && output::is_foreground(2) {
                    eprintln!("{prog}: ignoring invalid DOZE_OPTS: {reason}");
                }
            }
            let sink = output::open(&settings);
            run::run(total, &settings, sink);
        }
    }
}
