//! Language parser port.

use crate::domain::ast::FileData;

/// A parser that extracts AST data from source files of a specific language.
///
/// Implement this trait to add support for a new language.
pub trait LanguageParser: Send + Sync {
    /// Human-readable language name (e.g. "rust", "typescript").
    fn name(&self) -> &str;

    /// File extensions this parser handles (e.g. &["rs"]).
    fn extensions(&self) -> &[&str];

    /// Parse a single source file and extract AST nodes.
    ///
    /// `file_path` is the original path (for metadata).
    /// `rel_name` is the normalized module name (e.g. "src/main").
    fn parse_source(
        &self,
        source: &str,
        file_path: &str,
        rel_name: &str,
    ) -> Result<FileData, crate::Error>;
}
