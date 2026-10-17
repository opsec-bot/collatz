//! Which cycle each starting value of `3n-1` ends in.
//!
//! `3n-1` (3n+1 on the negative integers) has three known cycles. Under the
//! shortcut map they are `{1}`, `{5, 7, 10}` and an eleven-step cycle through
//! 17. Every number checked so far falls into one of them, and nothing is
//! known about *how often* each one wins. Do the three basins have natural
//! densities, and what are they?
//!
//! This module answers that numerically, in two ways:
//!
//! * [`BasinTable`] labels every `n` below a bound exactly, and [`count`]
//!   extends exact counts past the table by running each orbit until it lands
//!   inside it.
//! * [`sample`] estimates the fractions among uniformly random `n` of any
//!   size, using exact arithmetic throughout.

use crate::bigint::BigUint;
use crate::map::Variant;
use crate::orbit::{run_variant_until_below, run_variant_until_below_big, RunError};
use crate::par::parallel_map;

/// One of the three known cycles of `3n-1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Basin {
    /// The fixed point 1 (1, 2, 1, ... under the standard map).
    One,
    /// 5, 7, 10.
    Five,
    /// 17, 25, 37, 55, 82, 41, 61, 91, 136, 68, 34.
    Seventeen,
}

impl Basin {
    pub const ALL: [Basin; 3] = [Basin::One, Basin::Five, Basin::Seventeen];

    /// The cycle under the shortcut map, starting from its minimum.
    pub fn cycle(self) -> &'static [u128] {
        match self {
            Basin::One => &[1],
            Basin::Five => &[5, 7, 10],
            Basin::Seventeen => &[17, 25, 37, 55, 82, 41, 61, 91, 136, 68, 34],
        }
    }

    /// The smallest member of the cycle.
    pub fn minimum(self) -> u128 {
        self.cycle()[0]
    }

    fn index(self) -> usize {
        self as usize
    }

    fn from_index(i: u8) -> Basin {
        Basin::ALL[usize::from(i)]
    }
}

/// Orbits are abandoned (and reported) after this many shortcut steps.
const MAX_STEPS: u64 = 1 << 24;

/// Numbers below this are labeled one at a time; above it, level by level.
const SEQUENTIAL_END: usize = 1 << 16;

/// Label value for 0, which is in no basin.
const NONE: u8 = u8::MAX;

/// The basin of every `n` below a bound.
#[derive(Debug, Clone)]
pub struct BasinTable {
    labels: Vec<u8>,
}

/// An orbit that did not reach a known cycle within the step limit:
/// a new cycle or a divergent orbit, either of which would be news.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unresolved(pub BigUint);

impl std::fmt::Display for Unresolved {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the 3n-1 orbit of {} did not reach a known cycle within {MAX_STEPS} steps",
            self.0
        )
    }
}

impl std::error::Error for Unresolved {}

impl BasinTable {
    /// Labels every `n < len`.
    ///
    /// Below `2^16` each `n` follows its orbit until it drops below itself
    /// (cycle minima excepted) and copies that value's label. Above, the range
    /// is processed one dyadic level `[B, 2B)` at a time: no cycle member is
    /// that large, so every orbit drops below `B`, whose labels are all known,
    /// and the whole level can be done in parallel.
    pub fn build(len: usize, threads: usize) -> Result<Self, Unresolved> {
        let mut labels = vec![NONE; len.max(2)];
        for basin in Basin::ALL {
            if let Some(slot) = labels.get_mut(basin.minimum() as usize) {
                *slot = basin as u8;
            }
        }
        for n in 2..len.min(SEQUENTIAL_END) {
            if labels[n] == NONE {
                let run = run_variant_until_below(Variant::Minus, n as u128, n as u128, MAX_STEPS)
                    .map_err(|_| Unresolved(BigUint::from(n as u64)))?;
                labels[n] = labels[run.end as usize];
            }
        }

        let mut base = SEQUENTIAL_END;
        while base < len {
            let end = (2 * base).min(len);
            let chunk = 1 << 16;
            let chunks = (end - base).div_ceil(chunk);
            let table = BasinTable { labels };
            let filled = parallel_map(chunks, threads, |c| {
                let lo = base + c * chunk;
                let hi = (lo + chunk).min(end);
                (lo..hi)
                    .map(|n| table.label_via(n as u128, base as u128))
                    .collect::<Result<Vec<u8>, Unresolved>>()
            });
            labels = table.labels;
            for (c, part) in filled.into_iter().enumerate() {
                let part = part?;
                let lo = base + c * chunk;
                labels[lo..lo + part.len()].copy_from_slice(&part);
            }
            base = end;
        }
        labels.truncate(len);
        Ok(BasinTable { labels })
    }

    /// Numbers covered by the table.
    pub fn len(&self) -> usize {
        self.labels.len()
    }

    /// Whether the table covers nothing.
    pub fn is_empty(&self) -> bool {
        self.labels.len() <= 1
    }

    /// The basin of `n`, for `1 <= n < len`.
    pub fn get(&self, n: usize) -> Option<Basin> {
        match self.labels.get(n) {
            Some(&label) if label != NONE => Some(Basin::from_index(label)),
            _ => None,
        }
    }

    /// Follows `n` until it drops below `bound` (which must be at most `len`)
    /// and returns the label found there.
    fn label_via(&self, n: u128, bound: u128) -> Result<u8, Unresolved> {
        if n < bound {
            return Ok(self.labels[n as usize]);
        }
        let end = match run_variant_until_below(Variant::Minus, n, bound, MAX_STEPS) {
            Ok(run) => run.end,
            Err(RunError::Overflow) => {
                return self.label_big(&n.into(), &bound.into());
            }
            Err(RunError::StepLimit) => return Err(Unresolved(n.into())),
        };
        Ok(self.labels[end as usize])
    }

    fn label_big(&self, n: &BigUint, bound: &BigUint) -> Result<u8, Unresolved> {
        let run = run_variant_until_below_big(Variant::Minus, n, bound, MAX_STEPS)
            .map_err(|_| Unresolved(n.clone()))?;
        Ok(self.labels[run.end.to_u128().expect("below the table") as usize])
    }

    /// The basin of any positive `n`.
    pub fn basin_of(&self, n: u128) -> Result<Basin, Unresolved> {
        assert!(n > 0, "0 is in no basin");
        // Halving never changes the basin, so strip factors of 2 first.
        let odd = n >> n.trailing_zeros();
        self.label_via(odd, self.len() as u128)
            .map(Basin::from_index)
    }

    /// The basin of any positive `n`, however large.
    pub fn basin_of_big(&self, n: &BigUint) -> Result<Basin, Unresolved> {
        assert!(!n.is_zero(), "0 is in no basin");
        match n.to_u128() {
            Some(small) => self.basin_of(small),
            None => self
                .label_big(n, &BigUint::from(self.len() as u64))
                .map(Basin::from_index),
        }
    }
}

/// How many numbers fall into each basin, indexed like [`Basin::ALL`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally(pub [u64; 3]);

impl Tally {
    pub fn add(&mut self, basin: Basin) {
        self.0[basin.index()] += 1;
    }

    pub fn total(&self) -> u64 {
        self.0.iter().sum()
    }

    /// The fraction of the total in `basin`.
    pub fn fraction(&self, basin: Basin) -> f64 {
        self.0[basin.index()] as f64 / self.total().max(1) as f64
    }

    /// Half-width of a 95% confidence interval for [`Tally::fraction`],
    /// treating the tally as independent samples.
    pub fn margin(&self, basin: Basin) -> f64 {
        let p = self.fraction(basin);
        1.96 * (p * (1.0 - p) / self.total().max(1) as f64).sqrt()
    }

    fn merge(mut self, other: Tally) -> Tally {
        for i in 0..3 {
            self.0[i] += other.0[i];
        }
        self
    }
}

/// Exact basin counts for every `n` in `[start, end)`, `start >= 1`.
pub fn count(
    table: &BasinTable,
    start: u128,
    end: u128,
    threads: usize,
) -> Result<Tally, Unresolved> {
    assert!(start >= 1, "0 is in no basin");
    let chunk: u128 = 1 << 20;
    let chunks =
        usize::try_from(end.saturating_sub(start).div_ceil(chunk)).expect("range too large");
    parallel_map(chunks, threads, |c| {
        let lo = start + c as u128 * chunk;
        let hi = (lo + chunk).min(end);
        let mut tally = Tally::default();
        for n in lo..hi {
            tally.add(table.basin_of(n)?);
        }
        Ok(tally)
    })
    .into_iter()
    .try_fold(Tally::default(), |acc, part| Ok(acc.merge(part?)))
}

/// SplitMix64: a tiny, well-mixed generator, enough for sampling.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

/// A uniformly random integer in `[2^bits, 2^(bits+1))`.
fn random_with_bits(rng: &mut SplitMix64, bits: u32) -> BigUint {
    let limbs = (bits / 64 + 1) as usize;
    let mut words: Vec<u64> = (0..limbs).map(|_| rng.next()).collect();
    let top = bits % 64;
    let last = words.last_mut().unwrap();
    // Keep the bits below the top one random, and set the top one.
    *last &= (1u64 << top) - 1;
    *last |= 1u64 << top;
    BigUint::from_le_limbs(words)
}

/// Basin counts for `samples` uniformly random `n` in `[2^bits, 2^(bits+1))`.
///
/// The result depends only on `bits`, `samples` and `seed`, not on `threads`.
pub fn sample(
    table: &BasinTable,
    bits: u32,
    samples: u64,
    seed: u64,
    threads: usize,
) -> Result<Tally, Unresolved> {
    const PER_CHUNK: u64 = 1 << 12;
    let chunks = samples.div_ceil(PER_CHUNK) as usize;
    parallel_map(chunks, threads, |c| {
        let mut rng = SplitMix64(
            seed ^ (u64::from(bits) << 48) ^ (c as u64).wrapping_mul(0xa076_1d64_78bd_642f),
        );
        let count = PER_CHUNK.min(samples - c as u64 * PER_CHUNK);
        let mut tally = Tally::default();
        for _ in 0..count {
            tally.add(table.basin_of_big(&random_with_bits(&mut rng, bits))?);
        }
        Ok(tally)
    })
    .into_iter()
    .try_fold(Tally::default(), |acc, part| Ok(acc.merge(part?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Follows the 3n-1 orbit one step at a time until it hits a cycle member.
    fn naive_basin(mut n: u128) -> Basin {
        loop {
            for basin in Basin::ALL {
                if basin.cycle().contains(&n) {
                    return basin;
                }
            }
            n = Variant::Minus.shortcut(n).unwrap();
        }
    }

    #[test]
    fn cycles_are_cycles() {
        for basin in Basin::ALL {
            let cycle = basin.cycle();
            for (i, &x) in cycle.iter().enumerate() {
                let next = cycle[(i + 1) % cycle.len()];
                assert_eq!(Variant::Minus.shortcut(x), Some(next));
            }
        }
    }

    #[test]
    fn table_matches_naive_orbits() {
        let table = BasinTable::build(300_000, 4).unwrap();
        for n in 1..300_000u128 {
            assert_eq!(table.get(n as usize), Some(naive_basin(n)), "n = {n}");
        }
        assert_eq!(table.get(0), None);
    }

    #[test]
    fn lookups_beyond_the_table() {
        let table = BasinTable::build(1 << 12, 2).unwrap();
        for n in (1u128 << 20..(1 << 20) + 5000).chain([u128::MAX, 3 << 120]) {
            let big = BigUint::from(n);
            assert_eq!(table.basin_of(n), table.basin_of_big(&big), "n = {n}");
            if n < 1 << 64 {
                assert_eq!(table.basin_of(n).unwrap(), naive_basin(n), "n = {n}");
            }
        }
    }

    #[test]
    fn exact_counts_match_the_table() {
        let small = BasinTable::build(1 << 14, 3).unwrap();
        let big = BasinTable::build(1 << 18, 3).unwrap();
        let tally = count(&small, 1, 1 << 18, 3).unwrap();
        let mut expected = Tally::default();
        for n in 1..1usize << 18 {
            expected.add(big.get(n).unwrap());
        }
        assert_eq!(tally, expected);
        assert_eq!(tally.total(), (1 << 18) - 1);
    }

    #[test]
    fn sampling_is_deterministic_and_in_range() {
        let table = BasinTable::build(1 << 14, 2).unwrap();
        let a = sample(&table, 200, 10_000, 7, 1).unwrap();
        let b = sample(&table, 200, 10_000, 7, 4).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.total(), 10_000);
        let mut rng = SplitMix64(1);
        for bits in [0, 1, 63, 64, 65, 127, 128, 300] {
            for _ in 0..100 {
                assert_eq!(random_with_bits(&mut rng, bits).bits(), u64::from(bits) + 1);
            }
        }
    }
}
