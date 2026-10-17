//! Parses number expressions such as `2^40`, `1e9`, `3*2^60+1` or `(2^64-1)^2`.
//!
//! Grammar, with the usual precedence and a right-associative `^`:
//!
//! ```text
//! expr  = term (('+' | '-') term)*
//! term  = power ('*' power)*
//! power = atom ('^' power)?
//! atom  = digits ('e' digits)? | '(' expr ')'
//! ```
//!
//! Underscores inside numbers are ignored, so `1_000_000` works too.

use crate::bigint::BigUint;

/// Results larger than this many bits are rejected rather than computed.
pub const MAX_BITS: u64 = 1 << 20;

/// Parses and evaluates an expression.
pub fn parse(input: &str) -> Result<BigUint, String> {
    let mut parser = Parser {
        chars: input.chars().collect(),
        pos: 0,
    };
    let value = parser.expr()?;
    match parser.peek() {
        None => Ok(value),
        Some(c) => Err(format!("unexpected '{c}' in '{input}'")),
    }
}

/// Parses an expression that must fit in a `u128`.
pub fn parse_u128(input: &str) -> Result<u128, String> {
    parse(input)?
        .to_u128()
        .ok_or_else(|| format!("'{input}' does not fit in 128 bits"))
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    /// The next non-whitespace character.
    fn peek(&mut self) -> Option<char> {
        while self.chars.get(self.pos).is_some_and(|c| c.is_whitespace()) {
            self.pos += 1;
        }
        self.chars.get(self.pos).copied()
    }

    fn eat(&mut self, c: char) -> bool {
        let found = self.peek() == Some(c);
        self.pos += usize::from(found);
        found
    }

    fn expr(&mut self) -> Result<BigUint, String> {
        let mut value = self.term()?;
        loop {
            if self.eat('+') {
                value = &value + &self.term()?;
            } else if self.eat('-') {
                let rhs = self.term()?;
                value = value
                    .checked_sub(&rhs)
                    .ok_or("negative numbers are not supported")?;
            } else {
                return Ok(value);
            }
        }
    }

    fn term(&mut self) -> Result<BigUint, String> {
        let mut value = self.power()?;
        while self.eat('*') {
            value = &value * &self.power()?;
            check_size(&value)?;
        }
        Ok(value)
    }

    fn power(&mut self) -> Result<BigUint, String> {
        let base = self.atom()?;
        if !self.eat('^') {
            return Ok(base);
        }
        let exp = self.power()?;
        pow(&base, &exp)
    }

    fn atom(&mut self) -> Result<BigUint, String> {
        if self.eat('(') {
            let value = self.expr()?;
            return if self.eat(')') {
                Ok(value)
            } else {
                Err("missing ')'".into())
            };
        }
        let mantissa = self.digits()?;
        if self.eat('e') {
            let exp = self.digits()?;
            let scale = pow(&BigUint::from(10u64), &exp)?;
            let value = &mantissa * &scale;
            check_size(&value)?;
            return Ok(value);
        }
        Ok(mantissa)
    }

    fn digits(&mut self) -> Result<BigUint, String> {
        self.peek();
        let start = self.pos;
        while matches!(self.chars.get(self.pos), Some(c) if c.is_ascii_digit() || *c == '_') {
            self.pos += 1;
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        if text.is_empty() {
            return Err(match self.peek() {
                Some(c) => format!("expected a number, found '{c}'"),
                None => "expected a number".into(),
            });
        }
        text.parse().map_err(|e| format!("{e}"))
    }
}

fn pow(base: &BigUint, exp: &BigUint) -> Result<BigUint, String> {
    let exp = exp.to_u128().and_then(|e| u64::try_from(e).ok());
    let too_big = || format!("result would exceed {MAX_BITS} bits");
    let exp = exp.ok_or_else(too_big)?;
    if base.bits() > 1 && base.bits().saturating_sub(1).saturating_mul(exp) > MAX_BITS {
        return Err(too_big());
    }
    Ok(base.pow(exp))
}

fn check_size(value: &BigUint) -> Result<(), String> {
    if value.bits() > MAX_BITS {
        Err(format!("result would exceed {MAX_BITS} bits"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(s: &str) -> String {
        parse(s).unwrap().to_string()
    }

    #[test]
    fn arithmetic() {
        assert_eq!(eval("27"), "27");
        assert_eq!(eval("1_000_000"), "1000000");
        assert_eq!(eval("2^10"), "1024");
        assert_eq!(eval("2^3^2"), "512");
        assert_eq!(eval("1 + 2 * 3"), "7");
        assert_eq!(eval("(1 + 2) * 3"), "9");
        assert_eq!(eval("2^64 - 1"), u64::MAX.to_string());
        assert_eq!(eval("3*2^60+1"), (3u128 * (1 << 60) + 1).to_string());
        assert_eq!(eval("1e9"), "1000000000");
        assert_eq!(eval("5e0"), "5");
        assert_eq!(eval("10-3-2"), "5");
        assert_eq!(parse_u128("2^127").unwrap(), 1 << 127);
    }

    #[test]
    fn errors() {
        assert!(parse("").is_err());
        assert!(parse("2^").is_err());
        assert!(parse("(2").is_err());
        assert!(parse("1 - 2").is_err());
        assert!(parse("2 2").is_err());
        assert!(parse("abc").is_err());
        assert!(parse("2^(2^40)").is_err());
        assert!(parse_u128("2^128").is_err());
        // Trivial bases are fine at any exponent.
        assert_eq!(eval("1^(2^40)"), "1");
        assert_eq!(eval("0^5"), "0");
    }
}
