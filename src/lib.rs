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
