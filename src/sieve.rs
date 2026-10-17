//! Sieves that prove most numbers never need to be checked.
//!
//! # The 2^k descent sieve
//!
//! The first `k` shortcut steps of `n` depend only on `n mod 2^k`. More
//! precisely, if `n = r + a * 2^k` and `f_j` is the number of odd steps among
//! the first `j` steps of `r`, then for every `j <= k`
//!
//! ```text
//! T^j(n) = T^j(r) + 3^(f_j) * 2^(k-j) * a.
//! ```
//!
//! For fixed `j` both sides of `T^j(n) < n` are linear in `a`. If
//! `3^(f_j) * 2^(k-j) < 2^k` the left side grows more slowly, so the
//! inequality holds for every `a >= 1` exactly when it holds at `a = 1`.
//! That gives an exact test: the residue class `r` is *sieved out* when some
//! `j <= k` passes it, because then every `n >= 2^k` in the class drops below
//! itself within `k` steps. When numbers are verified in increasing order,
//! those `n` are already covered by smaller ones.
//!
//! Only about 1.7% of residues survive at `k = 24`.
//!
//! The test is monotone in `k`: if a class mod `2^k` is sieved out, so are
//! both of its children mod `2^(k+1)`. The sieve is therefore built one bit at
//! a time, testing only the children of survivors.
//!
//! # The mod 9 sieve
//!
//! If `n ≡ 2 (mod 3)` then `(2n - 1) / 3` is a smaller odd number whose next
//! shortcut step is `n`. If `n ≡ 4 (mod 9)` then `(8n - 5) / 9` is a smaller
//! number that reaches `n` in three steps. Either way the orbit of `n` is the
//! tail of a smaller orbit, so `n ≡ 2, 4, 5, 8 (mod 9)` can be skipped too.

use crate::map::POW3;

/// Bit `i` is set when residues `i mod 9` can be skipped: 2, 4, 5 and 8.
pub const MOD9_SKIP: u16 = 1 << 2 | 1 << 4 | 1 << 5 | 1 << 8;

/// Whether `n` is reached from a smaller starting value, so it can be skipped.
///
/// Valid for `n >= 4` (for `n = 2` the smaller value is 1, the end of every orbit).
#[inline]
pub fn mod9_skippable(n: u128) -> bool {
    MOD9_SKIP >> (n % 9) & 1 == 1
}

/// Outcome of testing one residue class with [`descent_test`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Descent {
    /// Every `n >= 2^k` in the class drops below `n` within `steps` steps.
    /// Until then, every value stays at most `growth * n`.
    Proven { steps: u32, growth: u128 },
    /// The class must be checked number by number.
    Survives,
}

/// Tests whether every `n ≡ r (mod 2^k)` with `n >= 2^k` provably descends
/// within `k` shortcut steps. See the module documentation for the argument.
///
/// # Panics
///
/// Panics if `k > 48` or `r >= 2^k`.
pub fn descent_test(r: u64, k: u32) -> Descent {
    assert!(k <= 48, "sieve depth is limited to 48 bits");
    assert!(u128::from(r) < 1u128 << k, "residue out of range");
    let modulus = 1u128 << k;
    let r = u128::from(r);
    let mut value = r;
    let mut odd = 0usize;
    // Upper bound on max_{i<j} T^i(n) / n over the class, rounded up.
    let mut growth = 1u128;

    for j in 1..=k {
        if value & 1 == 1 {
            value += value / 2 + 1;
            odd += 1;
        } else {
            value /= 2;
        }
        // T^j(n) = value + slope * a, with n = r + modulus * a.
        let slope = POW3[odd] << (k - j);
        if slope < modulus && value + slope < r + modulus {
            return Descent::Proven { steps: j, growth };
        }
        // The ratio (value + slope*a) / (r + modulus*a) is monotone in a, so its
        // supremum over a >= 1 is at a = 1 or in the limit slope / modulus.
        let at_one = (value + slope).div_ceil(r + modulus);
        let at_infinity = slope.div_ceil(modulus);
        growth = growth.max(at_one).max(at_infinity);
    }
    Descent::Survives
}

/// The residues mod `2^k` that survive the descent sieve.
#[derive(Debug, Clone)]
pub struct DescentSieve {
    k: u32,
    survivors: Vec<u64>,
    growth: u128,
}

impl DescentSieve {
    /// The deepest supported sieve.
    pub const MAX_K: u32 = 36;

    /// Builds the sieve for modulus `2^k`, one bit at a time.
    ///
    /// # Panics
    ///
    /// Panics if `k > MAX_K`.
    pub fn new(k: u32) -> Self {
        Self::build(k, |_, _| {})
    }

    /// Builds sieves for every depth up to `k`, returning the survivor count
    /// at each depth (index 0 is depth 0, where everything survives).
    pub fn survivor_counts(k: u32) -> Vec<u64> {
        let mut counts = vec![1];
        Self::build(k, |_, survivors| counts.push(survivors.len() as u64));
        counts
    }

    fn build(k: u32, mut on_level: impl FnMut(u32, &[u64])) -> Self {
        assert!(
            k <= Self::MAX_K,
            "sieve depth must be at most {}",
            Self::MAX_K
        );
        let mut survivors = vec![0u64];
        let mut growth = 1u128;
        for level in 1..=k {
            let half = 1u64 << (level - 1);
            let mut next = Vec::with_capacity(survivors.len() * 2);
            for &r in &survivors {
                for child in [r, r + half] {
                    match descent_test(child, level) {
                        Descent::Survives => next.push(child),
                        Descent::Proven { growth: g, .. } => growth = growth.max(g),
                    }
                }
            }
            next.sort_unstable();
            survivors = next;
            on_level(level, &survivors);
        }
        DescentSieve {
            k,
            survivors,
            growth,
        }
    }

    /// The sieve depth `k`.
    pub fn k(&self) -> u32 {
        self.k
    }

    /// The modulus `2^k`.
    pub fn modulus(&self) -> u64 {
        1 << self.k
    }

    /// Surviving residues in increasing order.
    pub fn survivors(&self) -> &[u64] {
        &self.survivors
    }

    /// Fraction of residues that survive.
    pub fn density(&self) -> f64 {
        self.survivors.len() as f64 / self.modulus() as f64
    }

    /// For every sieved-out `n >= 2^k`, the orbit stays at most `growth * n`
    /// under the shortcut map until it drops below `n`.
    pub fn growth(&self) -> u128 {
        self.growth
    }

    /// Whether `n`'s residue class survives.
    pub fn contains(&self, n: u128) -> bool {
        let r = (n % u128::from(self.modulus())) as u64;
        self.survivors.binary_search(&r).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbit::{run_until_below, stopping_time};

    #[test]
    fn survivor_counts_match_oeis_a076227() {
        // Residues mod 2^k that do not provably descend within k steps. When
        // 3^f > 2^k for every f a survivor can have (k = 19 here), nothing new
        // can descend and the count simply doubles.
        assert_eq!(
            DescentSieve::survivor_counts(20),
            [
                1, 1, 1, 2, 3, 4, 8, 13, 19, 38, 64, 128, 226, 367, 734, 1295, 2114, 4228, 7495,
                14990, 27328
            ]
        );
    }

    #[test]
    fn recursive_build_matches_direct_test() {
        for k in 1..=14 {
            let direct: Vec<u64> = (0..1u64 << k)
                .filter(|&r| descent_test(r, k) == Descent::Survives)
                .collect();
            assert_eq!(DescentSieve::new(k).survivors(), direct, "k = {k}");
        }
    }

    #[test]
    fn sieved_numbers_really_descend() {
        let k = 12;
        let sieve = DescentSieve::new(k);
        let m = u128::from(sieve.modulus());
        let mut checked = 0;
        for n in m..m * 32 {
            if !sieve.contains(n) {
                let steps = stopping_time(n).unwrap();
                assert!(steps <= u64::from(k), "n = {n} took {steps} steps");
                checked += 1;
            }
        }
        assert!(checked > 0);
    }

    #[test]
    fn growth_bound_holds() {
        let k = 12;
        let sieve = DescentSieve::new(k);
        let m = u128::from(sieve.modulus());
        for n in m..m * 16 {
            if !sieve.contains(n) {
                let run = run_until_below(n, n, u64::MAX).unwrap();
                // The standard-map peak is at most twice the shortcut-map peak.
                assert!(run.peak <= 2 * sieve.growth() * n, "n = {n}");
            }
        }
    }

    #[test]
    fn mod9_sieve() {
        for n in 4..10_000u128 {
            if !mod9_skippable(n) {
                continue;
            }
            let smaller = if n % 3 == 2 {
                (2 * n - 1) / 3
            } else {
                (8 * n - 5) / 9
            };
            assert!(smaller < n);
            let mut x = smaller;
            let mut hit = false;
            for _ in 0..3 {
                x = crate::map::shortcut(x).unwrap();
                hit |= x == n;
            }
            assert!(hit, "n = {n}");
        }
    }
}
