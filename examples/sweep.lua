--[[
  examples/sweep.lua — Binding sweep experiment

  Generates all Index × Normalizer combinations, measures differentiation,
  and recommends the binding set that maximizes information per visual channel.

  "Definitions are data, so they can be generated and swept."

  Usage:
    lua examples/sweep.lua                              -- sample data
    lua examples/sweep.lua path/to/enriched.json        -- custom data
    lua examples/sweep.lua path/to/enriched.json 3      -- top 3 bindings
]]

-- ================================================================
-- Setup
-- ================================================================
local info = debug.getinfo(1, "S")
local example_dir = info.source:match("^@(.*/)") or "./"
package.path = package.path
  .. ";" .. example_dir .. "../?.lua"
  .. ";" .. example_dir .. "../?/init.lua"

local loader     = require("codedash.eval.loader")
local idx_cat    = require("codedash.presets.indexes")
local norm_cat   = require("codedash.presets.normalizers")

-- ================================================================
-- Args
-- ================================================================
local source      = arg[1] or (example_dir .. "sample_enriched.json")
local top_n       = tonumber(arg[2]) or 5

-- ================================================================
-- Load nodes (once)
-- ================================================================
local ast_data = loader.load_json(source)
local nodes    = loader.to_nodes(ast_data)

io.write(string.format("Loaded %d nodes from %s\n\n", #nodes, source))

-- ================================================================
-- Stat helpers
-- ================================================================

--- Standard deviation of values in [0,1]
local function stddev(vals)
  local n = #vals
  if n < 2 then return 0 end
  local sum = 0
  for _, v in ipairs(vals) do sum = sum + v end
  local mean = sum / n
  local sq = 0
  for _, v in ipairs(vals) do sq = sq + (v - mean) ^ 2 end
  return math.sqrt(sq / (n - 1))
end

--- Interquartile range
local function iqr(sorted_vals)
  local n = #sorted_vals
  if n < 4 then return 0 end
  local q1 = sorted_vals[math.ceil(n * 0.25)]
  local q3 = sorted_vals[math.ceil(n * 0.75)]
  return q3 - q1
end

--- Count of distinct bins when quantized to 10 levels
local function unique_bins(vals)
  local seen = {}
  for _, v in ipairs(vals) do
    local bin = math.min(9, math.floor(v * 10))
    seen[bin] = true
  end
  local count = 0
  for _ in pairs(seen) do count = count + 1 end
  return count
end

--- Pearson correlation coefficient
local function pearson(xs, ys)
  local n = math.min(#xs, #ys)
  if n < 3 then return 0 end
  local sx, sy = 0, 0
  for i = 1, n do sx = sx + xs[i]; sy = sy + ys[i] end
  local mx, my = sx / n, sy / n
  local cov, vx, vy = 0, 0, 0
  for i = 1, n do
    local dx, dy = xs[i] - mx, ys[i] - my
    cov = cov + dx * dy
    vx  = vx + dx * dx
    vy  = vy + dy * dy
  end
  if vx == 0 or vy == 0 then return 0 end
  return cov / math.sqrt(vx * vy)
end

--- Star rating for spread
local function spread_stars(sd)
  if sd >= 0.25 then return "***" end
  if sd >= 0.15 then return "** " end
  return "*  "
end

-- ================================================================
-- Phase 1: Index × Normalizer spread
-- ================================================================

-- Collect all index names (sorted)
local index_names = {}
for name in pairs(idx_cat) do
  index_names[#index_names + 1] = name
end
table.sort(index_names)

-- Collect all normalizer names (sorted)
local norm_names = {}
for name in pairs(norm_cat) do
  norm_names[#norm_names + 1] = name
end
table.sort(norm_names)

-- For each (index, normalizer): compute normalized values per node
-- Store: candidates[] = { index_name, norm_name, sd, iqr, bins, valid_pct, per_node }
local candidates = {}

-- per_node_map[index_name..":"..norm_name] = { [node_idx] = normalized }
local per_node_map = {}

for _, idx_name in ipairs(index_names) do
  local idx = idx_cat[idx_name]

  -- Collect raw values
  local raws = {}   -- { node_idx, value }
  for i, node in ipairs(nodes) do
    local v = idx.resolver(node)
    if v ~= nil then
      raws[#raws + 1] = { idx = i, val = v }
    end
  end

  local valid_pct = #raws / #nodes * 100

  for _, norm_name in ipairs(norm_names) do
    local norm = norm_cat[norm_name]

    -- Build stats from raw values
    local raw_list = {}
    for _, r in ipairs(raws) do raw_list[#raw_list + 1] = r.val end
    local stats = norm.stats(raw_list)
    local normalize_fn = norm.normalizer(stats)

    -- Normalize each node
    local normalized = {}
    local per_node = {}
    for _, r in ipairs(raws) do
      local nv = normalize_fn(r.val)
      normalized[#normalized + 1] = nv
      per_node[r.idx] = nv
    end

    table.sort(normalized)

    local sd  = stddev(normalized)
    local iq  = iqr(normalized)
    local bins = unique_bins(normalized)

    local key = idx_name .. ":" .. norm_name
    per_node_map[key] = per_node

    candidates[#candidates + 1] = {
      index_name = idx_name,
      norm_name  = norm_name,
      sd         = sd,
      iqr        = iq,
      bins       = bins,
      valid_pct  = valid_pct,
      key        = key,
    }
  end
end

-- Sort by stddev descending
table.sort(candidates, function(a, b) return a.sd > b.sd end)

-- Print Phase 1
print("=== Phase 1: Index x Normalizer Spread ===")
print(string.format("  %-20s %-12s %7s %7s %7s %7s",
  "Index", "Normalizer", "stddev", "IQR", "bins/10", "valid%"))
print(string.format("  %s", string.rep("-", 74)))
for _, c in ipairs(candidates) do
  print(string.format("  %-20s %-12s %7.3f %7.3f %5d   %5.0f%%  %s",
    c.index_name, c.norm_name, c.sd, c.iqr, c.bins, c.valid_pct,
    spread_stars(c.sd)))
end
print()

-- ================================================================
-- Phase 2: Correlation matrix (best normalizer per index)
-- ================================================================

-- Pick best normalizer per index (highest stddev)
local best_per_index = {}
for _, c in ipairs(candidates) do
  local prev = best_per_index[c.index_name]
  if not prev or c.sd > prev.sd then
    best_per_index[c.index_name] = c
  end
end

-- Sorted list of best candidates
local best_list = {}
for _, name in ipairs(index_names) do
  if best_per_index[name] then
    best_list[#best_list + 1] = best_per_index[name]
  end
end
table.sort(best_list, function(a, b) return a.sd > b.sd end)

-- Build per-node vectors aligned by node index
local function aligned_vectors(c1, c2)
  local pn1 = per_node_map[c1.key]
  local pn2 = per_node_map[c2.key]
  local xs, ys = {}, {}
  for i = 1, #nodes do
    if pn1[i] and pn2[i] then
      xs[#xs + 1] = pn1[i]
      ys[#ys + 1] = pn2[i]
    end
  end
  return xs, ys
end

-- Compute correlation matrix
local corr_matrix = {}  -- [i][j] = pearson
for i, ci in ipairs(best_list) do
  corr_matrix[i] = {}
  for j, cj in ipairs(best_list) do
    if i == j then
      corr_matrix[i][j] = 1.0
    elseif j < i then
      corr_matrix[i][j] = corr_matrix[j][i]
    else
      local xs, ys = aligned_vectors(ci, cj)
      corr_matrix[i][j] = pearson(xs, ys)
    end
  end
end

-- Print Phase 2
print("=== Phase 2: Correlation Matrix (best normalizer per index) ===")

-- Header
local hdr = string.format("  %-14s", "")
for _, c in ipairs(best_list) do
  hdr = hdr .. string.format(" %8s", c.index_name:sub(1, 8))
end
print(hdr)
print(string.format("  %s", string.rep("-", 14 + #best_list * 9)))

for i, ci in ipairs(best_list) do
  local row = string.format("  %-14s", ci.index_name)
  for j = 1, #best_list do
    if j > i then
      row = row .. "         "
    else
      local r = corr_matrix[i][j]
      local mark = ""
      if i ~= j and math.abs(r) >= 0.7 then mark = "!" end
      row = row .. string.format(" %7.2f%s", r, mark)
    end
  end
  print(row)
end
print()
print("  (!) = |r| >= 0.7 — high correlation, likely redundant pair")
print()

-- ================================================================
-- Phase 3: Greedy best-N selection
-- ================================================================

-- Percepts in salience order (most noticeable first)
local percept_order = { "hue", "size", "border", "opacity", "clarity", "weight", "presence" }

local selected = {}
local selected_set = {}

for pick = 1, math.min(top_n, #best_list) do
  local best_score = -1
  local best_idx   = nil

  for i, ci in ipairs(best_list) do
    if not selected_set[i] then
      -- Spread component (stddev weighted by bin diversity)
      -- bins/10 in [0.1, 1.0]: penalizes binary-like distributions
      local bin_factor = math.max(0.1, ci.bins / 10)
      local spread_score = ci.sd * (0.5 + 0.5 * bin_factor)

      -- Valid data penalty: indexes with <50% valid nodes are less reliable
      local valid_factor = ci.valid_pct >= 50 and 1.0 or (ci.valid_pct / 50)
      spread_score = spread_score * valid_factor

      -- Independence component: max |correlation| with already-selected
      local max_corr = 0
      for _, sel in ipairs(selected) do
        local r = math.abs(corr_matrix[i][sel.matrix_idx])
        if r > max_corr then max_corr = r end
      end

      -- Score: spread * independence
      -- First pick has no correlation penalty
      local score
      if #selected == 0 then
        score = spread_score
      else
        score = spread_score * (1 - max_corr)
      end

      if score > best_score then
        best_score = score
        best_idx   = i
      end
    end
  end

  if best_idx then
    selected_set[best_idx] = true
    local ci = best_list[best_idx]
    local percept_name = percept_order[pick] or ("ch" .. pick)

    -- Max correlation with other selected
    local max_corr = 0
    local corr_with = ""
    for _, sel in ipairs(selected) do
      local r = math.abs(corr_matrix[best_idx][sel.matrix_idx])
      if r > max_corr then
        max_corr = r
        corr_with = sel.index_name
      end
    end

    selected[#selected + 1] = {
      rank        = pick,
      index_name  = ci.index_name,
      norm_name   = ci.norm_name,
      percept     = percept_name,
      sd          = ci.sd,
      bins        = ci.bins,
      valid_pct   = ci.valid_pct,
      max_corr    = max_corr,
      corr_with   = corr_with,
      matrix_idx  = best_idx,
    }
  end
end

-- Print Phase 3
print(string.format("=== Phase 3: Recommended %d Bindings ===", top_n))
print(string.format("  %-4s %-14s %-12s -> %-10s  %7s  %5s  %s",
  "#", "Index", "Normalizer", "Percept", "stddev", "bins", "Independence"))
print(string.format("  %s", string.rep("-", 80)))

for _, s in ipairs(selected) do
  local indep
  if s.rank == 1 then
    indep = "(anchor)"
  elseif s.max_corr < 0.4 then
    indep = string.format("r=%.2f ok", s.max_corr)
  elseif s.max_corr < 0.7 then
    indep = string.format("r=%.2f ~%s", s.max_corr, s.corr_with)
  else
    indep = string.format("r=%.2f !%s", s.max_corr, s.corr_with)
  end

  print(string.format("  %-4d %-14s %-12s -> %-10s  %7.3f  %3d    %s",
    s.rank, s.index_name, s.norm_name, s.percept, s.sd, s.bins, indep))
end
print()

-- ================================================================
-- Generate config snippet
-- ================================================================
print("=== Generated Config ===")
print([[
  -- .codedash.lua (paste and adjust)
  local bind = require("codedash.model.binding")
  local idx  = require("codedash.presets.indexes")
  local pct  = require("codedash.presets.percepts")
]])

print("  return {")
print("    bindings = {")
for _, s in ipairs(selected) do
  local norm_part = ""
  if s.norm_name ~= "percentile" then
    norm_part = string.format(", normalize = \"%s\"", s.norm_name)
  end
  print(string.format("      bind { idx.%-14s pct.%-10s },%s",
    s.index_name .. ",", s.percept, norm_part))
end
print("    },")
print("  }")
print()
