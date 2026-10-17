//! One function per subcommand.

use std::io::{IsTerminal, Write};
use std::time::Instant;

use collatz::cycles::{census, cycles_with_period, GeneralizedMap};
use collatz::map::{Map, Variant};
use collatz::orbit::{parity_vector, summarize, trajectory};
use collatz::plot::{line_chart, scatter_chart, Axes, Scale, Series};
use collatz::records::{delay_records, path_records};
use collatz::sieve::DescentSieve;
use collatz::verify::{naive_check_range, verify_with_progress, Config};
use collatz::{expr, BigUint};

use super::args::{Args, Spec};
use super::chart::bars;
use super::format::{abbreviate, duration, group, limit, table, words, Align};
use super::{CliError, CliResult};

fn default_threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

/// Draws a progress bar on stderr, if stderr is a terminal.
fn progress_bar(label: &'static str) -> impl Fn(usize, usize) + Sync {
    let enabled = std::io::stderr().is_terminal();
    let last = std::sync::atomic::AtomicUsize::new(usize::MAX);
    move |done, total| {
        if !enabled || total == 0 {
            return;
        }
        let percent = done * 100 / total;
        if last.swap(percent, std::sync::atomic::Ordering::Relaxed) == percent {
            return;
        }
        let filled = percent * 30 / 100;
        let mut err = std::io::stderr().lock();
        let _ = write!(
            err,
            "\r  {label} [{}{}] {percent:>3}%",
            "█".repeat(filled),
            "·".repeat(30 - filled)
        );
        if done == total {
            let _ = write!(err, "\r{}\r", " ".repeat(50));
        }
        let _ = err.flush();
    }
}

fn parse_limit(args: &Args) -> Result<u128, CliError> {
    Ok(expr::parse_u128(args.required(0, "limit")?)?)
}

fn parse_sieve(args: &Args, default: u32) -> Result<u32, CliError> {
    let k = args.get("sieve", default)?;
    if k > DescentSieve::MAX_K {
        return Err(CliError::Usage(format!(
            "--sieve must be at most {}",
            DescentSieve::MAX_K
        )));
    }
    Ok(k)
}

pub fn orbit(argv: &[String]) -> CliResult {
    let args = Args::parse(
        argv,
        &Spec {
            values: &["map", "parity", "width", "height"],
            switches: &["full"],
        },
    )?;
    args.no_extra_positional(1)?;
    let n = expr::parse(args.required(0, "a starting number")?)?;
    if n.is_zero() {
        return Err(CliError::Usage("the Collatz map starts at 1".into()));
    }
    let map: Map = args.get("map", Map::Standard)?;
    let parity_len = args.get("parity", 32u32)?.min(64);
    let width = args.get("width", 72usize)?;
    let height = args.get("height", 10usize)?;

    let summary = summarize(&n);
    let start = abbreviate(&n, 40);
    out!();
    out!(
        "  {start}  reaches 1 in {} steps",
        group(summary.standard_steps())
    );
    out!();
    let rows = [
        (
            "standard steps",
            format!(
                "{}  ({} odd, {} even)",
                group(summary.standard_steps()),
                group(summary.odd_steps),
                group(summary.even_steps())
            ),
        ),
        ("shortcut steps", group(summary.shortcut_steps)),
        (
            "stopping time",
            summary.stopping_time.map_or("undefined for 1".into(), |s| {
                format!("{} shortcut steps to drop below the start", group(s))
            }),
        ),
        ("peak", {
            let peak = abbreviate(&summary.peak, 40);
            let growth = summary.peak.log2() - n.log2();
            let times = if growth < 40.0 {
                format!("{}x", group(2f64.powf(growth).round()))
            } else {
                format!("2^{growth:.1} times")
            };
            format!("{peak}  (2^{:.2}, {times} the start)", summary.peak.log2())
        }),
        (
            "parity",
            parity_vector(&n, parity_len)
                .iter()
                .map(|&odd| if odd { '1' } else { '0' })
                .collect::<String>()
                + &format!("  (first {parity_len} shortcut steps, 1 = odd)"),
        ),
    ];
    for (name, value) in rows {
        out!("  {name:<15} {value}");
    }

    let values: Vec<BigUint> = trajectory(&n, map).collect();
    let logs: Vec<f64> = values.iter().map(BigUint::log2).collect();
    out!();
    out!(
        "  log2 of every value under the {} map ({} values)",
        map.name(),
        group(values.len())
    );
    out!();
    out!("{}", bars(&logs, width, height, |v| format!("2^{v:.1}")));

    if args.switch("full") {
        out!();
        let joined: Vec<String> = values.iter().map(ToString::to_string).collect();
        let mut line = String::from(" ");
        for value in joined {
            if line.len() + value.len() > 100 {
                out!("{line}");
                line = String::from(" ");
            }
            line.push(' ');
            line.push_str(&value);
        }
        out!("{line}");
    }
    out!();
    Ok(())
}

pub fn verify(argv: &[String]) -> CliResult {
    let args = Args::parse(
        argv,
        &Spec {
            values: &["threads", "sieve", "max-steps"],
            switches: &["minus"],
        },
    )?;
    args.no_extra_positional(1)?;
    let mut config = Config::new(parse_limit(&args)?);
    config.threads = args.get("threads", config.threads)?;
    config.sieve_k = parse_sieve(&args, config.sieve_k)?;
    config.max_steps = args.get("max-steps", config.max_steps)?;
    if args.switch("minus") {
        config.variant = Variant::Minus;
        if config.sieve_k < 4 {
            return Err(CliError::Usage("--minus needs --sieve 4 or more".into()));
        }
    }

    out!();
    out!(
        "  Verifying every n < {} under {} on {} threads with a 2^{} sieve",
        limit(config.limit),
        config.variant,
        config.threads,
        config.sieve_k
    );
    out!();
    let report = verify_with_progress(&config, progress_bar("verifying"))
        .map_err(|e| CliError::Failure(e.to_string()))?;

    let destination = match config.variant {
        Variant::Plus => "1",
        Variant::Minus => "one of the cycles through 1, 5 and 17",
    };
    out!(
        "  ✓ every n < {} reaches {destination}",
        group(report.limit)
    );
    out!();
    let rows = [
        (
            "orbits run",
            format!(
                "{}  ({:.2}% skipped by the sieves)",
                group(report.tested),
                report.skipped_fraction() * 100.0
            ),
        ),
        (
            "steps taken",
            format!("{} shortcut steps", group(report.total_steps)),
        ),
        (
            "slowest",
            format!(
                "{} needs {} steps to drop below itself",
                group(report.longest.0),
                report.longest.1
            ),
        ),
        ("checksum", format!("{:016x}", report.checksum)),
        (
            "time",
            format!(
                "{}  ({} numbers/s)",
                duration(report.elapsed),
                words(report.numbers_per_second())
            ),
        ),
    ];
    for (name, value) in rows {
        out!("  {name:<12} {value}");
    }
    out!();
    Ok(())
}

pub fn records(argv: &[String]) -> CliResult {
    let args = Args::parse(
        argv,
        &Spec {
            values: &["threads", "sieve"],
            switches: &[],
        },
    )?;
    args.no_extra_positional(1)?;
    let max = parse_limit(&args)?;
    let threads = args.get("threads", default_threads())?;
    let k = parse_sieve(&args, 24)?.max(3);

    let started = Instant::now();
    let records = path_records(max, k, threads);
    let rows: Vec<Vec<String>> = records
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let exponent = if r.n < 2 {
                "-".into()
            } else {
                format!("{:.3}", r.peak.log2() / (r.n as f64).log2())
            };
            vec![(i + 1).to_string(), group(r.n), group(&r.peak), exponent]
        })
        .collect();
    out!();
    out!("  Path records below {}", limit(max));
    out!("  (orbits that climb higher than every smaller start)");
    out!();
    out!(
        "{}",
        table(
            &[
                ("#", Align::Right),
                ("n", Align::Right),
                ("peak", Align::Right),
                ("log_n(peak)", Align::Right),
            ],
            &rows
        )
    );
    out!();
    out!(
        "  {} records in {}. Peaks grow roughly like n^2, as Lagarias and Weiss predicted.",
        records.len(),
        duration(started.elapsed())
    );
    out!();
    Ok(())
}

pub fn delays(argv: &[String]) -> CliResult {
    let args = Args::parse(
        argv,
        &Spec {
            values: &[],
            switches: &[],
        },
    )?;
    args.no_extra_positional(1)?;
    let max = parse_limit(&args)?;
    let max = usize::try_from(max)
        .ok()
        .filter(|&m| m <= 1 << 32)
        .ok_or_else(|| {
            CliError::Usage("delay records need a table; keep the limit at most 2^32".into())
        })?;

    let started = Instant::now();
    let records = delay_records(max);
    let rows: Vec<Vec<String>> = records
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let ratio = if r.n < 2 {
                "-".into()
            } else {
                format!("{:.2}", r.steps as f64 / (r.n as f64).ln())
            };
            vec![(i + 1).to_string(), group(r.n), group(r.steps), ratio]
        })
        .collect();
    out!();
    out!("  Delay records below {}", limit(max as u128));
    out!("  (starts that need more steps to reach 1 than every smaller start)");
    out!();
    out!(
        "{}",
        table(
            &[
                ("#", Align::Right),
                ("n", Align::Right),
                ("steps", Align::Right),
                ("steps / ln n", Align::Right),
            ],
            &rows
        )
    );
    out!();
    out!(
        "  {} records in {}.",
        records.len(),
        duration(started.elapsed())
    );
    out!();
    Ok(())
}

pub fn sieve(argv: &[String]) -> CliResult {
    let args = Args::parse(
        argv,
        &Spec {
            values: &["depth"],
            switches: &[],
        },
    )?;
    args.no_extra_positional(0)?;
    let depth = args.get("depth", 28u32)?;
    if depth > DescentSieve::MAX_K {
        return Err(CliError::Usage(format!(
            "--depth must be at most {}",
            DescentSieve::MAX_K
        )));
    }
    let started = Instant::now();
    let counts = DescentSieve::survivor_counts(depth);
    let rows: Vec<Vec<String>> = counts
        .iter()
        .enumerate()
        .skip(1)
        .map(|(k, &survivors)| {
            let density = survivors as f64 / (1u64 << k) as f64;
            // Of the survivors, 4 in 9 are also skipped by the mod 9 sieve.
            let combined = density * 5.0 / 9.0;
            vec![
                k.to_string(),
                group(1u64 << k),
                group(survivors),
                format!("{:.3}%", density * 100.0),
                format!("{:.3}%", combined * 100.0),
                format!("{:.0}x", 1.0 / combined),
            ]
        })
        .collect();
    out!();
    out!("  How much of each residue class mod 2^k must actually be run");
    out!();
    out!(
        "{}",
        table(
            &[
                ("k", Align::Right),
                ("residues", Align::Right),
                ("survivors", Align::Right),
                ("density", Align::Right),
                ("with mod 9", Align::Right),
                ("speedup", Align::Right),
            ],
            &rows
        )
    );
    out!();
    out!("  Built in {}.", duration(started.elapsed()));
    out!();
    Ok(())
}

pub fn cycles(argv: &[String]) -> CliResult {
    let args = Args::parse(
        argv,
        &Spec {
            values: &["a", "b", "range", "max-steps"],
            switches: &["positive"],
        },
    )?;
    args.no_extra_positional(0)?;
    let map = GeneralizedMap {
        a: args.get("a", 3i128)?,
        b: args.get("b", 1i128)?,
    };
    let range = args.get("range", 100_000i128)?;
    let max_steps = args.get("max-steps", 100_000u64)?;
    let lowest = if args.switch("positive") { 1 } else { -range };
    let starts = (lowest..=range).filter(|&n| n != 0);

    let started = Instant::now();
    let mut found = census(map, starts, 1 << 100, max_steps);
    found.cycles.sort_by_key(|c| {
        (
            c.cycle.members()[0].unsigned_abs(),
            c.cycle.members()[0] < 0,
        )
    });
    out!();
    out!(
        "  Cycles of n -> n/2 (even), {map} (odd), for starts {} to {}",
        group(lowest),
        group(range)
    );
    out!();
    for (i, c) in found.cycles.iter().enumerate() {
        let members = c.cycle.members();
        let shown: Vec<String> = members.iter().take(12).map(ToString::to_string).collect();
        let more = if members.len() > 12 { " ..." } else { "" };
        out!(
            "  cycle {}  length {}, {} odd, reached from {} starts",
            i + 1,
            members.len(),
            c.cycle.odd_members(),
            group(c.starts)
        );
        out!("           {}{more}", shown.join(" → "));
        out!();
    }
    if found.escaped.is_empty() {
        out!("  No orbit escaped past 2^100.");
    } else {
        let shown: Vec<String> = found
            .escaped
            .iter()
            .take(10)
            .map(ToString::to_string)
            .collect();
        out!(
            "  {} starts escaped past 2^100 or {} steps without repeating, e.g. {}",
            group(found.escaped.len()),
            group(max_steps),
            shown.join(", ")
        );
    }
    out!("  Done in {}.", duration(started.elapsed()));
    out!();
    Ok(())
}

pub fn patterns(argv: &[String]) -> CliResult {
    let args = Args::parse(
        argv,
        &Spec {
            values: &[],
            switches: &[],
        },
    )?;
    args.no_extra_positional(1)?;
    let max_len: u32 = match args.positional().first() {
        None => 20,
        Some(s) => s
            .parse()
            .map_err(|_| format!("invalid pattern length '{s}'"))?,
    };
    if !(1..=32).contains(&max_len) {
        return Err(CliError::Usage(
            "pattern length must be between 1 and 32".into(),
        ));
    }

    let started = Instant::now();
    out!();
    out!("  Solving n = c / (2^L - 3^k) for every parity pattern of length L <= {max_len}");
    out!();
    let mut seen = std::collections::BTreeSet::new();
    let mut tried = 0u64;
    for len in 1..=max_len {
        tried += 1 << len;
        for cycle in cycles_with_period(len) {
            if seen.insert(cycle.clone()) {
                let members: Vec<String> =
                    cycle.members().iter().map(ToString::to_string).collect();
                out!("  L = {len:>2}  {}", members.join(" → "));
            }
        }
    }
    out!();
    out!(
        "  {} patterns checked in {}: these are the only integer cycles of length <= {max_len}.",
        group(tried),
        duration(started.elapsed())
    );
    out!("  (Lengths are in shortcut steps. Only 1 → 2 is positive.)");
    out!();
    Ok(())
}

pub fn bench(argv: &[String]) -> CliResult {
    let args = Args::parse(
        argv,
        &Spec {
            values: &["threads"],
            switches: &[],
        },
    )?;
    args.no_extra_positional(1)?;
    let max = match args.positional().first() {
        Some(s) => expr::parse_u128(s)?,
        None => 1 << 26,
    };
    let threads = args.get("threads", default_threads())?;
    let k = (128 - max.leading_zeros()).saturating_sub(6).clamp(3, 24);

    out!();
    out!(
        "  Checking every n < {} four ways (2^{k} sieve), single-threaded unless noted",
        limit(max)
    );
    out!();
    let mut rows = Vec::new();
    let mut baseline = None;
    let mut time = |name: String, f: &dyn Fn()| {
        let started = Instant::now();
        f();
        let secs = started.elapsed().as_secs_f64();
        let base = *baseline.get_or_insert(secs);
        rows.push(vec![
            name,
            duration(started.elapsed()),
            format!("{} /s", words(max as f64 / secs)),
            format!("{:.1}x", base / secs),
        ]);
    };

    // The numbers the sieves leave over, exactly as `verify` would run them.
    let survivors = collatz::verify::candidates(max, k);
    let naive_descent = |n: u128| {
        let mut x = n;
        while x >= n {
            x = if x % 2 == 0 { x / 2 } else { 3 * x + 1 };
        }
        x
    };

    time("naive loop, every n".into(), &|| {
        std::hint::black_box(naive_check_range(2, max));
    });
    time("naive loop, sieved".into(), &|| {
        for &n in &survivors {
            std::hint::black_box(naive_descent(n));
        }
    });
    time("run-skipping engine, sieves".into(), &|| {
        for &n in &survivors {
            std::hint::black_box(collatz::orbit::run_until_below(n, n, u64::MAX).ok());
        }
    });
    time(format!("engine + sieves, {threads} threads"), &|| {
        let config = Config {
            threads,
            sieve_k: k,
            ..Config::new(max)
        };
        std::hint::black_box(collatz::verify::verify(&config).ok());
    });

    out!(
        "{}",
        table(
            &[
                ("method", Align::Left),
                ("time", Align::Right),
                ("numbers", Align::Right),
                ("speedup", Align::Right),
            ],
            &rows
        )
    );
    out!();
    Ok(())
}

pub fn plot(argv: &[String]) -> CliResult {
    let args = Args::parse(
        argv,
        &Spec {
            values: &["output", "threads"],
            switches: &[],
        },
    )?;
    let kind = args.required(0, "a chart kind (orbits, stopping or records)")?;
    let rest = &args.positional()[1..];
    let svg = match kind {
        "orbits" => {
            let starts: Vec<BigUint> = if rest.is_empty() {
                ["27", "703", "6171", "77671"]
                    .iter()
                    .map(|s| s.parse().unwrap())
                    .collect()
            } else {
                rest.iter()
                    .map(|s| expr::parse(s))
                    .collect::<Result<_, _>>()?
            };
            if starts.iter().any(BigUint::is_zero) {
                return Err(CliError::Usage("orbits start at 1".into()));
            }
            if starts.len() > collatz::plot::MAX_SERIES {
                return Err(CliError::Usage(format!(
                    "at most {} orbits fit on one chart",
                    collatz::plot::MAX_SERIES
                )));
            }
            let series: Vec<Series> = starts
                .iter()
                .map(|n| {
                    let points: Vec<(f64, f64)> = trajectory(n, Map::Standard)
                        .enumerate()
                        .map(|(i, x)| (i as f64, x.log2()))
                        .collect();
                    let peak =
                        points
                            .iter()
                            .copied()
                            .fold((0.0, f64::MIN), |a, b| if b.1 > a.1 { b } else { a });
                    let summary = summarize(n);
                    Series {
                        label: format!("{} ({} steps)", group(n), summary.standard_steps()),
                        points,
                        callout: Some((peak.0, peak.1, format!("peak {}", group(&summary.peak)))),
                    }
                })
                .collect();
            line_chart(
                "Collatz orbits climb, then fall",
                "Every value along each orbit under n → n/2 or 3n+1, on a log scale",
                &Axes {
                    x_label: "step".into(),
                    y_label: "value".into(),
                    x_scale: Scale::Linear,
                    y_scale: Scale::Log2,
                },
                &series,
            )
        }
        "stopping" => {
            let max = rest.first().map_or(Ok(10_000), |s| expr::parse_u128(s))?;
            let max = usize::try_from(max)
                .ok()
                .filter(|&m| (2..=1 << 20).contains(&m))
                .ok_or_else(|| {
                    CliError::Usage("keep the stopping-time limit between 2 and 2^20".into())
                })?;
            let table = collatz::records::total_stopping_times(max);
            let points: Vec<(f64, f64)> = table
                .iter()
                .enumerate()
                .skip(1)
                .map(|(n, &steps)| (n as f64, f64::from(steps)))
                .collect();
            let (n, steps) = table
                .iter()
                .enumerate()
                .max_by_key(|&(n, &s)| (s, std::cmp::Reverse(n)))
                .unwrap();
            scatter_chart(
                &format!("Steps to reach 1, for every n < {}", group(max)),
                "Total stopping time under the standard map; the bands come from merging orbits",
                &Axes {
                    x_label: "n".into(),
                    y_label: "steps".into(),
                    x_scale: Scale::Linear,
                    y_scale: Scale::Linear,
                },
                &points,
                None,
                &[(
                    n as f64,
                    f64::from(*steps),
                    format!("{} takes {steps} steps", group(n)),
                )],
            )
        }
        "records" => {
            let max = rest.first().map_or(Ok(1 << 32), |s| expr::parse_u128(s))?;
            let threads = args.get("threads", default_threads())?;
            let records = path_records(max, 24, threads);
            let points: Vec<(f64, f64)> = records
                .iter()
                .filter(|r| r.n > 1)
                .map(|r| ((r.n as f64).log2(), r.peak.log2()))
                .collect();
            let last = records
                .last()
                .ok_or_else(|| CliError::Usage("limit too small".into()))?;
            scatter_chart(
                &format!(
                    "Path records below {}",
                    limit(max).split(' ').next().unwrap_or("")
                ),
                "Each dot is an n whose orbit climbs higher than every smaller start's",
                &Axes {
                    x_label: "n".into(),
                    y_label: "peak of the orbit".into(),
                    x_scale: Scale::Log2,
                    y_scale: Scale::Log2,
                },
                &points,
                Some((2.0, "peak = n²")),
                &[(
                    (last.n as f64).log2(),
                    last.peak.log2(),
                    format!("{} peaks at {}", group(last.n), abbreviate(&last.peak, 24)),
                )],
            )
        }
        other => {
            return Err(CliError::Usage(format!(
                "unknown chart '{other}' (expected orbits, stopping or records)"
            )))
        }
    };
    match args.value("output") {
        Some(path) => {
            std::fs::write(path, svg).map_err(|e| CliError::Failure(format!("{path}: {e}")))?;
            eprintln!("  wrote {path}");
        }
        None => out!("{}", svg.trim_end()),
    }
    Ok(())
}
