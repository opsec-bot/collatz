//! The three classic forms of the Collatz map.
//!
//! * [`collatz`]: the standard map `C`, `n/2` for even `n` and `3n+1` for odd `n`.
//! * [`shortcut`]: the shortcut map `T`. An odd step is always followed by a
//!   halving, so `T` folds the two into `(3n+1)/2`.
//! * [`syracuse`]: the odd-to-odd map `S(n) = (3n+1) / 2^v2(3n+1)`.
//!
//! All functions work on `u128` and return `None` instead of silently
//! wrapping when the result does not fit.

/// `POW3[i] == 3^i` for every power of three that fits in a `u128` (`i <= 80`).
pub const POW3: [u128; 81] = {
    let mut table = [1u128; 81];
    let mut i = 1;
    while i < table.len() {
        table[i] = table[i - 1] * 3;
        i += 1;
    }
    table
};

/// The 2-adic valuation of `n`: how many times 2 divides it.
///
/// This is a single trailing-zero-count instruction. By convention
/// `v2(0) == 128`.
#[inline]
pub fn v2(n: u128) -> u32 {
    n.trailing_zeros()
}

/// The standard Collatz map `C(n)`.
#[inline]
pub fn collatz(n: u128) -> Option<u128> {
    if n % 2 == 0 {
        Some(n / 2)
    } else {
        n.checked_mul(3)?.checked_add(1)
    }
}

/// The shortcut map `T(n)`: `n/2` for even `n`, `(3n+1)/2` for odd `n`.
///
/// For odd `n`, `(3n+1)/2 == n + n/2 + 1`, which avoids overflowing on `3n`
/// when the final result still fits.
#[inline]
pub fn shortcut(n: u128) -> Option<u128> {
    if n % 2 == 0 {
        Some(n / 2)
    } else {
        n.checked_add(n / 2 + 1)
    }
}

/// The Syracuse map `S(n)`, defined on odd `n` only.
///
/// # Panics
///
/// Panics if `n` is even.
#[inline]
pub fn syracuse(n: u128) -> Option<u128> {
    assert!(
        n % 2 == 1,
        "the Syracuse map is only defined on odd numbers"
    );
    let t = shortcut(n)?;
    Some(t >> v2(t))
}

/// Which form of the map to iterate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Map {
    /// `C(n)`: one step per halving or tripling.
    #[default]
    Standard,
    /// `T(n)`: odd steps fold in the halving that always follows.
    Shortcut,
    /// `S(n)`: jumps straight from one odd number to the next.
    Syracuse,
}

impl Map {
    /// Applies one step of this map. For [`Map::Syracuse`], an even input is
    /// first reduced to its odd part.
    pub fn apply(self, n: u128) -> Option<u128> {
        match self {
            Map::Standard => collatz(n),
            Map::Shortcut => shortcut(n),
            Map::Syracuse if n % 2 == 0 => Some(n >> v2(n)),
            Map::Syracuse => syracuse(n),
        }
    }

    /// Short name used on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Map::Standard => "standard",
            Map::Shortcut => "shortcut",
            Map::Syracuse => "syracuse",
        }
    }
}

impl std::str::FromStr for Map {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "c" | "standard" => Ok(Map::Standard),
            "t" | "shortcut" => Ok(Map::Shortcut),
            "s" | "syracuse" => Ok(Map::Syracuse),
            other => Err(format!(
                "unknown map '{other}' (expected standard, shortcut or syracuse)"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn powers_of_three() {
        assert_eq!(POW3[0], 1);
        assert_eq!(POW3[5], 243);
        assert_eq!(
            POW3[80],
            147_808_829_414_345_923_316_083_210_206_383_297_601
        );
        assert!(POW3[80].checked_mul(3).is_none());
    }

    #[test]
    fn single_steps() {
        assert_eq!(collatz(27), Some(82));
        assert_eq!(collatz(82), Some(41));
        assert_eq!(shortcut(27), Some(41));
        assert_eq!(shortcut(82), Some(41));
        assert_eq!(syracuse(27), Some(41));
        assert_eq!(syracuse(5), Some(1));
        assert_eq!(syracuse(1), Some(1));
    }

    #[test]
    fn shortcut_matches_two_standard_steps() {
        for n in (1..10_000u128).step_by(2) {
            assert_eq!(shortcut(n), collatz(collatz(n).unwrap()));
        }
    }

    #[test]
    fn overflow_is_reported() {
        assert_eq!(collatz(u128::MAX), None);
        assert_eq!(shortcut(u128::MAX), None);
        // 3n+1 overflows here, but (3n+1)/2 still fits.
        let n = u128::MAX / 2;
        assert_eq!(collatz(n), None);
        assert_eq!(shortcut(n), Some(3 * (1u128 << 126) - 1));
    }

    #[test]
    fn map_enum() {
        assert_eq!(Map::Standard.apply(7), Some(22));
        assert_eq!(Map::Shortcut.apply(7), Some(11));
        assert_eq!(Map::Syracuse.apply(7), Some(11));
        assert_eq!(Map::Syracuse.apply(48), Some(3));
        assert_eq!("t".parse::<Map>(), Ok(Map::Shortcut));
        assert!("x".parse::<Map>().is_err());
    }

    #[test]
    #[should_panic]
    fn syracuse_rejects_even() {
        let _ = syracuse(4);
    }
}
