# codedash-schemas

Stable public schema types for [codedash](https://github.com/ynishi/codedash) code metrics output.

## Overview

This crate provides the **external contract** for codedash's analysis JSON output.
Internal domain model changes in codedash are absorbed by an Anti-Corruption Layer (ACL),
so this schema remains stable for downstream consumers.

```text
                          codedash (binary/lib)
                         ┌──────────────────────────────────┐
                         │  domain/ast.rs                    │
                         │  (internal model — may change)    │
                         │              │                    │
                         │              ▼                    │
                         │  port/schema.rs  (ACL: From impls)│
                         │              │                    │
                         └──────────────┼────────────────────┘
                                        ▼
                         ┌──────────────────────────────────┐
                         │  codedash-schemas                 │
                         │  (public contract — stable)       │
                         └──────────────────────────────────┘
                                        │
                    ┌───────────────────┼───────────────────┐
                    ▼                   ▼                   ▼
              Rust consumers      JSON consumers      GUI (egui-cha etc.)
              (serde deser)       (JSON Schema)       (Rust SDK)
```

## Design: Anti-Corruption Layer (ACL)

codedash follows DDD's Anti-Corruption Layer pattern to decouple the public schema from the internal domain model:

- **`domain/ast.rs`** (codedash internal) — Internal types optimized for parsing and enrichment logic. May change freely across codedash versions.
- **`port/schema.rs`** (codedash internal) — The ACL boundary. `From` trait implementations map domain types to schema types. This is the **only** place where the two models touch.
- **`codedash-schemas`** (this crate) — Stable public types. Versioned independently. Breaking changes follow semver.

This separation ensures:

1. codedash can refactor its internals without breaking consumers
2. The JSON output format has an explicit, documented contract
3. Schema types carry minimal dependencies (`serde` only, no tree-sitter/git2/mlua)

## Usage

### Rust consumers

Add to `Cargo.toml`:

```toml
[dependencies]
codedash-schemas = "0.1"
serde_json = "1"
```

Deserialize codedash's JSON output:

```rust
use codedash_schemas::AstData;

let json = std::fs::read_to_string("analysis.json")?;
let data: AstData = serde_json::from_str(&json)?;

for file in &data.files {
    for node in &file.nodes {
        if let Some(cc) = node.cyclomatic {
            println!("{}/{}: cyclomatic={}", file.name, node.name, cc);
        }
    }
}
```

### JSON Schema (non-Rust consumers)

A pre-generated JSON Schema file is available at:

```
codedash-schemas/schema/ast-data.schema.json
```

Non-Rust consumers (TypeScript, Python, Go, etc.) can use this file for validation or type generation:

- **TypeScript**: generate interfaces via `json-schema-to-typescript`
- **Python**: generate Pydantic models via `datamodel-code-generator`
- **Go**: generate structs via `go-jsonschema`

The schema file is kept up-to-date by CI. When a PR touches `codedash-schemas/`, GitHub Actions regenerates the JSON Schema and auto-commits any diff.

#### Generating from Rust

Enable the `schema` feature to derive `schemars::JsonSchema` on all types.

```toml
[dependencies]
codedash-schemas = { version = "0.1", features = ["schema"] }
schemars = "1"
```

```rust
let schema = schemars::schema_for!(codedash_schemas::AstData);
let json = serde_json::to_string_pretty(&schema).unwrap();
std::fs::write("ast-data.schema.json", json).unwrap();
```

To regenerate locally, run the bundled example:

```sh
cargo run -p codedash-schemas --example generate_schema --features schema \
  > codedash-schemas/schema/ast-data.schema.json
```

## Types

| Type | Description |
|------|-------------|
| `AstData` | Top-level output: files + dependency edges |
| `FileData` | Per-file data: path, module name, AST nodes, imports |
| `NodeData` | Single AST node: function, struct, enum, impl, etc. |
| `Edge` | Dependency edge between two files |
| `ImportInfo` | An internal import statement |
| `CallInfo` | A function call reference |

## Optional features

| Feature | Dependencies | Description |
|---------|-------------|-------------|
| `schema` | `schemars` | Derives `JsonSchema` on all types for JSON Schema generation |

## Minimum Supported Rust Version

1.80

## License

MIT
