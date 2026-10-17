//! Self-contained SVG charts, for the README and for exploring.
//!
//! Charts follow the system light or dark theme through a `prefers-color-scheme`
//! media query inside the SVG, so they read well on GitHub in either mode.
//! Series colors come from a fixed categorical order that has been checked
//! for color-vision-deficiency separation; identity is never carried by
//! color alone, since every series also gets a legend entry and a direct label.

use std::fmt::Write;

const WIDTH: f64 = 760.0;
const HEIGHT: f64 = 440.0;
const MARGIN_LEFT: f64 = 64.0;
const MARGIN_RIGHT: f64 = 28.0;
const MARGIN_TOP: f64 = 112.0;
const MARGIN_BOTTOM: f64 = 52.0;

/// Light and dark steps of the categorical palette, in slot order.
const SERIES: [(&str, &str); 4] = [
    ("#2a78d6", "#3987e5"),
    ("#eb6834", "#d95926"),
    ("#1baf7a", "#199e70"),
    ("#eda100", "#c98500"),
];

/// The most series one chart will draw.
pub const MAX_SERIES: usize = SERIES.len();

/// One line on a [`line_chart`].
#[derive(Debug, Clone)]
pub struct Series {
    pub label: String,
    pub points: Vec<(f64, f64)>,
    /// A point to call out with a dot and a text label.
    pub callout: Option<(f64, f64, String)>,
}

/// How an axis turns values into tick labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    /// Plain numbers, compacted from 1,000 up (`20K`, `1.5M`).
    Linear,
    /// Values are base-2 exponents, labeled `2^k`.
    Log2,
    /// Values are percentages, labeled `34%`.
    Percent,
}

/// Axis titles and scales.
#[derive(Debug, Clone)]
pub struct Axes {
    pub x_label: String,
    pub y_label: String,
    pub x_scale: Scale,
    pub y_scale: Scale,
    /// Fixed x range; by default the axis starts at zero and fits the data.
    pub x_range: Option<(f64, f64)>,
    /// Fixed y range; by default the axis starts at zero and fits the data.
    pub y_range: Option<(f64, f64)>,
}

/// Picks about `target` round tick values covering `[min, max]`.
fn nice_ticks(min: f64, max: f64, target: usize, scale: Scale) -> Vec<f64> {
    let span = (max - min).max(f64::EPSILON);
    let raw = span / target as f64;
    let magnitude = 10f64.powf(raw.log10().floor());
    let mut step = [1.0, 2.0, 2.5, 5.0, 10.0]
        .iter()
        .map(|m| m * magnitude)
        .find(|&s| s >= raw)
        .unwrap_or(10.0 * magnitude);
    if scale == Scale::Log2 {
        // Powers of two only make sense at integer exponents.
        step = step.max(1.0).round();
    }
    let first = (min / step).ceil() * step;
    (0..)
        .map(|i| first + i as f64 * step)
        .take_while(|&t| t <= max + step * 1e-9)
        .collect()
}

fn tick_label(value: f64, scale: Scale) -> String {
    match scale {
        Scale::Log2 => format!("2^{}", value.round() as i64),
        Scale::Linear => compact(value),
        Scale::Percent => format!("{}%", compact(value)),
    }
}

/// `1234` -> `1.2K`, `20000` -> `20K`, `1500000` -> `1.5M`.
fn compact(value: f64) -> String {
    let (scaled, suffix) = match value.abs() {
        v if v >= 1e9 => (value / 1e9, "B"),
        v if v >= 1e6 => (value / 1e6, "M"),
        v if v >= 1e3 => (value / 1e3, "K"),
        _ => (value, ""),
    };
    let text = format!("{scaled:.1}");
    let text = text.strip_suffix(".0").unwrap_or(&text);
    format!("{text}{suffix}")
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Maps data coordinates to pixels.
struct Frame {
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
}

impl Frame {
    fn fit(points: impl Iterator<Item = (f64, f64)>, axes: &Axes) -> Frame {
        let mut frame = Frame {
            x_min: f64::MAX,
            x_max: f64::MIN,
            y_min: f64::MAX,
            y_max: f64::MIN,
        };
        for (x, y) in points {
            frame.x_min = frame.x_min.min(x);
            frame.x_max = frame.x_max.max(x);
            frame.y_min = frame.y_min.min(y);
            frame.y_max = frame.y_max.max(y);
        }
        if frame.x_min > frame.x_max {
            (frame.x_min, frame.x_max, frame.y_min, frame.y_max) = (0.0, 1.0, 0.0, 1.0);
        }
        // Charts here measure from zero; give the top a little headroom.
        frame.x_min = frame.x_min.min(0.0);
        frame.y_min = frame.y_min.min(0.0);
        frame.y_max += (frame.y_max - frame.y_min).max(1.0) * 0.06;
        frame.x_max = frame.x_max.max(frame.x_min + 1.0);
        if let Some((lo, hi)) = axes.x_range {
            (frame.x_min, frame.x_max) = (lo, hi);
        }
        if let Some((lo, hi)) = axes.y_range {
            (frame.y_min, frame.y_max) = (lo, hi);
        }
        frame
    }

    fn px(&self, x: f64) -> f64 {
        MARGIN_LEFT
            + (x - self.x_min) / (self.x_max - self.x_min) * (WIDTH - MARGIN_LEFT - MARGIN_RIGHT)
    }

    fn py(&self, y: f64) -> f64 {
        HEIGHT
            - MARGIN_BOTTOM
            - (y - self.y_min) / (self.y_max - self.y_min) * (HEIGHT - MARGIN_TOP - MARGIN_BOTTOM)
    }
}

fn style(series: usize) -> String {
    let mut light = String::new();
    let mut dark = String::new();
    for (i, (l, d)) in SERIES.iter().take(series.max(1)).enumerate() {
        let _ = write!(light, ".s{i}{{stroke:{l};fill:{l}}}");
        let _ = write!(dark, ".s{i}{{stroke:{d};fill:{d}}}");
    }
    format!(
        "<style>\
         .surface{{fill:#fcfcfb}}\
         .grid{{stroke:#e8e7e3}}.axis{{stroke:#c3c2b7}}\
         .ink{{fill:#0b0b0b}}.muted{{fill:#52514e}}.ref{{stroke:#8a8983}}\
         .halo{{paint-order:stroke;stroke-width:4px;stroke-linejoin:round}}\
         text{{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Helvetica,Arial,sans-serif}}\
         {light}.ring,.halo{{stroke:#fcfcfb}}\
         @media (prefers-color-scheme:dark){{\
         .surface{{fill:#1a1a19}}\
         .grid{{stroke:#2c2c2a}}.axis{{stroke:#4a4a46}}\
         .ink{{fill:#f0efec}}.muted{{fill:#c3c2b7}}.ref{{stroke:#8a8983}}\
         {dark}.ring,.halo{{stroke:#1a1a19}}}}\
         </style>"
    )
}

/// Opens the SVG and draws the card, titles, grid and axes.
fn open(svg: &mut String, title: &str, subtitle: &str, axes: &Axes, frame: &Frame, series: usize) {
    let _ = write!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {WIDTH} {HEIGHT}\" \
         width=\"{WIDTH}\" height=\"{HEIGHT}\" role=\"img\" aria-labelledby=\"title desc\">\
         <title id=\"title\">{}</title><desc id=\"desc\">{}</desc>{}\
         <rect class=\"surface\" width=\"{WIDTH}\" height=\"{HEIGHT}\" rx=\"12\"/>\
         <text class=\"ink\" x=\"{MARGIN_LEFT}\" y=\"30\" font-size=\"16\" font-weight=\"600\">{}</text>\
         <text class=\"muted\" x=\"{MARGIN_LEFT}\" y=\"50\" font-size=\"12.5\">{}</text>",
        escape(title),
        escape(subtitle),
        style(series),
        escape(title),
        escape(subtitle),
    );

    let (x0, x1) = (frame.px(frame.x_min), frame.px(frame.x_max));
    let (y0, y1) = (frame.py(frame.y_min), frame.py(frame.y_max));
    for t in nice_ticks(frame.y_min, frame.y_max, 5, axes.y_scale) {
        let y = frame.py(t);
        let _ = write!(
            svg,
            "<line class=\"grid\" x1=\"{x0:.1}\" x2=\"{x1:.1}\" y1=\"{y:.1}\" y2=\"{y:.1}\" stroke-width=\"1\"/>\
             <text class=\"muted\" x=\"{:.1}\" y=\"{:.1}\" font-size=\"11\" text-anchor=\"end\">{}</text>",
            x0 - 8.0,
            y + 4.0,
            tick_label(t, axes.y_scale)
        );
    }
    for t in nice_ticks(frame.x_min, frame.x_max, 6, axes.x_scale) {
        let x = frame.px(t);
        let _ = write!(
            svg,
            "<text class=\"muted\" x=\"{x:.1}\" y=\"{:.1}\" font-size=\"11\" text-anchor=\"middle\">{}</text>",
            y0 + 18.0,
            tick_label(t, axes.x_scale)
        );
    }
    let _ = write!(
        svg,
        "<line class=\"axis\" x1=\"{x0:.1}\" x2=\"{x1:.1}\" y1=\"{y0:.1}\" y2=\"{y0:.1}\" stroke-width=\"1\"/>\
         <text class=\"muted\" x=\"{:.1}\" y=\"{:.1}\" font-size=\"11.5\" text-anchor=\"middle\">{}</text>\
         <text class=\"muted\" x=\"{MARGIN_LEFT}\" y=\"{:.1}\" font-size=\"11.5\">{}</text>",
        (x0 + x1) / 2.0,
        HEIGHT - 12.0,
        escape(&axes.x_label),
        y1 - 12.0,
        escape(&axes.y_label),
    );
}

/// A dot with a surface ring, and a label beside it that stays inside the plot.
fn callout(svg: &mut String, frame: &Frame, class: &str, (x, y, text): (f64, f64, &str)) {
    let (px, py) = (frame.px(x), frame.py(y));
    // Above and to the right, unless that would run off the right edge.
    let (lx, ly, anchor) = if px < WIDTH - 220.0 {
        (px + 9.0, py - 6.0, "start")
    } else {
        (px - 6.0, py + 22.0, "end")
    };
    let _ = write!(
        svg,
        "<circle class=\"{class} ring\" cx=\"{px:.1}\" cy=\"{py:.1}\" r=\"4.5\" stroke-width=\"2\"/>\
         <text class=\"ink halo\" x=\"{lx:.1}\" y=\"{ly:.1}\" font-size=\"12\" text-anchor=\"{anchor}\">{}</text>",
        escape(text)
    );
}

/// A line chart of up to [`MAX_SERIES`] series, with a legend and callouts.
pub fn line_chart(title: &str, subtitle: &str, axes: &Axes, series: &[Series]) -> String {
    let series = &series[..series.len().min(MAX_SERIES)];
    let frame = Frame::fit(series.iter().flat_map(|s| s.points.iter().copied()), axes);
    let mut svg = String::new();
    open(&mut svg, title, subtitle, axes, &frame, series.len());

    // Legend: a short line key beside each label, in one row under the subtitle.
    let mut x = MARGIN_LEFT;
    for (i, s) in series.iter().enumerate() {
        let _ = write!(
            svg,
            "<line class=\"s{i}\" x1=\"{x:.1}\" x2=\"{:.1}\" y1=\"76\" y2=\"76\" stroke-width=\"3\" stroke-linecap=\"round\"/>\
             <text class=\"ink\" x=\"{:.1}\" y=\"80\" font-size=\"12\">{}</text>",
            x + 16.0,
            x + 22.0,
            escape(&s.label)
        );
        x += 34.0 + 7.0 * s.label.chars().count() as f64;
    }

    for (i, s) in series.iter().enumerate() {
        let mut path = String::new();
        for (j, &(px, py)) in s.points.iter().enumerate() {
            let _ = write!(
                path,
                "{}{:.1} {:.1}",
                if j == 0 { "M" } else { "L" },
                frame.px(px),
                frame.py(py)
            );
        }
        let _ = write!(
            svg,
            "<path class=\"s{i}\" d=\"{path}\" fill=\"none\" stroke-width=\"2\" \
             stroke-linejoin=\"round\" stroke-linecap=\"round\" style=\"fill:none\"/>"
        );
    }
    for (i, s) in series.iter().enumerate() {
        if let Some((x, y, text)) = &s.callout {
            callout(&mut svg, &frame, &format!("s{i}"), (*x, *y, text));
        }
    }
    svg.push_str("</svg>\n");
    svg
}

/// A scatter plot of one series, with an optional dashed reference line
/// `y = slope * x` and labeled points.
pub fn scatter_chart(
    title: &str,
    subtitle: &str,
    axes: &Axes,
    points: &[(f64, f64)],
    reference: Option<(f64, &str)>,
    callouts: &[(f64, f64, String)],
) -> String {
    let frame = Frame::fit(points.iter().copied(), axes);
    let mut svg = String::new();
    open(&mut svg, title, subtitle, axes, &frame, 1);

    if let Some((slope, label)) = reference {
        // Clip the line to the plot area.
        let x_end = frame.x_max.min(frame.y_max / slope);
        let _ = write!(
            svg,
            "<line class=\"ref\" x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" \
             stroke-width=\"1.5\" stroke-dasharray=\"5 5\"/>\
             <text class=\"muted halo\" x=\"{:.1}\" y=\"{:.1}\" font-size=\"12\" text-anchor=\"end\">{}</text>",
            frame.px(0.0),
            frame.py(0.0),
            frame.px(x_end),
            frame.py(slope * x_end),
            // Label the line partway along, clear of the data at either end.
            frame.px(x_end * 0.4) - 8.0,
            frame.py(slope * x_end * 0.4) - 8.0,
            escape(label),
        );
    }

    // Dense scatters repeat pixels; drawing each half-pixel cell once keeps files small.
    let radius = if points.len() > 2_000 { 1.3 } else { 3.5 };
    let mut seen = std::collections::HashSet::new();
    svg.push_str("<g class=\"s0\" fill-opacity=\"0.55\">");
    for &(x, y) in points {
        let (px, py) = (frame.px(x), frame.py(y));
        if seen.insert(((px * 2.0) as i64, (py * 2.0) as i64)) {
            let _ = write!(
                svg,
                "<circle cx=\"{px:.1}\" cy=\"{py:.1}\" r=\"{radius}\"/>"
            );
        }
    }
    svg.push_str("</g>");
    for (x, y, text) in callouts {
        callout(&mut svg, &frame, "s0", (*x, *y, text));
    }
    svg.push_str("</svg>\n");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axes() -> Axes {
        Axes {
            x_label: "step".into(),
            y_label: "value".into(),
            x_scale: Scale::Linear,
            y_scale: Scale::Log2,
            x_range: None,
            y_range: None,
        }
    }

    #[test]
    fn ticks_are_round() {
        assert_eq!(
            nice_ticks(0.0, 100.0, 5, Scale::Linear),
            [0.0, 20.0, 40.0, 60.0, 80.0, 100.0]
        );
        assert_eq!(nice_ticks(0.0, 3.3, 5, Scale::Log2), [0.0, 1.0, 2.0, 3.0]);
        assert_eq!(compact(20_000.0), "20K");
        assert_eq!(compact(1_500_000.0), "1.5M");
        assert_eq!(compact(12.0), "12");
        assert_eq!(tick_label(10.0, Scale::Log2), "2^10");
        assert_eq!(tick_label(34.5, Scale::Percent), "34.5%");
    }

    #[test]
    fn line_chart_is_well_formed() {
        let series = vec![Series {
            label: "27 & friends".into(),
            points: vec![(0.0, 4.75), (1.0, 6.36), (2.0, 5.36)],
            callout: Some((1.0, 6.36, "peak <82>".into())),
        }];
        let svg = line_chart("Orbits", "a test", &axes(), &series);
        assert!(svg.starts_with("<svg"));
        assert!(svg.trim_end().ends_with("</svg>"));
        assert!(svg.contains("27 &amp; friends"));
        assert!(svg.contains("peak &lt;82&gt;"));
        assert!(svg.contains("prefers-color-scheme:dark"));
        assert_eq!(svg.matches("<svg").count(), svg.matches("</svg>").count());
    }

    #[test]
    fn scatter_dedupes_pixels() {
        let points: Vec<(f64, f64)> = (0..10_000).map(|i| ((i % 10) as f64, 1.0)).collect();
        let svg = scatter_chart("Dots", "", &axes(), &points, Some((2.0, "y = 2x")), &[]);
        assert!(svg.matches("<circle").count() <= 10);
        assert!(svg.contains("stroke-dasharray"));
    }
}
