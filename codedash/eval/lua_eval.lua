--[[
  eval/lua_eval.lua — codedash Evaluator

  Backend Contract: (bindings, nodes, context) → Report

  Pipeline:
    Step 1: Compute raw values (direct source or compute function)
    Step 2: Stats per binding
    Step 3: Normalize [0, 1]
    Step 4: Percept mapping (range)
    Step 5: Group by domain (optional)
    Step 6: Build report
]]

local M = {}

-- ============================================================
-- Step 1: Compute raw values
-- ============================================================

local function compute_raw(binding, node)
  return binding.index.resolver(node)
end

-- ============================================================
-- Step 2-4: Stats + Normalize + Percept
-- ============================================================

local function collect_raw_values(binding, nodes)
  local values = {}
  for _, node in ipairs(nodes) do
    local v = compute_raw(binding, node)
    if v ~= nil then
      values[#values + 1] = v
    end
  end
  return values
end


-- ============================================================
-- Step 5: Group by domain
-- ============================================================

local function build_groups(entries, domain_map, bindings)
  local group_map = {}

  for _, entry in ipairs(entries) do
    local domain = domain_map[entry.node.name] or "unknown"
    if not group_map[domain] then
      group_map[domain] = { name = domain, entries = {} }
    end
    local g = group_map[domain]
    g.entries[#g.entries + 1] = entry
  end

  local groups = {}
  local total = #entries

  local excluded_count = 0
  for _, g in pairs(group_map) do
    if g.name == "_excluded" then
      excluded_count = #g.entries
    end
  end

  for _, g in pairs(group_map) do
    if g.name == "_excluded" then goto continue end

    -- Per-binding stats within group
    local index_stats = {}
    for _, b in ipairs(bindings) do
      local ch = b.percept.name
      local vals = {}
      for _, e in ipairs(g.entries) do
        local v = e.index and e.index[ch]
        if v ~= nil then vals[#vals + 1] = v end
      end
      local valid = #vals
      if valid > 0 then
        table.sort(vals)
        local sum = 0
        for _, v in ipairs(vals) do sum = sum + v end
        index_stats[ch] = {
          avg   = sum / valid,
          max   = vals[valid],
          p90   = vals[math.max(1, math.ceil(valid * 0.9))],
          valid = valid,
        }
      end
    end

    groups[#groups + 1] = {
      name    = g.name,
      count   = #g.entries,
      pct     = total > 0 and (#g.entries / total * 100) or 0,
      index   = index_stats,
      entries = g.entries,
    }

    ::continue::
  end

  table.sort(groups, function(a, b) return a.count > b.count end)
  return groups, excluded_count
end

-- ============================================================
-- run: (bindings, nodes, context) → Report
-- ============================================================

---@param bindings table[]  Resolved bindings from settings
---@param nodes table[]     Node[]
---@param context table     { domain_map }
---@return table report
function M.run(bindings, nodes, context)
  context = context or {}
  local domain_map = context.domain_map or {}

  -- Step 1-2: Collect raw values + compute stats per binding
  local binding_data = {}
  for _, b in ipairs(bindings) do
    local raw_values = collect_raw_values(b, nodes)
    local norm_mod = b._resolved_normalizer
    local stats = norm_mod.stats(raw_values)
    binding_data[#binding_data + 1] = {
      binding    = b,
      stats      = stats,
      normalizer = norm_mod.normalizer(stats),
    }
  end

  -- Step 3-4: Normalize + Percept for each node → entries
  local entries = {}
  for _, node in ipairs(nodes) do
    local index = {}
    local percept = {}

    for _, bd in ipairs(binding_data) do
      local raw = compute_raw(bd.binding, node)
      if raw ~= nil then
        local normalized = bd.normalizer(raw)
        local ch = bd.binding.percept.name
        index[ch] = normalized
        percept[ch] = bd.binding.percept.mapper(normalized)
      end
    end

    entries[#entries + 1] = {
      node    = node,
      index   = index,
      percept = percept,
    }
  end

  -- Step 5: Group by domain
  local groups = {}
  local excluded = 0
  if next(domain_map) ~= nil then
    groups, excluded = build_groups(entries, domain_map, bindings)
  end

  -- Step 6: Report
  return {
    entries  = entries,
    groups   = groups,
    total    = #entries,
    excluded = excluded,
    bindings = bindings,
  }
end

return M
