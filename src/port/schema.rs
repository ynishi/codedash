//! Anti-Corruption Layer: domain model → public schema conversion.
//!
//! This module is the **only** place where internal domain types are mapped
//! to the stable [`codedash_schemas`] public contract. Changes to the domain
//! model are absorbed here so that the external JSON schema remains stable.

use crate::domain::ast as domain;

// ── Domain → Schema ────────────────────────────────────────────────

impl From<domain::AstData> for codedash_schemas::AstData {
    fn from(d: domain::AstData) -> Self {
        Self {
            files: d.files.into_iter().map(Into::into).collect(),
            edges: d.edges.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<domain::FileData> for codedash_schemas::FileData {
    fn from(d: domain::FileData) -> Self {
        Self {
            path: d.path,
            name: d.name,
            nodes: d.nodes.into_iter().map(Into::into).collect(),
            imports: d.imports.into_iter().map(Into::into).collect(),
            git_churn_30d: d.git_churn_30d,
        }
    }
}

impl From<domain::NodeData> for codedash_schemas::NodeData {
    fn from(d: domain::NodeData) -> Self {
        Self {
            kind: d.kind,
            name: d.name,
            exported: d.exported,
            visibility: d.visibility,
            is_async: d.is_async,
            is_unsafe: d.is_unsafe,
            start_line: d.start_line,
            end_line: d.end_line,
            lines: d.lines,
            params: d.params,
            field_count: d.field_count,
            depth: d.depth,
            cyclomatic: d.cyclomatic,
            trait_name: d.trait_name,
            git_churn_30d: d.git_churn_30d,
            coverage: d.coverage,
            co_changes: d.co_changes,
            calls: d.calls.map(|v| v.into_iter().map(Into::into).collect()),
        }
    }
}

impl From<domain::ImportInfo> for codedash_schemas::ImportInfo {
    fn from(d: domain::ImportInfo) -> Self {
        Self {
            from: d.from,
            names: d.names,
        }
    }
}

impl From<domain::CallInfo> for codedash_schemas::CallInfo {
    fn from(d: domain::CallInfo) -> Self {
        Self {
            symbol: d.symbol,
            module: d.module,
            count: d.count,
        }
    }
}

impl From<domain::Edge> for codedash_schemas::Edge {
    fn from(d: domain::Edge) -> Self {
        Self {
            from_file: d.from_file,
            to_file: d.to_file,
            symbol: d.symbol,
            edge_type: d.edge_type,
        }
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
}
