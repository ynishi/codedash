//! Internal domain model for AST data.
//!
//! These types are **internal** to codedash. External consumers should use
//! [`codedash_schemas`] instead. The conversion from domain model to public
//! schema is handled by the ACL in [`crate::port::schema`].

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Top-level AST output (internal).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AstData {
    pub files: Vec<FileData>,
    #[serde(default)]
    pub edges: Vec<Edge>,
}

/// Per-file AST data (internal).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileData {
    pub path: String,
    pub name: String,
    pub nodes: Vec<NodeData>,
    #[serde(default)]
    pub imports: Vec<ImportInfo>,
    /// Injected by enrichment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_churn_30d: Option<u32>,
}

/// A single AST node (internal).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeData {
    pub kind: String,
    pub name: String,

    #[serde(default)]
    pub exported: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_async: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_unsafe: Option<bool>,

    pub start_line: usize,
    pub end_line: usize,
    pub lines: usize,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<usize>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cyclomatic: Option<usize>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trait_name: Option<String>,

    /// Injected by enrichment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_churn_30d: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<f64>,
    /// Co-change counts: `"partner_file::*" → commit_count`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub co_changes: Option<HashMap<String, u32>>,

    /// Internal use calls detected in function bodies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calls: Option<Vec<CallInfo>>,
}

/// An internal import (use crate::..., use super::..., use self::...).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportInfo {
    pub from: String,
    pub names: Vec<String>,
}

/// A call reference from one node to an imported symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallInfo {
    pub symbol: String,
    pub module: String,
    pub count: usize,
}

/// A dependency edge between files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from_file: String,
    pub to_file: String,
    pub symbol: String,
    #[serde(rename = "type")]
    pub edge_type: String,
}
