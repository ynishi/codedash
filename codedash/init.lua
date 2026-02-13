--[[
  codedash — Public API entry point

  Usage:
    local codedash = require("codedash")
    local instance = codedash.init("enriched.json", config)
    local report   = instance:run()

  Re-exports core types for DSL authoring:
    codedash.index     — Index smart constructor
    codedash.percept   — Percept smart constructor
    codedash.bind      — Binding smart constructor
    codedash.report    — Report formatting utilities
]]

local M = {}

-- Pipeline
M.init   = require("codedash.eval.init").init
M.report = require("codedash.eval.report")

-- DSL constructors (for custom configs)
M.index      = require("codedash.model.index")
M.percept    = require("codedash.model.percept")
M.bind       = require("codedash.model.binding")
M.normalizer = require("codedash.model.normalizer")

-- Preset catalogs
M.indexes     = require("codedash.presets.indexes")
M.percepts    = require("codedash.presets.percepts")
M.normalizers = require("codedash.presets.normalizers")

return M
