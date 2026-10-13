//! `ndaal-sim run --seed N [--messages M]`: runs the J01 exit scenario.
//!
//! Arguments are parsed by hand: two options do not justify a dependency;
//! the product CLI (T01.12) brings `clap`.

#![forbid(unsafe_code)]
// Command-line tool: printing is its output.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::process::ExitCode;

use ndaal_sim::{Outcome, ping_pong};

const USAGE: &str = "usage: ndaal-sim run --seed <u64> [--messages <u32>]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (seed, messages) = match parse(&args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("ndaal-sim: {message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
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

/// `run --seed N [--messages M]`, options in any order.
fn parse(args: &[String]) -> Result<(u64, u32), String> {
    let Some((command, options)) = args.split_first() else {
        return Err("missing command".into());
    };
    if command != "run" {
        return Err(format!("unknown command {command:?}"));
    }
    let (mut seed, mut messages) = (None, 20);
    let mut rest = options.iter();
    while let Some(option) = rest.next() {
        let value = rest
            .next()
            .ok_or_else(|| format!("{option} needs a value"))?;
        match option.as_str() {
            "--seed" => seed = Some(value.parse().map_err(|_| format!("bad seed {value:?}"))?),
            "--messages" => {
                messages = value
                    .parse()
                    .map_err(|_| format!("bad message count {value:?}"))?;
            }
            _ => return Err(format!("unknown option {option:?}")),
        }
    }
    Ok((seed.ok_or("--seed is required")?, messages))
}

#[cfg(test)]
mod tests {
    use super::parse;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn parses_options_in_any_order() {
        assert_eq!(parse(&args(&["run", "--seed", "42"])), Ok((42, 20)));
        assert_eq!(
            parse(&args(&["run", "--messages", "5", "--seed", "7"])),
            Ok((7, 5))
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
        ] {
            assert!(parse(&args(bad)).is_err(), "{bad:?}");
        }
    }
}
