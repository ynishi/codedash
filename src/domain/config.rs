//! Analyze configuration.

use std::path::PathBuf;

use super::enrichment::EnrichConfig;

/// Per-call configuration for a codedash analyze run.
#[derive(Debug, Clone)]
pub struct AnalyzeConfig {
    /// Source directory to analyze.
    pub path: PathBuf,
    /// Language name (e.g. "rust", "typescript").
    pub lang: String,
    /// Enrichment settings.
    pub enrich: EnrichConfig,
}

impl AnalyzeConfig {
    pub fn new(path: PathBuf, lang: String) -> Self {
        Self {
            path,
            lang,
            enrich: EnrichConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sets_path_and_lang() {
        let config = AnalyzeConfig::new(PathBuf::from("/tmp/src"), "rust".to_string());
        assert_eq!(config.path, PathBuf::from("/tmp/src"));
        assert_eq!(config.lang, "rust");
    }

    #[test]
    fn new_uses_default_enrich_config() {
        let config = AnalyzeConfig::new(PathBuf::from("/tmp"), "typescript".to_string());
        assert_eq!(config.enrich.churn_days, 30);
        assert_eq!(config.enrich.cochange_days, 90);
        assert_eq!(config.enrich.min_cochange, 2);
    }
}
