--[[
  eval/init.lua — Init(Codebase, Settings).Run()

  Orchestrates the codedash pipeline:
    1. Load nodes
    2. Resolve settings (extends + merge)
    3. Build domain_map
    4. Run eval
    5. Return report
]]

local loader   = require("codedash.eval.loader")
local classify = require("codedash.eval.classify")
local settings = require("codedash.eval.settings")
local lua_eval = require("codedash.eval.lua_eval")

local M = {}

local Eye = {}
Eye.__index = Eye

--- Init: load codebase + resolve settings
---@param source string  Path to enriched JSON
---@param config table   User config (.codedash.lua contents)
---@return table Eye instance
function M.init(source, config)
  config = config or {}
  local self = setmetatable({}, Eye)

  -- 1. Load
  local ast_data = loader.load_json(source)
  self._nodes = loader.to_nodes(ast_data)

  -- 2. Resolve settings
  self._settings = settings.resolve(config)

  -- 3. Domain map
  if #self._settings.domains > 0 then
    self._domain_map = classify.build_domain_map({
      domains  = self._settings.domains,
      exclude  = self._settings.exclude,
      fallback = self._settings.fallback,
    }, self._nodes)
  else
    self._domain_map = {}
  end

  return self
end

--- Run: execute eval pipeline → report
---@return table report
function Eye:run()
  return lua_eval.run(self._settings.bindings, self._nodes, {
    domain_map = self._domain_map,
  })
end

--- Convenience: node count
function Eye:count()
  return #self._nodes
end

return M
