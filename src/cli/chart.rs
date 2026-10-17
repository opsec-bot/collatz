//! Bar charts drawn with Unicode block characters.

const EIGHTHS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Draws `values` as a bar chart `width` columns wide and `height` rows tall.
///
/// When there are more values than columns, each column shows the largest
/// value that falls into it, so peaks are never hidden. The y-axis is labeled
/// with `label(value)` at the top and bottom rows.
pub fn bars(values: &[f64], width: usize, height: usize, label: impl Fn(f64) -> String) -> String {
    if values.is_empty() || width == 0 || height == 0 {
        return String::new();
    }
    let columns: Vec<f64> = if values.len() <= width {
        values.to_vec()
    } else {
        (0..width)
            .map(|c| {
                let lo = c * values.len() / width;
                let hi = ((c + 1) * values.len() / width).max(lo + 1);
                values[lo..hi].iter().copied().fold(f64::MIN, f64::max)
            })
            .collect()
    };
    let max = columns.iter().copied().fold(f64::MIN, f64::max);
    let min = columns.iter().copied().fold(f64::MAX, f64::min).min(0.0);
    let span = (max - min).max(f64::EPSILON);
    // Height of each column in eighths of a row; every column gets at least one.
    let levels: Vec<usize> = columns
        .iter()
        .map(|&v| (((v - min) / span) * (height * 8) as f64).round().max(1.0) as usize)
        .collect();

    let top = label(max);
    let bottom = label(min);
    let gutter = top.chars().count().max(bottom.chars().count());
    let mut lines = Vec::with_capacity(height + 1);
    for row in (0..height).rev() {
        let tag = match row {
            r if r == height - 1 => top.as_str(),
            0 => bottom.as_str(),
            _ => "",
        };
        let cells: String = levels
            .iter()
            .map(|&level| EIGHTHS[level.saturating_sub(row * 8).min(8)])
            .collect();
        lines.push(format!("  {tag:>gutter$} ┤{cells}"));
    }
    lines.push(format!("  {:>gutter$} └{}", "", "─".repeat(columns.len())));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    #[test]
    fn draws_a_ramp() {
        let chart = super::bars(&[0.0, 1.0, 2.0, 3.0], 4, 1, |v| format!("{v:.0}"));
        assert_eq!(chart, "  3 ┤▁▃▅█\n    └────");
    }

    #[test]
    fn keeps_peaks_when_downsampling() {
        let mut values = vec![1.0; 1000];
        values[517] = 50.0;
        let chart = super::bars(&values, 10, 2, |v| format!("{v:.0}"));
        assert!(chart.contains('█'));
        assert_eq!(chart.lines().count(), 3);
    }
}
