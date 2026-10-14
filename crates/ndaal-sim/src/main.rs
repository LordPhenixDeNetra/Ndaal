//! `ndaal-sim run --seed N [--messages M]`: runs the J01 exit scenario.
//! `ndaal-sim sweep --from S --count C [--messages M]`: runs seeds S..S+C,
//! each twice, and lists those that fail or do not replay (nightly.yml).
//!
//! Arguments are parsed by hand: a few options do not justify a dependency;
//! the product CLI (T01.12) brings `clap`.

#![forbid(unsafe_code)]
// Command-line tool: printing is its output.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::io::Write;
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};

use ndaal_sim::{Outcome, ping_pong, ping_pong_result, sweep_with};

const USAGE: &str = "usage: ndaal-sim run --seed <u64> [--messages <u32>]\n       \
                     ndaal-sim sweep --from <u64> --count <u64> [--messages <u32>]";

/// Messages of the scenario when `--messages` is absent.
const DEFAULT_MESSAGES: u32 = 20;

/// A parsed command line.
#[derive(Debug, PartialEq, Eq)]
enum Command {
    Run {
        seed: u64,
        messages: u32,
    },
    Sweep {
        from: u64,
        count: u64,
        messages: u32,
    },
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match parse(&args) {
        Ok(Command::Run { seed, messages }) => run(seed, messages),
        Ok(Command::Sweep {
            from,
            count,
            messages,
        }) => run_sweep(from, count, messages),
        Err(message) => {
            eprintln!("ndaal-sim: {message}\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn run(seed: u64, messages: u32) -> ExitCode {
    let report = ping_pong(seed, messages);
    println!("seed        {seed}");
    println!("outcome     {:?}", report.outcome);
    println!("replies     {}/{messages}", report.replies);
    println!("partitions  {}", report.partitions);
    println!("reconnects  {}", report.reconnects);
    println!("time        {:?} (virtual)", report.virtual_time);
    println!("fingerprint {:016x}", report.fingerprint);
    if report.outcome == Outcome::Finished && report.replies == messages {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Seed being played, for the panic hook: a panic aborts the process
/// (`panic = "abort"`) and must still name the seed to replay.
static CURRENT_SEED: AtomicU64 = AtomicU64::new(0);

fn run_sweep(from: u64, count: u64, messages: u32) -> ExitCode {
    std::panic::set_hook(Box::new(|info| {
        let seed = CURRENT_SEED.load(Ordering::Relaxed);
        // On stdout, like the other failing seeds, so the nightly log has it.
        println!("seed {seed}: panic: {info}");
        let _ = std::io::stdout().flush();
    }));
    let run = |seed| {
        CURRENT_SEED.store(seed, Ordering::Relaxed);
        ping_pong_result(seed, messages)
    };
    let failures = sweep_with(from, count, run, |failure| {
        println!("seed {}: {}", failure.seed, failure.reason);
    });
    println!(
        "swept {count} seeds from {from} ({messages} messages), {} failed",
        failures.len()
    );
    if failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// `run --seed N [--messages M]` or `sweep --from S --count C [--messages M]`,
/// options in any order.
fn parse(args: &[String]) -> Result<Command, String> {
    let Some((command, options)) = args.split_first() else {
        return Err("missing command".into());
    };
    let (mut seed, mut from, mut count, mut messages) = (None, None, None, DEFAULT_MESSAGES);
    let mut rest = options.iter();
    while let Some(option) = rest.next() {
        let value = rest
            .next()
            .ok_or_else(|| format!("{option} needs a value"))?;
        let number = || -> Result<u64, String> {
            value
                .parse()
                .map_err(|_| format!("bad value {value:?} for {option}"))
        };
        match option.as_str() {
            "--seed" => seed = Some(number()?),
            "--from" => from = Some(number()?),
            "--count" => count = Some(number()?),
            "--messages" => {
                messages = value
                    .parse()
                    .map_err(|_| format!("bad message count {value:?}"))?;
            }
            _ => return Err(format!("unknown option {option:?}")),
        }
    }
    match command.as_str() {
        "run" if from.is_none() && count.is_none() => Ok(Command::Run {
            seed: seed.ok_or("--seed is required")?,
            messages,
        }),
        "sweep" if seed.is_none() => {
            let from = from.ok_or("--from is required")?;
            let count = count.ok_or("--count is required")?;
            if from.checked_add(count).is_none() {
                return Err(format!("seeds {from} + {count} go past u64::MAX"));
            }
            Ok(Command::Sweep {
                from,
                count,
                messages,
            })
        }
        "run" | "sweep" => Err(format!("option not valid for {command}")),
        other => Err(format!("unknown command {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{Command, parse};

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn parses_options_in_any_order() {
        assert_eq!(
            parse(&args(&["run", "--seed", "42"])),
            Ok(Command::Run {
                seed: 42,
                messages: 20
            })
        );
        assert_eq!(
            parse(&args(&["run", "--messages", "5", "--seed", "7"])),
            Ok(Command::Run {
                seed: 7,
                messages: 5
            })
        );
        assert_eq!(
            parse(&args(&["sweep", "--count", "10", "--from", "3"])),
            Ok(Command::Sweep {
                from: 3,
                count: 10,
                messages: 20
            })
        );
    }

    #[test]
    fn rejects_bad_input() {
        for bad in [
            &[][..],
            &["walk", "--seed", "1"],
            &["run"],
            &["run", "--seed"],
            &["run", "--seed", "x"],
            &["run", "--seed", "1", "--color", "red"],
            &["run", "--seed", "1", "--count", "2"],
            &["sweep", "--from", "1"],
            &["sweep", "--count", "1"],
            &["sweep", "--from", "1", "--count", "2", "--seed", "3"],
        ] {
            assert!(parse(&args(bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn sweep_rejects_overflowing_range() {
        let max = u64::MAX.to_string();
        assert!(parse(&args(&["sweep", "--from", &max, "--count", "2"])).is_err());
        assert!(parse(&args(&["sweep", "--from", &max, "--count", "0"])).is_ok());
    }
}
