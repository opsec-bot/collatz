//! Path records and delay records.
//!
//! * A **path record** is an `n` whose orbit climbs higher than the orbit of
//!   every smaller number. 27 is one: its orbit peaks at 9232.
//! * A **delay record** is an `n` that takes more steps to reach 1 than every
//!   smaller number. 837799 is the last one below a million, with 524 steps.
//!
//! # Searching for path records with a sieve
//!
//! If the orbit of `n` first drops below `n` at some value `v`, everything
//! after that is part of the orbit of `v < n`, whose peak is at most the
//! current record. So `n` is a path record exactly when the part of its orbit
//! *before* the descent beats the record, and only that part needs running.
//!
//! The descent sieve skips numbers known to descend quickly, and the sieve
//! also bounds how high those skipped orbits can go first: at most
//! `2 * growth * n` under the standard map. A block of skipped numbers below
//! `B` is therefore safe as soon as the record is at least `2 * growth * B`.
//! Records grow roughly like `n^2`, so this holds almost immediately, and the
//! search checks it after the fact and rescans any block where it did not.
//!
//! Each block is scanned in parallel for its own running maxima; a global
//! record must be one of those, so merging the blocks in order is exact.

use crate::bigint::BigUint;
use crate::orbit::{run_until_below, run_until_below_big, run_with_fallback, RunError};
use crate::par::parallel_map;
use crate::sieve::{DescentSieve, MOD9_SKIP};

/// A number whose orbit peaks higher than that of any smaller number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathRecord {
    pub n: u128,
    /// Highest value reached under the standard map.
    pub peak: BigUint,
}

/// A number that takes more standard-map steps to reach 1 than any smaller number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DelayRecord {
    pub n: u64,
    pub steps: u64,
}

/// An orbit peak. `Big` is only used for values that do not fit in a `u128`,
/// so the derived ordering (all `Small` before all `Big`) is numeric.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Peak {
    Small(u128),
    Big(BigUint),
}

impl From<BigUint> for Peak {
    fn from(value: BigUint) -> Self {
        value.to_u128().map_or(Peak::Big(value), Peak::Small)
    }
}

impl From<Peak> for BigUint {
    fn from(peak: Peak) -> Self {
        match peak {
            Peak::Small(value) => value.into(),
            Peak::Big(value) => value,
        }
    }
}

/// Peak of the orbit of `n` before it first drops below `n` (`n >= 2`).
fn peak_before_descent(n: u128) -> Peak {
    match run_until_below(n, n, u64::MAX) {
        Ok(run) => Peak::Small(run.peak),
        Err(RunError::Overflow) => run_until_below_big(&n.into(), &n.into(), u64::MAX)
            .expect("no step limit")
            .peak
            .into(),
        Err(RunError::StepLimit) => unreachable!("no step limit"),
    }
}

struct Search {
    limit: u128,
    modulus: u128,
    sieve: DescentSieve,
    survivors_mod9: Vec<u8>,
}

impl Search {
    fn block_range(&self, block: usize) -> (u128, u128) {
        let start = block as u128 * self.modulus;
        (start, (start + self.modulus).min(self.limit))
    }

    /// Running maxima of the pre-descent peak within one block.
    fn scan(&self, block: usize, sieved: bool) -> Vec<(u128, Peak)> {
        let (start, end) = self.block_range(block);
        let mut best = Peak::Small(0);
        let mut candidates = Vec::new();
        let mut consider = |n: u128, peak: Peak| {
            if peak > best {
                best = peak.clone();
                candidates.push((n, peak));
            }
        };

        if sieved {
            let base_mod9 = (start % 9) as u8;
            for (&r, &r_mod9) in self.sieve.survivors().iter().zip(&self.survivors_mod9) {
                let n = start + u128::from(r);
                if n >= end {
                    break;
                }
                if MOD9_SKIP >> ((base_mod9 + r_mod9) % 9) & 1 == 0 {
                    consider(n, peak_before_descent(n));
                }
            }
        } else {
            for n in start.max(1)..end {
                match n {
                    1 => consider(1, Peak::Small(1)),
                    // Even n > 2 never set a record: n - 1 already climbs past n.
                    _ if n > 2 && n % 2 == 0 => {}
                    _ => consider(n, peak_before_descent(n)),
                }
            }
        }
        candidates
    }
}

fn merge(blocks: &[Vec<(u128, Peak)>]) -> Vec<(u128, Peak)> {
    let mut records: Vec<(u128, Peak)> = Vec::new();
    for (n, peak) in blocks.iter().flatten() {
        if records.last().map_or(true, |(_, best)| peak > best) {
            records.push((*n, peak.clone()));
        }
    }
    records
}

/// All path records below `limit`, using a `2^sieve_k` descent sieve.
///
/// # Panics
///
/// Panics if `sieve_k < 3` (the mod 9 shortcut needs `n >= 4`) or is larger
/// than [`DescentSieve::MAX_K`].
pub fn path_records(limit: u128, sieve_k: u32, threads: usize) -> Vec<PathRecord> {
    assert!(
        sieve_k >= 3,
        "path-record search needs a sieve depth of at least 3"
    );
    let sieve = DescentSieve::new(sieve_k);
    let search = Search {
        limit,
        modulus: u128::from(sieve.modulus()),
        survivors_mod9: sieve.survivors().iter().map(|&r| (r % 9) as u8).collect(),
        sieve,
    };
    let blocks = usize::try_from(limit.div_ceil(search.modulus)).expect("limit too large");

    // Block 0 lies below 2^k, where the sieve says nothing; scan it in full.
    let mut scans = parallel_map(blocks, threads, |b| search.scan(b, b > 0));
    let mut records = merge(&scans);

    // Make sure no skipped number could have beaten the record of its time.
    let growth = 2 * search.sieve.growth();
    let mut unsafe_blocks = Vec::new();
    let mut idx = 0;
    for block in 1..blocks {
        let (start, _) = search.block_range(block);
        while idx + 1 < records.len() && records[idx + 1].0 < start {
            idx += 1;
        }
        let bound = growth.saturating_mul(start + search.modulus);
        if records[idx].1 < Peak::Small(bound) {
            unsafe_blocks.push(block);
        }
    }
    if !unsafe_blocks.is_empty() {
        let rescans = parallel_map(unsafe_blocks.len(), threads, |i| {
            search.scan(unsafe_blocks[i], false)
        });
        for (block, scan) in unsafe_blocks.into_iter().zip(rescans) {
            scans[block] = scan;
        }
        // Rescanning can only raise earlier records, which keeps later blocks safe.
        records = merge(&scans);
    }

    records
        .into_iter()
        .map(|(n, peak)| PathRecord {
            n,
            peak: peak.into(),
        })
        .collect()
}

/// Standard-map steps from `n` to 1 for every `n < limit` (entry 0 is unused).
///
/// Each value reuses the answer for the first smaller number its orbit
/// reaches, so only the stretch before the first descent is ever run.
pub fn total_stopping_times(limit: usize) -> Vec<u16> {
    let mut table = vec![0u16; limit.max(2)];
    for n in 2..limit {
        let run = run_with_fallback(n as u128, n as u128, u64::MAX).expect("no step limit");
        let rest = table[run.end.to_u128().unwrap() as usize];
        table[n] = u16::try_from(run.standard_steps() + u64::from(rest))
            .expect("total stopping time does not fit in u16");
    }
    table.truncate(limit);
    table
}

/// All delay records below `limit`.
pub fn delay_records(limit: usize) -> Vec<DelayRecord> {
    let table = total_stopping_times(limit);
    let mut records: Vec<DelayRecord> = Vec::new();
    for (n, &steps) in table.iter().enumerate().skip(1) {
        let steps = u64::from(steps);
        if records.last().map_or(true, |r| steps > r.steps) {
            records.push(DelayRecord { n: n as u64, steps });
        }
    }
    records
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orbit::{peak, total_stopping_time};

    fn brute_force_path_records(limit: u128) -> Vec<(u128, u128)> {
        let mut records: Vec<(u128, u128)> = Vec::new();
        for n in 1..limit {
            let p = peak(n).to_u128().unwrap();
            if records.last().map_or(true, |&(_, best)| p > best) {
                records.push((n, p));
            }
        }
        records
    }

    fn as_pairs(records: &[PathRecord]) -> Vec<(u128, u128)> {
        records
            .iter()
            .map(|r| (r.n, r.peak.to_u128().unwrap()))
            .collect()
    }

    #[test]
    fn known_path_records() {
        let records = as_pairs(&path_records(100_000, 10, 4));
        let expected = [
            (1, 1),
            (2, 2),
            (3, 16),
            (7, 52),
            (15, 160),
            (27, 9232),
            (255, 13120),
            (447, 39364),
            (639, 41524),
            (703, 250504),
            (1819, 1276936),
            (4255, 6810136),
            (4591, 8153620),
            (9663, 27114424),
            (20895, 50143264),
            (26623, 106358020),
            (31911, 121012864),
            (60975, 593279152),
            (77671, 1570824736),
        ];
        assert_eq!(records, expected);
    }

    #[test]
    fn sieved_search_matches_brute_force() {
        let limit = 400_000;
        let expected = brute_force_path_records(limit);
        // Depth 3 makes the safety check fail on early blocks and forces rescans;
        // depth 16 has a single sieved block.
        for k in [3, 6, 12, 16, 20] {
            for threads in [1, 4] {
                assert_eq!(
                    as_pairs(&path_records(limit, k, threads)),
                    expected,
                    "k = {k}"
                );
            }
        }
    }

    #[test]
    fn tiny_limits() {
        assert!(path_records(0, 8, 2).is_empty());
        assert!(path_records(1, 8, 2).is_empty());
        assert_eq!(as_pairs(&path_records(2, 8, 2)), [(1, 1)]);
        assert_eq!(as_pairs(&path_records(4, 8, 2)), [(1, 1), (2, 2), (3, 16)]);
    }

    #[test]
    fn stopping_time_table() {
        let table = total_stopping_times(10_000);
        for (n, &steps) in table.iter().enumerate().skip(1) {
            assert_eq!(u64::from(steps), total_stopping_time(n as u128), "n = {n}");
        }
    }

    #[test]
    fn known_delay_records() {
        let records = delay_records(1_000_000);
        let find = |n| records.iter().find(|r| r.n == n).map(|r| r.steps);
        assert_eq!(records[0], DelayRecord { n: 1, steps: 0 });
        assert_eq!(find(27), Some(111));
        assert_eq!(find(97), Some(118));
        assert_eq!(find(871), Some(178));
        assert_eq!(find(6171), Some(261));
        assert_eq!(find(77031), Some(350));
        assert_eq!(
            *records.last().unwrap(),
            DelayRecord {
                n: 837_799,
                steps: 524
            }
        );
    }
}
