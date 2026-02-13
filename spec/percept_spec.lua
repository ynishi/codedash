local percept = require("codedash.model.percept")

describe("Percept", function()
  describe("continuous", function()
    it("maps [0,1] to range via linear interpolation", function()
      local p = percept "hue" { range = { 240, 0 } }
      assert.equal(240, p.mapper(0))
      assert.equal(120, p.mapper(0.5))
      assert.equal(0,   p.mapper(1))
    end)

    it("handles inverted range", function()
      local p = percept "opacity" { range = { 1.0, 0.1 } }
      assert.equal(1.0, p.mapper(0))
      assert.near(0.1, p.mapper(1), 1e-10)
    end)
  end)

  describe("discrete (steps)", function()
    it("quantizes to N levels", function()
      local p = percept "hue" { range = { 0, 100 }, steps = 3 }
      -- steps=3 → levels at 0, 50, 100
      assert.equal(0,   p.mapper(0))
      assert.equal(100, p.mapper(1))
      assert.equal(50,  p.mapper(0.5))
    end)

    it("errors on steps < 2", function()
      assert.has_error(function()
        percept "bad" { range = { 0, 1 }, steps = 1 }
      end)
    end)

    it("errors on non-integer steps", function()
      assert.has_error(function()
        percept "bad" { range = { 0, 1 }, steps = 2.5 }
      end)
    end)
  end)

  describe("validation", function()
    it("errors without range", function()
      assert.has_error(function()
        percept "bad" {}
      end)
    end)

    it("errors on non-numeric range", function()
      assert.has_error(function()
        percept "bad" { range = { "a", "b" } }
      end)
    end)
  end)

  describe("is_percept_def", function()
    it("validates percept definitions", function()
      local p = percept "hue" { range = { 0, 1 } }
      assert.is_true(percept.is_percept_def(p))
      assert.is_false(percept.is_percept_def({ name = "fake" }))
    end)
  end)
end)
