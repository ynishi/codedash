--[[
  presets/percepts.lua — Available PerceptDef catalog

  Presets (recommended, minimal, etc.) pick from these to bind.
  User configs can also reference via require("codedash.presets.percepts").
]]

return {
  hue      = require("codedash.presets.percepts.hue"),
  size     = require("codedash.presets.percepts.size"),
  border   = require("codedash.presets.percepts.border"),
  opacity  = require("codedash.presets.percepts.opacity"),
  clarity  = require("codedash.presets.percepts.clarity"),
  weight   = require("codedash.presets.percepts.weight"),
  presence = require("codedash.presets.percepts.presence"),
}
