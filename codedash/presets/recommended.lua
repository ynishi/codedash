--[[
  presets/recommended.lua — Official recommended preset

  Default set that highlights risky areas in most projects.
  5 bindings, within human perceptual limits.

  Perception model:
    high churn    -> red      (hue)
    many lines    -> large    (size)
    many params   -> thick    (border)
    deep nesting  -> faded    (opacity)
    low coverage  -> dim      (clarity)
]]

local bind = require("codedash.model.binding")
local idx  = require("codedash.presets.indexes")
local pct  = require("codedash.presets.percepts")

return {
  bindings = {
    bind { idx.churn,    pct.hue },
    bind { idx.lines,    pct.size },
    bind { idx.params,   pct.border },
    bind { idx.depth,    pct.opacity },
    bind { idx.coverage, pct.clarity },
  },
}
