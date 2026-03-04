//! codedash — code metrics visualization CLI.
//!
//! # Architecture
//!
//! codedash follows a hexagonal architecture with an Anti-Corruption Layer (ACL):
//!
//! - **[`domain`]** — Internal model (`domain::ast`, `domain::config`). May change freely.
//! - **[`port`]** — Boundaries. [`port::schema`] converts domain types to the stable
//!   [`codedash_schemas`] public contract. [`port::parser`] and [`port::enricher`] define traits.
//! - **[`infra`]** — Implementations (tree-sitter parsers, git enrichment).
//! - **[`app`]** — Use cases ([`app::analyze::AnalyzePipeline`]).
//! - **[`cli`]** — CLI presentation layer.
//!
//! ## Public schema boundary
//!
//! JSON output is serialized from [`codedash_schemas`] types, **not** from the internal
//! domain model. The conversion happens in [`port::schema`] via `From` implementations.
//! This ensures external consumers (GUIs, dashboards, CI tools) depend on a stable contract
//! that is decoupled from internal refactoring.

pub mod app;
pub mod cli;
pub mod domain;
pub mod infra;
pub mod port;

/// Crate-wide error type.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("parse error: {0}")]
    Parse(String),

    #[error("enrichment error: {0}")]
    Enrich(String),

    #[error("lua error: {0}")]
    Lua(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<senl::SenlError> for Error {
    fn from(e: senl::SenlError) -> Self {
        Error::Lua(e.to_string())
    }
}
