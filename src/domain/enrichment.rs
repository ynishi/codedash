//! Enrichment data types for git metrics.

use std::collections::HashMap;

/// File-level churn counts: file_name → commit count within the period.
pub type ChurnMap = HashMap<String, u32>;

/// Co-change pair counts: "fileA|fileB" (sorted) → co-commit count.
pub type CoChangePairMap = HashMap<String, u32>;

/// Configuration for enrichment.
#[derive(Debug, Clone)]
pub struct EnrichConfig {
    /// Churn period in days (default: 30).
    pub churn_days: u32,
    /// Co-change period in days (default: 90).
    pub cochange_days: u32,
    /// Minimum co-change count to include (default: 2).
    pub min_cochange: u32,
}

impl Default for EnrichConfig {
    fn default() -> Self {
        Self {
            churn_days: 30,
            cochange_days: 90,
            min_cochange: 2,
        }
    }
}
