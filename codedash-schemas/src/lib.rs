//! Stable public schema for codedash code metrics output.
//!
//! This crate defines the **external contract** for codedash's analysis JSON.
//! It is decoupled from codedash's internal domain model via an Anti-Corruption
//! Layer (ACL), so internal refactoring does not break external consumers.
//!
//! # Design: Anti-Corruption Layer
//!
//! codedash maintains **two separate type hierarchies**:
//!
//! 1. **Internal domain model** (`codedash::domain::ast`) — optimized for
//!    parsing and enrichment logic. May change across codedash versions.
//! 2. **Public schema** (this crate) — stable, versioned contract for
//!    external consumers.
//!
//! The ACL boundary (`codedash::port::schema`) converts domain types into
//! these schema types via `From` implementations. This is the **only** place
//! where the two models touch. As a result:
//!
//! - Internal refactoring in codedash never breaks consumers of this crate.
//! - This crate carries minimal dependencies (`serde` only).
//! - Breaking changes to the schema follow semver.
//!
//! # For Rust consumers
//!
//! ```rust
//! use codedash_schemas::AstData;
//!
//! # let json = r#"{"files":[],"edges":[]}"#;
//! let data: AstData = serde_json::from_str(json).unwrap();
//! ```
//!
//! # For non-Rust consumers (JSON Schema)
//!
//! Enable the `schema` feature and generate a JSON Schema file:
//!
//! ```rust,ignore
//! let schema = schemars::schema_for!(codedash_schemas::AstData);
//! println!("{}", serde_json::to_string_pretty(&schema).unwrap());
//! ```
//!
//! The generated schema can then be used by TypeScript, Python, Go, etc.
//! to validate or generate types for codedash output.
//!
//! # Optional features
//!
//! - **`schema`** — derives [`schemars::JsonSchema`] on all types, enabling
//!   JSON Schema generation via `schemars::schema_for!`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Top-level AST output from a codedash analysis run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct AstData {
    /// Analyzed source files.
    pub files: Vec<FileData>,
    /// Dependency edges between files.
    #[serde(default)]
    pub edges: Vec<Edge>,
}

/// Per-file AST data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct FileData {
    /// Original file path (relative to the analysis root).
    pub path: String,
    /// Normalized module name (e.g. `"app/analyze"`).
    pub name: String,
    /// AST nodes (functions, structs, enums, etc.) found in this file.
    pub nodes: Vec<NodeData>,
    /// Internal imports (`use crate::...`, `use super::...`).
    #[serde(default)]
    pub imports: Vec<ImportInfo>,
    /// Git commit count within the churn period. Injected by enrichment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_churn_30d: Option<u32>,
}

/// A single AST node (function, struct, enum, impl, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct NodeData {
    /// Node kind: `"function"`, `"struct"`, `"enum"`, `"impl"`, `"method"`, etc.
    pub kind: String,
    /// Node name (identifier).
    pub name: String,

    /// Whether this node is exported / publicly visible.
    #[serde(default)]
    pub exported: bool,
    /// Visibility qualifier (e.g. `"pub"`, `"pub(crate)"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,

    /// Whether this function is `async`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_async: Option<bool>,
    /// Whether this function/trait is `unsafe`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_unsafe: Option<bool>,

    /// First line of the node (1-based).
    pub start_line: usize,
    /// Last line of the node (1-based, inclusive).
    pub end_line: usize,
    /// Total line count (`end_line - start_line + 1`).
    pub lines: usize,

    /// Number of parameters (functions/methods only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<usize>,
    /// Number of fields (structs/enums only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_count: Option<usize>,
    /// Maximum nesting depth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<usize>,

    /// Cyclomatic complexity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cyclomatic: Option<usize>,

    /// Trait name for impl blocks (e.g. `"Display"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trait_name: Option<String>,

    /// Git commit count within the churn period. Injected by enrichment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_churn_30d: Option<u32>,
    /// Region coverage ratio (0.0–1.0). Injected by enrichment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<f64>,
    /// Co-change counts: `"partner_file::*" → commit_count`.
    /// Injected by enrichment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub co_changes: Option<HashMap<String, u32>>,

    /// Internal function calls detected in the body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calls: Option<Vec<CallInfo>>,
}

/// An internal import (e.g. `use crate::domain::ast::AstData`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ImportInfo {
    /// Source module path.
    pub from: String,
    /// Imported symbol names.
    pub names: Vec<String>,
}

/// A call reference from a function body to an imported symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct CallInfo {
    /// Called symbol name.
    pub symbol: String,
    /// Module the symbol belongs to.
    pub module: String,
    /// Number of call sites within the function body.
    pub count: usize,
}

/// A dependency edge between two files.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Edge {
    /// Source file (the file that imports).
    pub from_file: String,
    /// Target file (the file being imported from).
    pub to_file: String,
    /// Imported symbol name.
    pub symbol: String,
    /// Edge type (currently always `"import"`).
    #[serde(rename = "type")]
    pub edge_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ast_data_roundtrip() {
        let data = AstData {
            files: vec![FileData {
                path: "src/main.rs".to_string(),
                name: "main".to_string(),
                nodes: vec![NodeData {
                    kind: "function".to_string(),
                    name: "main".to_string(),
                    exported: true,
                    visibility: Some("pub".to_string()),
                    is_async: None,
                    is_unsafe: None,
                    start_line: 1,
                    end_line: 5,
                    lines: 5,
                    params: Some(0),
                    field_count: None,
                    depth: Some(1),
                    cyclomatic: Some(1),
                    trait_name: None,
                    git_churn_30d: Some(3),
                    coverage: Some(0.85),
                    co_changes: Some(HashMap::from([("utils::*".to_string(), 5)])),
                    calls: Some(vec![CallInfo {
                        symbol: "helper".to_string(),
                        module: "utils".to_string(),
                        count: 1,
                    }]),
                }],
                imports: vec![ImportInfo {
                    from: "utils".to_string(),
                    names: vec!["helper".to_string()],
                }],
                git_churn_30d: Some(3),
            }],
            edges: vec![Edge {
                from_file: "main".to_string(),
                to_file: "utils".to_string(),
                symbol: "helper".to_string(),
                edge_type: "import".to_string(),
            }],
        };

        let json = serde_json::to_string(&data).unwrap();
        let parsed: AstData = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.files.len(), 1);
        assert_eq!(parsed.files[0].nodes[0].name, "main");
        assert_eq!(
            parsed.files[0].nodes[0].co_changes.as_ref().unwrap()["utils::*"],
            5
        );
        assert_eq!(parsed.edges[0].edge_type, "import");
    }

    #[test]
    fn deserialize_with_missing_optional_fields() {
        let json = r#"{
            "files": [{
                "path": "src/lib.rs",
                "name": "lib",
                "nodes": [{
                    "kind": "struct",
                    "name": "Foo",
                    "start_line": 1,
                    "end_line": 5,
                    "lines": 5
                }]
            }],
            "edges": []
        }"#;

        let parsed: AstData = serde_json::from_str(json).unwrap();
        let node = &parsed.files[0].nodes[0];
        assert!(!node.exported);
        assert!(node.co_changes.is_none());
        assert!(node.cyclomatic.is_none());
    }

    #[test]
    fn co_changes_serializes_as_object() {
        let mut co = HashMap::new();
        co.insert("auth::*".to_string(), 3);
        co.insert("db::*".to_string(), 7);

        let node = NodeData {
            kind: "function".to_string(),
            name: "handle".to_string(),
            exported: true,
            visibility: None,
            is_async: None,
            is_unsafe: None,
            start_line: 1,
            end_line: 10,
            lines: 10,
            params: None,
            field_count: None,
            depth: None,
            cyclomatic: None,
            trait_name: None,
            git_churn_30d: None,
            coverage: None,
            co_changes: Some(co),
            calls: None,
        };

        let json = serde_json::to_value(&node).unwrap();
        let co_obj = json["co_changes"].as_object().unwrap();
        assert_eq!(co_obj["auth::*"], 3);
        assert_eq!(co_obj["db::*"], 7);
    }

    #[test]
    fn edge_type_renames_to_type_in_json() {
        let edge = Edge {
            from_file: "a".to_string(),
            to_file: "b".to_string(),
            symbol: "Foo".to_string(),
            edge_type: "import".to_string(),
        };

        let json = serde_json::to_value(&edge).unwrap();
        assert!(json.get("type").is_some());
        assert!(json.get("edge_type").is_none());
    }
}
