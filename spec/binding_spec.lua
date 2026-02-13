local bind    = require("codedash.model.binding")
local index   = require("codedash.model.index")
local percept = require("codedash.model.percept")

describe("Binding", function()

  local idx = index "lines" { source = "lines" }
  local pct = percept "size" { range = { 0.2, 5.0 } }

  describe("construction", function()
    it("creates from index + percept (order 1)", function()
      local b = bind { idx, pct }
      assert.is_true(bind.is_binding(b))
      assert.equal("lines", b.index.name)
      assert.equal("size",  b.percept.name)
    end)

    it("creates from percept + index (order 2)", function()
      local b = bind { pct, idx }
      assert.is_true(bind.is_binding(b))
      assert.equal("lines", b.index.name)
      assert.equal("size",  b.percept.name)
    end)

    it("accepts normalize override", function()
      local b = bind { idx, pct, normalize = "rank" }
      assert.equal("rank", b.normalize)
    end)

    it("defaults normalize to nil", function()
      local b = bind { idx, pct }
      assert.is_nil(b.normalize)
    end)
  end)

  describe("labeled binding", function()
    it("creates with string label", function()
      local b = bind "my_binding" { idx, pct }
      assert.is_true(bind.is_binding(b))
    end)
  end)

  describe("validation", function()
    it("errors without IndexDef", function()
      assert.has_error(function() bind { pct } end)
    end)

    it("errors without PerceptDef", function()
      assert.has_error(function() bind { idx } end)
    end)

    it("errors with multiple IndexDefs", function()
      local idx2 = index "params" { source = "params" }
      assert.has_error(function() bind { idx, idx2, pct } end)
    end)

    it("errors with non-table spec", function()
      assert.has_error(function() bind(42) end)
    end)
  end)

  describe("introspection", function()
    it("key returns percept name", function()
      local b = bind { idx, pct }
      assert.equal("size", bind.key(b))
    end)

    it("source_label returns index name", function()
      local b = bind { idx, pct }
      assert.equal("lines", bind.source_label(b))
    end)

    it("tostring shows binding info", function()
      local b = bind { idx, pct }
      local s = tostring(b)
      assert.truthy(s:find("lines"))
      assert.truthy(s:find("size"))
    end)
  end)
end)
