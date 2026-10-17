//! Orbit statistics: stopping times, peaks and parity vectors.
//!
//! There are two engines:
//!
//! * [`run_until_below`] works on `u128` and skips whole runs of steps at once.
//!   A run of even steps is one trailing-zero count and a shift. A run of `a`
//!   odd steps is one multiplication: if `n + 1 = 2^a * m` with `m` odd, then
//!   `a` shortcut steps take `n` to `3^a * m - 1`.
//! * [`run_until_below_big`] steps one at a time with [`BigUint`]. It handles
//!   inputs of any size and is the fallback when `u128` overflows.
//!
//! Step counts are always in shortcut (`T`) steps plus the number of odd
//! steps among them. The standard (`C`) step count is the sum of the two,
//! because each odd `T` step is a `3n+1` followed by a halving.
//!
//! Peaks are measured under the standard map, which matches the published
//! path-record tables: the peak of 27 is 9232, the value of `3n+1` reached
//! from 3077.

use crate::bigint::BigUint;
use crate::map::{Map, POW3};

/// Why a run stopped before reaching its target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunError {
    /// A value along the orbit did not fit in a `u128`.
    Overflow,
    /// The orbit had not dropped below the target after the allowed number of steps.
    StepLimit,
}

/// The result of following an orbit with the `u128` engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run {
    /// Shortcut steps taken.
    pub steps: u64,
    /// How many of those steps were odd.
    pub odd_steps: u64,
    /// The largest value reached under the standard map, including the start.
    pub peak: u128,
    /// The first value below the target.
    pub end: u128,
}

impl Run {
    /// Steps under the standard map.
    pub fn standard_steps(&self) -> u64 {
        self.steps + self.odd_steps
    }
}

/// The result of following an orbit with the arbitrary-precision engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BigRun {
    pub steps: u64,
    pub odd_steps: u64,
    pub peak: BigUint,
    pub end: BigUint,
}

impl BigRun {
    /// Steps under the standard map.
    pub fn standard_steps(&self) -> u64 {
        self.steps + self.odd_steps
    }
}

impl From<Run> for BigRun {
    fn from(run: Run) -> Self {
        BigRun {
            steps: run.steps,
            odd_steps: run.odd_steps,
            peak: run.peak.into(),
            end: run.end.into(),
        }
    }
}

/// Follows the orbit of `start` until it first drops below `target`.
///
/// With `target == start` this measures the stopping time; with `target == 2`
/// it follows the orbit all the way to 1.
///
/// # Panics
///
/// Panics if `target < 2`: the orbit 1, 2, 1, ... never drops below 1.
pub fn run_until_below(start: u128, target: u128, max_steps: u64) -> Result<Run, RunError> {
    assert!(
        target >= 2,
        "every orbit ends in 1, 2, 1, ...; target must be at least 2"
    );
    let target_bits = 128 - target.leading_zeros();
    let mut x = start;
    let mut steps = 0u64;
    let mut odd_steps = 0u64;
    let mut peak = start;

    while x >= target {
        if steps >= max_steps {
            return Err(RunError::StepLimit);
        }
        if x & 1 == 1 {
            // n + 1 = 2^a * m  ==>  T^a(n) = 3^a * m - 1, an even number.
            let x1 = x.checked_add(1).ok_or(RunError::Overflow)?;
            let a = x1.trailing_zeros();
            let pow = *POW3.get(a as usize).ok_or(RunError::Overflow)?;
            x = pow.checked_mul(x1 >> a).ok_or(RunError::Overflow)? - 1;
            steps += u64::from(a);
            odd_steps += u64::from(a);
            // The last odd step went through 3y + 1 = 2x under the standard map.
            peak = peak.max(x.checked_mul(2).ok_or(RunError::Overflow)?);
        } else {
            let zeros = x.trailing_zeros();
            if x >> zeros >= target {
                x >>= zeros;
                steps += u64::from(zeros);
            } else {
                // Stop at the first halving that lands below the target. Shifting
                // to the target's bit length gets within one step of it.
                let x_bits = 128 - x.leading_zeros();
                let mut shift = x_bits.saturating_sub(target_bits).max(1);
                if x >> shift >= target {
                    shift += 1;
                }
                x >>= shift;
                steps += u64::from(shift);
            }
        }
    }

    Ok(Run {
        steps,
        odd_steps,
        peak,
        end: x,
    })
}

/// Arbitrary-precision version of [`run_until_below`]. Steps one at a time.
///
/// # Panics
///
/// Panics if `target < 2`.
pub fn run_until_below_big(
    start: &BigUint,
    target: &BigUint,
    max_steps: u64,
) -> Result<BigRun, RunError> {
    assert!(target.bits() >= 2, "target must be at least 2");
    let mut x = start.clone();
    let mut steps = 0u64;
    let mut odd_steps = 0u64;
    let mut peak = start.clone();

    while x >= *target {
        if steps >= max_steps {
            return Err(RunError::StepLimit);
        }
        if !x.is_even() {
            x.mul_add_small(3, 1);
            if x > peak {
                peak.clone_from(&x);
            }
            odd_steps += 1;
        }
        x.shr_assign(1);
        steps += 1;
    }

    Ok(BigRun {
        steps,
        odd_steps,
        peak,
        end: x,
    })
}

/// Runs with the fast engine and falls back to the exact one on overflow.
pub fn run_with_fallback(start: u128, target: u128, max_steps: u64) -> Result<BigRun, RunError> {
    match run_until_below(start, target, max_steps) {
        Ok(run) => Ok(run.into()),
        Err(RunError::Overflow) => run_until_below_big(&start.into(), &target.into(), max_steps),
        Err(err) => Err(err),
    }
}

/// The stopping time `σ(n)`: shortcut steps until the orbit first drops below `n`.
///
/// Returns `None` for `n < 2`, where it is undefined.
pub fn stopping_time(n: u128) -> Option<u64> {
    (n >= 2).then(|| run_with_fallback(n, n, u64::MAX).unwrap().steps)
}

/// The total stopping time under the standard map: `C` steps from `n` to 1.
///
/// # Panics
///
/// Panics if `n == 0`.
pub fn total_stopping_time(n: u128) -> u64 {
    assert!(n > 0, "0 is not in the domain of the Collatz map");
    run_with_fallback(n, 2, u64::MAX).unwrap().standard_steps()
}

/// The largest value in the orbit of `n` under the standard map.
pub fn peak(n: u128) -> BigUint {
    assert!(n > 0, "0 is not in the domain of the Collatz map");
    run_with_fallback(n, 2, u64::MAX).unwrap().peak
}

/// Everything interesting about a single orbit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub start: BigUint,
    /// Shortcut steps to drop below the start (`None` for 1).
    pub stopping_time: Option<u64>,
    /// Shortcut steps to reach 1.
    pub shortcut_steps: u64,
    /// Odd steps on the way to 1.
    pub odd_steps: u64,
    /// The largest value reached under the standard map.
    pub peak: BigUint,
}

impl Summary {
    /// Standard-map steps to reach 1 (the usual "total stopping time").
    pub fn standard_steps(&self) -> u64 {
        self.shortcut_steps + self.odd_steps
    }

    /// Halvings on the way to 1: the even steps of the standard map. Every
    /// shortcut step, odd or even, contains exactly one halving.
    pub fn even_steps(&self) -> u64 {
        self.shortcut_steps
    }
}

/// Summarizes the orbit of any positive integer.
///
/// # Panics
///
/// Panics if `n` is zero.
pub fn summarize(n: &BigUint) -> Summary {
    assert!(!n.is_zero(), "0 is not in the domain of the Collatz map");
    let run = |target: &BigUint| {
        match (n.to_u128(), target.to_u128()) {
            (Some(s), Some(t)) => run_with_fallback(s, t, u64::MAX),
            _ => run_until_below_big(n, target, u64::MAX),
        }
        .unwrap()
    };
    let total = run(&BigUint::from(2u64));
    let stopping_time = (!n.is_one()).then(|| run(n).steps);
    Summary {
        start: n.clone(),
        stopping_time,
        shortcut_steps: total.steps,
        odd_steps: total.odd_steps,
        peak: total.peak,
    }
}

/// Iterator over an orbit, from the start down to 1 inclusive.
#[derive(Debug, Clone)]
pub struct Trajectory {
    next: Option<BigUint>,
    map: Map,
}

/// Iterates the orbit of `n` under `map`, ending at 1.
///
/// For [`Map::Syracuse`], an even start is first reduced to its odd part.
pub fn trajectory(n: &BigUint, map: Map) -> Trajectory {
    assert!(!n.is_zero(), "0 is not in the domain of the Collatz map");
    Trajectory {
        next: Some(n.clone()),
        map,
    }
}

impl Iterator for Trajectory {
    type Item = BigUint;

    fn next(&mut self) -> Option<BigUint> {
        let current = self.next.take()?;
        if !current.is_one() {
            let mut x = current.clone();
            if x.is_even() {
                let shift = match self.map {
                    Map::Syracuse => x.trailing_zeros().unwrap(),
                    _ => 1,
                };
                x.shr_assign(shift);
            } else {
                x.mul_add_small(3, 1);
                match self.map {
                    Map::Standard => {}
                    Map::Shortcut => x.shr_assign(1),
                    Map::Syracuse => x.shr_assign(x.trailing_zeros().unwrap()),
                }
            }
            self.next = Some(x);
        }
        Some(current)
    }
}

/// The parity vector of `n`: bit `j` is the parity of `T^j(n)`, for `j < k`.
///
/// By a theorem of Terras and Everett this depends only on `n mod 2^k`, so it
/// is computed on that residue, which also keeps the arithmetic in range.
///
/// # Panics
///
/// Panics if `k > 64`.
pub fn parity_vector(n: &BigUint, k: u32) -> Vec<bool> {
    assert!(k <= 64, "parity vectors are limited to 64 steps");
    let mut low = n.clone();
    let mut high = n.clone();
    high.shr_assign(u64::from(k));
    let high = &high * &BigUint::power_of_two(u64::from(k));
    low = low.checked_sub(&high).unwrap();
    // T^j(r) < (3/2)^j * 2^k <= 3^64 for r < 2^k, which fits in a u128.
    let mut x = low.to_u128().unwrap();
    (0..k)
        .map(|_| {
            let odd = x & 1 == 1;
            x = crate::map::shortcut(x).unwrap();
            odd
        })
        .collect()
}

/// The unique residue `r mod 2^k` whose parity vector is `parity`.
///
/// This is the constructive half of the Terras–Everett bijection: adding
/// `2^j` to `r` flips the parity of `T^j(r)` without changing earlier steps,
/// so each bit can be fixed in turn.
///
/// # Panics
///
/// Panics if `parity` is longer than 64.
pub fn residue_with_parity(parity: &[bool]) -> u128 {
    assert!(parity.len() <= 64, "parity vectors are limited to 64 steps");
    let mut r = 0u128;
    for (j, &want) in parity.iter().enumerate() {
        let x = (0..j).fold(r, |x, _| crate::map::shortcut(x).unwrap());
        if (x & 1 == 1) != want {
            r += 1 << j;
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{collatz, shortcut};

    /// Plain loop, one standard step at a time: (T steps, odd steps, peak, end).
    fn naive(start: u128, target: u128) -> (u64, u64, u128, u128) {
        let (mut x, mut steps, mut odd, mut peak) = (start, 0, 0, start);
        while x >= target {
            if x % 2 == 1 {
                x = collatz(x).unwrap();
                peak = peak.max(x);
                odd += 1;
            }
            x /= 2;
            steps += 1;
        }
        (steps, odd, peak, x)
    }

    #[test]
    fn famous_values() {
        assert_eq!(total_stopping_time(1), 0);
        assert_eq!(total_stopping_time(27), 111);
        assert_eq!(peak(27).to_u128(), Some(9232));
        assert_eq!(total_stopping_time(837_799), 524);
        assert_eq!(stopping_time(27), Some(59));
        assert_eq!(stopping_time(1), None);
        assert_eq!(stopping_time(2), Some(1));
        assert_eq!(stopping_time(3), Some(4));
    }

    #[test]
    fn fast_engine_matches_naive_loop() {
        for n in 2..20_000u128 {
            let fast = run_until_below(n, n, u64::MAX).unwrap();
            assert_eq!(
                (fast.steps, fast.odd_steps, fast.peak, fast.end),
                naive(n, n),
                "n = {n}"
            );
            let fast = run_until_below(n, 2, u64::MAX).unwrap();
            assert_eq!(
                (fast.steps, fast.odd_steps, fast.peak, fast.end),
                naive(n, 2),
                "n = {n}"
            );
        }
    }

    #[test]
    fn fast_engine_handles_arbitrary_targets() {
        for start in [27u128, 97, 871, 1 << 40, (1 << 70) + 1] {
            for target in [2u128, 3, 10, 1000, 4096, 1 << 35] {
                let fast = run_until_below(start, target, u64::MAX).unwrap();
                let slow = run_until_below_big(&start.into(), &target.into(), u64::MAX).unwrap();
                assert_eq!(BigRun::from(fast), slow);
            }
        }
    }

    #[test]
    fn overflow_falls_back_to_big_engine() {
        // 2^127 - 1 climbs through 127 odd steps, far beyond u128.
        let n = u128::MAX >> 1;
        assert_eq!(run_until_below(n, n, u64::MAX), Err(RunError::Overflow));
        let run = run_with_fallback(n, n, u64::MAX).unwrap();
        let big = run_until_below_big(&n.into(), &n.into(), u64::MAX).unwrap();
        assert_eq!(run, big);
        assert!(run.peak.bits() > 128);
    }

    #[test]
    fn step_limit() {
        assert_eq!(run_until_below(27, 2, 10), Err(RunError::StepLimit));
        assert_eq!(
            run_until_below_big(&27u64.into(), &2u64.into(), 10),
            Err(RunError::StepLimit)
        );
    }

    #[test]
    fn summary_of_huge_number() {
        // 2^100 + 1 is far outside u128-friendly territory for the full orbit.
        let n = &BigUint::power_of_two(100) + &BigUint::one();
        let s = summarize(&n);
        let by_hand = trajectory(&n, Map::Standard).count() as u64 - 1;
        assert_eq!(s.standard_steps(), by_hand);
        assert_eq!(s.stopping_time, Some(2));
        assert_eq!(s.even_steps() + s.odd_steps, s.standard_steps());
    }

    #[test]
    fn step_counts_of_27() {
        let s = summarize(&27u64.into());
        assert_eq!(
            (s.standard_steps(), s.odd_steps, s.even_steps()),
            (111, 41, 70)
        );
        assert_eq!(s.shortcut_steps, 70);
    }

    #[test]
    fn trajectories() {
        let seq: Vec<u128> = trajectory(&6u64.into(), Map::Standard)
            .map(|x| x.to_u128().unwrap())
            .collect();
        assert_eq!(seq, [6, 3, 10, 5, 16, 8, 4, 2, 1]);
        let seq: Vec<u128> = trajectory(&6u64.into(), Map::Shortcut)
            .map(|x| x.to_u128().unwrap())
            .collect();
        assert_eq!(seq, [6, 3, 5, 8, 4, 2, 1]);
        let seq: Vec<u128> = trajectory(&6u64.into(), Map::Syracuse)
            .map(|x| x.to_u128().unwrap())
            .collect();
        assert_eq!(seq, [6, 3, 5, 1]);
        assert_eq!(trajectory(&BigUint::one(), Map::Standard).count(), 1);
    }

    #[test]
    fn parity_vectors_form_a_bijection() {
        let k = 10;
        let mut seen = std::collections::HashSet::new();
        for r in 0..1u128 << k {
            let parity = parity_vector(&r.into(), k);
            assert_eq!(residue_with_parity(&parity), r);
            assert!(seen.insert(parity));
        }
        // The vector really is the parity of the actual orbit.
        let n = 123_456_789u128;
        let mut x = n;
        for (j, odd) in parity_vector(&n.into(), 30).into_iter().enumerate() {
            assert_eq!(x & 1 == 1, odd, "step {j}");
            x = shortcut(x).unwrap();
        }
    }
}
