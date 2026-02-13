--[[
  model/node.lua — Node: immutable AST-derived data

  Node = structural facts about code. Not mutated after construction.
  Classification results are NOT written back to Node.

  Lifecycle:
    1. Load   — Node.new(raw) from AST JSON   (eval/loader.lua)

  Backend Contract:
    FIELDS maps 1:1 to the Rust struct for future eval backends.
]]

local M = {}

--- Field schema: AST-derived data only
M.FIELDS = {
  -- Identity
  name           = { type = "string",  required = true },
  short_name     = { type = "string",  required = true },
  file           = { type = "string",  required = true },
  semantic_type  = { type = "string",  required = true },

  -- Structural metrics
  lines          = { type = "number",  default = 1 },
  start_line     = { type = "number",  default = 0 },
  end_line       = { type = "number",  default = 0 },
  depth          = { type = "number",  default = 0 },
  params         = { type = "number",  default = 0 },
  field_count    = { type = "number",  default = 0 },
  cyclomatic     = { type = "number",  default = 1 },

  -- Visibility
  exported       = { type = "boolean", default = false },
  exported_score = { type = "number",  default = 0 },
  visibility     = { type = "string",  default = "private" },

  -- External data
  git_churn_30d  = { type = "number",  default = 0 },
  coverage       = { type = "number",  nullable = true },
}

-- ============================================================
-- Identity tag
-- ============================================================

local NODE_TAG = {}

-- ============================================================
-- Constructor
-- ============================================================

--- Construct a Node from raw field values.
---@param raw table  Raw field values (from loader)
---@return table Node
function M.new(raw)
  assert(type(raw) == "table", "Node.new: argument must be a table")

  local node = { _tag = NODE_TAG }
  local ctx = tostring(raw.name or raw.short_name or "?")

  for field_name, spec in pairs(M.FIELDS) do
    local val = raw[field_name]

    if val == nil then
      if spec.required then
        error(string.format("Node: required field '%s' is nil (%s)", field_name, ctx))
      end
      val = spec.default  -- nil if no default (nullable fields)
    elseif type(val) ~= spec.type then
      error(string.format("Node: field '%s' expected %s, got %s (%s)",
        field_name, spec.type, type(val), ctx))
    end

    node[field_name] = val
  end

  return setmetatable(node, {
    __tostring = function(self)
      return string.format("Node<%s %s>",
        self.semantic_type or "?",
        self.short_name or self.name or "?")
    end,
  })
end

-- ============================================================
-- Introspection
-- ============================================================

--- List all numeric field names (valid Index sources)
---@return string[]
function M.numeric_fields()
  local names = {}
  for name, spec in pairs(M.FIELDS) do
    if spec.type == "number" then
      names[#names + 1] = name
    end
  end
  table.sort(names)
  return names
end

--- Validate that a field name exists and is numeric (usable as Index source).
---@param source_name string
---@return boolean ok
---@return string|nil error_message
function M.validate_source(source_name)
  local spec = M.FIELDS[source_name]
  if not spec then
    return false, string.format("unknown field '%s' — not in Node.FIELDS", source_name)
  end
  if spec.type ~= "number" then
    return false, string.format(
      "field '%s' is %s, not number — cannot use as Index source", source_name, spec.type)
  end
  return true, nil
end

---@param v any
---@return boolean
function M.is_node(v)
  return type(v) == "table" and v._tag == NODE_TAG
end

return M
