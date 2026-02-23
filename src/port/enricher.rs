//! Enricher port.

use std::path::Path;

use crate::domain::ast::AstData;
use crate::domain::enrichment::EnrichConfig;

/// Runtime context for enrichment, constant per pipeline instance.
pub struct EnrichContext<'a> {
    pub repo_path: &'a Path,
    pub strip_prefix: &'a str,
    /// Source file extensions to normalize (e.g. `&["rs"]`), from `LanguageParser::extensions()`.
    pub extensions: &'a [&'a str],
}

/// Enriches AST data with external metrics (git history, coverage, etc.).
pub trait Enricher: Send + Sync {
    /// Enrich the given AST data in place.
    fn enrich(
        &self,
        data: &mut AstData,
        config: &EnrichConfig,
        ctx: &EnrichContext<'_>,
    ) -> Result<(), crate::Error>;
}

/// Runs multiple enrichers in sequence.
pub struct ChainEnricher {
    enrichers: Vec<Box<dyn Enricher>>,
}

impl ChainEnricher {
    pub fn new(enrichers: Vec<Box<dyn Enricher>>) -> Self {
        Self { enrichers }
    }
}

impl Enricher for ChainEnricher {
    fn enrich(
        &self,
        data: &mut AstData,
        config: &EnrichConfig,
        ctx: &EnrichContext<'_>,
    ) -> Result<(), crate::Error> {
        for enricher in &self.enrichers {
            enricher.enrich(data, config, ctx)?;
        }
        Ok(())
    }
}
