# doze

A `sleep` that can keep one eye open.

![doze demo: silent sleep, dots, countdown with end time, countdown with tenths](assets/demo.gif)

Without display options `doze` is a drop-in replacement for GNU `sleep`: same
arguments (`30`, `1.5`, `2m`, `1h`, `1d`, `infinity`, several values summed),
no output, same exit codes and the same error messages for ordinary input. On request it shows that it is
still waiting, and for how long.

## Examples

```sh
doze 30                  # exactly like sleep 30
doze -d 30               # a dot every second
doze -d -i 5s 2m         # a dot every 5 seconds for 2 minutes
doze -c 90               # countdown, refreshed every second: 01:30, 01:29, ...
doze -c -t 1h            # countdown plus end time: 59:59 → 14:05:12
doze -c -i 0.1 10        # countdown with tenths: 00:09.9
doze -c -o tty 30        # draw on the terminal even if stdout/stderr are redirected
doze -c --style lines 5  # one line per refresh instead of rewriting in place
```

## Options

| Option | Meaning |
|---|---|
| `-d`, `--dots` | print a dot every interval |
| `-c`, `--countdown` | show the time left, refreshed every interval |
| `-t`, `--target` | also show when the sleep ends; implies `-c` unless `-d` is set |
| `-i`, `--interval=TIME` | dot or refresh interval, same syntax as durations, default `1s` |
| `-o`, `--output=WHERE` | `stderr` (default), `stdout` or `tty` |
| `--style=STYLE` | `auto` (default: `inline` on a terminal, `lines` otherwise), `inline`, `lines` |
| `-q`, `--quiet` | show nothing, even if `DOZE_OPTS` asks for it |

Inline drawing rewrites one line and leaves nothing behind when the countdown
ends or is interrupted. Dots stay on screen and are followed by a newline.

## DOZE_OPTS

Default options can be set once, for example in `~/.zshrc`:

```sh
export DOZE_OPTS="--countdown -t"
```

Rules:

- Command-line options override `DOZE_OPTS`; `-q` on the command line always wins.
- A mode taken only from `DOZE_OPTS` is shown only when the output is a
  terminal held by the foreground job. Scripts with redirected output, cron
  jobs and background jobs (`./job.sh &`) stay silent.
- `-o tty` taken only from `DOZE_OPTS` is used only when stderr is a terminal too.
- `DOZE_OPTS` may contain options only. If it is invalid it is ignored as a
  whole (with a one-line warning on an interactive stderr); the sleep itself
  is never affected.

## Replacing sleep (opt-in, nothing is enabled by default)

- Interactive commands only: `alias sleep=doze` in `~/.zshrc`.
- Scripts too: a symlink placed in a directory that comes before `/usr/bin`
  in `PATH`, e.g. `ln -s doze ~/.local/bin/sleep`. Error messages then start
  with `sleep:`, exactly like GNU sleep.

## Install

Linux only.

- With Cargo (Rust 1.85 or newer): `cargo install doze-cli` (the command is `doze`).
- Prebuilt x86_64 binary: see [Releases](https://github.com/wwk-github/doze/releases).

## Build from source

```sh
git clone https://github.com/wwk-github/doze
cd doze
cargo build --release
install -m 0755 target/release/doze ~/.local/bin/doze
cargo test
```

The tests compare `doze` with `/usr/bin/sleep` and need GNU coreutils plus
the `en_US.UTF-8` and `pl_PL.UTF-8` locales.

## License

MIT, see [LICENSE](LICENSE).
