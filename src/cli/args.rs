//! A minimal argument parser: positional arguments, `--name value`,
//! `--name=value` and boolean `--switch` flags.

use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::str::FromStr;

/// The flags a command accepts.
pub struct Spec {
    /// Flags that take a value.
    pub values: &'static [&'static str],
    /// Flags that stand alone.
    pub switches: &'static [&'static str],
}

/// Single-letter aliases shared by every command.
const SHORT: &[(&str, &str)] = &[("-t", "threads"), ("-o", "output"), ("-k", "sieve")];

/// Parsed command-line arguments.
#[derive(Debug, Default)]
pub struct Args {
    positional: Vec<String>,
    values: HashMap<String, String>,
    switches: HashSet<String>,
}

impl Args {
    pub fn parse(argv: &[String], spec: &Spec) -> Result<Args, String> {
        let mut args = Args::default();
        let mut iter = argv.iter();
        while let Some(arg) = iter.next() {
            let (flag, inline) = match arg.split_once('=') {
                Some((f, v)) => (f, Some(v.to_string())),
                None => (arg.as_str(), None),
            };
            let name = match SHORT.iter().find(|(short, _)| *short == flag) {
                Some((_, long)) => long.to_string(),
                None => match flag.strip_prefix("--") {
                    Some(long) => long.to_string(),
                    None => {
                        args.positional.push(arg.clone());
                        continue;
                    }
                },
            };
            if spec.values.contains(&name.as_str()) {
                let value = match inline {
                    Some(v) => v,
                    None => iter
                        .next()
                        .cloned()
                        .ok_or_else(|| format!("--{name} needs a value"))?,
                };
                args.values.insert(name, value);
            } else if spec.switches.contains(&name.as_str()) && inline.is_none() {
                args.switches.insert(name);
            } else {
                return Err(format!("unknown option '{arg}'"));
            }
        }
        Ok(args)
    }

    pub fn positional(&self) -> &[String] {
        &self.positional
    }

    /// The `i`-th positional argument, described as `what` in errors.
    pub fn required(&self, i: usize, what: &str) -> Result<&str, String> {
        self.positional
            .get(i)
            .map(String::as_str)
            .ok_or_else(|| format!("missing {what}"))
    }

    pub fn no_extra_positional(&self, allowed: usize) -> Result<(), String> {
        match self.positional.get(allowed) {
            Some(extra) => Err(format!("unexpected argument '{extra}'")),
            None => Ok(()),
        }
    }

    pub fn value(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    pub fn switch(&self, name: &str) -> bool {
        self.switches.contains(name)
    }

    /// Parses the value of `--name`, or returns `default` if it was not given.
    pub fn get<T>(&self, name: &str, default: T) -> Result<T, String>
    where
        T: FromStr,
        T::Err: Display,
    {
        match self.value(name) {
            None => Ok(default),
            Some(v) => v
                .parse()
                .map_err(|e| format!("invalid --{name} '{v}': {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: Spec = Spec {
        values: &["threads", "output"],
        switches: &["full"],
    };

    fn parse(args: &[&str]) -> Result<Args, String> {
        let argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        Args::parse(&argv, &SPEC)
    }

    #[test]
    fn mixes_positional_and_flags() {
        let args = parse(&["27", "--threads", "4", "--full", "-o=x.svg", "99"]).unwrap();
        assert_eq!(args.positional(), ["27", "99"]);
        assert_eq!(args.get("threads", 1usize), Ok(4));
        assert_eq!(args.value("output"), Some("x.svg"));
        assert!(args.switch("full"));
        assert_eq!(args.get("missing", 7u32), Ok(7));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse(&["--nope"]).is_err());
        assert!(parse(&["--threads"]).is_err());
        assert!(parse(&["--full=yes"]).is_err());
        assert!(parse(&["--threads", "x"])
            .unwrap()
            .get("threads", 1u32)
            .is_err());
    }
}
