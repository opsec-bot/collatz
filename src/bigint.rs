//! A small arbitrary-precision unsigned integer.
//!
//! Orbits climb far above their starting value (numbers near 2^64 peak past
//! 2^128), so the fast `u128` code needs somewhere to fall back to. This
//! module implements only what the Collatz tooling needs, with simple
//! schoolbook algorithms; it is not a general-purpose bignum library.

use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, Mul};
use std::str::FromStr;

/// The largest power of ten that fits in a `u64`.
const DECIMAL_CHUNK: u64 = 10_000_000_000_000_000_000;
const DECIMAL_CHUNK_DIGITS: usize = 19;

/// An arbitrary-precision unsigned integer.
#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct BigUint {
    /// Little-endian 64-bit limbs with no trailing zero limbs, so zero is empty.
    limbs: Vec<u64>,
}

impl BigUint {
    /// Zero.
    pub const fn zero() -> Self {
        Self { limbs: Vec::new() }
    }

    /// One.
    pub fn one() -> Self {
        Self::from(1u64)
    }

    /// `2^exp`.
    pub fn power_of_two(exp: u64) -> Self {
        let mut limbs = vec![0; (exp / 64) as usize + 1];
        *limbs.last_mut().unwrap() = 1 << (exp % 64);
        Self { limbs }
    }

    pub fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    pub fn is_one(&self) -> bool {
        self.limbs == [1]
    }

    pub fn is_even(&self) -> bool {
        self.limbs.first().map_or(true, |limb| limb & 1 == 0)
    }

    /// Number of bits needed to write the value (`0` for zero).
    pub fn bits(&self) -> u64 {
        match self.limbs.last() {
            None => 0,
            Some(top) => self.limbs.len() as u64 * 64 - u64::from(top.leading_zeros()),
        }
    }

    /// Number of trailing zero bits, or `None` for zero.
    pub fn trailing_zeros(&self) -> Option<u64> {
        let i = self.limbs.iter().position(|&limb| limb != 0)?;
        Some(i as u64 * 64 + u64::from(self.limbs[i].trailing_zeros()))
    }

    /// The value as a `u128`, if it fits.
    pub fn to_u128(&self) -> Option<u128> {
        match *self.limbs.as_slice() {
            [] => Some(0),
            [lo] => Some(u128::from(lo)),
            [lo, hi] => Some(u128::from(lo) | u128::from(hi) << 64),
            _ => None,
        }
    }

    /// `self >>= bits`.
    pub fn shr_assign(&mut self, bits: u64) {
        let limb_shift = (bits / 64) as usize;
        if limb_shift >= self.limbs.len() {
            self.limbs.clear();
            return;
        }
        self.limbs.drain(..limb_shift);
        let shift = (bits % 64) as u32;
        if shift > 0 {
            for i in 0..self.limbs.len() {
                let hi = self.limbs.get(i + 1).copied().unwrap_or(0);
                self.limbs[i] = (self.limbs[i] >> shift) | (hi << (64 - shift));
            }
        }
        self.normalize();
    }

    /// `self = self * mul + add`. This is the `3n + 1` step.
    pub fn mul_add_small(&mut self, mul: u64, add: u64) {
        let mut carry = u128::from(add);
        for limb in &mut self.limbs {
            let t = u128::from(*limb) * u128::from(mul) + carry;
            *limb = t as u64;
            carry = t >> 64;
        }
        if carry > 0 {
            self.limbs.push(carry as u64);
        }
        self.normalize();
    }

    /// Quotient and remainder of division by a nonzero `u64`.
    ///
    /// # Panics
    ///
    /// Panics if `divisor` is zero.
    pub fn div_rem_small(&self, divisor: u64) -> (BigUint, u64) {
        assert!(divisor != 0, "division by zero");
        let divisor = u128::from(divisor);
        let mut quotient = vec![0u64; self.limbs.len()];
        let mut rem = 0u128;
        for i in (0..self.limbs.len()).rev() {
            let cur = (rem << 64) | u128::from(self.limbs[i]);
            quotient[i] = (cur / divisor) as u64;
            rem = cur % divisor;
        }
        let mut quotient = BigUint { limbs: quotient };
        quotient.normalize();
        (quotient, rem as u64)
    }

    /// `self - other`, or `None` if the result would be negative.
    pub fn checked_sub(&self, other: &BigUint) -> Option<BigUint> {
        if self < other {
            return None;
        }
        let mut limbs = self.limbs.clone();
        let mut borrow = false;
        for (i, limb) in limbs.iter_mut().enumerate() {
            let rhs = other.limbs.get(i).copied().unwrap_or(0);
            let (d1, b1) = limb.overflowing_sub(rhs);
            let (d2, b2) = d1.overflowing_sub(u64::from(borrow));
            *limb = d2;
            borrow = b1 || b2;
        }
        debug_assert!(!borrow);
        let mut result = BigUint { limbs };
        result.normalize();
        Some(result)
    }

    /// `self^exp` by repeated squaring.
    pub fn pow(&self, mut exp: u64) -> BigUint {
        let mut base = self.clone();
        let mut result = BigUint::one();
        while exp > 0 {
            if exp & 1 == 1 {
                result = &result * &base;
            }
            exp >>= 1;
            if exp > 0 {
                base = &base * &base;
            }
        }
        result
    }

    /// An approximation of `log2(self)`; negative infinity for zero.
    pub fn log2(&self) -> f64 {
        let bits = self.bits();
        if bits <= 64 {
            return (self.limbs.first().copied().unwrap_or(0) as f64).log2();
        }
        // Keep the top 64 bits; the rest only affects digits far past f64 precision.
        let mut top = self.clone();
        top.shr_assign(bits - 64);
        (top.limbs[0] as f64).log2() + (bits - 64) as f64
    }

    fn normalize(&mut self) {
        while self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
    }
}

impl From<u64> for BigUint {
    fn from(value: u64) -> Self {
        let mut n = BigUint { limbs: vec![value] };
        n.normalize();
        n
    }
}

impl From<u128> for BigUint {
    fn from(value: u128) -> Self {
        let mut n = BigUint {
            limbs: vec![value as u64, (value >> 64) as u64],
        };
        n.normalize();
        n
    }
}

impl Ord for BigUint {
    fn cmp(&self, other: &Self) -> Ordering {
        self.limbs
            .len()
            .cmp(&other.limbs.len())
            .then_with(|| self.limbs.iter().rev().cmp(other.limbs.iter().rev()))
    }
}

impl PartialOrd for BigUint {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Add for &BigUint {
    type Output = BigUint;

    fn add(self, other: &BigUint) -> BigUint {
        let (long, short) = if self.limbs.len() >= other.limbs.len() {
            (self, other)
        } else {
            (other, self)
        };
        let mut limbs = Vec::with_capacity(long.limbs.len() + 1);
        let mut carry = false;
        for (i, &a) in long.limbs.iter().enumerate() {
            let b = short.limbs.get(i).copied().unwrap_or(0);
            let (s1, c1) = a.overflowing_add(b);
            let (s2, c2) = s1.overflowing_add(u64::from(carry));
            limbs.push(s2);
            carry = c1 || c2;
        }
        if carry {
            limbs.push(1);
        }
        BigUint { limbs }
    }
}

impl Mul for &BigUint {
    type Output = BigUint;

    fn mul(self, other: &BigUint) -> BigUint {
        if self.is_zero() || other.is_zero() {
            return BigUint::zero();
        }
        let mut limbs = vec![0u64; self.limbs.len() + other.limbs.len()];
        for (i, &a) in self.limbs.iter().enumerate() {
            let mut carry = 0u128;
            for (j, &b) in other.limbs.iter().enumerate() {
                let t = u128::from(a) * u128::from(b) + u128::from(limbs[i + j]) + carry;
                limbs[i + j] = t as u64;
                carry = t >> 64;
            }
            limbs[i + other.limbs.len()] = carry as u64;
        }
        let mut result = BigUint { limbs };
        result.normalize();
        result
    }
}

impl fmt::Display for BigUint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_zero() {
            return f.pad("0");
        }
        let mut chunks = Vec::new();
        let mut rest = self.clone();
        while !rest.is_zero() {
            let (q, r) = rest.div_rem_small(DECIMAL_CHUNK);
            chunks.push(r);
            rest = q;
        }
        let mut s = chunks.pop().unwrap().to_string();
        for chunk in chunks.iter().rev() {
            s.push_str(&format!("{chunk:0width$}", width = DECIMAL_CHUNK_DIGITS));
        }
        f.pad(&s)
    }
}

impl fmt::Debug for BigUint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Error returned when parsing a [`BigUint`] from a string fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseBigUintError(String);

impl fmt::Display for ParseBigUintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid number '{}'", self.0)
    }
}

impl std::error::Error for ParseBigUintError {}

impl FromStr for BigUint {
    type Err = ParseBigUintError;

    /// Parses a decimal number. Underscores are allowed as digit separators.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let digits: Vec<u8> = s.bytes().filter(|&b| b != b'_').collect();
        if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
            return Err(ParseBigUintError(s.to_string()));
        }
        let mut n = BigUint::zero();
        for chunk in digits.chunks(DECIMAL_CHUNK_DIGITS) {
            let value = chunk
                .iter()
                .fold(0u64, |acc, &d| acc * 10 + u64::from(d - b'0'));
            n.mul_add_small(10u64.pow(chunk.len() as u32), value);
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn big(s: &str) -> BigUint {
        s.parse().unwrap()
    }

    #[test]
    fn round_trips_through_decimal() {
        for s in [
            "0",
            "1",
            "18446744073709551615",
            "18446744073709551616",
            "340282366920938463463374607431768211456",
            "1606938044258990275541962092341162602522202993782792835301376",
        ] {
            assert_eq!(big(s).to_string(), s);
        }
        assert_eq!(big("1_000_000").to_string(), "1000000");
        assert!("".parse::<BigUint>().is_err());
        assert!("12a".parse::<BigUint>().is_err());
        assert!("-5".parse::<BigUint>().is_err());
    }

    #[test]
    fn agrees_with_u128() {
        let values = [0u128, 1, 2, 3, 27, u64::MAX as u128, 1 << 64, u128::MAX];
        for &a in &values {
            let n = BigUint::from(a);
            assert_eq!(n.to_u128(), Some(a));
            assert_eq!(n.to_string(), a.to_string());
            assert_eq!(n.bits(), u64::from(128 - a.leading_zeros()));
            assert_eq!(n.is_even(), a % 2 == 0);
            for &b in &values {
                let m = BigUint::from(b);
                assert_eq!(n.cmp(&m), a.cmp(&b));
                if let Some(sum) = a.checked_add(b) {
                    assert_eq!((&n + &m).to_u128(), Some(sum));
                }
                if let Some(product) = a.checked_mul(b) {
                    assert_eq!((&n * &m).to_u128(), Some(product));
                }
                assert_eq!(
                    n.checked_sub(&m).map(|d| d.to_u128().unwrap()),
                    a.checked_sub(b)
                );
            }
        }
    }

    #[test]
    fn shifts_and_trailing_zeros() {
        let mut n = BigUint::power_of_two(200);
        assert_eq!(n.bits(), 201);
        assert_eq!(n.trailing_zeros(), Some(200));
        n.shr_assign(137);
        assert_eq!(n.to_u128(), Some(1 << 63));
        n.shr_assign(64);
        assert!(n.is_zero());
        assert_eq!(n.trailing_zeros(), None);
    }

    #[test]
    fn three_n_plus_one_carries() {
        let mut n = BigUint::from(u128::MAX);
        n.mul_add_small(3, 1);
        assert_eq!(n.to_string(), "1020847100762815390390123822295304634366");
    }

    #[test]
    fn powers() {
        assert_eq!(
            BigUint::from(3u64).pow(80).to_u128(),
            Some(crate::map::POW3[80])
        );
        assert_eq!(BigUint::from(2u64).pow(200), BigUint::power_of_two(200));
        assert_eq!(BigUint::from(7u64).pow(0), BigUint::one());
    }

    #[test]
    fn division_by_small() {
        let (q, r) = big("1000000000000000000000000000007").div_rem_small(10);
        assert_eq!(q.to_string(), "100000000000000000000000000000");
        assert_eq!(r, 7);
    }

    #[test]
    fn approximate_log2() {
        assert_eq!(BigUint::from(1024u64).log2(), 10.0);
        assert!((BigUint::power_of_two(1000).log2() - 1000.0).abs() < 1e-9);
        assert_eq!(BigUint::zero().log2(), f64::NEG_INFINITY);
    }
}
