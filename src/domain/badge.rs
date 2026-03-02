//! Badge data types for shields.io endpoint JSON generation.

use serde::Serialize;

/// shields.io endpoint badge JSON structure.
///
/// See: <https://shields.io/badges/endpoint-badge>
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BadgeOutput {
    pub schema_version: u8,
    pub label: String,
    pub message: String,
    pub color: BadgeColor,
}

impl BadgeOutput {
    pub fn new(label: impl Into<String>, message: impl Into<String>, color: BadgeColor) -> Self {
        Self {
            schema_version: 1,
            label: label.into(),
            message: message.into(),
            color,
        }
    }

    /// The filename for this badge in the given format.
    pub fn filename_for(&self, format: BadgeFormat) -> String {
        let stem = self.label.replace(' ', "-");
        format!("{stem}.{}", format.file_extension())
    }

    /// Render as a shields.io-compatible flat-style SVG badge.
    pub fn render_svg(&self) -> String {
        render_flat_svg(&self.label, &self.message, self.color.hex())
    }
}

/// Badge output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BadgeFormat {
    /// shields.io endpoint JSON (default).
    #[default]
    ShieldsEndpoint,
    /// Self-contained SVG (no external dependency).
    Svg,
}

impl BadgeFormat {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "shields-endpoint" | "json" => Some(Self::ShieldsEndpoint),
            "svg" => Some(Self::Svg),
            _ => None,
        }
    }

    pub fn file_extension(&self) -> &'static str {
        match self {
            Self::ShieldsEndpoint => "json",
            Self::Svg => "svg",
        }
    }
}

/// Badge color values supported by shields.io.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BadgeColor {
    BrightGreen,
    Green,
    Yellow,
    #[serde(rename = "orange")]
    Orange,
    Red,
    Blue,
}

impl BadgeColor {
    /// Hex color code for SVG rendering.
    pub fn hex(&self) -> &'static str {
        match self {
            Self::BrightGreen => "#4c1",
            Self::Green => "#97ca00",
            Self::Yellow => "#dfb317",
            Self::Orange => "#fe7d37",
            Self::Red => "#e05d44",
            Self::Blue => "#007ec6",
        }
    }
}

/// Threshold configuration for percent-based metrics (coverage, fn-coverage).
#[derive(Debug, Clone, Copy)]
pub struct PercentThreshold {
    /// Value >= this → green.
    pub green: f64,
    /// Value >= this (but < green) → yellow.
    pub yellow: f64,
}

impl Default for PercentThreshold {
    fn default() -> Self {
        Self {
            green: 80.0,
            yellow: 60.0,
        }
    }
}

impl PercentThreshold {
    pub fn color_for(&self, value: f64) -> BadgeColor {
        if value >= self.green {
            BadgeColor::BrightGreen
        } else if value >= self.yellow {
            BadgeColor::Yellow
        } else {
            BadgeColor::Red
        }
    }
}

/// Threshold configuration for complexity metric (lower is better).
#[derive(Debug, Clone, Copy)]
pub struct ComplexityThreshold {
    /// Value <= this → green.
    pub green: f64,
    /// Value <= this (but > green) → yellow.
    pub yellow: f64,
}

impl Default for ComplexityThreshold {
    fn default() -> Self {
        Self {
            green: 5.0,
            yellow: 10.0,
        }
    }
}

impl ComplexityThreshold {
    pub fn color_for(&self, value: f64) -> BadgeColor {
        if value <= self.green {
            BadgeColor::BrightGreen
        } else if value <= self.yellow {
            BadgeColor::Yellow
        } else {
            BadgeColor::Red
        }
    }
}

/// Aggregated badge threshold configuration.
#[derive(Debug, Clone, Default)]
pub struct BadgeThresholds {
    pub coverage: PercentThreshold,
    pub fn_coverage: PercentThreshold,
    pub complexity: ComplexityThreshold,
}

/// Which badge metrics to generate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BadgeMetric {
    Coverage,
    FnCoverage,
    Complexity,
    Modules,
}

impl BadgeMetric {
    /// Parse a comma-separated filter string into a list of metrics.
    pub fn parse_filter(filter: &str) -> Vec<Self> {
        filter
            .split(',')
            .filter_map(|s| match s.trim() {
                "coverage" => Some(Self::Coverage),
                "fn-coverage" => Some(Self::FnCoverage),
                "complexity" => Some(Self::Complexity),
                "modules" => Some(Self::Modules),
                _ => None,
            })
            .collect()
    }

    /// All available metrics.
    pub fn all() -> Vec<Self> {
        vec![
            Self::Coverage,
            Self::FnCoverage,
            Self::Complexity,
            Self::Modules,
        ]
    }
}

// ── SVG rendering (shields.io flat style) ──

/// Approximate text width for Verdana 11px in decipoints (1/10 px).
///
/// Uses per-character widths derived from the anafanafo Verdana 11px table
/// (same data source as shields.io). Unknown characters default to the width of 'm'.
fn text_width_dp(text: &str) -> u32 {
    text.chars().map(char_width_dp).sum()
}

/// Per-character width in decipoints for Verdana 11px.
fn char_width_dp(c: char) -> u32 {
    match c {
        ' ' => 34,
        '!' => 34,
        '"' => 43,
        '#' => 75,
        '%' => 96,
        '(' | ')' => 39,
        '+' => 75,
        ',' => 36,
        '-' => 47,
        '.' => 36,
        '/' => 43,
        '0' => 72,
        '1' => 55,
        '2' => 67,
        '3' => 67,
        '4' => 72,
        '5' => 67,
        '6' => 70,
        '7' => 62,
        '8' => 70,
        '9' => 70,
        ':' => 36,
        'A' => 73,
        'B' => 70,
        'C' => 72,
        'D' => 77,
        'E' => 63,
        'F' => 58,
        'G' => 78,
        'H' => 77,
        'I' => 34,
        'J' => 47,
        'K' => 71,
        'L' => 59,
        'M' => 89,
        'N' => 77,
        'O' => 81,
        'P' => 65,
        'Q' => 81,
        'R' => 70,
        'S' => 68,
        'T' => 65,
        'U' => 76,
        'V' => 73,
        'W' => 101,
        'X' => 69,
        'Y' => 65,
        'Z' => 69,
        'a' => 63,
        'b' => 68,
        'c' => 57,
        'd' => 68,
        'e' => 63,
        'f' => 38,
        'g' => 68,
        'h' => 67,
        'i' => 28,
        'j' => 32,
        'k' => 63,
        'l' => 28,
        'm' => 100,
        'n' => 67,
        'o' => 66,
        'p' => 68,
        'q' => 68,
        'r' => 42,
        's' => 54,
        't' => 40,
        'u' => 67,
        'v' => 59,
        'w' => 85,
        'x' => 59,
        'y' => 59,
        'z' => 55,
        _ => 100, // default to 'm' width
    }
}

/// Horizontal padding on each side of label/message text (in pixels).
const HORIZ_PAD: u32 = 5;

/// Render a shields.io flat-style SVG badge.
fn render_flat_svg(label: &str, message: &str, color_hex: &str) -> String {
    let label_width_dp = text_width_dp(label);
    let message_width_dp = text_width_dp(message);

    // Rect widths in pixels (decipoints / 10, rounded up, plus padding)
    let label_rect_w = label_width_dp.div_ceil(10) + HORIZ_PAD * 2;
    let msg_rect_w = message_width_dp.div_ceil(10) + HORIZ_PAD * 2;
    let total_w = label_rect_w + msg_rect_w;

    // Text x-center positions (in decipoints for SVG transform="scale(.1)")
    let label_x = label_rect_w * 10 / 2;
    let msg_x = label_rect_w * 10 + msg_rect_w * 10 / 2;

    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{total_w}" height="20" role="img" aria-label="{label}: {message}">
  <title>{label}: {message}</title>
  <linearGradient id="s" x2="0" y2="100%">
    <stop offset="0" stop-color="#bbb" stop-opacity=".1"/>
    <stop offset="1" stop-opacity=".1"/>
  </linearGradient>
  <clipPath id="r">
    <rect width="{total_w}" height="20" rx="3" fill="#fff"/>
  </clipPath>
  <g clip-path="url(#r)">
    <rect width="{label_rect_w}" height="20" fill="#555"/>
    <rect x="{label_rect_w}" width="{msg_rect_w}" height="20" fill="{color_hex}"/>
    <rect width="{total_w}" height="20" fill="url(#s)"/>
  </g>
  <g fill="#fff" text-anchor="middle" font-family="Verdana,Geneva,DejaVu Sans,sans-serif" text-rendering="geometricPrecision" font-size="110">
    <text aria-hidden="true" x="{label_x}" y="150" fill="#010101" fill-opacity=".3" transform="scale(.1)" textLength="{label_width_dp}">{label}</text>
    <text x="{label_x}" y="140" transform="scale(.1)" fill="#fff" textLength="{label_width_dp}">{label}</text>
    <text aria-hidden="true" x="{msg_x}" y="150" fill="#010101" fill-opacity=".3" transform="scale(.1)" textLength="{message_width_dp}">{message}</text>
    <text x="{msg_x}" y="140" transform="scale(.1)" fill="#fff" textLength="{message_width_dp}">{message}</text>
  </g>
</svg>"##
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── PercentThreshold ──

    #[test]
    fn percent_threshold_green() {
        let t = PercentThreshold::default();
        assert_eq!(t.color_for(80.0), BadgeColor::BrightGreen);
        assert_eq!(t.color_for(95.0), BadgeColor::BrightGreen);
        assert_eq!(t.color_for(100.0), BadgeColor::BrightGreen);
    }

    #[test]
    fn percent_threshold_yellow() {
        let t = PercentThreshold::default();
        assert_eq!(t.color_for(60.0), BadgeColor::Yellow);
        assert_eq!(t.color_for(79.9), BadgeColor::Yellow);
    }

    #[test]
    fn percent_threshold_red() {
        let t = PercentThreshold::default();
        assert_eq!(t.color_for(59.9), BadgeColor::Red);
        assert_eq!(t.color_for(0.0), BadgeColor::Red);
    }

    #[test]
    fn percent_threshold_custom() {
        let t = PercentThreshold {
            green: 90.0,
            yellow: 70.0,
        };
        assert_eq!(t.color_for(90.0), BadgeColor::BrightGreen);
        assert_eq!(t.color_for(70.0), BadgeColor::Yellow);
        assert_eq!(t.color_for(69.9), BadgeColor::Red);
    }

    // ── ComplexityThreshold ──

    #[test]
    fn complexity_threshold_green() {
        let t = ComplexityThreshold::default();
        assert_eq!(t.color_for(1.0), BadgeColor::BrightGreen);
        assert_eq!(t.color_for(5.0), BadgeColor::BrightGreen);
    }

    #[test]
    fn complexity_threshold_yellow() {
        let t = ComplexityThreshold::default();
        assert_eq!(t.color_for(5.1), BadgeColor::Yellow);
        assert_eq!(t.color_for(10.0), BadgeColor::Yellow);
    }

    #[test]
    fn complexity_threshold_red() {
        let t = ComplexityThreshold::default();
        assert_eq!(t.color_for(10.1), BadgeColor::Red);
        assert_eq!(t.color_for(50.0), BadgeColor::Red);
    }

    // ── BadgeOutput ──

    #[test]
    fn badge_output_serializes_correctly() {
        let badge = BadgeOutput::new("coverage", "86.3%", BadgeColor::BrightGreen);
        let json = serde_json::to_value(&badge).unwrap();
        assert_eq!(json["schemaVersion"], 1);
        assert_eq!(json["label"], "coverage");
        assert_eq!(json["message"], "86.3%");
        assert_eq!(json["color"], "brightgreen");
    }

    #[test]
    fn badge_output_filename_json() {
        let badge = BadgeOutput::new("fn-coverage", "90%", BadgeColor::Green);
        assert_eq!(
            badge.filename_for(BadgeFormat::ShieldsEndpoint),
            "fn-coverage.json"
        );
    }

    #[test]
    fn badge_output_filename_svg() {
        let badge = BadgeOutput::new("coverage", "86%", BadgeColor::BrightGreen);
        assert_eq!(badge.filename_for(BadgeFormat::Svg), "coverage.svg");
    }

    // ── BadgeColor hex ──

    #[test]
    fn badge_color_hex_values() {
        assert_eq!(BadgeColor::BrightGreen.hex(), "#4c1");
        assert_eq!(BadgeColor::Green.hex(), "#97ca00");
        assert_eq!(BadgeColor::Yellow.hex(), "#dfb317");
        assert_eq!(BadgeColor::Orange.hex(), "#fe7d37");
        assert_eq!(BadgeColor::Red.hex(), "#e05d44");
        assert_eq!(BadgeColor::Blue.hex(), "#007ec6");
    }

    #[test]
    fn badge_color_serializes() {
        assert_eq!(
            serde_json::to_string(&BadgeColor::BrightGreen).unwrap(),
            "\"brightgreen\""
        );
        assert_eq!(
            serde_json::to_string(&BadgeColor::Yellow).unwrap(),
            "\"yellow\""
        );
        assert_eq!(serde_json::to_string(&BadgeColor::Red).unwrap(), "\"red\"");
        assert_eq!(
            serde_json::to_string(&BadgeColor::Blue).unwrap(),
            "\"blue\""
        );
        assert_eq!(
            serde_json::to_string(&BadgeColor::Orange).unwrap(),
            "\"orange\""
        );
    }

    // ── BadgeMetric ──

    #[test]
    fn parse_filter_valid() {
        let metrics = BadgeMetric::parse_filter("coverage,complexity");
        assert_eq!(
            metrics,
            vec![BadgeMetric::Coverage, BadgeMetric::Complexity]
        );
    }

    #[test]
    fn parse_filter_with_spaces() {
        let metrics = BadgeMetric::parse_filter("coverage , fn-coverage");
        assert_eq!(
            metrics,
            vec![BadgeMetric::Coverage, BadgeMetric::FnCoverage]
        );
    }

    #[test]
    fn parse_filter_ignores_unknown() {
        let metrics = BadgeMetric::parse_filter("coverage,unknown,modules");
        assert_eq!(metrics, vec![BadgeMetric::Coverage, BadgeMetric::Modules]);
    }

    #[test]
    fn parse_filter_all() {
        let metrics = BadgeMetric::parse_filter("coverage,fn-coverage,complexity,modules");
        assert_eq!(metrics, BadgeMetric::all());
    }

    // ── BadgeFormat ──

    #[test]
    fn badge_format_parse() {
        assert_eq!(BadgeFormat::parse("svg"), Some(BadgeFormat::Svg));
        assert_eq!(
            BadgeFormat::parse("shields-endpoint"),
            Some(BadgeFormat::ShieldsEndpoint)
        );
        assert_eq!(
            BadgeFormat::parse("json"),
            Some(BadgeFormat::ShieldsEndpoint)
        );
        assert_eq!(BadgeFormat::parse("unknown"), None);
    }

    // ── SVG rendering ──

    #[test]
    fn text_width_dp_known_chars() {
        // "86.3%" = '8'(70) + '6'(70) + '.'(36) + '3'(67) + '%'(96) = 339
        assert_eq!(text_width_dp("86.3%"), 339);
    }

    #[test]
    fn text_width_dp_unknown_defaults_to_m() {
        // unknown char → 100 (width of 'm')
        assert_eq!(char_width_dp('\u{1F600}'), 100);
    }

    #[test]
    fn render_svg_is_valid_xml() {
        let svg = render_flat_svg("coverage", "86.3%", "#4c1");
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("coverage"));
        assert!(svg.contains("86.3%"));
        assert!(svg.contains("#4c1"));
        assert!(svg.contains(r#"aria-label="coverage: 86.3%""#));
    }

    #[test]
    fn render_svg_label_rect_width() {
        let svg = render_flat_svg("test", "ok", "#4c1");
        // "test" = t(40)+e(63)+s(54)+t(40) = 197dp → 20px + 10px pad = 30
        // "ok" = o(66)+k(63) = 129dp → 13px + 10px pad = 23
        assert!(svg.contains(r#"width="30""#)); // label rect
        assert!(svg.contains(r#"width="23""#)); // message rect
        assert!(svg.contains(r#"width="53""#)); // total
    }

    #[test]
    fn badge_output_render_svg() {
        let badge = BadgeOutput::new("modules", "24", BadgeColor::Blue);
        let svg = badge.render_svg();
        assert!(svg.contains("#007ec6"));
        assert!(svg.contains("modules"));
        assert!(svg.contains("24"));
    }
}
