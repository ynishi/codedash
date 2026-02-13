--[[
  model/binding.lua — Binding: Index × Percept pair

  Core DSL entity. Holds "what to measure" (Index) × "how to perceive" (Percept).
  Positional args are type-dispatched (order-independent).
  percept.name serves as the natural key.

  Usage:
    local bind    = require("codedash.model.binding")
    local index   = require("codedash.model.index")
    local percept = require("codedash.model.percept")

    bind { index "churn" { source = "git_churn_30d" },
           percept "hue" { range = { 240, 0 } } }

    -- normalize override (overrides the Index default)
    bind { idx.churn, pct.hue, normalize = "rank" }
]]

local Index      = require("codedash.model.index")
local Percept    = require("codedash.model.percept")
local Normalizer = require("codedash.model.normalizer")

local M = {}

local BINDING_TAG = {}

local BINDING_MT = {
  __tostring = function(self)
    local src = self.index.name or self.index.field or self.index.kind or "?"
    return string.format("Binding<%s -> %s>", src, self.percept.name)
  end,
}

-- ============================================================
-- Internal builder
-- ============================================================

local function build(spec, label)
  label = label or "bind"

  if type(spec) ~= "table" then
    error(string.format("%s: spec must be a table", label), 3)
  end

  local idx, pct
  for _, v in ipairs(spec) do
    if Index.is_index_def(v) then
      if idx then error(string.format("%s: multiple IndexDef provided", label), 3) end
      idx = v
    elseif Percept.is_percept_def(v) then
      if pct then error(string.format("%s: multiple PerceptDef provided", label), 3) end
      pct = v
    end
  end

  if not idx then
    error(string.format("%s: IndexDef required", label), 3)
  end
  if not pct then
    error(string.format("%s: PerceptDef required", label), 3)
  end

  -- normalize override (optional, string or NormalizerDef)
  local normalize = spec.normalize
  if normalize ~= nil
    and type(normalize) ~= "string"
    and not Normalizer.is_normalizer_def(normalize) then
    error(string.format("%s: normalize must be string or NormalizerDef, got %s", label, type(normalize)), 3)
  end

  return setmetatable({
    _tag      = BINDING_TAG,
    index     = idx,
    percept   = pct,
    normalize = normalize,  -- nil = use index default
  }, BINDING_MT)
end

-- ============================================================
-- DSL entry:
--   bind { IndexDef, PerceptDef }          -- anonymous
--   bind "label" { IndexDef, PerceptDef }  -- with label (optional)
-- ============================================================

setmetatable(M, {
  __call = function(_, first)
    if type(first) == "table" then
      -- bind { idx, pct }
      return build(first, "bind")
    elseif type(first) == "string" then
      -- bind "label" { idx, pct }
      return function(spec)
        return build(spec, string.format("bind '%s'", first))
      end
    else
      error(string.format("bind: expected table or string, got %s", type(first)), 2)
    end
  end,
})

-- ============================================================
-- Introspection
-- ============================================================

--- Natural key: percept.name (unique per binding set)
---@param b table Binding
---@return string
function M.key(b)
  return b.percept.name
end

---@param v any
---@return boolean
function M.is_binding(v)
  return type(v) == "table" and v._tag == BINDING_TAG
end

--- Human-readable source label (for error messages / reports)
---@param binding table Binding
---@return string
function M.source_label(binding)
  return binding.index.name or binding.index.field or binding.index.kind or "?"
end

return M
