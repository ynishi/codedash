//! Anti-Corruption Layer: domain model → public schema conversion.
//!
//! This module is the **only** place where internal domain types are mapped
//! to the stable [`codedash_schemas`] public contract. Changes to the domain
//! model are absorbed here so that the external JSON schema remains stable.

use crate::domain::ast as domain;

// ── Helpers ────────────────────────────────────────────────────────

/// Saturating `usize` → `u32` conversion for schema fields.
fn to_u32(v: usize) -> u32 {
    v.min(u32::MAX as usize) as u32
}

// ── Domain → Schema ────────────────────────────────────────────────

impl From<domain::AstData> for codedash_schemas::AstData {
    fn from(d: domain::AstData) -> Self {
        Self::new(
            d.files.into_iter().map(Into::into).collect(),
            d.edges.into_iter().map(Into::into).collect(),
        )
    }
}

impl From<domain::FileData> for codedash_schemas::FileData {
    fn from(d: domain::FileData) -> Self {
        let mut f = Self::new(d.path, d.name);
        f.nodes = d.nodes.into_iter().map(Into::into).collect();
        f.imports = d.imports.into_iter().map(Into::into).collect();
        f.git_churn_30d = d.git_churn_30d;
        f
    }
}

impl From<domain::NodeData> for codedash_schemas::NodeData {
    fn from(d: domain::NodeData) -> Self {
        let mut n = Self::new(
            d.kind,
            d.name,
            to_u32(d.start_line),
            to_u32(d.end_line),
            to_u32(d.lines),
        );
        n.exported = d.exported;
        n.visibility = d.visibility;
        n.is_async = d.is_async;
        n.is_unsafe = d.is_unsafe;
        n.params = d.params.map(to_u32);
        n.field_count = d.field_count.map(to_u32);
        n.depth = d.depth.map(to_u32);
        n.cyclomatic = d.cyclomatic.map(to_u32);
        n.trait_name = d.trait_name;
        n.git_churn_30d = d.git_churn_30d;
        n.coverage = d.coverage;
        n.co_changes = d.co_changes;
        n.calls = d.calls.map(|v| v.into_iter().map(Into::into).collect());
        n
    }
}

impl From<domain::ImportInfo> for codedash_schemas::ImportInfo {
    fn from(d: domain::ImportInfo) -> Self {
        Self::new(d.from, d.names)
    }
}

impl From<domain::CallInfo> for codedash_schemas::CallInfo {
    fn from(d: domain::CallInfo) -> Self {
        Self::new(d.symbol, d.module, to_u32(d.count))
    }
}

impl From<domain::Edge> for codedash_schemas::Edge {
    fn from(d: domain::Edge) -> Self {
        Self::new(d.from_file, d.to_file, d.symbol, d.edge_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn domain_to_schema_roundtrip() {
        let domain_data = domain::AstData {
            files: vec![domain::FileData {
                path: "src/main.rs".to_string(),
                name: "main".to_string(),
                nodes: vec![domain::NodeData {
                    kind: "function".to_string(),
                    name: "run".to_string(),
                    exported: true,
                    visibility: Some("pub".to_string()),
                    is_async: Some(true),
                    is_unsafe: None,
                    start_line: 1,
                    end_line: 10,
                    lines: 10,
                    params: Some(2),
                    field_count: None,
                    depth: Some(3),
                    cyclomatic: Some(4),
                    trait_name: None,
                    git_churn_30d: Some(5),
                    coverage: Some(0.85),
                    co_changes: Some(HashMap::from([("utils::*".to_string(), 3)])),
                    calls: Some(vec![domain::CallInfo {
                        symbol: "helper".to_string(),
                        module: "utils".to_string(),
                        count: 1,
                    }]),
                }],
                imports: vec![domain::ImportInfo {
                    from: "utils".to_string(),
                    names: vec!["helper".to_string()],
                }],
                git_churn_30d: Some(5),
            }],
            edges: vec![domain::Edge {
                from_file: "main".to_string(),
                to_file: "utils".to_string(),
                symbol: "helper".to_string(),
                edge_type: "import".to_string(),
            }],
        };

        let schema: codedash_schemas::AstData = domain_data.into();

        assert_eq!(schema.files.len(), 1);
        assert_eq!(schema.files[0].nodes[0].name, "run");
        assert_eq!(schema.files[0].nodes[0].is_async, Some(true));
        assert_eq!(schema.files[0].nodes[0].cyclomatic, Some(4));
        assert_eq!(
            schema.files[0].nodes[0].co_changes.as_ref().unwrap()["utils::*"],
            3
        );
        assert_eq!(schema.files[0].nodes[0].calls.as_ref().unwrap().len(), 1);
        assert_eq!(schema.edges[0].edge_type, "import");
    }

    #[test]
    fn domain_to_schema_preserves_none_fields() {
        let domain_node = domain::NodeData {
            kind: "struct".to_string(),
            name: "Foo".to_string(),
            exported: false,
            visibility: None,
            is_async: None,
            is_unsafe: None,
            start_line: 1,
            end_line: 5,
            lines: 5,
            params: None,
            field_count: None,
            depth: None,
            cyclomatic: None,
            trait_name: None,
            git_churn_30d: None,
            coverage: None,
            co_changes: None,
            calls: None,
        };

        let schema: codedash_schemas::NodeData = domain_node.into();

        assert!(schema.co_changes.is_none());
        assert!(schema.cyclomatic.is_none());
        assert!(schema.calls.is_none());
    }

    #[test]
    fn usize_to_u32_conversion() {
        let domain_node = domain::NodeData {
            kind: "function".to_string(),
            name: "big".to_string(),
            exported: false,
            visibility: None,
            is_async: None,
            is_unsafe: None,
            start_line: 100_000,
            end_line: 200_000,
            lines: 100_001,
            params: Some(5),
            field_count: None,
            depth: Some(8),
            cyclomatic: Some(15),
            trait_name: None,
            git_churn_30d: None,
            coverage: None,
            co_changes: None,
            calls: Some(vec![domain::CallInfo {
                symbol: "f".to_string(),
                module: "m".to_string(),
                count: 42,
            }]),
        };

        let schema: codedash_schemas::NodeData = domain_node.into();

        assert_eq!(schema.start_line, 100_000u32);
        assert_eq!(schema.end_line, 200_000u32);
        assert_eq!(schema.lines, 100_001u32);
        assert_eq!(schema.params, Some(5u32));
        assert_eq!(schema.depth, Some(8u32));
        assert_eq!(schema.cyclomatic, Some(15u32));
        assert_eq!(schema.calls.as_ref().unwrap()[0].count, 42u32);
    }
}
