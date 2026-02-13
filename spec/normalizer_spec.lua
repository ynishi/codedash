local percentile_mod = require("codedash.presets.normalizers.percentile")
local rank_mod       = require("codedash.presets.normalizers.rank")

describe("Normalizer: percentile", function()
  it("maps p10-p90 range to [0,1]", function()
    local values = { 1, 2, 3, 4, 5, 6, 7, 8, 9, 10 }
    local stats = percentile_mod.stats(values)
    local norm  = percentile_mod.normalizer(stats)

    assert.equal(0, norm(stats.p10))
    assert.equal(1, norm(stats.p90))
  end)

  it("clamps outliers", function()
    local values = { 1, 2, 3, 4, 5, 6, 7, 8, 9, 10 }
    local stats = percentile_mod.stats(values)
    local norm  = percentile_mod.normalizer(stats)

    -- Values below p10 clamp to 0
    assert.equal(0, norm(-100))
    -- Values above p90 clamp to 1
    assert.equal(1, norm(1000))
  end)

  it("returns 0.5 for flat distribution", function()
    local values = { 5, 5, 5, 5, 5 }
    local stats = percentile_mod.stats(values)
    local norm  = percentile_mod.normalizer(stats)

    assert.equal(0.5, norm(5))
    assert.equal(0.5, norm(100))
  end)

  it("handles empty values", function()
    local stats = percentile_mod.stats({})
    local norm  = percentile_mod.normalizer(stats)
    assert.equal(0.5, norm(42))
  end)

  it("handles single value", function()
    local stats = percentile_mod.stats({ 7 })
    local norm  = percentile_mod.normalizer(stats)
    assert.equal(0.5, norm(7))
  end)
end)

describe("Normalizer: rank", function()
  it("maps values to rank-based [0,1]", function()
    local values = { 10, 20, 30, 40, 50 }
    local stats = rank_mod.stats(values)
    local norm  = rank_mod.normalizer(stats)

    assert.equal(0,    norm(10))
    assert.equal(0.25, norm(20))
    assert.equal(0.5,  norm(30))
    assert.equal(0.75, norm(40))
    assert.equal(1,    norm(50))
  end)

  it("averages tied ranks", function()
    local values = { 10, 20, 20, 30 }
    local stats = rank_mod.stats(values)
    local norm  = rank_mod.normalizer(stats)

    -- 20 appears at positions 2 and 3 (0-indexed: 1 and 2)
    -- average rank = (1 + 2) / 2 = 1.5
    -- normalized = 1.5 / (4 - 1) = 0.5
    assert.equal(0.5, norm(20))
  end)

  it("returns 1.0 for values above max", function()
    local values = { 1, 2, 3 }
    local stats = rank_mod.stats(values)
    local norm  = rank_mod.normalizer(stats)
    assert.equal(1.0, norm(999))
  end)

  it("handles empty values", function()
    local stats = rank_mod.stats({})
    local norm  = rank_mod.normalizer(stats)
    assert.equal(0.5, norm(42))
  end)
end)
