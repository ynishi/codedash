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

pub mod analyze;

pub use analyze::{AnalyzeResult, Binding, EvalEntry, Group, PerceptValues};

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Top-level AST output from a codedash analysis run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct AstData {
    /// Analyzed source files.
    pub files: Vec<FileData>,
    /// Dependency edges between files.
    #[serde(default)]
    pub edges: Vec<Edge>,
}

impl AstData {
    /// Create a new [`AstData`].
    pub fn new(files: Vec<FileData>, edges: Vec<Edge>) -> Self {
        Self { files, edges }
    }
}

/// Per-file AST data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
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

impl FileData {
    /// Create a new [`FileData`] with required fields.
    ///
    /// `nodes`, `imports`, and `git_churn_30d` default to empty/`None`.
    pub fn new(path: String, name: String) -> Self {
        Self {
            path,
            name,
            nodes: Vec::new(),
            imports: Vec::new(),
            git_churn_30d: None,
        }
    }
}

/// A single AST node (function, struct, enum, impl, etc.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
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
    pub start_line: u32,
    /// Last line of the node (1-based, inclusive).
    pub end_line: u32,
    /// Total line count (`end_line - start_line + 1`).
    pub lines: u32,

    /// Number of parameters (functions/methods only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<u32>,
    /// Number of fields (structs/enums only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_count: Option<u32>,
    /// Maximum nesting depth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,

    /// Cyclomatic complexity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cyclomatic: Option<u32>,

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

impl NodeData {
    /// Create a new [`NodeData`] with required fields.
    ///
    /// All optional fields default to `None` / `false`.
    pub fn new(kind: String, name: String, start_line: u32, end_line: u32, lines: u32) -> Self {
        Self {
            kind,
            name,
            exported: false,
            visibility: None,
            is_async: None,
            is_unsafe: None,
            start_line,
            end_line,
            lines,
            params: None,
            field_count: None,
            depth: None,
            cyclomatic: None,
            trait_name: None,
            git_churn_30d: None,
            coverage: None,
            co_changes: None,
            calls: None,
        }
    }
}

/// An internal import (e.g. `use crate::domain::ast::AstData`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct ImportInfo {
    /// Source module path.
    pub from: String,
    /// Imported symbol names.
    pub names: Vec<String>,
}

impl ImportInfo {
    /// Create a new [`ImportInfo`].
    pub fn new(from: String, names: Vec<String>) -> Self {
        Self { from, names }
    }
}

/// A call reference from a function body to an imported symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct CallInfo {
    /// Called symbol name.
    pub symbol: String,
    /// Module the symbol belongs to.
    pub module: String,
    /// Number of call sites within the function body.
    pub count: u32,
}

impl CallInfo {
    /// Create a new [`CallInfo`].
    pub fn new(symbol: String, module: String, count: u32) -> Self {
        Self {
            symbol,
            module,
            count,
        }
    }
}

/// A dependency edge between two files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
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

impl Edge {
    /// Create a new [`Edge`].
    pub fn new(from_file: String, to_file: String, symbol: String, edge_type: String) -> Self {
        Self {
            from_file,
            to_file,
            symbol,
            edge_type,
        }
    }
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

    #[test]
    fn partial_eq_works_for_all_types() {
        let a = NodeData::new("function".into(), "foo".into(), 1, 10, 10);
        let b = NodeData::new("function".into(), "foo".into(), 1, 10, 10);
        assert_eq!(a, b);

        let c = NodeData::new("function".into(), "bar".into(), 1, 10, 10);
        assert_ne!(a, c);
    }

    #[test]
    fn constructors_produce_correct_defaults() {
        let node = NodeData::new("struct".into(), "Foo".into(), 1, 5, 5);
        assert!(!node.exported);
        assert!(node.visibility.is_none());
        assert!(node.cyclomatic.is_none());
        assert!(node.calls.is_none());

        let file = FileData::new("src/lib.rs".into(), "lib".into());
        assert!(file.nodes.is_empty());
        assert!(file.imports.is_empty());
        assert!(file.git_churn_30d.is_none());
    }
}

#[cfg(all(test, feature = "schema"))]
mod schema_snapshot {
    use super::*;

    #[test]
    fn ast_data_json_schema() {
        let schema = schemars::schema_for!(AstData);
        insta::assert_json_snapshot!("ast-data-schema", schema);
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    fn arb_call_info() -> impl Strategy<Value = CallInfo> {
        ("[a-z]{1,8}", "[a-z]{1,8}", 0u32..100).prop_map(|(s, m, c)| CallInfo {
            symbol: s,
            module: m,
            count: c,
        })
    }

    fn arb_import_info() -> impl Strategy<Value = ImportInfo> {
        ("[a-z]{1,8}", proptest::collection::vec("[a-z]{1,8}", 1..4))
            .prop_map(|(f, n)| ImportInfo { from: f, names: n })
    }

    // Note: coverage (f64) is excluded — arbitrary f64 values can lose
    // a ULP during JSON roundtrip. Deterministic tests cover f64 fields.
    fn arb_node_data() -> impl Strategy<Value = NodeData> {
        (
            "[a-z]{1,8}",
            "[a-z]{1,8}",
            1u32..10000,
            1u32..500,
            any::<bool>(),
            proptest::option::of(0u32..20),
            proptest::option::of(0u32..50),
            proptest::option::of(0u32..10),
        )
            .prop_map(
                |(kind, name, start, delta, exported, params, cyclomatic, depth)| {
                    let end = start + delta;
                    let lines = delta + 1;
                    NodeData {
                        kind,
                        name,
                        exported,
                        visibility: None,
                        is_async: None,
                        is_unsafe: None,
                        start_line: start,
                        end_line: end,
                        lines,
                        params,
                        field_count: None,
                        depth,
                        cyclomatic,
                        trait_name: None,
                        git_churn_30d: None,
                        coverage: None,
                        co_changes: None,
                        calls: None,
                    }
                },
            )
    }

    fn arb_edge() -> impl Strategy<Value = Edge> {
        ("[a-z]{1,8}", "[a-z]{1,8}", "[A-Z][a-z]{1,8}").prop_map(|(f, t, s)| Edge {
            from_file: f,
            to_file: t,
            symbol: s,
            edge_type: "import".to_string(),
        })
    }

    fn arb_node_data_with_calls() -> impl Strategy<Value = NodeData> {
        (
            arb_node_data(),
            proptest::option::of(proptest::collection::vec(arb_call_info(), 0..3)),
        )
            .prop_map(|(mut node, calls)| {
                node.calls = calls;
                node
            })
    }

    fn arb_file_data() -> impl Strategy<Value = FileData> {
        (
            "[a-z/]{1,20}",
            "[a-z]{1,8}",
            proptest::collection::vec(arb_node_data_with_calls(), 0..4),
            proptest::collection::vec(arb_import_info(), 0..3),
        )
            .prop_map(|(path, name, nodes, imports)| FileData {
                path,
                name,
                nodes,
                imports,
                git_churn_30d: None,
            })
    }

    fn arb_ast_data() -> impl Strategy<Value = AstData> {
        (
            proptest::collection::vec(arb_file_data(), 0..4),
            proptest::collection::vec(arb_edge(), 0..4),
        )
            .prop_map(|(files, edges)| AstData { files, edges })
    }

    proptest! {
        #[test]
        fn ast_data_serde_roundtrip(data in arb_ast_data()) {
            let json = serde_json::to_string(&data).unwrap();
            let parsed: AstData = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(data, parsed);
        }

        #[test]
        fn node_data_serde_roundtrip(node in arb_node_data()) {
            let json = serde_json::to_string(&node).unwrap();
            let parsed: NodeData = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(node, parsed);
        }
    }
}
