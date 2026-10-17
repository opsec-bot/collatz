//! Number and table formatting for terminal output.

use std::fmt::Display;
use std::time::Duration;

/// Writes an integer with thousands separators: `1234567` becomes `1,234,567`.
pub fn group(n: impl Display) -> String {
    let text = n.to_string();
    let (sign, digits) = match text.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", text.as_str()),
    };
    let mut out = String::with_capacity(text.len() * 4 / 3);
    out.push_str(sign);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Shortens huge decimal numbers to their first and last digits.
pub fn abbreviate(n: impl Display, max_digits: usize) -> String {
    let digits = n.to_string();
    if digits.len() <= max_digits {
        return group(&digits);
    }
    let keep = max_digits / 2;
    format!(
        "{}...{} ({} digits)",
        &digits[..keep],
        &digits[digits.len() - keep..],
        group(digits.len())
    )
}

/// Describes a limit, using power-of-two notation where it fits.
pub fn limit(n: u128) -> String {
    if n.is_power_of_two() && n >= 1 << 10 {
        format!("2^{} ({})", n.trailing_zeros(), group(n))
    } else {
        group(n)
    }
}

/// Human-friendly duration: `850 ms`, `12.4 s`, `3m 07s`.
pub fn duration(d: Duration) -> String {
    let secs = d.as_secs_f64();
    if secs < 1.0 {
        format!("{:.0} ms", secs * 1000.0)
    } else if secs < 60.0 {
        format!("{secs:.2} s")
    } else {
        format!("{}m {:02}s", d.as_secs() / 60, d.as_secs() % 60)
    }
}

/// Large counts in words: `2.83 billion`.
pub fn words(x: f64) -> String {
    const UNITS: [(f64, &str); 4] = [
        (1e12, "trillion"),
        (1e9, "billion"),
        (1e6, "million"),
        (1e3, "thousand"),
    ];
    for (scale, name) in UNITS {
        if x >= scale {
            return format!("{:.2} {name}", x / scale);
        }
    }
    format!("{x:.0}")
}

/// Column alignment.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
}

/// Renders rows as an aligned plain-text table with a header rule.
pub fn table(headers: &[(&str, Align)], rows: &[Vec<String>]) -> String {
    let width = |i: usize| {
        rows.iter()
            .map(|row| row[i].chars().count())
            .chain([headers[i].0.chars().count()])
            .max()
            .unwrap_or(0)
    };
    let widths: Vec<usize> = (0..headers.len()).map(width).collect();
    let line = |cells: Vec<&str>| {
        let parts: Vec<String> = cells
            .iter()
            .zip(&widths)
            .zip(headers)
            .map(|((cell, &w), &(_, align))| match align {
                Align::Left => format!("{cell:<w$}"),
                Align::Right => format!("{cell:>w$}"),
            })
            .collect();
        format!("  {}", parts.join("  ").trim_end())
    };
    let mut out = vec![line(headers.iter().map(|h| h.0).collect())];
    out.push(format!(
        "  {}",
        widths
            .iter()
            .map(|&w| "─".repeat(w))
            .collect::<Vec<_>>()
            .join("  ")
    ));
    out.extend(
        rows.iter()
            .map(|row| line(row.iter().map(String::as_str).collect())),
    );
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouping() {
        assert_eq!(group(0), "0");
        assert_eq!(group(999), "999");
        assert_eq!(group(1000), "1,000");
        assert_eq!(group(1_234_567u64), "1,234,567");
        assert_eq!(group(-100_000), "-100,000");
        assert_eq!(group(-12), "-12");
        assert_eq!(limit(1 << 32), "2^32 (4,294,967,296)");
        assert_eq!(limit(1000), "1,000");
        assert_eq!(
            abbreviate("12345678901234567890", 8),
            "1234...7890 (20 digits)"
        );
    }

    #[test]
    fn durations_and_words() {
        assert_eq!(duration(Duration::from_millis(250)), "250 ms");
        assert_eq!(duration(Duration::from_millis(12_340)), "12.34 s");
        assert_eq!(duration(Duration::from_secs(187)), "3m 07s");
        assert_eq!(words(2.83e9), "2.83 billion");
        assert_eq!(words(512.0), "512");
    }

    #[test]
    fn tables() {
        let t = table(
            &[("n", Align::Right), ("name", Align::Left)],
            &[
                vec!["1".into(), "one".into()],
                vec!["27".into(), "x".into()],
            ],
        );
        assert_eq!(t, "   n  name\n  ──  ────\n   1  one\n  27  x");
    }
}
