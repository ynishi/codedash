--[[
  presets/normalizers.lua — Available NormalizerDef catalog

  Presets and user configs reference these by object:
    local nrm = require("codedash.presets.normalizers")
    bind { idx.churn, pct.hue, normalize = nrm.rank }
]]

return {
  percentile = require("codedash.presets.normalizers.percentile"),
  rank       = require("codedash.presets.normalizers.rank"),
}
