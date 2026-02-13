--[[
  presets/indexes.lua — Available IndexDef catalog

  Presets (recommended, minimal, etc.) pick from these to bind.
  User configs can also reference via require("codedash.presets.indexes").
]]

return {
  churn          = require("codedash.presets.indexes.churn"),
  lines          = require("codedash.presets.indexes.lines"),
  params         = require("codedash.presets.indexes.params"),
  depth          = require("codedash.presets.indexes.depth"),
  coverage       = require("codedash.presets.indexes.coverage"),
  cyclomatic     = require("codedash.presets.indexes.cyclomatic"),
  field_count    = require("codedash.presets.indexes.field_count"),
  exported_score = require("codedash.presets.indexes.exported_score"),
  complexity     = require("codedash.presets.indexes.complexity"),
}
