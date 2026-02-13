--[[
  model/percept.lua — Percept definition (Smart Constructor)

  Constructed via functions. Validated at definition time.

  Uniform signature: mapper(normalized) -> value
  All percepts share this shape. Range validation is delegated to lib.range.

  Usage:
    local percept = require("codedash.model.percept")
    return percept "hue" { range = { 240, 0 } }
    return percept "size" { range = { 0.2, 5.0 } }
    return percept "hue" { range = { 240, 0 }, steps = 5 }  -- 5-level discrete
]]

local Range = require("codedash.lib.range")

local M = {}

local PERCEPT_TAG = {}

-- ============================================================
-- Internal builder
-- ============================================================

--- Build a PerceptDef from name + range spec + optional steps.
local function build(name, range_pair, steps)
  local r = Range.from_pair(range_pair, name)

  if steps ~= nil then
    if type(steps) ~= "number" or steps < 2 or steps ~= math.floor(steps) then
      error(string.format("percept '%s': steps must be integer >= 2, got %s", name, tostring(steps)), 3)
    end
    local base_mapper = r.mapper
    local n = steps - 1
    return {
      _tag   = PERCEPT_TAG,
      name   = name,
      steps  = steps,
      mapper = function(normalized)
        local quantized = math.floor(normalized * n + 0.5) / n
        return base_mapper(quantized)
      end,
    }
  end

  return {
    _tag   = PERCEPT_TAG,
    name   = name,
    mapper = r.mapper,
  }
end

-- ============================================================
-- DSL entry: percept "name" { range = { lo, hi } }
-- ============================================================

setmetatable(M, {
  __call = function(_, name)
    if type(name) ~= "string" then
      error(string.format("percept: name must be string, got %s", type(name)), 2)
    end
    return function(spec)
      if type(spec) ~= "table" then
        error(string.format("percept '%s': spec must be a table", name), 2)
      end
      if spec.range then
        return build(name, spec.range, spec.steps)
      end
      error(string.format("percept '%s': spec requires 'range' (e.g. { range = {0, 1} })", name), 2)
    end
  end,
})

-- ============================================================
-- Introspection
-- ============================================================

---@param v any
---@return boolean
function M.is_percept_def(v)
  return type(v) == "table" and v._tag == PERCEPT_TAG
end

return M
