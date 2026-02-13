--[[
  eval/loader.lua — JSON data loading + Node construction

  Loads enriched.json (tree-sitter output) and constructs Node[].
  AST facts + minimal derived fields (cyclomatic estimate, exported_score).
  No classify. Domain assignment is handled by classify.lua.
]]

local std      = require("codedash.std")
local node_mod = require("codedash.model.node")

local M = {}

--- Load and parse a JSON file
---@param path string
---@return table
function M.load_json(path)
  local content = std.fs.read_file(path)
  return std.json.decode(content)
end

--- Estimate cyclomatic complexity from raw AST data.
--- NOTE: This is a heuristic (lines/10 + params*0.5), not true McCabe complexity.
--- If the input JSON provides a `cyclomatic` field, it is used as-is instead.
local function estimate_cyclomatic(raw)
  if raw.cyclomatic then return raw.cyclomatic end
  if raw.kind == "interface" or raw.kind == "type_alias" then
    return 1
  end
  local lines = raw.lines or 1
  local params = raw.params or 0
  return math.max(1, math.floor(lines / 10) + math.floor(params * 0.5))
end

--- Build fully-qualified node name
local function build_node_name(file_name, raw_name, is_container, current_parent, raw_depth, parent_depth)
  if not is_container and current_parent and raw_depth > parent_depth then
    return file_name .. "::" .. current_parent .. "." .. raw_name
  end
  return file_name .. "::" .. raw_name
end

--- Convert tree-sitter JSON → Node[]
---@param ast_data table  { files: [...] }
---@return table Node[]
function M.to_nodes(ast_data)
  local nodes = {}

  for _, file in ipairs(ast_data.files) do
    local current_parent = nil
    local parent_depth = 0

    for _, raw in ipairs(file.nodes) do
      local raw_depth = raw.depth or 0
      local is_container = raw.kind == "class" or raw.kind == "interface"

      if is_container then
        current_parent = raw.name
        parent_depth = raw_depth
      elseif raw_depth <= parent_depth then
        current_parent = nil
      end

      local is_exported = raw.exported or false

      local node = node_mod.new({
        name           = build_node_name(file.name, raw.name, is_container, current_parent, raw_depth, parent_depth),
        short_name     = raw.name,
        file           = file.name,
        semantic_type  = raw.kind,

        lines          = raw.lines,
        start_line     = raw.start_line,
        end_line       = raw.end_line,
        depth          = raw_depth,
        params         = raw.params,
        field_count    = raw.field_count,

        exported       = is_exported,
        exported_score = is_exported and 1 or 0,
        visibility     = raw.visibility or (is_exported and "pub" or "private"),

        cyclomatic     = estimate_cyclomatic(raw),
        git_churn_30d  = raw.git_churn_30d,
        coverage       = raw.coverage,
      })

      nodes[#nodes + 1] = node
    end
  end

  return nodes
end

return M
