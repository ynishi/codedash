--[[
  normalizers/rank.lua — Rank-based [0,1] normalization

  Binary search on sorted values to map raw values to [0, 1] by rank.
  Ties receive the average rank, ensuring spread even with skewed data.
]]

local normalizer = require("codedash.model.normalizer")

return normalizer "rank" {
  stats = function(values)
    local count = #values
    if count == 0 then
      return { count = 0, sorted = {} }
    end

    local sorted = {}
    for i, v in ipairs(values) do sorted[i] = v end
    table.sort(sorted)

    return {
      count  = count,
      sorted = sorted,
    }
  end,

  normalizer = function(stats)
    if stats.count == 0 then
      return function() return 0.5 end
    end

    local sorted = stats.sorted
    local count = stats.count

    return function(raw)
      -- binary search: find first position where sorted[lo] >= raw
      local lo, hi = 1, count
      while lo <= hi do
        local mid = math.floor((lo + hi) / 2)
        if sorted[mid] < raw then
          lo = mid + 1
        else
          hi = mid - 1
        end
      end

      if lo > count then
        return 1.0
      end
      if sorted[lo] ~= raw then
        return count > 1 and ((lo - 1) / (count - 1)) or 0.5
      end

      -- Find end of tie group and return average rank
      local first = lo
      local last = lo
      while last < count and sorted[last + 1] == raw do
        last = last + 1
      end
      local avg_rank = ((first - 1) + (last - 1)) / 2
      return count > 1 and (avg_rank / (count - 1)) or 0.5
    end
  end,
}
