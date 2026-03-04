//! Generate `schema/ast-data.schema.json` from Rust types.
//!
//! ```sh
//! cargo run --example generate_schema --features schema
//! ```

fn main() {
    let schema = schemars::schema_for!(codedash_schemas::AstData);
    let json = serde_json::to_string_pretty(&schema).expect("serialize schema");
    print!("{json}");
}
