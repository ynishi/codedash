//! Coverage enrichment from llvm-cov JSON export.
//!
//! Reads a `cargo llvm-cov --json` output file and maps function-level
//! region coverage to `NodeData.coverage` using file path + line-range overlap.

use std::collections::HashMap;
use std::path::Path;

use crate::domain::ast::AstData;
use crate::domain::enrichment::EnrichConfig;
use crate::port::enricher::{EnrichContext, Enricher};
use crate::Error;

pub struct CoverageEnricher;

impl CoverageEnricher {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CoverageEnricher {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-function coverage entry after parsing.
#[derive(Debug)]
struct FunctionCoverage {
    /// Normalized file name (e.g. "src/infra/git").
    file_name: String,
    start_line: usize,
    end_line: usize,
    /// Region coverage ratio 0.0..1.0.
    coverage: f64,
}

/// Parse llvm-cov JSON and extract function-level coverage.
fn parse_llvm_cov(
    json_str: &str,
    repo_path: &Path,
    strip_prefix: &str,
    extensions: &[&str],
) -> Result<Vec<FunctionCoverage>, Error> {
    let root: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| Error::Enrich(format!("coverage JSON parse error: {e}")))?;

    let data = root
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| Error::Enrich("coverage JSON: missing 'data' array".into()))?;

    let mut entries = Vec::new();

    for export_obj in data {
        let functions = match export_obj.get("functions").and_then(|f| f.as_array()) {
            Some(f) => f,
            None => continue,
        };

        for func in functions {
            let filenames = match func.get("filenames").and_then(|f| f.as_array()) {
                Some(f) => f,
                None => continue,
            };
            let regions = match func.get("regions").and_then(|r| r.as_array()) {
                Some(r) if !r.is_empty() => r,
                _ => continue,
            };

            // Get primary file path
            let abs_path = match filenames.first().and_then(|f| f.as_str()) {
                Some(p) => p,
                None => continue,
            };

            // Normalize: absolute path → relative to repo → strip prefix → strip extension
            let file_name = normalize_coverage_path(abs_path, repo_path, strip_prefix, extensions);

            // Compute line range from regions
            // Region format: [line_start, col_start, line_end, col_end, exec_count, file_id, expanded_file_id, kind]
            let mut min_line = usize::MAX;
            let mut max_line = 0usize;
            let mut total_regions = 0u64;
            let mut covered_regions = 0u64;

            for region in regions {
                let arr = match region.as_array() {
                    Some(a) if a.len() >= 5 => a,
                    _ => continue,
                };

                let line_start = arr[0].as_u64().unwrap_or(0) as usize;
                let line_end = arr[2].as_u64().unwrap_or(0) as usize;
                let exec_count = arr[4].as_u64().unwrap_or(0);
                // kind: 0 = code region (skip expansion/skipped regions)
                let kind = arr.get(7).and_then(|k| k.as_u64()).unwrap_or(0);
                if kind != 0 {
                    continue;
                }

                if line_start < min_line {
                    min_line = line_start;
                }
                if line_end > max_line {
                    max_line = line_end;
                }

                total_regions += 1;
                if exec_count > 0 {
                    covered_regions += 1;
                }
            }

            if total_regions == 0 || min_line == usize::MAX {
                continue;
            }

            let coverage = covered_regions as f64 / total_regions as f64;

            entries.push(FunctionCoverage {
                file_name,
                start_line: min_line,
                end_line: max_line,
                coverage,
            });
        }
    }

    Ok(entries)
}

/// Normalize an absolute file path from llvm-cov to match codedash's file naming.
fn normalize_coverage_path(
    abs_path: &str,
    repo_path: &Path,
    strip_prefix: &str,
    extensions: &[&str],
) -> String {
    let path = Path::new(abs_path);

    // Strip repo root to get relative path
    let rel = path
        .strip_prefix(repo_path)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let mut clean = rel;

    // Strip file extension
    for ext in extensions {
        let dot_ext = format!(".{ext}");
        if let Some(stripped) = clean.strip_suffix(&dot_ext) {
            clean = stripped.to_string();
            break;
        }
    }

    // Strip configured prefix
    if !strip_prefix.is_empty() {
        if let Some(rest) = clean.strip_prefix(strip_prefix) {
            clean = rest.to_string();
        }
    }

    clean
}

/// Apply coverage data to AST nodes by file + line-range overlap.
fn apply_coverage(data: &mut AstData, cov_entries: &[FunctionCoverage]) {
    // Build lookup: file_name → vec of (start, end, coverage)
    let mut file_cov: HashMap<&str, Vec<(usize, usize, f64)>> = HashMap::new();
    for entry in cov_entries {
        file_cov.entry(entry.file_name.as_str()).or_default().push((
            entry.start_line,
            entry.end_line,
            entry.coverage,
        ));
    }

    for file in &mut data.files {
        let ranges = match file_cov.get(file.name.as_str()) {
            Some(r) => r,
            None => continue,
        };

        for node in &mut file.nodes {
            // Find the best matching coverage entry by line overlap
            let best = ranges
                .iter()
                .filter(|(start, end, _)| {
                    // Overlap check: node's line range intersects coverage entry's line range
                    node.start_line <= *end && node.end_line >= *start
                })
                .min_by_key(|(start, _, _)| {
                    // Prefer the entry whose start_line is closest to node's start_line
                    (*start as isize - node.start_line as isize).unsigned_abs()
                });

            if let Some((_, _, cov)) = best {
                node.coverage = Some(*cov);
            }
        }
    }
}

impl Enricher for CoverageEnricher {
    fn enrich(
        &self,
        data: &mut AstData,
        config: &EnrichConfig,
        ctx: &EnrichContext<'_>,
    ) -> Result<(), Error> {
        let coverage_file = match &config.coverage_file {
            Some(path) => path,
            None => return Ok(()), // No coverage file → skip silently
        };

        let json_str = std::fs::read_to_string(coverage_file).map_err(|e| {
            Error::Enrich(format!(
                "failed to read coverage file '{coverage_file}': {e}"
            ))
        })?;

        let entries = parse_llvm_cov(&json_str, ctx.repo_path, ctx.strip_prefix, ctx.extensions)?;

        apply_coverage(data, &entries);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ast::{AstData, FileData, NodeData};

    fn make_node(name: &str, start: usize, end: usize) -> NodeData {
        NodeData {
            kind: "function".to_string(),
            name: name.to_string(),
            exported: true,
            visibility: Some("pub".to_string()),
            is_async: None,
            is_unsafe: None,
            start_line: start,
            end_line: end,
            lines: end - start + 1,
            params: None,
            field_count: None,
            depth: None,
            cyclomatic: None,
            trait_name: None,
            git_churn_30d: None,
            coverage: None,
            co_changes: None,
            calls: None,
        }
    }

    fn make_ast(files: Vec<(&str, Vec<NodeData>)>) -> AstData {
        AstData {
            files: files
                .into_iter()
                .map(|(name, nodes)| FileData {
                    path: format!("{name}.rs"),
                    name: name.to_string(),
                    nodes,
                    imports: Vec::new(),
                    git_churn_30d: None,
                })
                .collect(),
            edges: Vec::new(),
        }
    }

    #[test]
    fn normalize_strips_repo_prefix_and_extension() {
        let repo = Path::new("/home/user/project");
        let result = normalize_coverage_path("/home/user/project/src/main.rs", repo, "", &["rs"]);
        assert_eq!(result, "src/main");
    }

    #[test]
    fn normalize_strips_configured_prefix() {
        let repo = Path::new("/home/user/project");
        let result =
            normalize_coverage_path("/home/user/project/src/lib.rs", repo, "src/", &["rs"]);
        assert_eq!(result, "lib");
    }

    #[test]
    fn apply_coverage_matches_by_line_overlap() {
        let mut data = make_ast(vec![(
            "src/main",
            vec![make_node("foo", 10, 20), make_node("bar", 25, 40)],
        )]);

        let entries = vec![
            FunctionCoverage {
                file_name: "src/main".to_string(),
                start_line: 10,
                end_line: 20,
                coverage: 0.8,
            },
            FunctionCoverage {
                file_name: "src/main".to_string(),
                start_line: 25,
                end_line: 40,
                coverage: 0.5,
            },
        ];

        apply_coverage(&mut data, &entries);

        assert_eq!(data.files[0].nodes[0].coverage, Some(0.8));
        assert_eq!(data.files[0].nodes[1].coverage, Some(0.5));
    }

    #[test]
    fn apply_coverage_skips_unmatched_files() {
        let mut data = make_ast(vec![("src/other", vec![make_node("baz", 1, 10)])]);

        let entries = vec![FunctionCoverage {
            file_name: "src/main".to_string(),
            start_line: 1,
            end_line: 10,
            coverage: 1.0,
        }];

        apply_coverage(&mut data, &entries);

        assert_eq!(data.files[0].nodes[0].coverage, None);
    }

    #[test]
    fn apply_coverage_no_entries_leaves_none() {
        let mut data = make_ast(vec![("src/main", vec![make_node("foo", 1, 5)])]);

        apply_coverage(&mut data, &[]);

        assert_eq!(data.files[0].nodes[0].coverage, None);
    }

    #[test]
    fn parse_llvm_cov_extracts_functions() {
        let json = r#"{
            "version": "3.1.0",
            "type": "llvm.coverage.json.export",
            "data": [{
                "functions": [{
                    "name": "mymod::my_func",
                    "count": 3,
                    "filenames": ["/repo/src/main.rs"],
                    "regions": [
                        [10, 1, 15, 2, 3, 0, 0, 0],
                        [12, 5, 13, 6, 0, 0, 0, 0]
                    ]
                }],
                "totals": {}
            }]
        }"#;

        let entries = parse_llvm_cov(json, Path::new("/repo"), "", &["rs"]).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].file_name, "src/main");
        assert_eq!(entries[0].start_line, 10);
        assert_eq!(entries[0].end_line, 15);
        assert!((entries[0].coverage - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_llvm_cov_skips_non_code_regions() {
        let json = r#"{
            "version": "3.1.0",
            "type": "llvm.coverage.json.export",
            "data": [{
                "functions": [{
                    "name": "func",
                    "count": 1,
                    "filenames": ["/repo/src/lib.rs"],
                    "regions": [
                        [1, 1, 5, 2, 1, 0, 0, 0],
                        [2, 1, 3, 2, 0, 0, 0, 1]
                    ]
                }],
                "totals": {}
            }]
        }"#;

        let entries = parse_llvm_cov(json, Path::new("/repo"), "", &["rs"]).unwrap();

        assert_eq!(entries.len(), 1);
        // Only 1 code region (kind=0), and it was executed
        assert!((entries[0].coverage - 1.0).abs() < f64::EPSILON);
    }
}
