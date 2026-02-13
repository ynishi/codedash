--[[
  model/normalizer.lua — Normalizer definition (Smart Constructor)

  Constructed via functions. Validated at definition time.

  Contract:
    stats(values: number[]) → stats_table
    normalizer(stats_table) → function(raw: number) → normalized: [0, 1]

  Usage:
    local normalizer = require("codedash.model.normalizer")
    return normalizer "percentile" {
      stats = function(values) ... end,
      normalizer = function(stats) return function(raw) ... end end,
    }
]]

local M = {}

local NORMALIZER_TAG = {}

-- ============================================================
-- DSL entry: normalizer "name" { stats = fn, normalizer = fn }
-- ============================================================

setmetatable(M, {
  __call = function(_, name)
    if type(name) ~= "string" then
      error(string.format("normalizer: name must be string, got %s", type(name)), 2)
    end
    return function(spec)
      if type(spec) ~= "table" then
        error(string.format("normalizer '%s': spec must be a table", name), 2)
      end
      if type(spec.stats) ~= "function" then
        error(string.format("normalizer '%s': stats must be function, got %s", name, type(spec.stats)), 2)
      end
      if type(spec.normalizer) ~= "function" then
        error(string.format("normalizer '%s': normalizer must be function, got %s", name, type(spec.normalizer)), 2)
      end

      return {
        _tag       = NORMALIZER_TAG,
        name       = name,
        stats      = spec.stats,
        normalizer = spec.normalizer,
      }
    end
  end,
})

-- ============================================================
-- Introspection
-- ============================================================

---@param v any
---@return boolean
function M.is_normalizer_def(v)
  return type(v) == "table" and v._tag == NORMALIZER_TAG
end

return M
