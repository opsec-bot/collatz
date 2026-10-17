//! `collatz`: verify and explore the Collatz (3n+1) conjecture.

#[macro_use]
mod cli;

use cli::{commands, CliError};

const USAGE: &str = "\
collatz: a fast, dependency-free Collatz (3n+1) verifier and explorer

USAGE
  collatz <command> [arguments]

COMMANDS
  orbit <n>          Summarize and chart the orbit of n (any size, e.g. 2^100+1)
      --map <m>        standard, shortcut or syracuse            [standard]
      --parity <k>     length of the parity vector shown (<= 64)  [32]
      --width <w>      chart width                                [72]
      --height <h>     chart height                               [10]
      --full           also print every value

  verify <limit>     Prove every n < limit reaches 1, in parallel
      -t, --threads    worker threads                             [all cores]
      -k, --sieve      descent sieve depth                        [24]
      --max-steps      report orbits that take longer than this   [2^20]

  records <limit>    Path records: orbits that climb higher than all before
      -t, --threads, -k, --sieve

  delays <limit>     Delay records: starts that take longer than all before

  sieve              Survival table for the 2^k descent sieve
      --depth <k>      deepest level to show                      [28]

  cycles             Find cycles of n/2 (even), a*n+b (odd)
      --a <a> --b <b>  the odd step                               [3, 1]
      --range <r>      try every start in -r..=r                  [100000]
      --positive       only positive starts
      --max-steps      give up on an orbit after this many steps  [100000]

  patterns [L]       Find every integer cycle of length <= L via the cycle equation [20]

  bench [limit]      Compare the naive loop with each optimization  [2^26]

  help               Show this message

Numbers may be written as expressions: 2^32, 1e9, 3*2^60+1, (2^64-1)^2.
";

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = argv.split_first() else {
        out!("{}", USAGE.trim_end());
        return;
    };
    let result = match command.as_str() {
        "orbit" => commands::orbit(rest),
        "verify" => commands::verify(rest),
        "records" => commands::records(rest),
        "delays" => commands::delays(rest),
        "sieve" => commands::sieve(rest),
        "cycles" => commands::cycles(rest),
        "patterns" => commands::patterns(rest),
        "bench" => commands::bench(rest),
        "help" | "--help" | "-h" => {
            out!("{}", USAGE.trim_end());
            Ok(())
        }
        other => Err(CliError::Usage(format!("unknown command '{other}'"))),
    };
    match result {
        Ok(()) => {}
        Err(CliError::Usage(message)) => {
            eprintln!("error: {message}\n\nRun 'collatz help' for usage.");
            std::process::exit(2);
        }
        Err(CliError::Failure(message)) => {
            eprintln!("error: {message}");
            std::process::exit(1);
        }
    }
}
