//! Cycles, for 3n+1 and its relatives.
//!
//! The only known positive cycle of `3n+1` is 1, 4, 2. A proof of the
//! conjecture has to explain why there are no others, and any argument that
//! would also work for a near miss is broken:
//!
//! * `3n+1` on the negative integers has three more cycles, through -1, -5 and -17;
//! * `5n+1` has cycles through 1, 13 and 17, and orbits (like that of 7) that
//!   appear to grow forever.
//!
//! [`census`] finds the cycles of any map `n/2` (even), `an+b` (odd) by
//! following orbits with Brent's cycle detection.
//!
//! [`cycles_with_period`] takes the other route. Every parity pattern of
//! length `L` with `k` odd steps has exactly one rational fixed point of the
//! shortcut map, given by the cycle equation
//!
//! ```text
//! n = c / (2^L - 3^k),   where   c = sum over odd steps i of 3^(odd steps after i) * 2^i.
//! ```
//!
//! Integer cycles are the patterns where that fraction is an integer, so
//! trying all `2^L` patterns finds every cycle of length `L` exactly.

use std::collections::HashMap;

use crate::map::POW3;

/// The map `n/2` for even `n`, `a*n + b` for odd `n`, on signed integers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneralizedMap {
    pub a: i128,
    pub b: i128,
}

impl GeneralizedMap {
    /// The Collatz map itself.
    pub const COLLATZ: GeneralizedMap = GeneralizedMap { a: 3, b: 1 };

    /// One step, or `None` on overflow.
    #[inline]
    pub fn step(self, n: i128) -> Option<i128> {
        if n % 2 == 0 {
            Some(n / 2)
        } else {
            n.checked_mul(self.a)?.checked_add(self.b)
        }
    }
}

impl std::fmt::Display for GeneralizedMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.b {
            b if b < 0 => write!(f, "{}n-{}", self.a, -b),
            b => write!(f, "{}n+{}", self.a, b),
        }
    }
}

/// A cycle, rotated to start at its member of smallest absolute value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cycle {
    members: Vec<i128>,
}

impl Cycle {
    /// Builds the cycle through `member`.
    fn through(map: GeneralizedMap, member: i128) -> Option<Cycle> {
        let mut members = vec![member];
        let mut x = map.step(member)?;
        while x != member {
            members.push(x);
            x = map.step(x)?;
        }
        Some(Cycle::from_members(members))
    }

    fn from_members(mut members: Vec<i128>) -> Cycle {
        let start = (0..members.len())
            .min_by_key(|&i| (members[i].unsigned_abs(), members[i]))
            .unwrap_or(0);
        members.rotate_left(start);
        Cycle { members }
    }

    /// Members in orbit order, starting from the smallest in absolute value.
    pub fn members(&self) -> &[i128] {
        &self.members
    }

    /// Number of steps in the cycle.
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Always false: a cycle has at least one member.
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// How many members are odd.
    pub fn odd_members(&self) -> usize {
        self.members.iter().filter(|&&x| x % 2 != 0).count()
    }
}

/// Where the orbit of one start ended up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fate {
    /// Fell into the cycle with this index in [`Census::cycles`].
    Cycle(usize),
    /// Grew past the bound (or the step limit) without repeating: possibly divergent.
    Escaped,
}

/// One cycle and how many starts fell into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleCount {
    pub cycle: Cycle,
    pub starts: u64,
    /// The first start (in the order given) that reached this cycle.
    pub first_start: i128,
}

/// The cycles reached from a set of starting values.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Census {
    /// Cycles in order of discovery.
    pub cycles: Vec<CycleCount>,
    /// Starts whose orbits escaped.
    pub escaped: Vec<i128>,
}

/// Finds the fate of every start under `map`.
///
/// An orbit counts as escaped if it exceeds `bound` in absolute value or runs
/// for `max_steps` steps without repeating.
pub fn census(
    map: GeneralizedMap,
    starts: impl IntoIterator<Item = i128>,
    bound: i128,
    max_steps: u64,
) -> Census {
    let mut census = Census::default();
    let mut owner: HashMap<i128, usize> = HashMap::new();
    for start in starts {
        match fate(map, start, bound, max_steps, &mut census, &mut owner) {
            Fate::Cycle(id) => {
                census.cycles[id].starts += 1;
            }
            Fate::Escaped => census.escaped.push(start),
        }
    }
    census
}

/// Brent's cycle detection, cut short whenever the orbit hits a known cycle.
fn fate(
    map: GeneralizedMap,
    start: i128,
    bound: i128,
    max_steps: u64,
    census: &mut Census,
    owner: &mut HashMap<i128, usize>,
) -> Fate {
    let (mut tortoise, mut hare) = (start, start);
    let (mut power, mut lambda, mut steps) = (1u64, 0u64, 0u64);
    loop {
        if let Some(&id) = owner.get(&hare) {
            return Fate::Cycle(id);
        }
        if hare.unsigned_abs() > bound.unsigned_abs() || steps >= max_steps {
            return Fate::Escaped;
        }
        hare = match map.step(hare) {
            Some(next) => next,
            None => return Fate::Escaped,
        };
        steps += 1;
        lambda += 1;
        if hare == tortoise {
            let Some(cycle) = Cycle::through(map, hare) else {
                return Fate::Escaped;
            };
            let id = census.cycles.len();
            owner.extend(cycle.members().iter().map(|&x| (x, id)));
            census.cycles.push(CycleCount {
                cycle,
                starts: 0,
                first_start: start,
            });
            return Fate::Cycle(id);
        }
        if power == lambda {
            tortoise = hare;
            power *= 2;
            lambda = 0;
        }
    }
}

/// The unique rational fixed point of the shortcut map for one parity
/// pattern, as a fraction `(numerator, denominator)`.
///
/// Bit `i` of `pattern` says whether step `i` is odd. Returns `None` for
/// patterns longer than 40 steps.
pub fn rational_cycle(pattern: u64, len: u32) -> Option<(i128, i128)> {
    if len > 40 {
        return None;
    }
    let mut c: i128 = 0;
    let mut k = 0;
    for i in 0..len {
        if pattern >> i & 1 == 1 {
            // An odd step at position i: (3x + 1)/2 contributes 2^i, scaled by 3
            // for each later odd step.
            c = 3 * c + (1 << i);
            k += 1;
        }
    }
    Some((c, (1i128 << len) - POW3[k] as i128))
}

/// Every integer cycle of the `3n+1` shortcut map whose length divides `len`,
/// found by solving the cycle equation for all `2^len` parity patterns.
///
/// Cycles are reported in shortcut steps, so 1, 2 is the trivial cycle, and
/// are ordered by their smallest member in absolute value.
///
/// # Panics
///
/// Panics if `len` is 0 or larger than 32.
pub fn cycles_with_period(len: u32) -> Vec<Cycle> {
    assert!(
        (1..=32).contains(&len),
        "pattern length must be between 1 and 32"
    );
    let mut found = std::collections::BTreeSet::new();
    for pattern in 0..1u64 << len {
        let (num, den) = rational_cycle(pattern, len).expect("len <= 32");
        if num % den != 0 {
            continue;
        }
        let n = num / den;
        if n == 0 {
            continue;
        }
        // Follow the cycle with the shortcut map to collect its members.
        let shortcut = |x: i128| if x % 2 == 0 { x / 2 } else { (3 * x + 1) / 2 };
        let mut members = vec![n];
        let mut x = shortcut(n);
        while x != n {
            members.push(x);
            x = shortcut(x);
        }
        found.insert(Cycle::from_members(members));
    }
    let mut cycles: Vec<Cycle> = found.into_iter().collect();
    cycles.sort_by_key(|c| (c.members[0].unsigned_abs(), c.members[0]));
    cycles
}

#[cfg(test)]
mod tests {
    use super::*;

    fn members(census: &Census) -> Vec<Vec<i128>> {
        census
            .cycles
            .iter()
            .map(|c| c.cycle.members().to_vec())
            .collect()
    }

    #[test]
    fn positive_collatz_has_one_cycle() {
        let census = census(GeneralizedMap::COLLATZ, 1..=100_000, 1 << 100, 100_000);
        assert_eq!(members(&census), [vec![1, 4, 2]]);
        assert_eq!(census.cycles[0].starts, 100_000);
        assert!(census.escaped.is_empty());
    }

    #[test]
    fn negative_collatz_has_three_cycles() {
        let census = census(
            GeneralizedMap::COLLATZ,
            (1..=10_000).map(|n| -n),
            1 << 100,
            100_000,
        );
        let mut found = members(&census);
        found.sort_by_key(|c| c.len());
        assert_eq!(found[0], [-1, -2]);
        assert_eq!(found[1], [-5, -14, -7, -20, -10]);
        assert_eq!(found[2][0], -17);
        assert_eq!(found[2].len(), 18);
        assert_eq!(found.len(), 3);
    }

    #[test]
    fn five_n_plus_one_misbehaves() {
        let map = GeneralizedMap { a: 5, b: 1 };
        let census = census(map, 1..=20, 1 << 100, 100_000);
        let mut starts: Vec<i128> = census.cycles.iter().map(|c| c.cycle.members()[0]).collect();
        starts.sort_unstable();
        assert_eq!(starts, [1, 13, 17]);
        assert!(census.escaped.contains(&7));
        assert_eq!(map.to_string(), "5n+1");
        assert_eq!(GeneralizedMap { a: 3, b: -1 }.to_string(), "3n-1");
    }

    #[test]
    fn cycle_equation() {
        // The pattern "odd, even" is the trivial cycle 1 -> 2 -> 1.
        assert_eq!(rational_cycle(0b01, 2), Some((1, 1)));
        // A single odd step is the fixed point -1.
        assert_eq!(rational_cycle(0b1, 1), Some((1, -1)));
        // An all-even pattern only fixes 0.
        assert_eq!(rational_cycle(0, 5), Some((0, 31)));
    }

    #[test]
    fn exhaustive_pattern_search_finds_the_known_cycles() {
        let smallest = |len| -> Vec<i128> {
            cycles_with_period(len)
                .iter()
                .map(|c| c.members()[0])
                .collect()
        };
        assert_eq!(smallest(1), [-1]);
        assert_eq!(smallest(2), [-1, 1]);
        // -5 -> -7 -> -10 -> -5 under the shortcut map.
        assert_eq!(smallest(3), [-1, -5]);
        // The -17 cycle has 11 shortcut steps.
        assert_eq!(smallest(11), [-1, -17]);
        // Length 22 contains everything of length 1, 2 and 11.
        assert_eq!(smallest(22), [-1, 1, -17]);
    }
}
