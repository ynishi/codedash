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
        :option("o", "output", "Output format: report, json", { default = "report" })
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
-- Serialize eval result to JSON
-- ================================================================
local function eval_to_json(r, bindings)
  local out_entries = {}
  for _, entry in ipairs(r.entries) do
    local n = entry.node
    local item = {
      -- Identity
      file       = n.file,
      name       = n.short_name or n.name,
      full_name  = n.name,
      kind       = n.semantic_type or "unknown",
      -- Raw metrics
      lines      = n.lines,
      start_line = n.start_line,
      end_line   = n.end_line,
      params     = n.params,
      depth      = n.depth,
      field_count = n.field_count,
      cyclomatic = n.cyclomatic,
      exported   = n.exported,
      visibility = n.visibility,
      git_churn_30d = n.git_churn_30d,
      coverage   = n.coverage,
      -- Computed percept values (from Lua DSL eval)
      percept    = {},
      -- Normalized [0,1] values
      normalized = {},
    }
    for _, b in ipairs(bindings) do
      local ch = b.percept.name
      item.percept[ch]    = entry.percept[ch]
      item.normalized[ch] = entry.index[ch]
    end
    out_entries[#out_entries + 1] = item
  end

  local out_groups = {}
  for _, g in ipairs(r.groups) do
    out_groups[#out_groups + 1] = {
      name  = g.name,
      count = g.count,
      pct   = g.pct,
    }
  end

  local out_bindings = {}
  for _, b in ipairs(bindings) do
    out_bindings[#out_bindings + 1] = {
      index   = b.index.name,
      percept = b.percept.name,
    }
  end

  return __rustlib.json.encode({
    entries  = out_entries,
    groups   = out_groups,
    total    = r.total,
    excluded = r.excluded,
    bindings = out_bindings,
  })
end

-- ================================================================
-- Analyze route
-- ================================================================
app:route("analyze", function(ctx)
  local path = ctx.args.path
  local lang = ctx.args.lang or "rust"
  local output_format = ctx.args.output or "report"
  local top_n = tonumber(ctx.args.top) or 10
  local config_path = ctx.args.config
  local domain_filter = ctx.args.domain

  -- Step 1: Rust-side parse + enrich
  local enriched_json = __rustlib.analyze(path, lang)

  -- Step 2: Lua-side eval using codedash engine
  local config = resolve_config(config_path)
  if not config then
    return sen.err("Failed to load config")
  end

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

  -- JSON output: emit full eval result with percept values
  if output_format == "json" then
    return sen.ok(eval_to_json(r, resolved.bindings))
  end

  -- Step 3: Format report (default)
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
