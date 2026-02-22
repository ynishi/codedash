//! TypeScript/TSX language parser using tree-sitter-typescript.
//!
//! Reimplements parse_tsx.js functionality:
//! - Extracts function, class, interface, enum, type_alias, component, hook nodes
//! - Detects arrow function components (PascalCase) and hooks (useXxx)
//! - Computes cyclomatic complexity for functions
//! - Extracts import statements
//! - Detects visibility (public/private/protected) for class members

use tree_sitter::{Node, Parser};

use crate::domain::ast::{CallInfo, FileData, ImportInfo, NodeData};
use crate::port::parser::LanguageParser;
use crate::Error;

pub struct TypeScriptParser;

impl Default for TypeScriptParser {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeScriptParser {
    pub fn new() -> Self {
        Self
    }

    fn create_parser(tsx: bool) -> Result<Parser, Error> {
        let mut parser = Parser::new();
        let language = if tsx {
            tree_sitter_typescript::LANGUAGE_TSX
        } else {
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT
        };
        parser
            .set_language(&language.into())
            .map_err(|e| Error::Parse(format!("failed to set TypeScript language: {e}")))?;
        Ok(parser)
    }
}

impl LanguageParser for TypeScriptParser {
    fn name(&self) -> &str {
        "typescript"
    }

    fn extensions(&self) -> &[&str] {
        &["ts", "tsx"]
    }

    fn parse_source(
        &self,
        source: &str,
        file_path: &str,
        rel_name: &str,
    ) -> Result<FileData, Error> {
        let tsx = file_path.ends_with(".tsx") || file_path.ends_with(".jsx");
        let mut parser = Self::create_parser(tsx)?;
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

        visit_node(tree.root_node(), source, 0, false, &mut file_data);
        resolve_calls(source, &file_data.imports, &mut file_data.nodes);

        Ok(file_data)
    }
}

// ── AST visitor ──────────────────────────────────

fn visit_node(node: Node, source: &str, depth: usize, exported: bool, file_data: &mut FileData) {
    let is_export = node.kind() == "export_statement";
    let child_exported = exported || is_export;

    if let Some(items) = extract_node(node, source, depth, child_exported) {
        for item in items {
            match item {
                Extracted::Node(n) => file_data.nodes.push(*n),
                Extracted::Import(imp) => file_data.imports.push(imp),
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_node(child, source, depth + 1, child_exported, file_data);
    }
}

enum Extracted {
    Node(Box<NodeData>),
    Import(ImportInfo),
}

fn extract_node(node: Node, source: &str, depth: usize, exported: bool) -> Option<Vec<Extracted>> {
    match node.kind() {
        "import_statement" => extract_import(node, source).map(|imp| vec![Extracted::Import(imp)]),

        "function_declaration" => {
            let name = name_or_anon(node, source);
            let mut n = base_node("function", name, exported, node, depth);
            n.is_async = Some(has_child_kind(node, "async"));
            n.params = Some(count_params(node));
            n.cyclomatic = Some(compute_cyclomatic(node, source));
            Some(vec![Extracted::Node(Box::new(n))])
        }

        "method_definition" => {
            let name = name_or_anon(node, source);
            let vis = get_accessibility(node, source);
            let mut n = base_node("method", name, false, node, depth);
            n.visibility = Some(vis);
            n.is_async = Some(has_child_kind(node, "async"));
            n.params = Some(count_params(node));
            n.cyclomatic = Some(compute_cyclomatic(node, source));
            Some(vec![Extracted::Node(Box::new(n))])
        }

        "class_declaration" => {
            let name = name_or_anon(node, source);
            let n = base_node("class", name, exported, node, depth);
            Some(vec![Extracted::Node(Box::new(n))])
        }

        "interface_declaration" => {
            let name = name_or_anon(node, source);
            let fc = node
                .child_by_field_name("body")
                .map(|b| count_interface_fields(b))
                .unwrap_or(0);
            let mut n = base_node("interface", name, exported, node, depth);
            n.field_count = Some(fc);
            Some(vec![Extracted::Node(Box::new(n))])
        }

        "type_alias_declaration" => {
            let name = name_or_anon(node, source);
            let n = base_node("type_alias", name, exported, node, depth);
            Some(vec![Extracted::Node(Box::new(n))])
        }

        "enum_declaration" => {
            let name = name_or_anon(node, source);
            let fc = node
                .child_by_field_name("body")
                .map(|b| count_enum_members(b))
                .unwrap_or(0);
            let mut n = base_node("enum", name, exported, node, depth);
            n.field_count = Some(fc);
            Some(vec![Extracted::Node(Box::new(n))])
        }

        // Arrow function components, hooks, stores, variables
        "lexical_declaration" if depth <= 2 => extract_lexical(node, source, depth, exported),

        _ => None,
    }
}

/// Extract variable declarations: arrow functions, components, hooks, stores.
fn extract_lexical(
    node: Node,
    source: &str,
    depth: usize,
    exported: bool,
) -> Option<Vec<Extracted>> {
    let mut results = Vec::new();
    let mut cursor = node.walk();

    for child in node.named_children(&mut cursor) {
        if child.kind() != "variable_declarator" {
            continue;
        }

        let var_name = child
            .child_by_field_name("name")
            .map(|n| node_text(n, source))
            .unwrap_or_else(|| "(anonymous)".to_string());

        let value = child.child_by_field_name("value");

        match value {
            Some(value_node) if matches!(value_node.kind(), "arrow_function" | "function") => {
                let kind = classify_function_name(&var_name);
                let mut n = base_node(kind, var_name, exported, node, depth);
                n.is_async = Some(has_child_kind(value_node, "async"));
                n.params = Some(count_params(value_node));
                n.cyclomatic = Some(compute_cyclomatic(value_node, source));
                results.push(Extracted::Node(Box::new(n)));
            }
            Some(value_node) if value_node.kind() == "call_expression" => {
                let callee = value_node
                    .child_by_field_name("function")
                    .map(|f| node_text(f, source))
                    .unwrap_or_default();
                let kind = match callee.as_str() {
                    "create" | "createStore" => "store",
                    "memo" => "component",
                    "createContext" => "context",
                    _ => "variable",
                };
                let n = base_node(kind, var_name, exported, node, depth);
                results.push(Extracted::Node(Box::new(n)));
            }
            _ => {
                let n = base_node("variable", var_name, exported, node, depth);
                results.push(Extracted::Node(Box::new(n)));
            }
        }
    }

    if results.is_empty() {
        None
    } else {
        Some(results)
    }
}

/// Classify a variable name: PascalCase → component, useXxx → hook, else function.
fn classify_function_name(name: &str) -> &'static str {
    if let Some(rest) = name.strip_prefix("use") {
        if rest.starts_with(|c: char| c.is_ascii_uppercase()) {
            return "hook";
        }
    }
    if name.starts_with(|c: char| c.is_ascii_uppercase()) {
        "component"
    } else {
        "function"
    }
}

// ── Node construction ────────────────────────────

fn base_node(kind: &str, name: String, exported: bool, node: Node, depth: usize) -> NodeData {
    NodeData {
        kind: kind.to_string(),
        name,
        exported,
        visibility: None,
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

fn name_or_anon(node: Node, source: &str) -> String {
    node.child_by_field_name("name")
        .map(|n| node_text(n, source))
        .unwrap_or_else(|| "(anonymous)".to_string())
}

fn node_text(node: Node, source: &str) -> String {
    source
        .get(node.start_byte()..node.end_byte())
        .unwrap_or("")
        .to_string()
}

fn has_child_kind(node: Node, kind: &str) -> bool {
    let mut cursor = node.walk();
    let result = node.children(&mut cursor).any(|c| c.kind() == kind);
    result
}

fn get_accessibility(node: Node, source: &str) -> String {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "accessibility_modifier" {
            return node_text(child, source);
        }
    }
    "public".to_string()
}

fn count_params(node: Node) -> usize {
    let params = match node.child_by_field_name("parameters") {
        Some(p) => p,
        None => return 0,
    };
    let mut cursor = params.walk();
    params
        .named_children(&mut cursor)
        .filter(|c| {
            matches!(
                c.kind(),
                "required_parameter" | "optional_parameter" | "rest_parameter"
            )
        })
        .count()
}

fn count_interface_fields(body: Node) -> usize {
    let mut cursor = body.walk();
    body.named_children(&mut cursor)
        .filter(|c| matches!(c.kind(), "property_signature" | "method_signature"))
        .count()
}

fn count_enum_members(body: Node) -> usize {
    let mut cursor = body.walk();
    body.named_children(&mut cursor)
        .filter(|c| matches!(c.kind(), "property_identifier" | "enum_assignment"))
        .count()
}

// ── Cyclomatic complexity ────────────────────────

fn compute_cyclomatic(fn_node: Node, source: &str) -> usize {
    let body = match fn_node.child_by_field_name("body") {
        Some(b) => b,
        None => return 1,
    };
    let mut complexity: i32 = 1;
    walk_cyclomatic(body, source, &mut complexity);
    complexity.max(1) as usize
}

fn walk_cyclomatic(node: Node, source: &str, complexity: &mut i32) {
    match node.kind() {
        "if_statement" | "while_statement" | "for_statement" | "for_in_statement"
        | "do_statement" | "catch_clause" => {
            *complexity += 1;
        }
        "switch_case" => {
            // Only count non-default cases
            if node.child_by_field_name("value").is_some() {
                *complexity += 1;
            }
        }
        "ternary_expression" => {
            *complexity += 1;
        }
        "binary_expression" => {
            if let Some(op) = node.child_by_field_name("operator") {
                let op_text = node_text(op, source);
                if op_text == "&&" || op_text == "||" || op_text == "??" {
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

// ── Import parsing ───────────────────────────────

fn extract_import(node: Node, source: &str) -> Option<ImportInfo> {
    // Find the module path (string node)
    let source_node = find_child_by_kind(node, "string")?;
    let module_path = node_text(source_node, source)
        .trim_matches(|c| c == '\'' || c == '"')
        .to_string();

    let mut names = Vec::new();

    if let Some(clause) = find_child_by_kind(node, "import_clause") {
        // Default import: import Foo from "..."
        if let Some(id) = find_child_by_kind(clause, "identifier") {
            names.push(node_text(id, source));
        }

        // Named imports: import { Foo, Bar } from "..."
        collect_named_imports(clause, source, &mut names);

        // Namespace import: import * as X from "..."
        collect_namespace_imports(clause, source, &mut names);
    }

    Some(ImportInfo {
        from: module_path,
        names,
    })
}

fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    let result = node.children(&mut cursor).find(|c| c.kind() == kind);
    result
}

fn collect_named_imports(clause: Node, source: &str, names: &mut Vec<String>) {
    let mut stack = vec![clause];
    while let Some(current) = stack.pop() {
        if current.kind() == "import_specifier" {
            // Prefer alias: import { Foo as Bar } → Bar
            let name_node = current
                .child_by_field_name("alias")
                .or_else(|| current.child_by_field_name("name"));
            if let Some(n) = name_node {
                names.push(node_text(n, source));
            }
            continue;
        }
        let mut cursor = current.walk();
        for child in current.children(&mut cursor) {
            stack.push(child);
        }
    }
}

fn collect_namespace_imports(clause: Node, source: &str, names: &mut Vec<String>) {
    let mut stack = vec![clause];
    while let Some(current) = stack.pop() {
        if current.kind() == "namespace_import" {
            if let Some(id) = find_child_by_kind(current, "identifier") {
                names.push(node_text(id, source));
            }
            continue;
        }
        let mut cursor = current.walk();
        for child in current.children(&mut cursor) {
            stack.push(child);
        }
    }
}

// ── Call resolution ──────────────────────────────

fn resolve_calls(source: &str, imports: &[ImportInfo], nodes: &mut [NodeData]) {
    let mut symbol_map: Vec<(&str, &str)> = Vec::new();
    for imp in imports {
        for name in &imp.names {
            symbol_map.push((name.as_str(), imp.from.as_str()));
        }
    }

    if symbol_map.is_empty() {
        return;
    }

    let callable_kinds = ["function", "method", "component", "hook", "store"];

    for node in nodes.iter_mut() {
        if !callable_kinds.contains(&node.kind.as_str()) {
            continue;
        }

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

fn extract_body_text(source: &str, start_line: usize, end_line: usize) -> &str {
    if start_line == 0 || end_line == 0 || start_line > end_line {
        return "";
    }
    let start = line_start_offset(source, start_line);
    let end = line_end_offset(source, end_line);
    if start >= end {
        return "";
    }
    &source[start..end]
}

fn line_start_offset(source: &str, line: usize) -> usize {
    if line <= 1 {
        return 0;
    }
    source
        .match_indices('\n')
        .nth(line - 2)
        .map(|(i, _)| i + 1)
        .unwrap_or(source.len())
}

fn line_end_offset(source: &str, line: usize) -> usize {
    if line == 0 {
        return 0;
    }
    source
        .match_indices('\n')
        .nth(line - 1)
        .map(|(i, _)| i)
        .unwrap_or(source.len())
}

fn count_symbol_references(body: &str, symbol: &str) -> usize {
    let mut count = 0;
    // Function call: symbol(
    count += count_pattern(body, symbol, "(");
    // JSX tag: <Symbol  or <Symbol>  or <Symbol/
    count += count_jsx_usage(body, symbol);
    // Property access: symbol.
    count += count_pattern(body, symbol, ".");
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

fn count_jsx_usage(body: &str, symbol: &str) -> usize {
    // Match <Symbol followed by whitespace, >, or /
    let pattern = format!("<{symbol}");
    let mut count = 0;
    let mut haystack = body;
    while let Some(pos) = haystack.find(&pattern) {
        let after = pos + pattern.len();
        if let Some(next_ch) = haystack[after..].chars().next() {
            if next_ch.is_whitespace() || next_ch == '>' || next_ch == '/' {
                count += 1;
            }
        }
        haystack = &haystack[after..];
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ts(source: &str) -> FileData {
        let parser = TypeScriptParser::new();
        parser.parse_source(source, "test.ts", "test").unwrap()
    }

    fn parse_tsx(source: &str) -> FileData {
        let parser = TypeScriptParser::new();
        parser.parse_source(source, "test.tsx", "test").unwrap()
    }

    #[test]
    fn parse_function_declaration() {
        let result = parse_ts("export function greet(name: string): string { return name; }");
        let fns: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.kind == "function")
            .collect();
        assert_eq!(fns.len(), 1);
        assert_eq!(fns[0].name, "greet");
        assert!(fns[0].exported);
        assert_eq!(fns[0].params, Some(1));
        assert_eq!(fns[0].cyclomatic, Some(1));
    }

    #[test]
    fn parse_async_function() {
        let result = parse_ts("export async function fetchData(url: string) { return url; }");
        let node = &result.nodes.iter().find(|n| n.kind == "function").unwrap();
        assert_eq!(node.is_async, Some(true));
    }

    #[test]
    fn parse_class() {
        let result = parse_ts("export class MyService { }");
        let cls: Vec<_> = result.nodes.iter().filter(|n| n.kind == "class").collect();
        assert_eq!(cls.len(), 1);
        assert_eq!(cls[0].name, "MyService");
        assert!(cls[0].exported);
    }

    #[test]
    fn parse_interface() {
        let result = parse_ts(
            "export interface User {\n  name: string;\n  age: number;\n  greet(): void;\n}",
        );
        let iface = result.nodes.iter().find(|n| n.kind == "interface").unwrap();
        assert_eq!(iface.name, "User");
        assert_eq!(iface.field_count, Some(3));
    }

    #[test]
    fn parse_type_alias() {
        let result = parse_ts("export type ID = string | number;");
        let ta = result
            .nodes
            .iter()
            .find(|n| n.kind == "type_alias")
            .unwrap();
        assert_eq!(ta.name, "ID");
        assert!(ta.exported);
    }

    #[test]
    fn parse_enum() {
        let result = parse_ts("export enum Direction { Up, Down, Left, Right }");
        let e = result.nodes.iter().find(|n| n.kind == "enum").unwrap();
        assert_eq!(e.name, "Direction");
        assert_eq!(e.field_count, Some(4));
    }

    #[test]
    fn parse_arrow_component() {
        let result = parse_tsx("export const Button = (props: Props) => { return <div />; }");
        let comp = result.nodes.iter().find(|n| n.kind == "component").unwrap();
        assert_eq!(comp.name, "Button");
        assert!(comp.exported);
        assert_eq!(comp.params, Some(1));
    }

    #[test]
    fn parse_hook() {
        let result = parse_ts("export const useAuth = () => { return {}; }");
        let hook = result.nodes.iter().find(|n| n.kind == "hook").unwrap();
        assert_eq!(hook.name, "useAuth");
    }

    #[test]
    fn parse_method_visibility() {
        let result =
            parse_ts("class Foo {\n  private secret(): void {}\n  public open(): void {}\n}");
        let methods: Vec<_> = result.nodes.iter().filter(|n| n.kind == "method").collect();
        assert_eq!(methods.len(), 2);
        let private_m = methods.iter().find(|m| m.name == "secret").unwrap();
        assert_eq!(private_m.visibility.as_deref(), Some("private"));
        let public_m = methods.iter().find(|m| m.name == "open").unwrap();
        assert_eq!(public_m.visibility.as_deref(), Some("public"));
    }

    #[test]
    fn parse_import() {
        let result = parse_ts("import { useState, useEffect } from 'react';");
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].from, "react");
        assert!(result.imports[0].names.contains(&"useState".to_string()));
        assert!(result.imports[0].names.contains(&"useEffect".to_string()));
    }

    #[test]
    fn parse_default_import() {
        let result = parse_ts("import React from 'react';");
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].from, "react");
        assert!(result.imports[0].names.contains(&"React".to_string()));
    }

    #[test]
    fn parse_namespace_import() {
        let result = parse_ts("import * as Utils from './utils';");
        assert_eq!(result.imports.len(), 1);
        assert_eq!(result.imports[0].from, "./utils");
        assert!(result.imports[0].names.contains(&"Utils".to_string()));
    }

    #[test]
    fn cyclomatic_simple() {
        let result = parse_ts("function simple() { return 1; }");
        let node = result.nodes.iter().find(|n| n.kind == "function").unwrap();
        assert_eq!(node.cyclomatic, Some(1));
    }

    #[test]
    fn cyclomatic_with_branches() {
        let result = parse_ts(
            "function complex(x: number) { if (x > 0) { return 1; } else if (x < 0) { return -1; } return 0; }",
        );
        let node = result.nodes.iter().find(|n| n.kind == "function").unwrap();
        // base 1 + 2 if_statements = 3
        assert_eq!(node.cyclomatic, Some(3));
    }

    #[test]
    fn cyclomatic_with_logical_ops() {
        let result = parse_ts("function check(a: boolean, b: boolean) { return a && b || !a; }");
        let node = result.nodes.iter().find(|n| n.kind == "function").unwrap();
        // base 1 + && + || = 3
        assert_eq!(node.cyclomatic, Some(3));
    }

    #[test]
    fn non_exported_function() {
        let result = parse_ts("function internal() { }");
        let node = result.nodes.iter().find(|n| n.kind == "function").unwrap();
        assert!(!node.exported);
    }

    #[test]
    fn variable_declaration() {
        let result = parse_ts("export const MAX = 100;");
        let var = result.nodes.iter().find(|n| n.kind == "variable").unwrap();
        assert_eq!(var.name, "MAX");
        assert!(var.exported);
    }
}
