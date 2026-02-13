local node_mod = require("codedash.model.node")

describe("Node", function()

  local function valid_raw()
    return {
      name          = "auth.ts::login",
      short_name    = "login",
      file          = "auth.ts",
      semantic_type = "function",
      lines         = 20,
      params        = 2,
      depth         = 1,
    }
  end

  describe("new", function()
    it("creates a node from valid raw data", function()
      local n = node_mod.new(valid_raw())
      assert.equal("auth.ts::login", n.name)
      assert.equal("login",          n.short_name)
      assert.equal("function",       n.semantic_type)
      assert.equal(20,               n.lines)
      assert.equal(2,                n.params)
    end)

    it("applies defaults for optional fields", function()
      local n = node_mod.new(valid_raw())
      assert.equal(0,     n.git_churn_30d)
      assert.equal(false, n.exported)
      assert.equal(0,     n.exported_score)
      assert.is_nil(n.coverage)  -- nullable
    end)

    it("errors on missing required field", function()
      local raw = valid_raw()
      raw.name = nil
      assert.has_error(function() node_mod.new(raw) end, "Node: required field 'name' is nil (login)")
    end)

    it("errors on wrong type", function()
      local raw = valid_raw()
      raw.lines = "not_a_number"
      assert.has_error(function() node_mod.new(raw) end)
    end)
  end)

  describe("is_node", function()
    it("returns true for valid node", function()
      local n = node_mod.new(valid_raw())
      assert.is_true(node_mod.is_node(n))
    end)

    it("returns false for plain table", function()
      assert.is_false(node_mod.is_node({ name = "x" }))
    end)
  end)

  describe("validate_source", function()
    it("accepts numeric fields", function()
      local ok, _ = node_mod.validate_source("lines")
      assert.is_true(ok)
    end)

    it("rejects non-numeric fields", function()
      local ok, msg = node_mod.validate_source("name")
      assert.is_false(ok)
      assert.truthy(msg:find("not number"))
    end)

    it("rejects unknown fields", function()
      local ok, msg = node_mod.validate_source("nonexistent")
      assert.is_false(ok)
      assert.truthy(msg:find("unknown field"))
    end)
  end)

  describe("numeric_fields", function()
    it("returns sorted list of numeric field names", function()
      local fields = node_mod.numeric_fields()
      assert.is_table(fields)
      assert.truthy(#fields > 0)
      -- sorted
      for i = 2, #fields do
        assert.truthy(fields[i - 1] < fields[i])
      end
    end)
  end)
end)
