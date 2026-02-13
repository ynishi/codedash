local index      = require("codedash.model.index")
local percept    = require("codedash.model.percept")
local normalizer = require("codedash.model.normalizer")

local idx_catalog = require("codedash.presets.indexes")
local pct_catalog = require("codedash.presets.percepts")
local nrm_catalog = require("codedash.presets.normalizers")

-- Helper: minimal node with all numeric fields
local function make_node(overrides)
  local base = {
    lines = 10, depth = 2, params = 3, field_count = 5,
    exported_score = 1, cyclomatic = 4, coverage = 0.8,
    git_churn_30d = 7,
  }
  if overrides then
    for k, v in pairs(overrides) do base[k] = v end
  end
  return base
end

-- ============================================================
-- Preset Indexes
-- ============================================================

describe("Preset Indexes", function()
  local node = make_node()

  describe("catalog completeness", function()
    it("has all 9 indexes", function()
      local expected = {
        "churn", "lines", "params", "depth", "coverage",
        "cyclomatic", "field_count", "exported_score", "complexity",
      }
      for _, name in ipairs(expected) do
        assert.is_not_nil(idx_catalog[name], "missing index: " .. name)
      end
    end)

    it("all entries are valid IndexDef", function()
      for name, def in pairs(idx_catalog) do
        assert.is_true(index.is_index_def(def), name .. " is not IndexDef")
      end
    end)
  end)

  describe("source indexes resolve correctly", function()
    local source_cases = {
      { name = "churn",          field = "git_churn_30d", expected = 7 },
      { name = "lines",          field = "lines",         expected = 10 },
      { name = "params",         field = "params",        expected = 3 },
      { name = "depth",          field = "depth",         expected = 2 },
      { name = "coverage",       field = "coverage",      expected = 0.8 },
      { name = "cyclomatic",     field = "cyclomatic",    expected = 4 },
      { name = "field_count",    field = "field_count",   expected = 5 },
      { name = "exported_score", field = "exported_score", expected = 1 },
    }

    for _, c in ipairs(source_cases) do
      it(c.name .. " -> node." .. c.field, function()
        local val = idx_catalog[c.name].resolver(node)
        assert.equal(c.expected, val)
      end)
    end
  end)

  describe("complexity (combinator)", function()
    it("computes lines * 0.3 + params * 2.0", function()
      local val = idx_catalog.complexity.resolver(node)
      -- lines=10, params=3 -> 10*0.3 + 3*2.0 = 3.0 + 6.0 = 9.0
      assert.near(9.0, val, 1e-10)
    end)

    it("returns nil when a source is missing", function()
      local bad_node = make_node()
      bad_node.lines = nil
      -- lines resolver returns nil -> combine returns nil
      local val = idx_catalog.complexity.resolver(bad_node)
      assert.is_nil(val)
    end)
  end)

  describe("default normalizer", function()
    it("all indexes default to percentile", function()
      for name, def in pairs(idx_catalog) do
        assert.equal("percentile", def.normalize,
          name .. " should default to percentile")
      end
    end)
  end)
end)

-- ============================================================
-- Preset Percepts
-- ============================================================

describe("Preset Percepts", function()
  describe("catalog completeness", function()
    it("has all 7 percepts", function()
      local expected = {
        "hue", "size", "border", "opacity", "clarity", "weight", "presence",
      }
      for _, name in ipairs(expected) do
        assert.is_not_nil(pct_catalog[name], "missing percept: " .. name)
      end
    end)

    it("all entries are valid PerceptDef", function()
      for name, def in pairs(pct_catalog) do
        assert.is_true(percept.is_percept_def(def), name .. " is not PerceptDef")
      end
    end)
  end)

  describe("boundary mapping", function()
    local range_cases = {
      { name = "hue",      lo = 240, hi = 0 },
      { name = "size",     lo = 0.2, hi = 5.0 },
      { name = "border",   lo = 0,   hi = 3.0 },
      { name = "opacity",  lo = 1.0, hi = 0.1 },
      { name = "clarity",  lo = 0,   hi = 1.0 },
      { name = "weight",   lo = 0.2, hi = 5.0 },
      { name = "presence", lo = 0,   hi = 1.0 },
    }

    for _, c in ipairs(range_cases) do
      it(c.name .. " maps 0 -> " .. c.lo .. ", 1 -> " .. c.hi, function()
        local p = pct_catalog[c.name]
        assert.near(c.lo, p.mapper(0), 1e-10)
        assert.near(c.hi, p.mapper(1), 1e-10)
      end)
    end
  end)

  describe("midpoint interpolation", function()
    it("hue midpoint is 120", function()
      assert.equal(120, pct_catalog.hue.mapper(0.5))
    end)

    it("size midpoint is 2.6", function()
      assert.near(2.6, pct_catalog.size.mapper(0.5), 1e-10)
    end)
  end)
end)

-- ============================================================
-- Preset Normalizers
-- ============================================================

describe("Preset Normalizers", function()
  describe("catalog completeness", function()
    it("has percentile and rank", function()
      assert.is_not_nil(nrm_catalog.percentile)
      assert.is_not_nil(nrm_catalog.rank)
    end)

    it("all entries are valid NormalizerDef", function()
      for name, def in pairs(nrm_catalog) do
        assert.is_true(normalizer.is_normalizer_def(def), name .. " is not NormalizerDef")
      end
    end)
  end)

  describe("contract: stats + normalizer produce [0,1]", function()
    local values = { 1, 5, 10, 20, 50, 80, 90, 95, 99, 100 }

    for nrm_name, def in pairs(nrm_catalog) do
      it(nrm_name .. " output is within [0,1]", function()
        local stats = def.stats(values)
        local norm  = def.normalizer(stats)

        for _, v in ipairs(values) do
          local result = norm(v)
          assert.is_true(result >= 0 and result <= 1,
            string.format("%s(%d) = %f out of [0,1]", nrm_name, v, result))
        end
      end)
    end
  end)

  describe("monotonicity", function()
    local values = { 1, 2, 3, 4, 5, 6, 7, 8, 9, 10 }

    for nrm_name, def in pairs(nrm_catalog) do
      it(nrm_name .. " is non-decreasing for sorted input", function()
        local stats = def.stats(values)
        local norm  = def.normalizer(stats)

        local prev = -1
        for _, v in ipairs(values) do
          local result = norm(v)
          assert.is_true(result >= prev,
            string.format("%s: norm(%d)=%f < prev=%f", nrm_name, v, result, prev))
          prev = result
        end
      end)
    end
  end)
end)
