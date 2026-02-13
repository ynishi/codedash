local codedash = require("codedash")

describe("E2E pipeline", function()

  local sample_path = "examples/sample_enriched.json"

  describe("init + run with recommended preset", function()
    it("loads and evaluates sample data", function()
      local instance = codedash.init(sample_path, { extends = "recommended" })
      assert.truthy(instance)
      assert.equal(26, instance:count())

      local r = instance:run()
      assert.equal(26, r.total)
      assert.equal(26, #r.entries)
      assert.equal(5,  #r.bindings)
    end)

    it("produces valid percept values for each entry", function()
      local instance = codedash.init(sample_path, { extends = "recommended" })
      local r = instance:run()

      for _, entry in ipairs(r.entries) do
        assert.is_table(entry.node)
        assert.is_table(entry.percept)
        -- hue binding exists for all entries with churn data
        if entry.percept.hue then
          assert.is_number(entry.percept.hue)
        end
      end
    end)
  end)

  describe("domain classification", function()
    it("groups entries by domain", function()
      local config = {
        extends = "recommended",
        domains = {
          { name = "auth",   patterns = { "auth" } },
          { name = "crypto", patterns = { "crypto" } },
        },
      }
      local instance = codedash.init(sample_path, config)
      local r = instance:run()

      assert.truthy(#r.groups > 0)
      local names = {}
      for _, g in ipairs(r.groups) do
        names[g.name] = g.count
      end
      assert.truthy(names["auth"])
      assert.truthy(names["crypto"])
    end)

    it("excludes _excluded from report groups", function()
      local config = {
        extends = "recommended",
        domains = {
          { name = "auth", patterns = { "auth" } },
        },
        exclude = { "AuthConfig" },
      }
      local instance = codedash.init(sample_path, config)
      local r = instance:run()

      -- _excluded should NOT appear in groups
      for _, g in ipairs(r.groups) do
        assert.not_equal("_excluded", g.name)
      end
      -- excluded count should be reported
      assert.truthy(r.excluded > 0)
    end)
  end)

  describe("report utilities", function()
    it("top returns sorted entries", function()
      local instance = codedash.init(sample_path, { extends = "recommended" })
      local r = instance:run()

      local top5 = codedash.report.top(r, 5)
      assert.equal(5, #top5)

      -- Verify descending order on first binding channel
      local ch = r.bindings[1].percept.name
      for i = 2, #top5 do
        local prev = top5[i - 1].index[ch] or 0
        local curr = top5[i].index[ch] or 0
        assert.truthy(prev >= curr)
      end
    end)

    it("summary returns non-empty string", function()
      local instance = codedash.init(sample_path, { extends = "recommended" })
      local r = instance:run()
      local s = codedash.report.summary(r)
      assert.is_string(s)
      assert.truthy(#s > 0)
      assert.truthy(s:find("26 nodes"))
    end)

    it("format_entry returns readable string", function()
      local instance = codedash.init(sample_path, { extends = "recommended" })
      local r = instance:run()
      local s = codedash.report.format_entry(r.entries[1], r.bindings)
      assert.is_string(s)
      assert.truthy(#s > 0)
    end)
  end)

  describe("public API re-exports", function()
    it("exposes DSL constructors", function()
      assert.is_table(codedash.index)
      assert.is_table(codedash.percept)
      assert.is_table(codedash.bind)
      assert.is_table(codedash.normalizer)
    end)

    it("exposes preset catalogs", function()
      assert.is_table(codedash.indexes)
      assert.truthy(codedash.indexes.lines)
      assert.truthy(codedash.indexes.churn)

      assert.is_table(codedash.percepts)
      assert.truthy(codedash.percepts.hue)
      assert.truthy(codedash.percepts.size)
    end)
  end)
end)
