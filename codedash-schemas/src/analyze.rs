//! Evaluated analysis output schema (`codedash analyze -o json`).
//!
//! These types represent the **evaluated** output from the codedash analysis
//! pipeline — the result after raw AST metrics have been normalized and mapped
//! to visual percept channels (hue, size, border, opacity, clarity).
//!
//! This is distinct from [`crate::AstData`], which captures the raw parse output.
//! `AnalyzeResult` is what downstream consumers (e.g. egui-cha UI components)
//! should depend on for visualization.

use serde::{Deserialize, Serialize};

/// Top-level output from `codedash analyze -o json`.
///
/// Contains evaluated entries with both raw metrics and visual encoding
/// values, plus metadata (bindings, groups, totals).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct AnalyzeResult {
    /// Metric-to-percept bindings defining how code metrics map to visual channels.
    pub bindings: Vec<Binding>,
    /// Evaluated code units with raw metrics and visual encoding values.
    pub entries: Vec<EvalEntry>,
    /// Domain groups with count and percentage.
    #[serde(default)]
    pub groups: Vec<Group>,
    /// Total number of analyzed nodes.
    pub total: u32,
    /// Number of excluded nodes.
    #[serde(default)]
    pub excluded: u32,
}

impl AnalyzeResult {
    /// Create a new [`AnalyzeResult`].
    pub fn new(bindings: Vec<Binding>, entries: Vec<EvalEntry>, total: u32) -> Self {
        Self {
            bindings,
            entries,
            groups: Vec::new(),
            total,
            excluded: 0,
        }
    }
}

/// A binding maps a code metric (index) to a visual channel (percept).
///
/// For example, `{ index: "cyclomatic", percept: "hue" }` means cyclomatic
/// complexity is encoded as the hue channel in the visualization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct Binding {
    /// Metric name (e.g. `"cyclomatic"`, `"lines"`, `"params"`, `"depth"`, `"coverage"`).
    pub index: String,
    /// Visual channel name (e.g. `"hue"`, `"size"`, `"border"`, `"opacity"`, `"clarity"`).
    pub percept: String,
}

impl Binding {
    /// Create a new [`Binding`].
    pub fn new(index: String, percept: String) -> Self {
        Self { index, percept }
    }
}

/// A single evaluated code unit with raw metrics and visual encoding values.
///
/// Combines source identity, raw metrics from AST parsing/enrichment, and
/// the normalized + percept values computed by the eval pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct EvalEntry {
    // ── Source identity ──
    /// Node kind: `"function"`, `"struct"`, `"enum"`, `"impl"`, `"method"`, etc.
    pub kind: String,
    /// Node name (identifier).
    pub name: String,
    /// Fully qualified name (e.g. `"src/app::MyStruct.method"`).
    pub full_name: String,
    /// Source file path (relative, without extension).
    pub file: String,
    /// First line of the node (1-based).
    pub start_line: u32,
    /// Last line of the node (1-based, inclusive).
    pub end_line: u32,
    /// Total line count.
    pub lines: u32,
    /// Whether this node is exported / publicly visible.
    #[serde(default)]
    pub exported: bool,
    /// Visibility qualifier (e.g. `"pub"`, `"private"`).
    #[serde(default)]
    pub visibility: String,

    // ── Raw metrics ──
    /// Number of parameters (functions/methods).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<u32>,
    /// Cyclomatic complexity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cyclomatic: Option<u32>,
    /// Maximum nesting depth.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
    /// Number of fields (structs/enums).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_count: Option<u32>,
    /// Git commit count within the churn period.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub git_churn_30d: Option<u32>,
    /// Region coverage ratio (0.0–1.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<f64>,

    // ── Evaluated values ──
    /// Normalized values (0.0–1.0 range) for each percept channel.
    pub normalized: PerceptValues,
    /// Final percept values after mapping (may exceed 0.0–1.0 depending on the percept).
    pub percept: PerceptValues,
}

impl EvalEntry {
    /// Create a new [`EvalEntry`] with required fields.
    ///
    /// Optional metric fields default to `None`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind: String,
        name: String,
        full_name: String,
        file: String,
        start_line: u32,
        end_line: u32,
        lines: u32,
        normalized: PerceptValues,
        percept: PerceptValues,
    ) -> Self {
        Self {
            kind,
            name,
            full_name,
            file,
            start_line,
            end_line,
            lines,
            exported: false,
            visibility: String::new(),
            params: None,
            cyclomatic: None,
            depth: None,
            field_count: None,
            git_churn_30d: None,
            coverage: None,
            normalized,
            percept,
        }
    }
}

/// Visual encoding values computed by the eval pipeline.
///
/// Each field corresponds to a percept channel. The `hue`, `size`, `border`,
/// and `opacity` channels are always present. The `clarity` channel is only
/// present when coverage data is available.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct PerceptValues {
    /// Hue channel value (mapped from cyclomatic complexity by default).
    #[serde(default)]
    pub hue: f64,
    /// Size channel value (mapped from lines by default).
    #[serde(default)]
    pub size: f64,
    /// Border channel value (mapped from params by default).
    #[serde(default)]
    pub border: f64,
    /// Opacity channel value (mapped from depth by default).
    #[serde(default)]
    pub opacity: f64,
    /// Clarity channel value (mapped from coverage when available).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clarity: Option<f64>,
}

impl PerceptValues {
    /// Create a new [`PerceptValues`] with the four core channels.
    pub fn new(hue: f64, size: f64, border: f64, opacity: f64) -> Self {
        Self {
            hue,
            size,
            border,
            opacity,
            clarity: None,
        }
    }

    /// Create a new [`PerceptValues`] with all channels including clarity.
    pub fn with_clarity(hue: f64, size: f64, border: f64, opacity: f64, clarity: f64) -> Self {
        Self {
            hue,
            size,
            border,
            opacity,
            clarity: Some(clarity),
        }
    }
}

impl Default for PerceptValues {
    fn default() -> Self {
        Self::new(0.0, 0.0, 0.0, 0.0)
    }
}

/// A domain group with count and percentage.
///
/// Groups categorize analyzed nodes by their containing module/domain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct Group {
    /// Group name (e.g. domain/module name).
    pub name: String,
    /// Number of nodes in this group.
    pub count: u32,
    /// Percentage of total nodes (0.0–100.0).
    pub pct: f64,
}

impl Group {
    /// Create a new [`Group`].
    pub fn new(name: String, count: u32, pct: f64) -> Self {
        Self { name, count, pct }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyze_result_roundtrip() {
        let result = AnalyzeResult {
            bindings: vec![
                Binding::new("cyclomatic".into(), "hue".into()),
                Binding::new("lines".into(), "size".into()),
            ],
            entries: vec![EvalEntry {
                kind: "function".into(),
                name: "main".into(),
                full_name: "src/main::main".into(),
                file: "src/main".into(),
                start_line: 1,
                end_line: 10,
                lines: 10,
                exported: false,
                visibility: "private".into(),
                params: Some(0),
                cyclomatic: Some(3),
                depth: Some(2),
                field_count: None,
                git_churn_30d: Some(5),
                coverage: None,
                normalized: PerceptValues::new(0.5, 0.3, 0.1, 0.2),
                percept: PerceptValues::new(60.0, 0.8, 0.5, 0.7),
            }],
            groups: vec![Group::new("src".into(), 10, 50.0)],
            total: 20,
            excluded: 0,
        };

        let json = serde_json::to_string(&result).unwrap();
        let parsed: AnalyzeResult = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.bindings.len(), 2);
        assert_eq!(parsed.bindings[0].index, "cyclomatic");
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].name, "main");
        assert_eq!(parsed.entries[0].cyclomatic, Some(3));
        assert_eq!(parsed.groups.len(), 1);
        assert_eq!(parsed.total, 20);
    }

    #[test]
    fn deserialize_actual_codedash_output_entry() {
        // Matches the actual `codedash analyze -o json` output format
        let json = r#"{
            "bindings": [
                {"index": "cyclomatic", "percept": "hue"},
                {"index": "lines", "percept": "size"},
                {"index": "params", "percept": "border"},
                {"index": "depth", "percept": "opacity"},
                {"index": "coverage", "percept": "clarity"}
            ],
            "entries": [{
                "cyclomatic": 1,
                "depth": 1,
                "end_line": 11,
                "exported": false,
                "field_count": 0,
                "file": "codedash-schemas/examples/generate_schema",
                "full_name": "codedash-schemas/examples/generate_schema::main",
                "git_churn_30d": 2,
                "kind": "function",
                "lines": 5,
                "name": "main",
                "normalized": {"border": 0.346, "hue": 0.0, "opacity": 0.232, "size": 0.071},
                "params": 0,
                "percept": {"border": 1.038, "hue": 120.0, "opacity": 0.790, "size": 0.542},
                "start_line": 7,
                "visibility": "private"
            }],
            "groups": [],
            "total": 437,
            "excluded": 0
        }"#;

        let parsed: AnalyzeResult = serde_json::from_str(json).unwrap();

        assert_eq!(parsed.bindings.len(), 5);
        assert_eq!(parsed.total, 437);
        assert_eq!(parsed.entries.len(), 1);

        let entry = &parsed.entries[0];
        assert_eq!(entry.kind, "function");
        assert_eq!(entry.name, "main");
        assert_eq!(entry.cyclomatic, Some(1));
        assert_eq!(entry.params, Some(0));
        assert_eq!(entry.field_count, Some(0));
        assert_eq!(entry.normalized.hue, 0.0);
        assert!((entry.percept.hue - 120.0).abs() < f64::EPSILON);
        assert!(entry.normalized.clarity.is_none());
    }

    #[test]
    fn deserialize_with_missing_optional_fields() {
        let json = r#"{
            "bindings": [],
            "entries": [{
                "kind": "struct",
                "name": "Foo",
                "full_name": "src/lib::Foo",
                "file": "src/lib",
                "start_line": 1,
                "end_line": 5,
                "lines": 5,
                "normalized": {"hue": 0.0, "size": 0.0, "border": 0.0, "opacity": 0.0},
                "percept": {"hue": 0.0, "size": 0.0, "border": 0.0, "opacity": 0.0}
            }],
            "total": 1
        }"#;

        let parsed: AnalyzeResult = serde_json::from_str(json).unwrap();
        let entry = &parsed.entries[0];

        assert!(!entry.exported);
        assert!(entry.visibility.is_empty());
        assert!(entry.params.is_none());
        assert!(entry.cyclomatic.is_none());
        assert!(entry.coverage.is_none());
        assert_eq!(parsed.excluded, 0);
        assert!(parsed.groups.is_empty());
    }

    #[test]
    fn percept_values_with_clarity() {
        let pv = PerceptValues::with_clarity(120.0, 0.5, 1.0, 0.8, 0.9);
        let json = serde_json::to_string(&pv).unwrap();
        let parsed: PerceptValues = serde_json::from_str(&json).unwrap();

        assert!((parsed.hue - 120.0).abs() < f64::EPSILON);
        assert_eq!(parsed.clarity, Some(0.9));
    }

    #[test]
    fn percept_values_without_clarity_omits_field() {
        let pv = PerceptValues::new(0.5, 0.3, 0.1, 0.2);
        let json = serde_json::to_value(&pv).unwrap();

        assert!(json.get("clarity").is_none());
        assert!(json.get("hue").is_some());
    }

    #[test]
    fn group_serialization() {
        let group = Group::new("domain".into(), 42, 33.5);
        let json = serde_json::to_value(&group).unwrap();

        assert_eq!(json["name"], "domain");
        assert_eq!(json["count"], 42);
        assert!((json["pct"].as_f64().unwrap() - 33.5).abs() < f64::EPSILON);
    }

    #[test]
    fn binding_eq() {
        let a = Binding::new("cyclomatic".into(), "hue".into());
        let b = Binding::new("cyclomatic".into(), "hue".into());
        let c = Binding::new("lines".into(), "size".into());
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn constructors_produce_correct_defaults() {
        let result = AnalyzeResult::new(vec![], vec![], 0);
        assert!(result.groups.is_empty());
        assert_eq!(result.excluded, 0);

        let entry = EvalEntry::new(
            "function".into(),
            "f".into(),
            "mod::f".into(),
            "mod".into(),
            1,
            5,
            5,
            PerceptValues::default(),
            PerceptValues::default(),
        );
        assert!(!entry.exported);
        assert!(entry.visibility.is_empty());
        assert!(entry.params.is_none());
        assert!(entry.coverage.is_none());
    }
}

#[cfg(all(test, feature = "schema"))]
mod schema_snapshot {
    use super::*;

    #[test]
    fn analyze_result_json_schema() {
        let schema = schemars::schema_for!(AnalyzeResult);
        insta::assert_json_snapshot!("analyze-result-schema", schema);
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    fn arb_binding() -> impl Strategy<Value = Binding> {
        (
            prop_oneof!["cyclomatic", "lines", "params", "depth", "coverage"],
            prop_oneof!["hue", "size", "border", "opacity", "clarity"],
        )
            .prop_map(|(index, percept)| Binding { index, percept })
    }

    // Note: f64 fields use integer-derived values to avoid ULP precision
    // loss during JSON roundtrip. Deterministic tests cover fractional f64.
    fn arb_percept_values() -> impl Strategy<Value = PerceptValues> {
        (0i32..360, 0i32..100, 0i32..100, 0i32..100).prop_map(|(hue, size, border, opacity)| {
            PerceptValues {
                hue: f64::from(hue),
                size: f64::from(size) / 100.0,
                border: f64::from(border) / 100.0,
                opacity: f64::from(opacity) / 100.0,
                clarity: None,
            }
        })
    }

    fn arb_eval_entry() -> impl Strategy<Value = EvalEntry> {
        (
            "[a-z]{1,8}",
            "[a-z]{1,8}",
            "[a-z/]{1,15}::[a-z]{1,8}",
            "[a-z/]{1,15}",
            1u32..10000,
            1u32..500,
            any::<bool>(),
            arb_percept_values(),
            arb_percept_values(),
        )
            .prop_map(
                |(kind, name, full_name, file, start, delta, exported, normalized, percept)| {
                    let end = start + delta;
                    let lines = delta + 1;
                    EvalEntry {
                        kind,
                        name,
                        full_name,
                        file,
                        start_line: start,
                        end_line: end,
                        lines,
                        exported,
                        visibility: "private".into(),
                        params: None,
                        cyclomatic: None,
                        depth: None,
                        field_count: None,
                        git_churn_30d: None,
                        coverage: None,
                        normalized,
                        percept,
                    }
                },
            )
    }

    fn arb_group() -> impl Strategy<Value = Group> {
        ("[a-z]{1,8}", 0u32..1000, 0u32..1000).prop_map(|(name, count, pct_raw)| Group {
            name,
            count,
            pct: f64::from(pct_raw) / 10.0,
        })
    }

    fn arb_analyze_result() -> impl Strategy<Value = AnalyzeResult> {
        (
            proptest::collection::vec(arb_binding(), 0..6),
            proptest::collection::vec(arb_eval_entry(), 0..4),
            proptest::collection::vec(arb_group(), 0..4),
            0u32..1000,
            0u32..100,
        )
            .prop_map(
                |(bindings, entries, groups, total, excluded)| AnalyzeResult {
                    bindings,
                    entries,
                    groups,
                    total,
                    excluded,
                },
            )
    }

    proptest! {
        #[test]
        fn analyze_result_serde_roundtrip(data in arb_analyze_result()) {
            let json = serde_json::to_string(&data).unwrap();
            let parsed: AnalyzeResult = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(data, parsed);
        }

        #[test]
        fn eval_entry_serde_roundtrip(entry in arb_eval_entry()) {
            let json = serde_json::to_string(&entry).unwrap();
            let parsed: EvalEntry = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(entry, parsed);
        }

        #[test]
        fn percept_values_serde_roundtrip(pv in arb_percept_values()) {
            let json = serde_json::to_string(&pv).unwrap();
            let parsed: PerceptValues = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(pv, parsed);
        }
    }
}
