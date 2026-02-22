--[[
  app.lua — codedash CLI entry point (senl-powered)

  Defines the CLI commands using senl's DSL.
  Heavy lifting (parse + enrich) is done by Rust via __rustlib.
  Evaluation uses the existing codedash Lua engine.
]]

local sen = require("lua_sen")
local codedash = require("codedash")

local app = sen.app("codedash", "Code metrics visualization")
    :command("analyze", "Analyze a codebase and produce metrics report")
        :arg("path", "Source directory to analyze")
        :option("l", "lang", "Language: rust, typescript", { default = "rust" })
        :option("t", "top", "Show top N entries", { default = "10" })
        :option("c", "config", "Config file path (.codedash.lua)")
        :option("d", "domain", "Filter output to a specific domain")
        :done()

-- ================================================================
-- Config resolution (same logic as cli.lua)
-- ================================================================
local function load_config_file(path)
  local fn, err = loadfile(path)
  if not fn then return nil, err end
  return fn()
end

local function resolve_config(config_path)
  -- 1. Explicit --config
  if config_path then
    local cfg, err = load_config_file(config_path)
    if not cfg then
      io.stderr:write(string.format("Error loading config '%s': %s\n", config_path, err))
      return nil
    end
    return cfg
  end

  -- 2. .codedash.lua in CWD
  local cfg = load_config_file(".codedash.lua")
  if cfg then return cfg end

  -- 3. Fallback: recommended preset only
  return { extends = "recommended" }
end

-- ================================================================
-- Analyze route
-- ================================================================
app:route("analyze", function(ctx)
  local path = ctx.args.path
  local lang = ctx.args.lang or "rust"
  local top_n = tonumber(ctx.args.top) or 10
  local config_path = ctx.args.config
  local domain_filter = ctx.args.domain

  -- Step 1: Rust-side parse + enrich → JSON string
  local enriched_json = __rustlib.analyze(path, lang)

  -- Step 2: Lua-side eval using codedash engine
  local config = resolve_config(config_path)
  if not config then
    return sen.err("Failed to load config")
  end

  -- Parse the enriched JSON and feed to codedash
  local ast_data = __rustlib.json.decode(enriched_json)
  local loader = require("codedash.eval.loader")
  local nodes = loader.to_nodes(ast_data)

  local settings_mod = require("codedash.eval.settings")
  local resolved = settings_mod.resolve(config)

  local classify = require("codedash.eval.classify")
  local domain_map = {}
  if #resolved.domains > 0 then
    domain_map = classify.build_domain_map({
      domains  = resolved.domains,
      exclude  = resolved.exclude,
      fallback = resolved.fallback,
    }, nodes)
  end

  local lua_eval = require("codedash.eval.lua_eval")
  local r = lua_eval.run(resolved.bindings, nodes, {
    domain_map = domain_map,
  })

  -- Step 3: Format report
  local report = codedash.report
  local lines = {}
  lines[#lines+1] = string.format("Loaded %d nodes", #nodes)
  lines[#lines+1] = ""
  lines[#lines+1] = report.summary(r)
  lines[#lines+1] = ""

  -- Top N
  lines[#lines+1] = string.format("--- Top %d ---", top_n)
  local top = report.top(r, top_n)
  for _, entry in ipairs(top) do
    lines[#lines+1] = "  " .. report.format_entry(entry, r.bindings)
  end
  lines[#lines+1] = ""

  -- Domain groups
  if #r.groups > 0 then
    lines[#lines+1] = "--- Domains ---"
    for _, g in ipairs(r.groups) do
      if not domain_filter or g.name == domain_filter then
        lines[#lines+1] = string.format("  %-15s  %d nodes (%.1f%%)", g.name, g.count, g.pct)
      end
    end
  end

  return sen.ok(table.concat(lines, "\n"))
end)

return app:build()
