local settings   = require("codedash.eval.settings")
local index      = require("codedash.model.index")
local percept    = require("codedash.model.percept")
local bind       = require("codedash.model.binding")
local normalizer = require("codedash.model.normalizer")

describe("settings.resolve", function()

  describe("normalizer resolution", function()
    it("resolves string normalizer names at resolve time", function()
      local idx = index "lines" { source = "lines", normalize = "rank" }
      local pct = percept "size" { range = { 0.2, 5.0 } }
      local config = {
        bindings = { bind { idx, pct } },
      }
      local s = settings.resolve(config)
      -- _resolved_normalizer should be a NormalizerDef object
      assert.is_true(normalizer.is_normalizer_def(s.bindings[1]._resolved_normalizer))
      assert.equal("rank", s.bindings[1]._resolved_normalizer.name)
    end)

    it("accepts NormalizerDef object directly", function()
      local nrm = require("codedash.presets.normalizers")
      local idx = index "lines" { source = "lines", normalize = nrm.percentile }
      local pct = percept "size" { range = { 0.2, 5.0 } }
      local config = {
        bindings = { bind { idx, pct } },
      }
      local s = settings.resolve(config)
      assert.is_true(normalizer.is_normalizer_def(s.bindings[1]._resolved_normalizer))
      assert.equal("percentile", s.bindings[1]._resolved_normalizer.name)
    end)

    it("binding-level normalize overrides index default", function()
      local idx = index "lines" { source = "lines" }  -- default: percentile
      local pct = percept "size" { range = { 0.2, 5.0 } }
      local config = {
        bindings = { bind { idx, pct, normalize = "rank" } },
      }
      local s = settings.resolve(config)
      assert.equal("rank", s.bindings[1]._resolved_normalizer.name)
    end)

    it("errors on unknown normalizer at resolve time", function()
      local idx = index "lines" { source = "lines", normalize = "nonexistent" }
      local pct = percept "size" { range = { 0.2, 5.0 } }
      local config = {
        bindings = { bind { idx, pct } },
      }
      local ok, err = pcall(settings.resolve, config)
      assert.is_false(ok)
      assert.truthy(err:find("unknown normalizer 'nonexistent'"))
    end)

    it("error message lists available normalizers", function()
      local idx = index "lines" { source = "lines", normalize = "typo" }
      local pct = percept "size" { range = { 0.2, 5.0 } }
      local config = {
        bindings = { bind { idx, pct } },
      }
      local ok, err = pcall(settings.resolve, config)
      assert.is_false(ok)
      assert.truthy(err:find("percentile"))
      assert.truthy(err:find("rank"))
    end)
  end)

  describe("preset loading", function()
    it("loads recommended preset", function()
      local s = settings.resolve({ extends = "recommended" })
      assert.equal(5, #s.bindings)
    end)

    it("falls back to empty bindings for unknown preset", function()
      local s = settings.resolve({ extends = "nonexistent_preset" })
      assert.equal(0, #s.bindings)
    end)
  end)

  describe("duplicate percept check", function()
    it("errors on duplicate percept names", function()
      local idx1 = index "lines"  { source = "lines" }
      local idx2 = index "params" { source = "params" }
      local pct  = percept "size" { range = { 0.2, 5.0 } }
      local config = {
        bindings = { bind { idx1, pct }, bind { idx2, pct } },
      }
      assert.has_error(function()
        settings.resolve(config)
      end)
    end)
  end)
end)
