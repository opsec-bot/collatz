//! Parallel verification that every `n` below a limit reaches 1.
//!
//! Numbers are verified in increasing order, so it is enough to show that
//! each `n >= 2` eventually drops below itself: by induction the smaller
//! value is already known to reach 1. On top of that, most `n` never need to
//! be run at all (see [`crate::sieve`]):
//!
//! 1. `[2, 2^16)` is checked directly, number by number.
//! 2. `[2^16, 2^k)` is split into blocks of `2^16`, and only residues that
//!    survive a `2^16` descent sieve are run, minus `n ≡ 2, 4, 5, 8 (mod 9)`.
//! 3. `[2^k, limit)` is split into blocks of `2^k` and sieved the same way
//!    with the full `2^k` sieve.
//!
//! A depth-`d` sieve is only valid for `n >= 2^d`, which is why each sieve
//! takes over where it does.
//!
//! Each block is one unit of work, handed out to threads through an atomic
//! counter. Every unit produces a small summary that is folded into a
//! checksum in unit order, so the checksum depends on the limit and the sieve
//! depth but not on the number of threads.
//!
//! The same machinery verifies the mirror map `3n-1` ([`Variant::Minus`]),
//! whose conjecture is that every positive integer reaches one of three
//! cycles, with minima 1, 5 and 17. A cycle minimum never drops below
//! itself, so 5 and 17 count as settled; every other number must descend.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crate::map::Variant;
use crate::orbit::{run_variant_until_below, run_variant_until_below_big, RunError};
use crate::sieve::DescentSieve;

/// Numbers per unit in the directly checked region.
const DIRECT_CHUNK: u128 = 1 << 16;

/// Depth of the smaller sieve that covers `[2^16, 2^k)`, where the main
/// `2^k` sieve does not apply yet.
const INNER_SIEVE_K: u32 = 16;

/// Settings for a verification run.
#[derive(Debug, Clone)]
pub struct Config {
    /// Verify every `n < limit`.
    pub limit: u128,
    /// Depth of the descent sieve (modulus `2^sieve_k`).
    pub sieve_k: u32,
    /// Worker threads.
    pub threads: usize,
    /// Give up on (and report) any orbit that has not descended after this many steps.
    pub max_steps: u64,
    /// Which sign of the map to verify.
    pub variant: Variant,
}

impl Config {
    /// Default settings: a `2^24` sieve and one thread per available core.
    pub fn new(limit: u128) -> Self {
        Config {
            limit,
            sieve_k: 24,
            threads: std::thread::available_parallelism().map_or(1, |n| n.get()),
            max_steps: 1 << 20,
            variant: Variant::Plus,
        }
    }
}

/// What a successful run found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub limit: u128,
    pub sieve_k: u32,
    /// Numbers whose orbits were actually followed.
    pub tested: u64,
    /// Shortcut steps taken over all tested numbers.
    pub total_steps: u128,
    /// The tested number with the longest stopping time, and that time.
    pub longest: (u128, u64),
    /// FNV-1a hash of the per-unit results, for comparing runs.
    pub checksum: u64,
    pub threads: usize,
    pub elapsed: Duration,
}

impl Report {
    /// Fraction of the range that never had to be run.
    pub fn skipped_fraction(&self) -> f64 {
        1.0 - self.tested as f64 / self.limit.max(1) as f64
    }

    /// Numbers covered per second, counting skipped ones.
    pub fn numbers_per_second(&self) -> f64 {
        self.limit as f64 / self.elapsed.as_secs_f64().max(1e-9)
    }
}

/// An orbit that refused to come down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyError {
    /// `n` did not drop below itself within the step limit. This is either a
    /// counterexample or (far more likely) a step limit set too low.
    StepLimit { n: u128, max_steps: u64 },
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifyError::StepLimit { n, max_steps } => write!(
                f,
                "{n} did not drop below itself within {max_steps} steps; \
                 re-check it with the exact engine before believing anything"
            ),
        }
    }
}

impl std::error::Error for VerifyError {}

#[derive(Debug, Clone, Copy, Default)]
struct UnitResult {
    tested: u64,
    steps: u64,
    longest: (u128, u64),
}

impl UnitResult {
    #[inline]
    fn record(&mut self, n: u128, steps: u64) {
        self.tested += 1;
        self.steps += steps;
        if steps > self.longest.1 {
            self.longest = (n, steps);
        }
    }
}

/// Shortcut steps until `n` drops below itself, falling back to exact
/// arithmetic on overflow.
#[inline]
fn descent_steps(variant: Variant, n: u128, max_steps: u64) -> Result<u64, VerifyError> {
    if variant == Variant::Minus && (n == 5 || n == 17) {
        // Minima of the two extra 3n-1 cycles: they are their own destination.
        return Ok(0);
    }
    let limit_hit = VerifyError::StepLimit { n, max_steps };
    match run_variant_until_below(variant, n, n, max_steps) {
        Ok(run) => Ok(run.steps),
        Err(RunError::Overflow) => {
            run_variant_until_below_big(variant, &n.into(), &n.into(), max_steps)
                .map(|run| run.steps)
                .map_err(|_| limit_hit)
        }
        Err(RunError::StepLimit) => Err(limit_hit),
    }
}

/// One sieved stretch of the range, cut into blocks of `2^k`.
struct Level {
    modulus: u128,
    survivors: Vec<u64>,
    survivors_mod9: Vec<u8>,
    /// The first block starts at `first_block * modulus`.
    first_block: u128,
    units: usize,
}

/// How the range is cut into units: a directly checked start, then one level
/// per sieve depth, each starting where its proof becomes valid.
struct Layout {
    limit: u128,
    max_steps: u64,
    variant: Variant,
    mod9_mask: u16,
    direct_end: u128,
    direct_units: usize,
    levels: Vec<Level>,
    total_units: usize,
}

impl Layout {
    fn new(config: &Config) -> Self {
        // The 3n-1 mod 9 rule needs n >= 14, so its sieved region must start at 16 or above.
        assert!(
            config.variant == Variant::Plus || config.sieve_k >= 4,
            "verifying 3n-1 needs a sieve depth of at least 4"
        );
        let mut depths = vec![config.sieve_k.min(INNER_SIEVE_K), config.sieve_k];
        depths.dedup();
        let direct_end = config.limit.min(1 << depths[0]);
        let direct_units = direct_end.div_ceil(DIRECT_CHUNK) as usize;
        let mut total_units = direct_units;
        let mut levels = Vec::new();
        for (i, &k) in depths.iter().enumerate() {
            let modulus = 1u128 << k;
            // A depth-k sieve is valid from 2^k on, and hands over to the next level.
            let end = depths
                .get(i + 1)
                .map_or(config.limit, |&next| config.limit.min(1 << next));
            let units = end.saturating_sub(modulus).div_ceil(modulus);
            let units = usize::try_from(units).expect("limit too large for this sieve");
            let sieve = DescentSieve::with_variant(config.variant, k);
            levels.push(Level {
                modulus,
                survivors_mod9: sieve.survivors().iter().map(|&r| (r % 9) as u8).collect(),
                survivors: sieve.survivors().to_vec(),
                first_block: 1,
                units,
            });
            total_units += units;
        }
        Layout {
            limit: config.limit,
            max_steps: config.max_steps,
            variant: config.variant,
            mod9_mask: crate::sieve::mod9_skip_mask(config.variant),
            direct_end,
            direct_units,
            levels,
            total_units,
        }
    }

    /// Calls `f` on every number in `unit` that has to be run, in increasing order.
    #[inline]
    fn for_each_candidate<E>(
        &self,
        mut unit: usize,
        mut f: impl FnMut(u128) -> Result<(), E>,
    ) -> Result<(), E> {
        if unit < self.direct_units {
            let start = (unit as u128 * DIRECT_CHUNK).max(2);
            let end = ((unit as u128 + 1) * DIRECT_CHUNK).min(self.direct_end);
            return (start..end).try_for_each(f);
        }
        unit -= self.direct_units;
        let level = self
            .levels
            .iter()
            .find(|level| {
                let here = unit < level.units;
                if !here {
                    unit -= level.units;
                }
                here
            })
            .expect("unit index in range");
        let base = (level.first_block + unit as u128) * level.modulus;
        let base_mod9 = (base % 9) as u8;
        for (&r, &r_mod9) in level.survivors.iter().zip(&level.survivors_mod9) {
            let n = base + u128::from(r);
            if n >= self.limit {
                break;
            }
            if self.mod9_mask >> ((base_mod9 + r_mod9) % 9) & 1 == 0 {
                f(n)?;
            }
        }
        Ok(())
    }

    fn run_unit(&self, unit: usize) -> Result<UnitResult, VerifyError> {
        let mut result = UnitResult::default();
        self.for_each_candidate(unit, |n| {
            result.record(n, descent_steps(self.variant, n, self.max_steps)?);
            Ok(())
        })?;
        Ok(result)
    }
}

/// Every number a verification run with this limit and sieve depth actually
/// follows, in increasing order. Everything else below the limit is covered
/// by the sieves.
pub fn candidates(limit: u128, sieve_k: u32) -> Vec<u128> {
    let layout = Layout::new(&Config {
        sieve_k,
        ..Config::new(limit)
    });
    let mut out = Vec::new();
    for unit in 0..layout.total_units {
        let _ = layout.for_each_candidate(unit, |n| {
            out.push(n);
            Ok::<(), ()>(())
        });
    }
    out
}

/// FNV-1a, 64-bit.
fn fnv1a(hash: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(hash, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Verifies every `n < config.limit`.
pub fn verify(config: &Config) -> Result<Report, VerifyError> {
    verify_with_progress(config, |_, _| {})
}

/// Like [`verify`], calling `progress(done, total)` as units finish.
pub fn verify_with_progress(
    config: &Config,
    progress: impl Fn(usize, usize) + Sync,
) -> Result<Report, VerifyError> {
    let started = Instant::now();
    let layout = Layout::new(config);
    let threads = config.threads.max(1);

    let next_unit = AtomicUsize::new(0);
    let done_units = AtomicUsize::new(0);
    let failed = AtomicBool::new(false);

    let worker = || {
        let mut results = Vec::new();
        while !failed.load(Ordering::Relaxed) {
            let unit = next_unit.fetch_add(1, Ordering::Relaxed);
            if unit >= layout.total_units {
                break;
            }
            match layout.run_unit(unit) {
                Ok(result) => results.push((unit, result)),
                Err(err) => {
                    failed.store(true, Ordering::Relaxed);
                    return Err(err);
                }
            }
            progress(
                done_units.fetch_add(1, Ordering::Relaxed) + 1,
                layout.total_units,
            );
        }
        Ok(results)
    };

    let outcomes: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads).map(|_| scope.spawn(worker)).collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("worker thread panicked"))
            .collect()
    });

    let mut units = Vec::with_capacity(layout.total_units);
    let mut first_error: Option<VerifyError> = None;
    for outcome in outcomes {
        match outcome {
            Ok(results) => units.extend(results),
            Err(err) => {
                let VerifyError::StepLimit { n, .. } = err;
                if first_error.map_or(true, |VerifyError::StepLimit { n: m, .. }| n < m) {
                    first_error = Some(err);
                }
            }
        }
    }
    if let Some(err) = first_error {
        return Err(err);
    }
    units.sort_unstable_by_key(|&(unit, _)| unit);

    let mut report = Report {
        limit: config.limit,
        sieve_k: config.sieve_k,
        tested: 0,
        total_steps: 0,
        longest: (0, 0),
        checksum: 0xcbf2_9ce4_8422_2325,
        threads,
        elapsed: Duration::ZERO,
    };
    for (_, unit) in &units {
        report.tested += unit.tested;
        report.total_steps += u128::from(unit.steps);
        if unit.longest.1 > report.longest.1 {
            report.longest = unit.longest;
        }
        report.checksum = fnv1a(report.checksum, &unit.tested.to_le_bytes());
        report.checksum = fnv1a(report.checksum, &unit.steps.to_le_bytes());
    }
    report.elapsed = started.elapsed();
    Ok(report)
}

/// The obvious algorithm, for comparison: run every `n` in `[start, end)` one
/// standard step at a time until it drops below itself. Returns the total
/// number of standard steps.
pub fn naive_check_range(start: u128, end: u128) -> u64 {
    let mut total = 0;
    for n in start.max(2)..end {
        let mut x = n;
        while x >= n {
            x = if x % 2 == 0 { x / 2 } else { 3 * x + 1 };
            total += 1;
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbit::stopping_time;

    fn config(limit: u128, sieve_k: u32, threads: usize) -> Config {
        Config {
            limit,
            sieve_k,
            threads,
            max_steps: 1 << 20,
            variant: Variant::Plus,
        }
    }

    #[test]
    fn tiny_limits() {
        for limit in 0..4 {
            let report = verify(&config(limit, 4, 2)).unwrap();
            assert_eq!(report.tested, limit.saturating_sub(2) as u64);
        }
    }

    #[test]
    fn checksum_does_not_depend_on_threads() {
        let one = verify(&config(5_000_000, 12, 1)).unwrap();
        for threads in [2, 3, 8] {
            let many = verify(&config(5_000_000, 12, threads)).unwrap();
            assert_eq!(one.checksum, many.checksum);
            assert_eq!(one.tested, many.tested);
            assert_eq!(one.total_steps, many.total_steps);
            assert_eq!(one.longest, many.longest);
        }
    }

    #[test]
    fn tests_exactly_the_unsieved_numbers() {
        // Depth 10 uses one sieve; depth 18 adds the inner 2^16 sieve below 2^18.
        for (limit, k) in [(300_000u128, 10), (1_500_000, 18)] {
            let inner = DescentSieve::new(k.min(INNER_SIEVE_K));
            let outer = DescentSieve::new(k);
            let mut tested = 0;
            let mut steps = 0u128;
            let mut longest = (0, 0);
            for n in 2..limit {
                let sieve = if n < u128::from(outer.modulus()) {
                    &inner
                } else {
                    &outer
                };
                let sieved = n >= u128::from(sieve.modulus())
                    && (!sieve.contains(n) || crate::sieve::mod9_skippable(n));
                if sieved {
                    continue;
                }
                let s = stopping_time(n).unwrap();
                tested += 1;
                steps += u128::from(s);
                if s > longest.1 {
                    longest = (n, s);
                }
            }
            let report = verify(&config(limit, k, 4)).unwrap();
            assert_eq!(candidates(limit, k).len() as u64, tested, "k = {k}");
            assert_eq!(report.tested, tested, "k = {k}");
            assert_eq!(report.total_steps, steps, "k = {k}");
            assert_eq!(report.longest, longest, "k = {k}");
        }
    }

    #[test]
    fn longest_stopping_time_below_a_million() {
        // 626331 holds the stopping-time record below 10^6 (A060412).
        let report = verify(&config(1_000_000, 16, 4)).unwrap();
        assert_eq!(report.longest.0, 626_331);
    }

    #[test]
    fn overflowing_orbits_use_the_exact_engine() {
        // 2^127 - 1 climbs far past u128 before it comes back down.
        let n = u128::MAX >> 1;
        let exact = crate::orbit::run_until_below_big(&n.into(), &n.into(), u64::MAX).unwrap();
        assert_eq!(descent_steps(Variant::Plus, n, u64::MAX), Ok(exact.steps));
    }

    #[test]
    fn minus_variant_tests_exactly_the_unsieved_numbers() {
        let (limit, k) = (1_500_000u128, 18);
        let inner = DescentSieve::with_variant(Variant::Minus, k.min(INNER_SIEVE_K));
        let outer = DescentSieve::with_variant(Variant::Minus, k);
        let skip = crate::sieve::MOD9_SKIP_MINUS;
        let (mut tested, mut steps) = (0u64, 0u128);
        for n in 2..limit {
            let sieve = if n < u128::from(outer.modulus()) {
                &inner
            } else {
                &outer
            };
            if n >= u128::from(sieve.modulus()) && (!sieve.contains(n) || skip >> (n % 9) & 1 == 1)
            {
                continue;
            }
            tested += 1;
            if n != 5 && n != 17 {
                let run = run_variant_until_below(Variant::Minus, n, n, u64::MAX).unwrap();
                steps += u128::from(run.steps);
            }
        }
        let mut cfg = config(limit, k, 4);
        cfg.variant = Variant::Minus;
        let report = verify(&cfg).unwrap();
        assert_eq!((report.tested, report.total_steps), (tested, steps));
        // The checksum tells the two maps apart.
        assert_ne!(
            report.checksum,
            verify(&config(limit, k, 4)).unwrap().checksum
        );
    }

    #[test]
    fn step_limit_is_reported() {
        let mut cfg = config(100_000, 8, 4);
        cfg.max_steps = 50;
        let err = verify(&cfg).unwrap_err();
        // 27 is the smallest number that needs more than 50 shortcut steps.
        assert_eq!(
            err,
            VerifyError::StepLimit {
                n: 27,
                max_steps: 50
            }
        );
    }

    #[test]
    fn naive_agrees_with_stopping_times() {
        let expected: u64 = (2..5_000u128)
            .map(|n| {
                crate::orbit::run_until_below(n, n, u64::MAX)
                    .unwrap()
                    .standard_steps()
            })
            .sum();
        assert_eq!(naive_check_range(0, 5_000), expected);
    }
}
