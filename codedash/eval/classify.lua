--[[
  eval/classify.lua — Domain classification (pure function)

  Returns a domain_map (node index → domain name).
  Does NOT mutate nodes.
]]

local M = {}

--- Escape a literal string for Lua pattern matching.
---@param s string
---@return string
local function escape_pattern(s)
  return s:gsub("%%", "%%%%"):gsub("([%.%-%+%[%]%(%)%$%^])", "%%%1")
end

--- Test if a pattern matches any of the node's identifiers
---@param node table
---@param escaped string
---@return boolean
local function matches_node(node, escaped)
  return node.name:find(escaped) ~= nil
    or (node.file and node.file:find(escaped) ~= nil)
    or (node.short_name and node.short_name:find(escaped) ~= nil)
end

--- Classify a single node → domain name
---@param config table  { domains, exclude, fallback }
---@param node table
---@return string domain name
function M.classify_node(config, node)
  for _, pat in ipairs(config.exclude or {}) do
    local escaped = escape_pattern(pat)
    if node.name:find(escaped) or (node.file and node.file:find(escaped)) then
      return "_excluded"
    end
  end

  for _, dom in ipairs(config.domains or {}) do
    for _, pat in ipairs(dom.patterns) do
      if matches_node(node, escape_pattern(pat)) then
        return dom.name
      end
    end
  end

  return config.fallback or "unknown"
end

--- Build domain_map for all nodes (pure function, no mutation)
---@param config table  { domains, exclude, fallback }
---@param nodes table   Node[]
---@return table domain_map  { [node.name] = domain_name }
function M.build_domain_map(config, nodes)
  local domain_map = {}
  for _, node in ipairs(nodes) do
    domain_map[node.name] = M.classify_node(config, node)
  end
  return domain_map
end

return M
