//! Analyze use case — orchestrates parse → enrich → serialize.

use std::path::Path;

use crate::domain::ast::AstData;
use crate::domain::config::AnalyzeConfig;
use crate::infra::parser::registry::ParserRegistry;
use crate::port::enricher::{EnrichContext, Enricher};
use crate::Error;

/// The core pipeline: parse a codebase and enrich with git metrics.
///
/// Called by both CLI and MCP presentation layers.
pub struct AnalyzePipeline {
    registry: ParserRegistry,
    enricher: Box<dyn Enricher>,
    repo_path: std::path::PathBuf,
    strip_prefix: String,
}

impl AnalyzePipeline {
    pub fn new(
        registry: ParserRegistry,
        enricher: Box<dyn Enricher>,
        repo_path: std::path::PathBuf,
    ) -> Self {
        Self {
            registry,
            enricher,
            repo_path,
            strip_prefix: String::new(),
        }
    }

    pub fn with_strip_prefix(mut self, prefix: String) -> Self {
        self.strip_prefix = prefix;
        self
    }

    /// Run the full pipeline: discover files → parse → enrich → JSON string.
    pub fn run(&self, config: &AnalyzeConfig) -> Result<String, Error> {
        let parser = self
            .registry
            .for_name(&config.lang)
            .ok_or_else(|| Error::Parse(format!("unsupported language: {}", config.lang)))?;

        // Discover source files
        let files = discover_files(&config.path, parser.extensions())?;

        // Parse each file
        let mut ast_data = AstData {
            files: Vec::new(),
            edges: Vec::new(),
        };

        let strip = (!self.strip_prefix.is_empty()).then_some(self.strip_prefix.as_str());
        for file_path in &files {
            let source = std::fs::read_to_string(file_path)?;
            let rel_name = compute_rel_name(file_path, &config.path, strip);
            match parser.parse_source(&source, &file_path.to_string_lossy(), &rel_name) {
                Ok(file_data) => ast_data.files.push(file_data),
                Err(e) => {
                    eprintln!("[WARN] parse failed for {}: {e}", file_path.display());
                }
            }
        }

        // Build edges from imports
        build_edges(&mut ast_data);

        // Enrich with git metrics
        let ctx = EnrichContext {
            repo_path: &self.repo_path,
            strip_prefix: &self.strip_prefix,
            extensions: parser.extensions(),
        };
        self.enricher.enrich(&mut ast_data, &config.enrich, &ctx)?;

        // Serialize to JSON (compatible with codedash Lua eval)
        let json = serde_json::to_string(&ast_data)?;
        Ok(json)
    }
}

/// Discover source files matching the given extensions.
fn discover_files(base: &Path, extensions: &[&str]) -> Result<Vec<std::path::PathBuf>, Error> {
    let mut files = Vec::new();
    visit_dir(base, extensions, &mut files)?;
    files.sort();
    Ok(files)
}

/// Directory names to skip during source file discovery.
const SKIP_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "target",
    "node_modules",
    ".next",
    "dist",
    "build",
    "__pycache__",
];

fn should_skip_dir(name: &str) -> bool {
    name.starts_with('.') || SKIP_DIRS.contains(&name)
}

fn visit_dir(
    dir: &Path,
    extensions: &[&str],
    out: &mut Vec<std::path::PathBuf>,
) -> Result<(), Error> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if should_skip_dir(name) {
                    continue;
                }
            }
            visit_dir(&path, extensions, out)?;
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if extensions.contains(&ext) {
                out.push(path);
            }
        }
    }
    Ok(())
}

/// Compute a relative module name from a file path.
fn compute_rel_name(file_path: &Path, base: &Path, strip_prefix: Option<&str>) -> String {
    let rel = file_path
        .strip_prefix(base)
        .unwrap_or(file_path)
        .to_string_lossy()
        .to_string();

    let mut name = rel;
    // Strip configured prefix
    if let Some(prefix) = strip_prefix {
        if let Some(rest) = name.strip_prefix(prefix) {
            name = rest.to_string();
        }
    }
    // Remove file extension
    if let Some(pos) = name.rfind('.') {
        name.truncate(pos);
    }
    name
}

/// Build import edges from file-level import info.
fn build_edges(data: &mut AstData) {
    let mut edges = Vec::new();
    for file in &data.files {
        for imp in &file.imports {
            for name in &imp.names {
                edges.push(crate::domain::ast::Edge {
                    from_file: file.name.clone(),
                    to_file: imp.from.clone(),
                    symbol: name.clone(),
                    edge_type: "import".to_string(),
                });
            }
        }
    }
    data.edges = edges;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ast::{FileData, ImportInfo};

    #[test]
    fn compute_rel_name_strips_base_and_extension() {
        let base = Path::new("/project/src");
        let file = Path::new("/project/src/utils/helper.rs");
        assert_eq!(compute_rel_name(file, base, None), "utils/helper");
    }

    #[test]
    fn compute_rel_name_strips_prefix() {
        let base = Path::new("/project");
        let file = Path::new("/project/src/main.rs");
        assert_eq!(compute_rel_name(file, base, Some("src/")), "main");
    }

    #[test]
    fn compute_rel_name_no_extension() {
        let base = Path::new("/project");
        let file = Path::new("/project/Makefile");
        assert_eq!(compute_rel_name(file, base, None), "Makefile");
    }

    #[test]
    fn compute_rel_name_fallback_when_not_under_base() {
        let base = Path::new("/other");
        let file = Path::new("/project/src/main.rs");
        // strip_prefix fails, falls back to full path without extension
        let result = compute_rel_name(file, base, None);
        assert!(result.ends_with("main"));
    }

    #[test]
    fn build_edges_from_imports() {
        let mut data = AstData {
            files: vec![FileData {
                path: "src/app.rs".to_string(),
                name: "app".to_string(),
                nodes: Vec::new(),
                imports: vec![ImportInfo {
                    from: "utils".to_string(),
                    names: vec!["Helper".to_string(), "Config".to_string()],
                }],
                git_churn_30d: None,
            }],
            edges: Vec::new(),
        };

        build_edges(&mut data);

        assert_eq!(data.edges.len(), 2);
        assert_eq!(data.edges[0].from_file, "app");
        assert_eq!(data.edges[0].to_file, "utils");
        assert_eq!(data.edges[0].symbol, "Helper");
        assert_eq!(data.edges[1].symbol, "Config");
    }

    #[test]
    fn build_edges_empty_imports() {
        let mut data = AstData {
            files: vec![FileData {
                path: "src/main.rs".to_string(),
                name: "main".to_string(),
                nodes: Vec::new(),
                imports: Vec::new(),
                git_churn_30d: None,
            }],
            edges: Vec::new(),
        };

        build_edges(&mut data);

        assert!(data.edges.is_empty());
    }

    #[test]
    fn discover_files_filters_by_extension() {
        // Use the actual src directory as a test fixture
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let files = discover_files(&base, &["rs"]).unwrap();
        assert!(!files.is_empty());
        for f in &files {
            assert_eq!(f.extension().and_then(|e| e.to_str()), Some("rs"));
        }
    }

    #[test]
    fn discover_files_returns_empty_for_nonexistent() {
        let files = discover_files(Path::new("/nonexistent/path"), &["rs"]).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn discover_files_ignores_non_matching() {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let files = discover_files(&base, &["zzz_nonexistent"]).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn should_skip_dir_filters_hidden_dirs() {
        assert!(should_skip_dir(".git"));
        assert!(should_skip_dir(".hg"));
        assert!(should_skip_dir(".hidden_anything"));
    }

    #[test]
    fn should_skip_dir_filters_build_dirs() {
        assert!(should_skip_dir("target"));
        assert!(should_skip_dir("node_modules"));
        assert!(should_skip_dir("dist"));
        assert!(should_skip_dir("build"));
    }

    #[test]
    fn should_skip_dir_allows_normal_dirs() {
        assert!(!should_skip_dir("src"));
        assert!(!should_skip_dir("lib"));
        assert!(!should_skip_dir("tests"));
    }

    #[test]
    fn discover_files_skips_target_dir() {
        // The project root contains target/ — discover from root should still work
        // and none of the results should be under target/
        let base = Path::new(env!("CARGO_MANIFEST_DIR"));
        let files = discover_files(&base, &["rs"]).unwrap();
        assert!(!files.is_empty());
        for f in &files {
            let rel = f.strip_prefix(base).unwrap_or(f);
            assert!(
                !rel.starts_with("target"),
                "should not include file under target/: {}",
                rel.display()
            );
        }
    }
}
