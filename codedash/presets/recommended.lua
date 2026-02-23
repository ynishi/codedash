--[[
  presets/recommended.lua — Official recommended preset

  Default set that highlights risky areas in most projects.
  5 bindings, within human perceptual limits.

  Perception model:
    high complexity -> red, low -> green  (hue)
    many lines      -> large    (size)
    many params     -> thick    (border)
    deep nesting    -> faded    (opacity)
    low coverage    -> dim      (clarity)

  Normalizer choices:
    - cyclomatic, lines: percentile (outlier-resistant, good natural spread)
    - params, depth: rank (skewed distributions need rank-based spread)
    - coverage: percentile (when data available)
]]

local bind  = require("codedash.model.binding")
local Index = require("codedash.model.index")
local idx   = require("codedash.presets.indexes")
local pct   = require("codedash.presets.percepts")

return {
  bindings = {
    bind { idx.cyclomatic,                       pct.hue },
    bind { idx.lines,                            pct.size },
    bind { Index.with_normalize(idx.params, "rank"), pct.border },
    bind { Index.with_normalize(idx.depth,  "rank"), pct.opacity },
    bind { idx.coverage,                         pct.clarity },
  },
}
