--[[
  lib/range.lua — Range value object

  Validated {lo, hi} with linear mapper.

  Usage:
    local Range = require("codedash.lib.range")
    local r = Range.new(240, 0)
    r.lo       -- 240
    r.hi       -- 0
    r.mapper(0.5)  -- 120
]]

local M = {}

local RANGE_TAG = {}

--- Create a Range with linear mapper.
---@param lo number
---@param hi number
---@param ctx string|nil  Context label for error messages (e.g. percept name)
---@return table  Range { lo, hi, mapper }
function M.new(lo, hi, ctx)
  local prefix = ctx and string.format("range(%s)", ctx) or "range"
  if type(lo) ~= "number" then
    error(string.format("%s: lo must be number, got %s", prefix, type(lo)), 2)
  end
  if type(hi) ~= "number" then
    error(string.format("%s: hi must be number, got %s", prefix, type(hi)), 2)
  end

  return {
    _tag   = RANGE_TAG,
    lo     = lo,
    hi     = hi,
    mapper = function(normalized)
      return lo + normalized * (hi - lo)
    end,
  }
end

--- Parse a {lo, hi} table into a Range.
---@param tbl table  { [1]=lo, [2]=hi }
---@param ctx string|nil  Context label for error messages
---@return table  Range
function M.from_pair(tbl, ctx)
  local prefix = ctx and string.format("range(%s)", ctx) or "range"
  if type(tbl) ~= "table" or #tbl ~= 2 then
    error(string.format("%s: must be { lo, hi }", prefix), 2)
  end
  return M.new(tbl[1], tbl[2], ctx)
end

---@param v any
---@return boolean
function M.is_range(v)
  return type(v) == "table" and v._tag == RANGE_TAG
end

return M
