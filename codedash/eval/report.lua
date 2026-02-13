--[[
  eval/report.lua — Report formatting

  Produces human-readable output from eval results.
]]

local M = {}

--- Top N entries by a specific percept channel (descending)
---@param report table
---@param n number
---@param channel string|nil  percept channel to sort by (default: first binding)
---@return table[] entries
function M.top(report, n, channel)
  -- Default to first binding's channel
  if not channel and report.bindings and #report.bindings > 0 then
    channel = report.bindings[1].percept.name
  end
  if not channel then return {} end

  local sorted = {}
  for i, e in ipairs(report.entries) do
    sorted[i] = e
  end
  table.sort(sorted, function(a, b)
    local ai = a.index and a.index[channel] or 0
    local bi = b.index and b.index[channel] or 0
    return ai > bi
  end)

  local out = {}
  for i = 1, math.min(n, #sorted) do
    out[i] = sorted[i]
  end
  return out
end

--- Summary text
---@param report table
---@return string
function M.summary(report)
  local lines = {}
  lines[#lines + 1] = string.format("Total: %d nodes", report.total)

  if (report.excluded or 0) > 0 then
    lines[#lines + 1] = string.format("Excluded: %d nodes", report.excluded)
  end

  if #report.groups > 0 then
    lines[#lines + 1] = ""
    lines[#lines + 1] = "Domains:"
    for _, g in ipairs(report.groups) do
      lines[#lines + 1] = string.format("  %-15s  %d nodes (%.1f%%)",
        g.name, g.count, g.pct)
    end
  end

  return table.concat(lines, "\n")
end

--- Detailed entry display
---@param entry table
---@param bindings table[]
---@return string
function M.format_entry(entry, bindings)
  local parts = { entry.node.short_name or entry.node.name }

  for _, b in ipairs(bindings) do
    local ch = b.percept.name
    local pv = entry.percept and entry.percept[ch]
    if pv then
      parts[#parts + 1] = string.format("%s=%.2f", ch, pv)
    end
  end

  return table.concat(parts, "  ")
end

return M
