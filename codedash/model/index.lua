--[[
  model/index.lua — Index definition (Smart Constructor)

  Constructed via functions + composed with combinators.

  Uniform signature: resolver(node) -> number | nil
  All indexes share this shape. source/compute distinction is hidden at build time.

  Usage:
    local index = require("codedash.model.index")
    return index "lines" { source = "lines" }
    return index "custom" { compute = function(node) ... end }
    return index "complexity" {
      combine = { idx_a, idx_b, function(a, b) ... end },
    }
]]

local node_mod   = require("codedash.model.node")
local Normalizer = require("codedash.model.normalizer")

local M = {}

--- Default normalizer name (single source of truth)
M.DEFAULT_NORMALIZER = "percentile"

local INDEX_TAG = {}

-- ============================================================
-- Internal: safe resolver wrapper
-- ============================================================

local function safe_resolver(fn)
  return function(node)
    local ok, val = pcall(fn, node)
    if ok and type(val) == "number" then return val end
    return nil
  end
end

-- ============================================================
-- Internal builders
-- ============================================================

local function build_source(name, field_name, normalize)
  if type(field_name) ~= "string" then
    error(string.format("index '%s': source must be string, got %s", name, type(field_name)), 3)
  end
  local valid, msg = node_mod.validate_source(field_name)
  if not valid then
    error(string.format("index '%s': %s", name, msg), 3)
  end

  return {
    _tag      = INDEX_TAG,
    name      = name,
    resolver  = function(node)
      local val = node[field_name]
      if type(val) == "number" then return val end
      return nil
    end,
    normalize = normalize or M.DEFAULT_NORMALIZER,
    kind      = "source",
    field     = field_name,
  }
end

local function build_compute(name, fn, normalize)
  if type(fn) ~= "function" then
    error(string.format("index '%s': compute must be function, got %s", name, type(fn)), 3)
  end

  return {
    _tag      = INDEX_TAG,
    name      = name,
    resolver  = safe_resolver(fn),
    normalize = normalize or M.DEFAULT_NORMALIZER,
    kind      = "compute",
  }
end

local function build_combine(name, spec, normalize)
  if type(spec) ~= "table" or #spec < 3 then
    error(string.format("index '%s': combine must be { IndexDef, IndexDef, function }", name), 3)
  end
  local idx_a, idx_b, fn = spec[1], spec[2], spec[3]

  if not M.is_index_def(idx_a) then
    error(string.format("index '%s': combine[1] must be IndexDef", name), 3)
  end
  if not M.is_index_def(idx_b) then
    error(string.format("index '%s': combine[2] must be IndexDef", name), 3)
  end
  if type(fn) ~= "function" then
    error(string.format("index '%s': combine[3] must be function, got %s", name, type(fn)), 3)
  end

  local resolver_a = idx_a.resolver
  local resolver_b = idx_b.resolver

  return {
    _tag      = INDEX_TAG,
    name      = name,
    resolver  = safe_resolver(function(node)
      local a = resolver_a(node)
      local b = resolver_b(node)
      if a == nil or b == nil then return nil end
      return fn(a, b)
    end),
    normalize = normalize or M.DEFAULT_NORMALIZER,
    kind      = "combine",
  }
end

-- ============================================================
-- DSL entry: index "name" { source = "field" }
-- ============================================================

setmetatable(M, {
  __call = function(_, name)
    if type(name) ~= "string" then
      error(string.format("index: name must be string, got %s", type(name)), 2)
    end
    return function(spec)
      if type(spec) ~= "table" then
        error(string.format("index '%s': spec must be a table", name), 2)
      end
      local normalize = spec.normalize
      if normalize ~= nil
        and type(normalize) ~= "string"
        and not Normalizer.is_normalizer_def(normalize) then
        error(string.format("index '%s': normalize must be string or NormalizerDef, got %s", name, type(normalize)), 2)
      end

      if spec.source then
        return build_source(name, spec.source, normalize)
      elseif spec.compute then
        return build_compute(name, spec.compute, normalize)
      elseif spec.combine then
        return build_combine(name, spec.combine, normalize)
      end
      error(string.format("index '%s': spec requires 'source', 'compute', or 'combine'", name), 2)
    end
  end,
})

-- ============================================================
-- Layer 3: Programmatic combinators (for settings.lua etc.)
-- ============================================================

--- Map: transform the output of a single Index
---@param idx table  IndexDef
---@param fn function(v: number) -> number
---@param opts table|nil  { normalize?: string }
---@return table IndexDef
function M.map(idx, fn, opts)
  opts = opts or {}

  if not M.is_index_def(idx) then
    error("Index.map: first argument must be IndexDef", 2)
  end
  if type(fn) ~= "function" then
    error(string.format("Index.map: transform must be function, got %s", type(fn)), 2)
  end

  local resolver_src = idx.resolver

  return {
    _tag      = INDEX_TAG,
    name      = idx.name,
    resolver  = safe_resolver(function(node)
      local v = resolver_src(node)
      if v == nil then return nil end
      return fn(v)
    end),
    normalize = opts.normalize or idx.normalize,
    kind      = "map",
  }
end

--- Create a new IndexDef with a different normalizer name.
---@param idx table  IndexDef
---@param norm_name string  normalizer name
---@return table IndexDef
function M.with_normalize(idx, norm_name)
  if not M.is_index_def(idx) then
    error("Index.with_normalize: first argument must be IndexDef", 2)
  end
  if type(norm_name) ~= "string" then
    error(string.format("Index.with_normalize: name must be string, got %s", type(norm_name)), 2)
  end

  return {
    _tag      = INDEX_TAG,
    name      = idx.name,
    resolver  = idx.resolver,
    normalize = norm_name,
    kind      = idx.kind,
    field     = idx.field,
  }
end

-- ============================================================
-- Introspection
-- ============================================================

---@param v any
---@return boolean
function M.is_index_def(v)
  return type(v) == "table" and v._tag == INDEX_TAG
end

return M
