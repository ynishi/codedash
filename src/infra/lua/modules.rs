//! Embedded codedash Lua modules via include_str!.
//!
//! These are registered as preloaded modules via senl's with_preload_dir().
//! Path format follows senl's convention: "init.lua" → base, "sub/file.lua" → base.sub.file.

/// All codedash Lua files as (relative_path, source) pairs.
pub const CODEDASH_FILES: &[(&str, &str)] = &[
    // Entry point
    ("init.lua", include_str!("../../../codedash/init.lua")),
    // Std lib shim
    ("std.lua", include_str!("../../../codedash/std.lua")),
    // Lib
    (
        "lib/range.lua",
        include_str!("../../../codedash/lib/range.lua"),
    ),
    // Model
    (
        "model/binding.lua",
        include_str!("../../../codedash/model/binding.lua"),
    ),
    (
        "model/index.lua",
        include_str!("../../../codedash/model/index.lua"),
    ),
    (
        "model/node.lua",
        include_str!("../../../codedash/model/node.lua"),
    ),
    (
        "model/normalizer.lua",
        include_str!("../../../codedash/model/normalizer.lua"),
    ),
    (
        "model/percept.lua",
        include_str!("../../../codedash/model/percept.lua"),
    ),
    // Eval
    (
        "eval/init.lua",
        include_str!("../../../codedash/eval/init.lua"),
    ),
    (
        "eval/classify.lua",
        include_str!("../../../codedash/eval/classify.lua"),
    ),
    (
        "eval/loader.lua",
        include_str!("../../../codedash/eval/loader.lua"),
    ),
    (
        "eval/lua_eval.lua",
        include_str!("../../../codedash/eval/lua_eval.lua"),
    ),
    (
        "eval/report.lua",
        include_str!("../../../codedash/eval/report.lua"),
    ),
    (
        "eval/settings.lua",
        include_str!("../../../codedash/eval/settings.lua"),
    ),
    // Presets
    (
        "presets/recommended.lua",
        include_str!("../../../codedash/presets/recommended.lua"),
    ),
    (
        "presets/indexes.lua",
        include_str!("../../../codedash/presets/indexes.lua"),
    ),
    (
        "presets/percepts.lua",
        include_str!("../../../codedash/presets/percepts.lua"),
    ),
    (
        "presets/normalizers.lua",
        include_str!("../../../codedash/presets/normalizers.lua"),
    ),
    // Preset indexes
    (
        "presets/indexes/churn.lua",
        include_str!("../../../codedash/presets/indexes/churn.lua"),
    ),
    (
        "presets/indexes/complexity.lua",
        include_str!("../../../codedash/presets/indexes/complexity.lua"),
    ),
    (
        "presets/indexes/coverage.lua",
        include_str!("../../../codedash/presets/indexes/coverage.lua"),
    ),
    (
        "presets/indexes/cyclomatic.lua",
        include_str!("../../../codedash/presets/indexes/cyclomatic.lua"),
    ),
    (
        "presets/indexes/depth.lua",
        include_str!("../../../codedash/presets/indexes/depth.lua"),
    ),
    (
        "presets/indexes/exported_score.lua",
        include_str!("../../../codedash/presets/indexes/exported_score.lua"),
    ),
    (
        "presets/indexes/field_count.lua",
        include_str!("../../../codedash/presets/indexes/field_count.lua"),
    ),
    (
        "presets/indexes/lines.lua",
        include_str!("../../../codedash/presets/indexes/lines.lua"),
    ),
    (
        "presets/indexes/params.lua",
        include_str!("../../../codedash/presets/indexes/params.lua"),
    ),
    // Preset normalizers
    (
        "presets/normalizers/percentile.lua",
        include_str!("../../../codedash/presets/normalizers/percentile.lua"),
    ),
    (
        "presets/normalizers/rank.lua",
        include_str!("../../../codedash/presets/normalizers/rank.lua"),
    ),
    // Preset percepts
    (
        "presets/percepts/border.lua",
        include_str!("../../../codedash/presets/percepts/border.lua"),
    ),
    (
        "presets/percepts/clarity.lua",
        include_str!("../../../codedash/presets/percepts/clarity.lua"),
    ),
    (
        "presets/percepts/hue.lua",
        include_str!("../../../codedash/presets/percepts/hue.lua"),
    ),
    (
        "presets/percepts/opacity.lua",
        include_str!("../../../codedash/presets/percepts/opacity.lua"),
    ),
    (
        "presets/percepts/presence.lua",
        include_str!("../../../codedash/presets/percepts/presence.lua"),
    ),
    (
        "presets/percepts/size.lua",
        include_str!("../../../codedash/presets/percepts/size.lua"),
    ),
    (
        "presets/percepts/weight.lua",
        include_str!("../../../codedash/presets/percepts/weight.lua"),
    ),
];
