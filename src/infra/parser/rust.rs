//! Rust language parser using tree-sitter-rust.
//!
//! Reimplements parse_rust.js functionality:
//! - Extracts function, struct, enum, trait, impl, type_alias, const, static, module, macro nodes
//! - Computes cyclomatic complexity for functions
//! - Extracts internal imports (use crate::, super::, self::)
//! - Detects visibility (pub, pub(crate), etc.)

use tree_sitter::{Node, Parser};

use crate::domain::ast::{CallInfo, FileData, ImportInfo, NodeData};
use crate::port::parser::LanguageParser;
use crate::Error;

pub struct RustParser;

impl Default for RustParser {
    fn default() -> Self {
        Self::new()
    }
}

impl RustParser {
    pub fn new() -> Self {
        Self
    }

    fn create_parser() -> Result<Parser, Error> {
        let mut parser = Parser::new();
        let language = tree_sitter_rust::LANGUAGE;
        parser
            .set_language(&language.into())
            .map_err(|e| Error::Parse(format!("failed to set Rust language: {e}")))?;
        Ok(parser)
    }
}

impl LanguageParser for RustParser {
    fn name(&self) -> &str {
        "rust"
    }

    fn extensions(&self) -> &[&str] {
        &["rs"]
    }

    fn parse_source(
        &self,
        source: &str,
        file_path: &str,
        rel_name: &str,
    ) -> Result<FileData, Error> {
        let mut parser = Self::create_parser()?;
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| Error::Parse(format!("tree-sitter parse failed for {file_path}")))?;

        let mut file_data = FileData {
            path: file_path.to_string(),
            name: rel_name.to_string(),
            nodes: Vec::new(),
            imports: Vec::new(),
            git_churn_30d: None,
        };

        visit_node(tree.root_node(), source, 0, None, &mut file_data, rel_name);
        resolve_calls(source, &file_data.imports, &mut file_data.nodes);

        Ok(file_data)
    }
}

/// Recursively visit AST nodes and extract codedash-relevant data.
fn visit_node(
    node: Node,
    source: &str,
    depth: usize,
    impl_target: Option<&str>,
    file_data: &mut FileData,
    rel_name: &str,
) {
    if let Some(extracted) = extract_node(node, source, depth, impl_target, rel_name) {
        match extracted {
            Extracted::Node(n) => file_data.nodes.push(*n),
            Extracted::Import(imp) => file_data.imports.push(imp),
        }
    }

    let child_impl_target = if node.kind() == "impl_item" {
        get_impl_target(node, source)
    } else {
        impl_target.map(|s| s.to_string())
    };

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_node(
            child,
            source,
            depth + 1,
            child_impl_target.as_deref().or(impl_target),
            file_data,
            rel_name,
        );
    }
}

enum Extracted {
    Node(Box<NodeData>),
    Import(ImportInfo),
}

fn extract_node(
    node: Node,
    source: &str,
    depth: usize,
    impl_target: Option<&str>,
    rel_name: &str,
) -> Option<Extracted> {
    match node.kind() {
        "use_declaration" => extract_use(node, source, rel_name).map(Extracted::Import),

        "function_item" => {
            let name = name_or_anon(node, source);
            let vis = get_visibility(node, source);
            let kind = if impl_target.is_some() {
                "method"
            } else {
                "function"
            };
            let mut n = base_node(kind, name, vis, node, depth);
            n.is_async = Some(has_keyword(node, "async"));
            n.is_unsafe = Some(has_keyword(node, "unsafe"));
            n.params = Some(count_params(node));
            n.cyclomatic = Some(compute_cyclomatic(node, source));
            Some(Extracted::Node(Box::new(n)))
        }

        "struct_item" => {
            let name = name_or_anon(node, source);
            let vis = get_visibility(node, source);
            let body = node.child_by_field_name("body");
            let fc = body
                .map(|b| count_struct_fields(b))
                .unwrap_or_else(|| count_tuple_fields(node));
            let mut n = base_node("struct", name, vis, node, depth);
            n.field_count = Some(fc);
            Some(Extracted::Node(Box::new(n)))
        }

        "enum_item" => {
            let name = name_or_anon(node, source);
            let vis = get_visibility(node, source);
            let fc = node
                .child_by_field_name("body")
                .map(|b| count_enum_variants(b))
                .unwrap_or(0);
            let mut n = base_node("enum", name, vis, node, depth);
            n.field_count = Some(fc);
            Some(Extracted::Node(Box::new(n)))
        }

        "trait_item" => {
            let name = name_or_anon(node, source);
            let vis = get_visibility(node, source);
            let fc = node
                .child_by_field_name("body")
                .map(|b| count_trait_items(b))
                .unwrap_or(0);
            let mut n = base_node("trait", name, vis, node, depth);
            n.field_count = Some(fc);
            Some(Extracted::Node(Box::new(n)))
        }

        "impl_item" => {
            let target = get_impl_target(node, source).unwrap_or_else(|| "(anonymous)".to_string());
            let vis = Visibility {
                exported: false,
                text: "private".to_string(),
            };
            let mut n = base_node("impl", target, vis, node, depth);
            n.trait_name = get_impl_trait(node, source);
            Some(Extracted::Node(Box::new(n)))
        }

        "type_item" => Some(Extracted::Node(Box::new(base_node(
            "type_alias",
            name_or_anon(node, source),
            get_visibility(node, source),
            node,
            depth,
        )))),

        "const_item" | "static_item" | "mod_item" | "macro_definition" => {
            let kind = match node.kind() {
                "const_item" => "const",
                "static_item" => "static",
                "mod_item" => "module",
                _ => "macro",
            };
            Some(Extracted::Node(Box::new(base_node(
                kind,
                name_or_anon(node, source),
                get_visibility(node, source),
                node,
                depth,
            ))))
        }

        _ => None,
    }
}

fn name_or_anon(node: Node, source: &str) -> String {
    child_field_text(node, "name", source).unwrap_or_else(|| "(anonymous)".to_string())
}

/// Create a base node with common fields; callers set optional fields on the returned value.
fn base_node(kind: &str, name: String, vis: Visibility, node: Node, depth: usize) -> NodeData {
    NodeData {
        kind: kind.to_string(),
        name,
        exported: vis.exported,
        visibility: Some(vis.text),
        is_async: None,
        is_unsafe: None,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        lines: node.end_position().row - node.start_position().row + 1,
        params: None,
        field_count: None,
        depth: Some(depth),
        cyclomatic: None,
        trait_name: None,
        git_churn_30d: None,
        coverage: None,
        co_changes: None,
        calls: None,
    }
}

// ── Helpers ──────────────────────────────────────

struct Visibility {
    exported: bool,
    text: String,
}

fn get_visibility(node: Node, source: &str) -> Visibility {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            let text = node_text(child, source);
            return Visibility {
                exported: text == "pub",
                text,
            };
        }
    }
    Visibility {
        exported: false,
        text: "private".to_string(),
    }
}

fn has_keyword(node: Node, keyword: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == keyword {
            return true;
        }
        // tree-sitter-rust 0.24 wraps async/unsafe in function_modifiers
        if child.kind() == "function_modifiers" {
            let mut inner = child.walk();
            for grandchild in child.children(&mut inner) {
                if grandchild.kind() == keyword {
                    return true;
                }
            }
        }
    }
    false
}

fn child_field_text(node: Node, field: &str, source: &str) -> Option<String> {
    node.child_by_field_name(field)
        .map(|n| node_text(n, source))
}

fn node_text(node: Node, source: &str) -> String {
    let start = node.start_byte();
    let end = node.end_byte();
    source.get(start..end).unwrap_or("").to_string()
}

/// Count explicit parameters (excludes `self`/`&self` which are `self_parameter` in tree-sitter).
fn count_params(fn_node: Node) -> usize {
    let Some(params) = fn_node.child_by_field_name("parameters") else {
        return 0;
    };
    let mut count = 0;
    let mut cursor = params.walk();
    for child in params.named_children(&mut cursor) {
        if child.kind() == "parameter" {
            count += 1;
        }
    }
    count
}

fn count_struct_fields(body: Node) -> usize {
    if body.kind() != "field_declaration_list" {
        return 0;
    }
    let mut cursor = body.walk();
    body.named_children(&mut cursor)
        .filter(|c| c.kind() == "field_declaration")
        .count()
}

fn count_tuple_fields(struct_node: Node) -> usize {
    let mut cursor = struct_node.walk();
    for child in struct_node.children(&mut cursor) {
        if child.kind() == "ordered_field_declaration_list" {
            return child.named_child_count();
        }
    }
    0
}

fn count_enum_variants(body: Node) -> usize {
    if body.kind() != "enum_variant_list" {
        return 0;
    }
    let mut cursor = body.walk();
    body.named_children(&mut cursor)
        .filter(|c| c.kind() == "enum_variant")
        .count()
}

fn count_trait_items(body: Node) -> usize {
    if body.kind() != "declaration_list" {
        return 0;
    }
    let mut cursor = body.walk();
    body.named_children(&mut cursor)
        .filter(|c| {
            matches!(
                c.kind(),
                "function_item" | "function_signature_item" | "associated_type" | "const_item"
            )
        })
        .count()
}

fn get_impl_target(node: Node, source: &str) -> Option<String> {
    let type_node = node.child_by_field_name("type")?;
    match type_node.kind() {
        "type_identifier" => Some(node_text(type_node, source)),
        "generic_type" => {
            let base = type_node.child_by_field_name("type");
            Some(base.map_or_else(|| node_text(type_node, source), |b| node_text(b, source)))
        }
        _ => Some(node_text(type_node, source)),
    }
}

fn get_impl_trait(node: Node, source: &str) -> Option<String> {
    let trait_node = node.child_by_field_name("trait")?;
    match trait_node.kind() {
        "type_identifier" => Some(node_text(trait_node, source)),
        "generic_type" => {
            let base = trait_node.child_by_field_name("type");
            Some(base.map_or_else(|| node_text(trait_node, source), |b| node_text(b, source)))
        }
        _ => Some(node_text(trait_node, source)),
    }
}

/// McCabe cyclomatic complexity.
///
/// Base: 1 (straight-line path).
/// +1 for: if, while, for, loop, match_arm, &&, ||, ? operator.
/// -1 for: match_expression (one arm is the baseline).
fn compute_cyclomatic(fn_node: Node, source: &str) -> usize {
    let Some(body) = fn_node.child_by_field_name("body") else {
        return 1;
    };
    let mut complexity: i32 = 1;
    walk_cyclomatic(body, source, &mut complexity);
    complexity.max(1) as usize
}

fn walk_cyclomatic(node: Node, source: &str, complexity: &mut i32) {
    match node.kind() {
        "if_expression" | "while_expression" | "for_expression" | "loop_expression" => {
            *complexity += 1;
        }
        "match_expression" => {
            *complexity -= 1;
        }
        "match_arm" => {
            *complexity += 1;
        }
        "try_expression" => {
            *complexity += 1;
        }
        "binary_expression" => {
            // Check for && or || short-circuit operators
            if let Some(op) = node.child(1) {
                let op_text = node_text(op, source);
                if op_text == "&&" || op_text == "||" {
                    *complexity += 1;
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_cyclomatic(child, source, complexity);
    }
}

// ── Use declaration parsing ──────────────────────

fn extract_use(node: Node, source: &str, rel_name: &str) -> Option<ImportInfo> {
    let text = node_text(node, source);
    let text = text.trim();

    // Only internal imports
    if !text.starts_with("use crate::")
        && !text.starts_with("use super::")
        && !text.starts_with("use self::")
    {
        return None;
    }

    // Strip "use " prefix and ";" suffix
    let use_path = text
        .strip_prefix("use ")?
        .strip_suffix(';')
        .unwrap_or(text.strip_prefix("use ")?)
        .trim();

    // Skip glob imports
    if use_path.ends_with("::*") {
        return None;
    }

    // Group import: use crate::module::{Foo, Bar}
    if let Some((base, group)) = parse_group_import(use_path) {
        let from = normalize_use_path(base, rel_name);
        if from.is_empty() {
            return None;
        }
        let names: Vec<String> = group
            .split(',')
            .filter_map(|s| {
                let s = s.trim();
                if s == "self" || s.is_empty() || s.contains('{') || s.contains('}') {
                    return None;
                }
                // Handle "Name as Alias"
                if let Some(alias) = s.split(" as ").nth(1) {
                    return Some(alias.trim().to_string());
                }
                // Nested path: take last segment
                let last = s.split("::").last()?;
                Some(last.to_string())
            })
            .collect();
        if names.is_empty() {
            return None;
        }
        return Some(ImportInfo { from, names });
    }

    // Alias import: use crate::module::Type as Alias
    if use_path.contains(" as ") {
        let parts: Vec<&str> = use_path.splitn(2, " as ").collect();
        if parts.len() == 2 {
            let path = parts[0].trim();
            let alias = parts[1].trim().to_string();
            if let Some(pos) = path.rfind("::") {
                let from = normalize_use_path(&path[..pos], rel_name);
                if from.is_empty() {
                    return None;
                }
                return Some(ImportInfo {
                    from,
                    names: vec![alias],
                });
            }
        }
    }

    // Simple import: use crate::module::Type
    if let Some(pos) = use_path.rfind("::") {
        let from = normalize_use_path(&use_path[..pos], rel_name);
        if from.is_empty() {
            return None;
        }
        let name = use_path[pos + 2..].to_string();
        return Some(ImportInfo {
            from,
            names: vec![name],
        });
    }

    None
}

fn parse_group_import(use_path: &str) -> Option<(&str, &str)> {
    let brace_start = use_path.find("::{")? + 2;
    let brace_end = use_path.rfind('}')?;
    let base = &use_path[..brace_start - 2];
    let group = &use_path[brace_start + 1..brace_end];
    Some((base, group))
}

/// Resolve a use-path to a crate-relative module path.
///
/// `rel_name` is the current file's relative name (e.g., `domain/config` for
/// `src/domain/config.rs`). It is needed to resolve `super::` and `self::`.
fn normalize_use_path(path: &str, rel_name: &str) -> String {
    // Bare "crate" — top-level re-export, no specific module
    if path == "crate" {
        return String::new();
    }
    if let Some(rest) = path.strip_prefix("crate::") {
        return rest.replace("::", "/");
    }

    // super:: — resolve relative to parent module
    if path == "super" {
        return resolve_super(rel_name).to_string();
    }
    if let Some(rest) = path.strip_prefix("super::") {
        let parent = resolve_super(rel_name);
        let resolved = rest.replace("::", "/");
        if parent.is_empty() {
            return resolved;
        }
        return format!("{parent}/{resolved}");
    }

    // self:: — resolve relative to current module
    if path == "self" {
        return resolve_self(rel_name).to_string();
    }
    if let Some(rest) = path.strip_prefix("self::") {
        let current = resolve_self(rel_name);
        let resolved = rest.replace("::", "/");
        if current.is_empty() {
            return resolved;
        }
        return format!("{current}/{resolved}");
    }

    path.replace("::", "/")
}

/// Module path that `super` refers to from the given file's rel_name.
///
/// - `domain/config` → `domain` (parent of module `domain::config`)
/// - `infra/parser/rust` → `infra/parser`
/// - `domain/mod` → `""` (mod.rs defines `domain`; its parent is crate root)
/// - `main` → `""` (top-level; parent is crate root)
fn resolve_super(rel_name: &str) -> &str {
    let module_path = rel_name.strip_suffix("/mod").unwrap_or(rel_name);
    module_path
        .rsplit_once('/')
        .map_or("", |(parent, _)| parent)
}

/// Module path that `self` refers to from the given file's rel_name.
///
/// - `domain/config` → `domain/config`
/// - `domain/mod` → `domain` (mod.rs IS the directory module)
fn resolve_self(rel_name: &str) -> &str {
    rel_name.strip_suffix("/mod").unwrap_or(rel_name)
}

// ── Call resolution ──────────────────────────────

/// Resolve function calls by scanning body text for imported symbols.
fn resolve_calls(source: &str, imports: &[ImportInfo], nodes: &mut [NodeData]) {
    // Build symbol → module map from imports
    let mut symbol_map: Vec<(&str, &str)> = Vec::new();
    for imp in imports {
        for name in &imp.names {
            symbol_map.push((name.as_str(), imp.from.as_str()));
        }
    }

    if symbol_map.is_empty() {
        return;
    }

    let callable_kinds = ["function", "method", "macro"];

    for node in nodes.iter_mut() {
        if !callable_kinds.contains(&node.kind.as_str()) {
            continue;
        }

        // Extract body text from source using line range
        let body_text = extract_body_text(source, node.start_line, node.end_line);
        if body_text.is_empty() {
            continue;
        }

        let mut calls = Vec::new();
        for &(symbol, module) in &symbol_map {
            let count = count_symbol_references(body_text, symbol);
            if count > 0 {
                calls.push(CallInfo {
                    symbol: symbol.to_string(),
                    module: module.to_string(),
                    count,
                });
            }
        }

        if !calls.is_empty() {
            node.calls = Some(calls);
        }
    }
}

/// Byte offset where the given 1-based line starts.
fn line_start_offset(source: &str, line: usize) -> usize {
    if line <= 1 {
        return 0;
    }
    // line 2 → 0th newline + 1, line 3 → 1st newline + 1, …
    source
        .match_indices('\n')
        .nth(line - 2)
        .map(|(i, _)| i + 1)
        .unwrap_or(source.len())
}

/// Byte offset at the newline (or EOF) that terminates the given 1-based line.
fn line_end_offset(source: &str, line: usize) -> usize {
    source
        .match_indices('\n')
        .nth(line - 1)
        .map(|(i, _)| i)
        .unwrap_or(source.len())
}

/// Extract source text for the given 1-based line range without intermediate allocations.
fn extract_body_text(source: &str, start_line: usize, end_line: usize) -> &str {
    let start = line_start_offset(source, start_line);
    let end = line_end_offset(source, end_line);
    &source[start..end]
}

fn count_symbol_references(body: &str, symbol: &str) -> usize {
    let mut count = 0;

    // Pattern 1: function call — symbol(
    count += count_pattern(body, symbol, "(");
    // Pattern 2: associated function — Symbol::
    count += count_pattern(body, symbol, "::");
    // Pattern 3: macro call — symbol!(
    count += count_pattern(body, symbol, "!(");
    // Pattern 4: type context (rough heuristic)
    count += count_type_usage(body, symbol);

    count
}

fn count_pattern(body: &str, symbol: &str, suffix: &str) -> usize {
    let mut count = 0;
    let mut haystack = body;
    while let Some(pos) = haystack.find(symbol) {
        let after = pos + symbol.len();
        if haystack[after..].starts_with(suffix) {
            count += 1;
        }
        haystack = &haystack[after..];
    }
    count
}

fn count_type_usage(body: &str, symbol: &str) -> usize {
    let mut count = 0;
    // <Symbol> or : Symbol or -> Symbol
    for prefix in ["<", ": ", "-> "] {
        count += count_pattern(body, prefix, symbol);
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_rust(source: &str) -> FileData {
        let parser = RustParser::new();
        parser.parse_source(source, "test.rs", "test").unwrap()
    }

    #[test]
    fn parse_function() {
        let result = parse_rust("pub fn hello(x: i32, y: i32) -> bool { true }");
        assert_eq!(result.nodes.len(), 1);
        let node = &result.nodes[0];
        assert_eq!(node.kind, "function");
        assert_eq!(node.name, "hello");
        assert!(node.exported);
        assert_eq!(node.params, Some(2));
    }

    #[test]
    fn parse_struct_with_fields() {
        let result =
            parse_rust("pub struct Point {\n    pub x: f64,\n    pub y: f64,\n    pub z: f64,\n}");
        assert_eq!(result.nodes.len(), 1);
        let node = &result.nodes[0];
        assert_eq!(node.kind, "struct");
        assert_eq!(node.name, "Point");
        assert_eq!(node.field_count, Some(3));
    }

    #[test]
    fn parse_enum() {
        let result = parse_rust("pub enum Color {\n    Red,\n    Green,\n    Blue,\n}");
        assert_eq!(result.nodes.len(), 1);
        let node = &result.nodes[0];
        assert_eq!(node.kind, "enum");
        assert_eq!(node.field_count, Some(3));
    }

    #[test]
    fn parse_impl_with_method() {
        let result = parse_rust("struct Foo;\nimpl Foo {\n    pub fn bar(&self) {}\n}");
        // struct + impl + method
        assert_eq!(result.nodes.len(), 3);
        let method = result.nodes.iter().find(|n| n.kind == "method").unwrap();
        assert_eq!(method.name, "bar");
    }

    #[test]
    fn parse_trait_items() {
        let result = parse_rust(
            "pub trait MyTrait {\n    fn required(&self);\n    fn provided(&self) {}\n}",
        );
        let t = result.nodes.iter().find(|n| n.kind == "trait").unwrap();
        assert_eq!(t.field_count, Some(2));
    }

    #[test]
    fn parse_use_crate() {
        let result = parse_rust("use crate::foo::bar::Baz;");
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].from, "foo/bar");
        assert_eq!(result.imports[0].names, vec!["Baz"]);
    }

    #[test]
    fn parse_use_group() {
        let result = parse_rust("use crate::utils::{Helper, Config};");
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].from, "utils");
        assert_eq!(result.imports[0].names.len(), 2);
    }

    #[test]
    fn cyclomatic_simple() {
        let result = parse_rust("fn simple() { let x = 1; }");
        let node = &result.nodes[0];
        assert_eq!(node.cyclomatic, Some(1));
    }

    #[test]
    fn cyclomatic_with_if() {
        let result = parse_rust("fn branching(x: bool) { if x { } else { } }");
        let node = &result.nodes[0];
        assert_eq!(node.cyclomatic, Some(2));
    }

    #[test]
    fn skip_external_use() {
        let result = parse_rust("use std::collections::HashMap;");
        assert!(result.imports.is_empty());
    }

    #[test]
    fn visibility_pub_crate() {
        let result = parse_rust("pub(crate) fn internal() {}");
        let node = &result.nodes[0];
        assert!(!node.exported);
        assert_eq!(node.visibility.as_deref(), Some("pub(crate)"));
    }

    #[test]
    fn async_unsafe_detection() {
        let result = parse_rust("pub async unsafe fn danger() {}");
        let node = &result.nodes[0];
        assert_eq!(node.is_async, Some(true));
        assert_eq!(node.is_unsafe, Some(true));
    }

    // ── Import resolution with rel_name ──

    fn parse_rust_as(source: &str, rel_name: &str) -> FileData {
        let parser = RustParser::new();
        parser.parse_source(source, "test.rs", rel_name).unwrap()
    }

    #[test]
    fn use_crate_error_skipped() {
        // `use crate::Error` has no module path — should be filtered out
        let result = parse_rust("use crate::Error;");
        assert!(result.imports.is_empty());
    }

    #[test]
    fn use_crate_group_at_root_skipped() {
        let result = parse_rust("use crate::{Error, Result};");
        assert!(result.imports.is_empty());
    }

    #[test]
    fn use_super_resolves_to_parent() {
        let result = parse_rust_as("use super::enrichment::Enricher;", "domain/config");
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].from, "domain/enrichment");
        assert_eq!(result.imports[0].names, vec!["Enricher"]);
    }

    #[test]
    fn use_super_from_deep_module() {
        let result = parse_rust_as("use super::registry::ParserRegistry;", "infra/parser/rust");
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].from, "infra/parser/registry");
    }

    #[test]
    fn use_super_from_mod_rs() {
        // mod.rs defines the directory module; super goes to its parent
        let result = parse_rust_as("use super::other::Foo;", "domain/mod");
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].from, "other");
    }

    #[test]
    fn use_super_from_top_level() {
        let result = parse_rust_as("use super::sibling::Bar;", "main");
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].from, "sibling");
    }

    #[test]
    fn use_super_group_import() {
        let result = parse_rust_as("use super::types::{Config, Error};", "domain/config");
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].from, "domain/types");
        assert_eq!(result.imports[0].names.len(), 2);
    }

    #[test]
    fn use_self_resolves_to_current() {
        let result = parse_rust_as("use self::sub::Widget;", "ui/panel");
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].from, "ui/panel/sub");
    }

    // ── resolve_super / resolve_self unit tests ──

    #[test]
    fn resolve_super_nested() {
        assert_eq!(resolve_super("domain/config"), "domain");
    }

    #[test]
    fn resolve_super_deep() {
        assert_eq!(resolve_super("infra/parser/rust"), "infra/parser");
    }

    #[test]
    fn resolve_super_mod_rs() {
        assert_eq!(resolve_super("domain/mod"), "");
    }

    #[test]
    fn resolve_super_top_level() {
        assert_eq!(resolve_super("main"), "");
    }

    #[test]
    fn resolve_self_regular_file() {
        assert_eq!(resolve_self("domain/config"), "domain/config");
    }

    #[test]
    fn resolve_self_mod_rs() {
        assert_eq!(resolve_self("domain/mod"), "domain");
    }
}
