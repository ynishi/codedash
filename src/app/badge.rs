//! Badge generation use case — compute metrics from AstData and write shields.io endpoint JSON.

use std::path::Path;

use crate::domain::ast::AstData;
use crate::domain::badge::{BadgeColor, BadgeFormat, BadgeMetric, BadgeOutput, BadgeThresholds};
use crate::Error;

/// Generates shields.io endpoint badge data from analyzed AST data.
pub struct BadgeGenerator {
    thresholds: BadgeThresholds,
}

impl BadgeGenerator {
    pub fn new(thresholds: BadgeThresholds) -> Self {
        Self { thresholds }
    }

    /// Generate badges for the requested metrics.
    ///
    /// Returns only badges that have meaningful data. For example, coverage badges
    /// are omitted when no coverage data is present in the AST.
    pub fn generate(&self, data: &AstData, metrics: &[BadgeMetric]) -> Vec<BadgeOutput> {
        let mut badges = Vec::new();

        for metric in metrics {
            match metric {
                BadgeMetric::Coverage => {
                    if let Some(badge) = self.coverage_badge(data) {
                        badges.push(badge);
                    }
                }
                BadgeMetric::FnCoverage => {
                    if let Some(badge) = self.fn_coverage_badge(data) {
                        badges.push(badge);
                    }
                }
                BadgeMetric::Complexity => {
                    if let Some(badge) = self.complexity_badge(data) {
                        badges.push(badge);
                    }
                }
                BadgeMetric::Modules => {
                    badges.push(self.modules_badge(data));
                }
            }
        }

        badges
    }

    fn coverage_badge(&self, data: &AstData) -> Option<BadgeOutput> {
        let pct = compute_coverage(data)?;
        let color = self.thresholds.coverage.color_for(pct);
        Some(BadgeOutput::new("coverage", format!("{pct:.1}%"), color))
    }

    fn fn_coverage_badge(&self, data: &AstData) -> Option<BadgeOutput> {
        let pct = compute_fn_coverage(data)?;
        let color = self.thresholds.fn_coverage.color_for(pct);
        Some(BadgeOutput::new("fn-coverage", format!("{pct:.1}%"), color))
    }

    fn complexity_badge(&self, data: &AstData) -> Option<BadgeOutput> {
        let (avg, _max) = compute_complexity(data)?;
        let color = self.thresholds.complexity.color_for(avg);
        Some(BadgeOutput::new("complexity", format!("{avg:.1}"), color))
    }

    fn modules_badge(&self, data: &AstData) -> BadgeOutput {
        let count = data.files.len();
        BadgeOutput::new("modules", count.to_string(), BadgeColor::Blue)
    }
}

/// Average line coverage (0–100%) across all nodes that have coverage data.
fn compute_coverage(data: &AstData) -> Option<f64> {
    let mut sum = 0.0;
    let mut count = 0u64;
    for file in &data.files {
        for node in &file.nodes {
            if let Some(cov) = node.coverage {
                sum += cov;
                count += 1;
            }
        }
    }
    if count == 0 {
        return None;
    }
    Some((sum / count as f64) * 100.0)
}

/// Fraction of function nodes that have coverage > 0 (0–100%).
fn compute_fn_coverage(data: &AstData) -> Option<f64> {
    let mut total_fns = 0u64;
    let mut covered_fns = 0u64;
    for file in &data.files {
        for node in &file.nodes {
            if node.kind == "function" {
                if let Some(cov) = node.coverage {
                    total_fns += 1;
                    if cov > 0.0 {
                        covered_fns += 1;
                    }
                }
            }
        }
    }
    if total_fns == 0 {
        return None;
    }
    Some((covered_fns as f64 / total_fns as f64) * 100.0)
}

/// Average and max cyclomatic complexity across all nodes.
/// Returns None if no nodes have cyclomatic data.
fn compute_complexity(data: &AstData) -> Option<(f64, usize)> {
    let mut sum = 0usize;
    let mut max = 0usize;
    let mut count = 0u64;
    for file in &data.files {
        for node in &file.nodes {
            if let Some(c) = node.cyclomatic {
                sum += c;
                if c > max {
                    max = c;
                }
                count += 1;
            }
        }
    }
    if count == 0 {
        return None;
    }
    Some((sum as f64 / count as f64, max))
}

/// Write badge files to the specified output directory in the given format.
pub fn write_badges(
    badges: &[BadgeOutput],
    out_dir: &Path,
    format: BadgeFormat,
) -> Result<Vec<String>, Error> {
    std::fs::create_dir_all(out_dir)?;

    let mut written = Vec::new();
    for badge in badges {
        let path = out_dir.join(badge.filename_for(format));
        let content = match format {
            BadgeFormat::ShieldsEndpoint => serde_json::to_string_pretty(badge)?,
            BadgeFormat::Svg => badge.render_svg(),
        };
        std::fs::write(&path, content)?;
        written.push(path.to_string_lossy().to_string());
    }

    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ast::{AstData, FileData, NodeData};

    fn make_node(
        kind: &str,
        name: &str,
        cyclomatic: Option<usize>,
        coverage: Option<f64>,
    ) -> NodeData {
        NodeData {
            kind: kind.to_string(),
            name: name.to_string(),
            exported: true,
            visibility: None,
            is_async: None,
            is_unsafe: None,
            start_line: 1,
            end_line: 10,
            lines: 10,
            params: None,
            field_count: None,
            depth: None,
            cyclomatic,
            trait_name: None,
            git_churn_30d: None,
            coverage,
            co_changes: None,
            calls: None,
        }
    }

    fn make_ast(nodes: Vec<NodeData>) -> AstData {
        AstData {
            files: vec![FileData {
                path: "src/lib.rs".to_string(),
                name: "lib".to_string(),
                nodes,
                imports: Vec::new(),
                git_churn_30d: None,
            }],
            edges: Vec::new(),
        }
    }

    // ── compute_coverage ──

    #[test]
    fn coverage_average() {
        let data = make_ast(vec![
            make_node("function", "a", None, Some(0.8)),
            make_node("function", "b", None, Some(0.6)),
        ]);
        let pct = compute_coverage(&data).unwrap();
        assert!((pct - 70.0).abs() < 0.01);
    }

    #[test]
    fn coverage_none_when_no_data() {
        let data = make_ast(vec![make_node("function", "a", None, None)]);
        assert!(compute_coverage(&data).is_none());
    }

    #[test]
    fn coverage_ignores_nodes_without_data() {
        let data = make_ast(vec![
            make_node("function", "a", None, Some(1.0)),
            make_node("function", "b", None, None),
        ]);
        let pct = compute_coverage(&data).unwrap();
        assert!((pct - 100.0).abs() < 0.01);
    }

    // ── compute_fn_coverage ──

    #[test]
    fn fn_coverage_all_covered() {
        let data = make_ast(vec![
            make_node("function", "a", None, Some(0.8)),
            make_node("function", "b", None, Some(0.5)),
        ]);
        let pct = compute_fn_coverage(&data).unwrap();
        assert!((pct - 100.0).abs() < 0.01);
    }

    #[test]
    fn fn_coverage_partial() {
        let data = make_ast(vec![
            make_node("function", "a", None, Some(0.8)),
            make_node("function", "b", None, Some(0.0)),
        ]);
        let pct = compute_fn_coverage(&data).unwrap();
        assert!((pct - 50.0).abs() < 0.01);
    }

    #[test]
    fn fn_coverage_skips_non_functions() {
        let data = make_ast(vec![
            make_node("function", "a", None, Some(1.0)),
            make_node("struct", "B", None, Some(0.0)),
        ]);
        let pct = compute_fn_coverage(&data).unwrap();
        assert!((pct - 100.0).abs() < 0.01);
    }

    #[test]
    fn fn_coverage_none_when_no_functions_with_coverage() {
        let data = make_ast(vec![make_node("function", "a", None, None)]);
        assert!(compute_fn_coverage(&data).is_none());
    }

    // ── compute_complexity ──

    #[test]
    fn complexity_average_and_max() {
        let data = make_ast(vec![
            make_node("function", "a", Some(3), None),
            make_node("function", "b", Some(7), None),
        ]);
        let (avg, max) = compute_complexity(&data).unwrap();
        assert!((avg - 5.0).abs() < 0.01);
        assert_eq!(max, 7);
    }

    #[test]
    fn complexity_none_when_no_data() {
        let data = make_ast(vec![make_node("function", "a", None, None)]);
        assert!(compute_complexity(&data).is_none());
    }

    // ── BadgeGenerator ──

    #[test]
    fn generate_all_badges_with_data() {
        let data = make_ast(vec![
            make_node("function", "a", Some(3), Some(0.9)),
            make_node("function", "b", Some(7), Some(0.5)),
        ]);
        let gen = BadgeGenerator::new(BadgeThresholds::default());
        let badges = gen.generate(&data, &BadgeMetric::all());

        assert_eq!(badges.len(), 4);
        assert_eq!(badges[0].label, "coverage");
        assert_eq!(badges[1].label, "fn-coverage");
        assert_eq!(badges[2].label, "complexity");
        assert_eq!(badges[3].label, "modules");
    }

    #[test]
    fn generate_skips_coverage_without_data() {
        let data = make_ast(vec![make_node("function", "a", Some(3), None)]);
        let gen = BadgeGenerator::new(BadgeThresholds::default());
        let badges = gen.generate(&data, &BadgeMetric::all());

        // coverage and fn-coverage skipped, complexity + modules present
        assert_eq!(badges.len(), 2);
        assert_eq!(badges[0].label, "complexity");
        assert_eq!(badges[1].label, "modules");
    }

    #[test]
    fn generate_only_filtered_metrics() {
        let data = make_ast(vec![make_node("function", "a", Some(3), Some(0.9))]);
        let gen = BadgeGenerator::new(BadgeThresholds::default());
        let badges = gen.generate(&data, &[BadgeMetric::Modules]);

        assert_eq!(badges.len(), 1);
        assert_eq!(badges[0].label, "modules");
    }

    #[test]
    fn modules_badge_always_blue() {
        let data = make_ast(vec![]);
        let gen = BadgeGenerator::new(BadgeThresholds::default());
        let badges = gen.generate(&data, &[BadgeMetric::Modules]);

        assert_eq!(badges.len(), 1);
        assert_eq!(badges[0].color, BadgeColor::Blue);
        assert_eq!(badges[0].message, "1"); // 1 file in make_ast
    }

    #[test]
    fn coverage_badge_color_green() {
        let data = make_ast(vec![make_node("function", "a", None, Some(0.85))]);
        let gen = BadgeGenerator::new(BadgeThresholds::default());
        let badges = gen.generate(&data, &[BadgeMetric::Coverage]);

        assert_eq!(badges[0].color, BadgeColor::BrightGreen);
        assert_eq!(badges[0].message, "85.0%");
    }

    #[test]
    fn coverage_badge_color_red() {
        let data = make_ast(vec![make_node("function", "a", None, Some(0.3))]);
        let gen = BadgeGenerator::new(BadgeThresholds::default());
        let badges = gen.generate(&data, &[BadgeMetric::Coverage]);

        assert_eq!(badges[0].color, BadgeColor::Red);
    }

    // ── write_badges ──

    #[test]
    fn write_badges_creates_json_files() {
        let dir = std::env::temp_dir().join("codedash_badge_test_json");
        let _ = std::fs::remove_dir_all(&dir);

        let badges = vec![
            BadgeOutput::new("coverage", "86.3%", BadgeColor::BrightGreen),
            BadgeOutput::new("modules", "42", BadgeColor::Blue),
        ];

        let written = write_badges(&badges, &dir, BadgeFormat::ShieldsEndpoint).unwrap();
        assert_eq!(written.len(), 2);

        let content = std::fs::read_to_string(dir.join("coverage.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["schemaVersion"], 1);
        assert_eq!(parsed["label"], "coverage");
        assert_eq!(parsed["message"], "86.3%");
        assert_eq!(parsed["color"], "brightgreen");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_badges_creates_svg_files() {
        let dir = std::env::temp_dir().join("codedash_badge_test_svg");
        let _ = std::fs::remove_dir_all(&dir);

        let badges = vec![BadgeOutput::new(
            "coverage",
            "86.3%",
            BadgeColor::BrightGreen,
        )];

        let written = write_badges(&badges, &dir, BadgeFormat::Svg).unwrap();
        assert_eq!(written.len(), 1);
        assert!(written[0].ends_with("coverage.svg"));

        let content = std::fs::read_to_string(dir.join("coverage.svg")).unwrap();
        assert!(content.starts_with("<svg"));
        assert!(content.contains("86.3%"));
        assert!(content.contains("#4c1"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
