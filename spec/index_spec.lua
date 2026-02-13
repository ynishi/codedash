local index    = require("codedash.model.index")
local node_mod = require("codedash.model.node")

describe("Index", function()

  local function make_node(overrides)
    local raw = {
      name = "test.ts::fn", short_name = "fn",
      file = "test.ts", semantic_type = "function",
      lines = 50, params = 3, depth = 2,
    }
    for k, v in pairs(overrides or {}) do raw[k] = v end
    return node_mod.new(raw)
  end

  describe("source index", function()
    it("resolves a node field by name", function()
      local idx = index "lines" { source = "lines" }
      local n = make_node({ lines = 42 })
      assert.equal(42, idx.resolver(n))
    end)

    it("returns nil for missing nullable field", function()
      local idx = index "coverage" { source = "coverage" }
      local n = make_node()
      assert.is_nil(idx.resolver(n))
    end)

    it("errors on unknown source field", function()
      assert.has_error(function()
        index "bad" { source = "nonexistent_field" }
      end)
    end)

    it("errors on non-numeric source field", function()
      assert.has_error(function()
        index "bad" { source = "name" }
      end)
    end)
  end)

  describe("compute index", function()
    it("evaluates custom function", function()
      local idx = index "risk" {
        compute = function(node) return node.lines * node.params end,
      }
      local n = make_node({ lines = 10, params = 3 })
      assert.equal(30, idx.resolver(n))
    end)

    it("returns nil on compute error (safe resolver)", function()
      local idx = index "broken" {
        compute = function() error("boom") end,
      }
      local n = make_node()
      assert.is_nil(idx.resolver(n))
    end)
  end)

  describe("combine index", function()
    it("composes two indexes", function()
      local a = index "lines"  { source = "lines" }
      local b = index "params" { source = "params" }
      local c = index "combo"  {
        combine = { a, b, function(l, p) return l + p end },
      }
      local n = make_node({ lines = 10, params = 5 })
      assert.equal(15, c.resolver(n))
    end)

    it("returns nil if either input is nil", function()
      local a = index "coverage" { source = "coverage" }
      local b = index "lines"    { source = "lines" }
      local c = index "combo"    {
        combine = { a, b, function(x, y) return x + y end },
      }
      local n = make_node()  -- coverage is nil
      assert.is_nil(c.resolver(n))
    end)
  end)

  describe("map combinator", function()
    it("transforms index output", function()
      local base = index "lines" { source = "lines" }
      local doubled = index.map(base, function(v) return v * 2 end)
      local n = make_node({ lines = 25 })
      assert.equal(50, doubled.resolver(n))
    end)
  end)

  describe("with_normalize", function()
    it("overrides normalizer name", function()
      local base = index "lines" { source = "lines" }
      assert.equal("percentile", base.normalize)
      local ranked = index.with_normalize(base, "rank")
      assert.equal("rank", ranked.normalize)
    end)
  end)

  describe("is_index_def", function()
    it("validates index definitions", function()
      local idx = index "lines" { source = "lines" }
      assert.is_true(index.is_index_def(idx))
      assert.is_false(index.is_index_def({ name = "fake" }))
    end)
  end)

  describe("DEFAULT_NORMALIZER", function()
    it("is percentile", function()
      assert.equal("percentile", index.DEFAULT_NORMALIZER)
    end)
  end)
end)
