//! Git enrichment using git2.
//!
//! Reimplements enrich_generic.js:
//! - File-level churn (commit count per file within N days)
//! - Co-change pairs (files changed together in the same commit)

use std::collections::{HashMap, HashSet};

use git2::{Repository, Sort};

use crate::domain::ast::AstData;
use crate::domain::enrichment::{ChurnMap, CoChangePairMap, EnrichConfig};
use crate::port::enricher::{EnrichContext, Enricher};
use crate::Error;

pub struct GitEnricher;

impl GitEnricher {
    pub fn new() -> Self {
        Self
    }

    fn normalize_path(file_path: &str, strip_prefix: &str, extensions: &[&str]) -> String {
        let mut clean = file_path.trim().to_string();
        // Remove source file extensions (provided by LanguageParser::extensions())
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

    /// Walk git history once and compute both churn and co-change maps.
    ///
    /// Uses the longer of `churn_days` / `cochange_days` as the walk window,
    /// accumulating churn only for commits within `churn_days`.
    fn compute_metrics(
        repo: &Repository,
        config: &EnrichConfig,
        strip_prefix: &str,
        extensions: &[&str],
    ) -> Result<(ChurnMap, CoChangePairMap), Error> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| Error::Enrich(format!("system time error: {e}")))?
            .as_secs() as i64;

        let max_days = config.churn_days.max(config.cochange_days);
        let since_max = now - (i64::from(max_days) * 86400);
        let since_churn = now - (i64::from(config.churn_days) * 86400);

        let mut churn = ChurnMap::new();
        let mut pair_counts = CoChangePairMap::new();

        let mut revwalk = repo.revwalk().map_err(git_err)?;
        revwalk.push_head().map_err(git_err)?;
        revwalk.set_sorting(Sort::TIME).map_err(git_err)?;

        for oid in revwalk {
            let oid = oid.map_err(git_err)?;
            let commit = repo.find_commit(oid).map_err(git_err)?;
            let commit_time = commit.time().seconds();

            if commit_time < since_max {
                break;
            }

            let changed = Self::changed_files_for_commit(repo, &commit)?;
            let normalized: Vec<String> = changed
                .iter()
                .map(|f| Self::normalize_path(f, strip_prefix, extensions))
                .collect();

            // Accumulate churn (only within churn window)
            if commit_time >= since_churn {
                for name in &normalized {
                    *churn.entry(name.clone()).or_insert(0) += 1;
                }
            }

            // Accumulate co-change pairs (within cochange window — already guaranteed by since_max)
            let unique: HashSet<&str> = normalized.iter().map(String::as_str).collect();
            let mut sorted: Vec<&str> = unique.into_iter().collect();
            sorted.sort_unstable();

            for i in 0..sorted.len() {
                for j in (i + 1)..sorted.len() {
                    let key = format!("{}|{}", sorted[i], sorted[j]);
                    *pair_counts.entry(key).or_insert(0) += 1;
                }
            }
        }

        // Filter co-change by minimum count
        pair_counts.retain(|_, v| *v >= config.min_cochange);
        Ok((churn, pair_counts))
    }

    fn changed_files_for_commit(
        repo: &Repository,
        commit: &git2::Commit,
    ) -> Result<Vec<String>, Error> {
        let tree = commit.tree().map_err(git_err)?;
        let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

        let diff = repo
            .diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)
            .map_err(git_err)?;

        let mut changed = Vec::new();
        for delta in diff.deltas() {
            if let Some(path) = delta.new_file().path().and_then(|p| p.to_str()) {
                changed.push(path.to_string());
            }
        }

        Ok(changed)
    }
}

impl Default for GitEnricher {
    fn default() -> Self {
        Self::new()
    }
}

fn git_err(e: git2::Error) -> Error {
    Error::Enrich(e.message().to_string())
}

/// Apply file-level churn counts to AST data.
fn apply_churn(data: &mut AstData, churn: &ChurnMap) {
    for file in &mut data.files {
        let file_churn = churn.get(&file.name).copied().unwrap_or(0);
        file.git_churn_30d = Some(file_churn);

        for node in &mut file.nodes {
            node.git_churn_30d = Some(file_churn);
        }
    }
}

/// Build a map from file name to representative node indices (exported callables, or first 3).
fn build_file_reps(data: &AstData) -> HashMap<String, Vec<usize>> {
    let callable_kinds: HashSet<&str> = ["function", "method", "macro"].iter().copied().collect();
    let mut file_reps: HashMap<String, Vec<usize>> = HashMap::new();

    for file in &data.files {
        let mut callables: Vec<(usize, bool)> = Vec::new();
        for (i, node) in file.nodes.iter().enumerate() {
            if callable_kinds.contains(node.kind.as_str()) {
                callables.push((i, node.exported));
            }
        }
        let exported: Vec<usize> = callables
            .iter()
            .filter(|(_, exp)| *exp)
            .map(|(i, _)| *i)
            .collect();
        let reps = if exported.is_empty() {
            callables.iter().take(3).map(|(i, _)| *i).collect()
        } else {
            exported
        };
        file_reps.insert(file.name.clone(), reps);
    }

    file_reps
}

/// Inject co-change data into representative nodes.
fn apply_cochange(data: &mut AstData, cochange: &CoChangePairMap) {
    let file_reps = build_file_reps(data);

    // O(1) file lookup by name (owned keys to avoid borrowing data.files)
    let file_index: HashMap<String, usize> = data
        .files
        .iter()
        .enumerate()
        .map(|(i, f)| (f.name.clone(), i))
        .collect();

    for (pair_key, count) in cochange {
        let parts: Vec<&str> = pair_key.splitn(2, '|').collect();
        if parts.len() != 2 {
            continue;
        }
        let (file_a, file_b) = (parts[0], parts[1]);

        if let (Some(reps_a), Some(reps_b)) = (file_reps.get(file_a), file_reps.get(file_b)) {
            if let Some(&idx) = file_index.get(file_a) {
                inject_cochange_to_reps(&mut data.files[idx].nodes, reps_a, file_b, *count);
            }
            if let Some(&idx) = file_index.get(file_b) {
                inject_cochange_to_reps(&mut data.files[idx].nodes, reps_b, file_a, *count);
            }
        }
    }
}

fn inject_cochange_to_reps(
    nodes: &mut [crate::domain::ast::NodeData],
    reps: &[usize],
    partner_file: &str,
    count: u32,
) {
    for &idx in reps {
        if idx < nodes.len() {
            let co = nodes[idx]
                .co_changes
                .get_or_insert_with(|| serde_json::json!({}));
            if let serde_json::Value::Object(map) = co {
                map.insert(format!("{partner_file}::*"), serde_json::json!(count));
            }
        }
    }
}

impl Enricher for GitEnricher {
    fn enrich(
        &self,
        data: &mut AstData,
        config: &EnrichConfig,
        ctx: &EnrichContext<'_>,
    ) -> Result<(), Error> {
        let repo = Repository::open(ctx.repo_path)
            .map_err(|e| Error::Enrich(format!("failed to open repo: {e}")))?;

        let (churn, cochange) =
            Self::compute_metrics(&repo, config, ctx.strip_prefix, ctx.extensions)?;

        apply_churn(data, &churn);
        apply_cochange(data, &cochange);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ast::{FileData, NodeData};

    #[test]
    fn normalize_strips_rs_extension() {
        assert_eq!(
            GitEnricher::normalize_path("src/main.rs", "", &["rs"]),
            "src/main"
        );
    }

    #[test]
    fn normalize_strips_ts_extension() {
        assert_eq!(
            GitEnricher::normalize_path("src/index.ts", "", &["ts"]),
            "src/index"
        );
    }

    #[test]
    fn normalize_strips_tsx_extension() {
        assert_eq!(
            GitEnricher::normalize_path("components/App.tsx", "", &["tsx"]),
            "components/App"
        );
    }

    #[test]
    fn normalize_strips_configured_prefix() {
        assert_eq!(
            GitEnricher::normalize_path("src/lib.rs", "src/", &["rs"]),
            "lib"
        );
    }

    #[test]
    fn normalize_no_extension_passthrough() {
        assert_eq!(
            GitEnricher::normalize_path("Makefile", "", &["rs"]),
            "Makefile"
        );
    }

    #[test]
    fn normalize_trims_whitespace() {
        assert_eq!(
            GitEnricher::normalize_path("  src/main.rs  ", "", &["rs"]),
            "src/main"
        );
    }

    fn make_ast_data(file_names: &[&str]) -> AstData {
        AstData {
            files: file_names
                .iter()
                .map(|name| FileData {
                    path: format!("{name}.rs"),
                    name: name.to_string(),
                    nodes: vec![NodeData {
                        kind: "function".to_string(),
                        name: "test_fn".to_string(),
                        exported: true,
                        visibility: Some("pub".to_string()),
                        is_async: None,
                        is_unsafe: None,
                        start_line: 1,
                        end_line: 5,
                        lines: 5,
                        params: None,
                        field_count: None,
                        depth: None,
                        cyclomatic: None,
                        trait_name: None,
                        git_churn_30d: None,
                        coverage: None,
                        co_changes: None,
                        calls: None,
                    }],
                    imports: Vec::new(),
                    git_churn_30d: None,
                })
                .collect(),
            edges: Vec::new(),
        }
    }

    #[test]
    fn apply_churn_sets_file_and_node_values() {
        let mut data = make_ast_data(&["src/main"]);
        let mut churn = ChurnMap::new();
        churn.insert("src/main".to_string(), 5);

        apply_churn(&mut data, &churn);

        assert_eq!(data.files[0].git_churn_30d, Some(5));
        assert_eq!(data.files[0].nodes[0].git_churn_30d, Some(5));
    }

    #[test]
    fn apply_churn_defaults_to_zero() {
        let mut data = make_ast_data(&["src/missing"]);
        let churn = ChurnMap::new();

        apply_churn(&mut data, &churn);

        assert_eq!(data.files[0].git_churn_30d, Some(0));
        assert_eq!(data.files[0].nodes[0].git_churn_30d, Some(0));
    }

    #[test]
    fn apply_cochange_injects_both_directions() {
        let mut data = make_ast_data(&["alpha", "beta"]);
        let mut cochange = CoChangePairMap::new();
        cochange.insert("alpha|beta".to_string(), 3);

        apply_cochange(&mut data, &cochange);

        let alpha_co = data.files[0].nodes[0].co_changes.as_ref();
        assert!(alpha_co.is_some());
        let alpha_map = alpha_co.unwrap().as_object().unwrap();
        assert_eq!(alpha_map.get("beta::*").unwrap(), &serde_json::json!(3));

        let beta_co = data.files[1].nodes[0].co_changes.as_ref();
        assert!(beta_co.is_some());
        let beta_map = beta_co.unwrap().as_object().unwrap();
        assert_eq!(beta_map.get("alpha::*").unwrap(), &serde_json::json!(3));
    }

    #[test]
    fn apply_cochange_skips_unknown_files() {
        let mut data = make_ast_data(&["alpha"]);
        let mut cochange = CoChangePairMap::new();
        cochange.insert("alpha|unknown".to_string(), 2);

        apply_cochange(&mut data, &cochange);

        // Should not crash; alpha node should have no co_changes since "unknown" has no reps
        // (the pair is skipped because reps_b is None)
        assert!(data.files[0].nodes[0].co_changes.is_none());
    }
}
