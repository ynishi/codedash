--[[
  normalizers/percentile.lua — p10/p90-based [0,1] normalization

  Maps the p10-p90 range to [0, 1].
  Outliers are clamped.
  When p10 == p90 (skewed distribution), falls back to min/max range.
  Returns 0.5 only when all values are identical (truly flat).
]]

local normalizer = require("codedash.model.normalizer")

return normalizer "percentile" {
  stats = function(values)
    local count = #values
    if count == 0 then
      return { min = 0, max = 0, p10 = 0, p50 = 0, p90 = 0, count = 0 }
    end

    local sorted = {}
    for i, v in ipairs(values) do sorted[i] = v end
    table.sort(sorted)

    return {
      min   = sorted[1],
      max   = sorted[count],
      p10   = sorted[math.max(1, math.ceil(count * 0.1))],
      p50   = sorted[math.max(1, math.ceil(count * 0.5))],
      p90   = sorted[math.max(1, math.ceil(count * 0.9))],
      count = count,
    }
  end,

  normalizer = function(stats)
    local lo = stats.p10
    local hi = stats.p90
    if hi <= lo then
      -- Fallback: use min/max when p10/p90 range is degenerate
      lo = stats.min
      hi = stats.max
      if hi <= lo then
        -- Truly flat: all values identical
        return function() return 0.5 end
      end
    end
    return function(raw)
      local clamped = math.max(lo, math.min(hi, raw))
      return (clamped - lo) / (hi - lo)
    end
  end,
}
