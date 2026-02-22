//! __rustlib table construction for Lua injection.
//!
//! Builds the global `__rustlib` table that codedash/std.lua detects
//! to use Rust-provided implementations instead of lua-cjson / io.open.

use std::sync::Arc;

use crate::app::analyze::AnalyzePipeline;
use crate::Error;

/// Build the `__rustlib` table and set it as a Lua global.
///
/// The table provides:
/// - `__rustlib.json.decode(str)` → Lua table
/// - `__rustlib.json.encode(table)` → string
/// - `__rustlib.fs.read_file(path)` → string
/// - `__rustlib.fs.file_exists(path)` → boolean
/// - `__rustlib.analyze(path, lang)` → string (enriched JSON)
pub fn inject_rustlib(lua: &mlua::Lua, pipeline: Arc<AnalyzePipeline>) -> Result<(), Error> {
    let rustlib = lua.create_table().map_err(lua_err)?;

    // json module (senl already provides this via senl.json, but we also
    // inject it into __rustlib for codedash std.lua compatibility)
    let json_table = lua.create_table().map_err(lua_err)?;

    let decode = lua
        .create_function(|lua, s: String| {
            let value: serde_json::Value =
                serde_json::from_str(&s).map_err(mlua::Error::external)?;
            json_value_to_lua(lua, &value)
        })
        .map_err(lua_err)?;

    let encode = lua
        .create_function(|_lua, tbl: mlua::Value| {
            let value = lua_value_to_json(&tbl)?;
            serde_json::to_string(&value).map_err(mlua::Error::external)
        })
        .map_err(lua_err)?;

    json_table.set("decode", decode).map_err(lua_err)?;
    json_table.set("encode", encode).map_err(lua_err)?;
    rustlib.set("json", json_table).map_err(lua_err)?;

    // fs module
    let fs_table = lua.create_table().map_err(lua_err)?;

    let read_file = lua
        .create_function(|_, path: String| {
            std::fs::read_to_string(&path)
                .map_err(|e| mlua::Error::external(format!("std.fs.read_file: {e}")))
        })
        .map_err(lua_err)?;

    let file_exists = lua
        .create_function(|_, path: String| Ok(std::path::Path::new(&path).exists()))
        .map_err(lua_err)?;

    fs_table.set("read_file", read_file).map_err(lua_err)?;
    fs_table.set("file_exists", file_exists).map_err(lua_err)?;
    rustlib.set("fs", fs_table).map_err(lua_err)?;

    // analyze function
    let analyze_fn = lua
        .create_function(move |_, (path, lang): (String, String)| {
            let config =
                crate::domain::config::AnalyzeConfig::new(std::path::PathBuf::from(&path), lang);
            pipeline
                .run(&config)
                .map_err(|e| mlua::Error::external(e.to_string()))
        })
        .map_err(lua_err)?;

    rustlib.set("analyze", analyze_fn).map_err(lua_err)?;

    // Set global
    lua.globals().set("__rustlib", rustlib).map_err(lua_err)?;

    Ok(())
}

fn lua_err(e: mlua::Error) -> Error {
    Error::Lua(e.to_string())
}

/// Convert serde_json::Value to mlua::Value.
fn json_value_to_lua(lua: &mlua::Lua, value: &serde_json::Value) -> mlua::Result<mlua::Value> {
    match value {
        serde_json::Value::Null => Ok(mlua::Value::Nil),
        serde_json::Value::Bool(b) => Ok(mlua::Value::Boolean(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(mlua::Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(mlua::Value::Number(f))
            } else {
                Err(mlua::Error::external("unsupported JSON number type"))?
            }
        }
        serde_json::Value::String(s) => lua.create_string(s).map(mlua::Value::String),
        serde_json::Value::Array(arr) => {
            let tbl = lua.create_table()?;
            for (i, v) in arr.iter().enumerate() {
                tbl.set(i + 1, json_value_to_lua(lua, v)?)?;
            }
            Ok(mlua::Value::Table(tbl))
        }
        serde_json::Value::Object(obj) => {
            let tbl = lua.create_table()?;
            for (k, v) in obj {
                tbl.set(k.as_str(), json_value_to_lua(lua, v)?)?;
            }
            Ok(mlua::Value::Table(tbl))
        }
    }
}

/// Check whether a Lua table is a pure array (sequential integer keys 1..n).
fn is_lua_array(tbl: &mlua::Table) -> mlua::Result<bool> {
    let raw_len = tbl.raw_len();
    if raw_len == 0 {
        // Could be an empty table — treat as object unless no keys exist at all.
        // Check if there are any keys; if none, it's an empty array `[]`.
        let has_any: bool = tbl.pairs::<mlua::Value, mlua::Value>().next().is_some();
        return Ok(!has_any);
    }
    // Verify total key count matches raw_len (no extra string/hash keys).
    let mut total_keys: usize = 0;
    for pair in tbl.pairs::<mlua::Value, mlua::Value>() {
        let _ = pair?;
        total_keys += 1;
    }
    Ok(total_keys == raw_len)
}

/// Convert mlua::Value → serde_json::Value.
fn lua_value_to_json(value: &mlua::Value) -> mlua::Result<serde_json::Value> {
    match value {
        mlua::Value::Nil => Ok(serde_json::Value::Null),
        mlua::Value::Boolean(b) => Ok(serde_json::json!(*b)),
        mlua::Value::Integer(i) => Ok(serde_json::json!(*i)),
        mlua::Value::Number(n) => Ok(serde_json::json!(*n)),
        mlua::Value::String(s) => {
            let str_val = s.to_str()?.to_string();
            Ok(serde_json::Value::String(str_val))
        }
        mlua::Value::Table(tbl) => {
            if is_lua_array(tbl)? {
                let len = tbl.raw_len();
                let mut arr = Vec::with_capacity(len);
                for i in 1..=len {
                    let v: mlua::Value = tbl.raw_get(i)?;
                    arr.push(lua_value_to_json(&v)?);
                }
                Ok(serde_json::Value::Array(arr))
            } else {
                let mut map = serde_json::Map::new();
                for pair in tbl.pairs::<String, mlua::Value>() {
                    let (k, v) = pair?;
                    map.insert(k, lua_value_to_json(&v)?);
                }
                Ok(serde_json::Value::Object(map))
            }
        }
        _ => Ok(serde_json::Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_lua<F: FnOnce(&mlua::Lua)>(f: F) {
        let lua = mlua::Lua::new();
        f(&lua);
    }

    // ── json_value_to_lua ──

    #[test]
    fn json_null_to_lua_nil() {
        with_lua(|lua| {
            let val = json_value_to_lua(lua, &serde_json::Value::Null).unwrap();
            assert!(matches!(val, mlua::Value::Nil));
        });
    }

    #[test]
    fn json_bool_to_lua() {
        with_lua(|lua| {
            let val = json_value_to_lua(lua, &serde_json::json!(true)).unwrap();
            assert_eq!(val, mlua::Value::Boolean(true));
        });
    }

    #[test]
    fn json_integer_to_lua() {
        with_lua(|lua| {
            let val = json_value_to_lua(lua, &serde_json::json!(42)).unwrap();
            assert_eq!(val, mlua::Value::Integer(42));
        });
    }

    #[test]
    fn json_float_to_lua() {
        with_lua(|lua| {
            let val = json_value_to_lua(lua, &serde_json::json!(3.14)).unwrap();
            if let mlua::Value::Number(n) = val {
                assert!((n - 3.14).abs() < f64::EPSILON);
            } else {
                panic!("expected Number");
            }
        });
    }

    #[test]
    fn json_string_to_lua() {
        with_lua(|lua| {
            let val = json_value_to_lua(lua, &serde_json::json!("hello")).unwrap();
            if let mlua::Value::String(s) = val {
                assert_eq!(s.to_str().unwrap(), "hello");
            } else {
                panic!("expected String");
            }
        });
    }

    #[test]
    fn json_array_to_lua_table() {
        with_lua(|lua| {
            let val = json_value_to_lua(lua, &serde_json::json!([1, 2, 3])).unwrap();
            if let mlua::Value::Table(tbl) = val {
                assert_eq!(tbl.raw_len(), 3);
                let v: i64 = tbl.raw_get(2).unwrap();
                assert_eq!(v, 2);
            } else {
                panic!("expected Table");
            }
        });
    }

    #[test]
    fn json_object_to_lua_table() {
        with_lua(|lua| {
            let val = json_value_to_lua(lua, &serde_json::json!({"key": "value"})).unwrap();
            if let mlua::Value::Table(tbl) = val {
                let v: String = tbl.get("key").unwrap();
                assert_eq!(v, "value");
            } else {
                panic!("expected Table");
            }
        });
    }

    // ── lua_value_to_json ──

    #[test]
    fn lua_nil_to_json_null() {
        let result = lua_value_to_json(&mlua::Value::Nil).unwrap();
        assert_eq!(result, serde_json::Value::Null);
    }

    #[test]
    fn lua_bool_to_json() {
        let result = lua_value_to_json(&mlua::Value::Boolean(false)).unwrap();
        assert_eq!(result, serde_json::json!(false));
    }

    #[test]
    fn lua_integer_to_json() {
        let result = lua_value_to_json(&mlua::Value::Integer(99)).unwrap();
        assert_eq!(result, serde_json::json!(99));
    }

    #[test]
    fn lua_number_to_json() {
        let result = lua_value_to_json(&mlua::Value::Number(2.5)).unwrap();
        assert_eq!(result, serde_json::json!(2.5));
    }

    #[test]
    fn lua_array_table_to_json_array() {
        with_lua(|lua| {
            let tbl = lua.create_table().unwrap();
            tbl.raw_set(1, 10).unwrap();
            tbl.raw_set(2, 20).unwrap();
            let result = lua_value_to_json(&mlua::Value::Table(tbl)).unwrap();
            assert_eq!(result, serde_json::json!([10, 20]));
        });
    }

    #[test]
    fn lua_object_table_to_json_object() {
        with_lua(|lua| {
            let tbl = lua.create_table().unwrap();
            tbl.set("a", 1).unwrap();
            tbl.set("b", 2).unwrap();
            let result = lua_value_to_json(&mlua::Value::Table(tbl)).unwrap();
            assert!(result.is_object());
            assert_eq!(result["a"], serde_json::json!(1));
            assert_eq!(result["b"], serde_json::json!(2));
        });
    }

    #[test]
    fn lua_empty_table_to_json_empty_array() {
        with_lua(|lua| {
            let tbl = lua.create_table().unwrap();
            let result = lua_value_to_json(&mlua::Value::Table(tbl)).unwrap();
            assert_eq!(result, serde_json::json!([]));
        });
    }

    #[test]
    fn lua_mixed_table_to_json_object() {
        with_lua(|lua| {
            let tbl = lua.create_table().unwrap();
            tbl.raw_set(1, "first").unwrap();
            tbl.set("key", "value").unwrap();
            let result = lua_value_to_json(&mlua::Value::Table(tbl)).unwrap();
            // Mixed table should be treated as object (is_lua_array returns false)
            assert!(result.is_object());
        });
    }

    // ── is_lua_array ──

    #[test]
    fn is_lua_array_pure_sequence() {
        with_lua(|lua| {
            let tbl = lua.create_table().unwrap();
            tbl.raw_set(1, "a").unwrap();
            tbl.raw_set(2, "b").unwrap();
            assert!(is_lua_array(&tbl).unwrap());
        });
    }

    #[test]
    fn is_lua_array_with_string_keys_false() {
        with_lua(|lua| {
            let tbl = lua.create_table().unwrap();
            tbl.set("key", "val").unwrap();
            assert!(!is_lua_array(&tbl).unwrap());
        });
    }

    #[test]
    fn is_lua_array_empty_is_true() {
        with_lua(|lua| {
            let tbl = lua.create_table().unwrap();
            assert!(is_lua_array(&tbl).unwrap());
        });
    }

    // ── roundtrip ──

    #[test]
    fn roundtrip_json_to_lua_to_json() {
        with_lua(|lua| {
            let original = serde_json::json!({
                "name": "test",
                "count": 42,
                "items": [1, 2, 3],
                "nested": {"a": true}
            });
            let lua_val = json_value_to_lua(lua, &original).unwrap();
            let back = lua_value_to_json(&lua_val).unwrap();
            assert_eq!(original, back);
        });
    }
}
