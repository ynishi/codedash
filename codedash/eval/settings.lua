--[[
  eval/settings.lua — Settings loader and merger

  Reads user config, loads preset, merges bindings.
  Returns a resolved Settings table ready for eval.

  Binding construction is handled by the bind DSL.
  This module handles preset loading, merging, and duplicate checks.
  Merge key is percept.name (the Binding's natural key).
]]

local Binding    = require("codedash.model.binding")
local Normalizer = require("codedash.model.normalizer")

local M = {}

-- ============================================================
-- Normalizer resolution
-- ============================================================

local normalizer_catalog  -- lazy-loaded

--- Resolve a normalizer reference (string or NormalizerDef) to a NormalizerDef.
---@param ref string|table  Normalizer name or NormalizerDef object
---@param context string    Error context (e.g. "binding 'hue'")
---@return table NormalizerDef
local function resolve_normalizer(ref, context)
  if Normalizer.is_normalizer_def(ref) then
    return ref
  end

  if type(ref) ~= "string" then
    error(string.format("%s: normalize must be string or NormalizerDef, got %s", context, type(ref)))
  end

  if not normalizer_catalog then
    normalizer_catalog = require("codedash.presets.normalizers")
  end

  local mod = normalizer_catalog[ref]
  if not mod then
    local known = {}
    for k in pairs(normalizer_catalog) do known[#known + 1] = k end
    table.sort(known)
    error(string.format(
      "%s: unknown normalizer '%s' (available: %s)",
      context, ref, table.concat(known, ", ")
    ))
  end

  return mod
end

-- ============================================================
-- Preset loader
-- ============================================================

--- Load a preset by name.
---@param preset_name string  e.g. "codedash:recommended" or "recommended"
---@return table|nil
local function load_preset(preset_name)
  local short = preset_name:match("^codedash:(.+)$") or preset_name
  local ok, preset = pcall(require, "codedash.presets." .. short)
  if not ok then return nil end

  if type(preset) ~= "table" then
    error(string.format("preset '%s': must return a table", preset_name))
  end
  if preset.bindings then
    for i, b in ipairs(preset.bindings) do
      if not Binding.is_binding(b) then
        error(string.format("preset '%s': bindings[%d] must be a Binding", preset_name, i))
      end
    end
  end

  return preset
end

-- ============================================================
-- Merge: user bindings override preset by percept.name
-- ============================================================

---@param preset_bindings table[]  Binding[]
---@param user_bindings table[]|nil  Binding[]
---@return table[] merged
local function merge_bindings(preset_bindings, user_bindings)
  if not user_bindings then
    return preset_bindings
  end

  local user_by_key = {}
  for _, b in ipairs(user_bindings) do
    if Binding.is_binding(b) then
      user_by_key[Binding.key(b)] = b
    end
  end

  -- Preset bindings, with user overrides applied
  local merged = {}
  local seen = {}
  for _, pb in ipairs(preset_bindings) do
    local key = Binding.key(pb)
    local ub = user_by_key[key]
    if ub then
      seen[key] = true
      merged[#merged + 1] = ub
    else
      merged[#merged + 1] = pb
    end
  end

  -- User bindings not in preset (additions)
  for _, ub in ipairs(user_bindings) do
    if Binding.is_binding(ub) then
      local key = Binding.key(ub)
      if not seen[key] then
        merged[#merged + 1] = ub
      end
    end
  end

  return merged
end

-- ============================================================
-- Resolve
-- ============================================================

--- Load and resolve settings
---@param user_config table|nil  User config from .codedash.lua
---@return table Settings
function M.resolve(user_config)
  user_config = user_config or {}

  -- Load preset
  local preset_bindings = {}
  if user_config.extends then
    local preset = load_preset(user_config.extends)
    if preset and preset.bindings then
      preset_bindings = preset.bindings
    end
  end

  -- Merge
  local bindings = merge_bindings(preset_bindings, user_config.bindings)

  -- Duplicate percept check + normalizer resolution
  local seen_percepts = {}
  for _, b in ipairs(bindings) do
    local key = Binding.key(b)
    if seen_percepts[key] then
      error(string.format("percept '%s' used more than once", key))
    end
    seen_percepts[key] = true

    -- Resolve normalizer: string → NormalizerDef
    local ctx = string.format("binding '%s'", key)
    local norm_ref = b.normalize or b.index.normalize
    b._resolved_normalizer = resolve_normalizer(norm_ref, ctx)
  end

  return {
    bindings = bindings,
    domains  = user_config.domains or {},
    exclude  = user_config.exclude or {},
    fallback = user_config.fallback or "unknown",
  }
end

return M
