local classify = require("codedash.eval.classify")
local node_mod = require("codedash.model.node")

describe("classify", function()

  local function make_node(name, file)
    return node_mod.new({
      name          = file .. "::" .. name,
      short_name    = name,
      file          = file,
      semantic_type = "function",
    })
  end

  describe("classify_node", function()
    it("matches by name substring", function()
      local config = {
        domains = { { name = "auth", patterns = { "auth" } } },
      }
      local n = make_node("authenticate", "auth.ts")
      assert.equal("auth", classify.classify_node(config, n))
    end)

    it("matches by file substring", function()
      local config = {
        domains = { { name = "crypto", patterns = { "crypto" } } },
      }
      local n = make_node("hash", "crypto/utils.ts")
      assert.equal("crypto", classify.classify_node(config, n))
    end)

    it("returns fallback for unmatched nodes", function()
      local config = {
        domains  = { { name = "auth", patterns = { "auth" } } },
        fallback = "other",
      }
      local n = make_node("unrelated", "utils.ts")
      assert.equal("other", classify.classify_node(config, n))
    end)

    it("defaults fallback to 'unknown'", function()
      local config = { domains = {} }
      local n = make_node("anything", "file.ts")
      assert.equal("unknown", classify.classify_node(config, n))
    end)

    it("excludes matching nodes", function()
      local config = {
        domains = { { name = "auth", patterns = { "auth" } } },
        exclude = { "index" },
      }
      local n = make_node("index", "index.ts")
      assert.equal("_excluded", classify.classify_node(config, n))
    end)

    it("first matching domain wins", function()
      local config = {
        domains = {
          { name = "first",  patterns = { "auth" } },
          { name = "second", patterns = { "auth" } },
        },
      }
      local n = make_node("auth", "auth.ts")
      assert.equal("first", classify.classify_node(config, n))
    end)
  end)

  describe("build_domain_map", function()
    it("returns map of node names to domains", function()
      local nodes = {
        make_node("login",  "auth.ts"),
        make_node("hash",   "crypto.ts"),
        make_node("helper", "utils.ts"),
      }
      local config = {
        domains = {
          { name = "auth",   patterns = { "auth" } },
          { name = "crypto", patterns = { "crypto" } },
        },
        fallback = "other",
      }
      local map = classify.build_domain_map(config, nodes)
      assert.equal("auth",   map["auth.ts::login"])
      assert.equal("crypto", map["crypto.ts::hash"])
      assert.equal("other",  map["utils.ts::helper"])
    end)
  end)
end)
