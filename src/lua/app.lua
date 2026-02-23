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
        :arg("path", "Source directory to analyze (default: .)", { required = false })
        :option("l", "lang", "Language: rust, typescript", { default = "rust" })
        :option("o", "output", "Output format: report, json", { default = "report" })
        :option("t", "top", "Show top N entries", { default = "10" })
        :option("c", "config", "Config file path (.codedash.lua)")
        :option("d", "domain", "Filter output to a specific domain")
        :done()
    :command("parse", "Parse source files without enrichment or evaluation")
        :arg("path", "Source directory to parse (default: .)", { required = false })
        :option("l", "lang", "Language: rust, typescript", { default = "rust" })
        :done()
    :command("graph", "Show dependency graph from import edges")
        :arg("path", "Source directory to analyze (default: .)", { required = false })
        :option("l", "lang", "Language: rust, typescript", { default = "rust" })
        :option("f", "format", "Output format: dot, mermaid", { default = "dot" })
        :done()
    :command("list-parsers", "List supported languages and file extensions")
        :done()
    :command("config-init", "Generate a .codedash.lua template in current directory")
        :done()
    :command("view", "Generate interactive HTML dependency map")
        :arg("path", "Source directory (default: .)", { required = false })
        :option("l", "lang", "Language: rust, typescript", { default = "rust" })
        :option("o", "out", "Output file path", { default = "codedash-view.html" })
        :option("c", "config", "Config file path (.codedash.lua)")
        :done()
    :command("check-health", "Diagnose parser, git, and config status")
        :arg("path", "Source directory to check (default: .)", { required = false })
        :option("l", "lang", "Language: rust, typescript", { default = "rust" })
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
  local path = ctx.args.path or "."
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

-- ================================================================
-- Parse route (AST only, no enrichment)
-- ================================================================
app:route("parse", function(ctx)
  local path = ctx.args.path or "."
  local lang = ctx.args.lang or "rust"

  local json = __rustlib.parse_only(path, lang)
  return sen.ok(json)
end)

-- ================================================================
-- Graph route (dependency edges as DOT or mermaid)
-- ================================================================
app:route("graph", function(ctx)
  local path = ctx.args.path or "."
  local lang = ctx.args.lang or "rust"
  local fmt  = ctx.args.format or "dot"

  local json = __rustlib.parse_only(path, lang)
  local ast_data = __rustlib.json.decode(json)

  -- Normalize: strip leading "src/" for cleaner graph node names
  local function strip_src(p)
    return p:match("^src/(.+)") or p
  end

  -- Filter out unresolvable edges and normalize paths
  local raw_edges = ast_data.edges or {}
  local edges = {}
  for _, e in ipairs(raw_edges) do
    if e.to_file and e.to_file ~= "" and e.to_file ~= "crate" then
      edges[#edges + 1] = {
        from_file = strip_src(e.from_file),
        to_file   = strip_src(e.to_file),
        symbol    = e.symbol,
      }
    end
  end
  if #edges == 0 then
    return sen.ok("No edges found (no internal imports detected).")
  end

  local lines = {}

  if fmt == "mermaid" then
    lines[#lines+1] = "graph LR"
    for _, e in ipairs(edges) do
      local from = e.from_file:gsub("[/%.%-]", "_")
      local to   = e.to_file:gsub("[/%.%-]", "_")
      lines[#lines+1] = string.format("  %s -->|%s| %s", from, e.symbol, to)
    end
  else
    -- DOT format
    lines[#lines+1] = "digraph codedash {"
    lines[#lines+1] = '  rankdir=LR;'
    lines[#lines+1] = '  node [shape=box, style=rounded];'

    -- Deduplicate edges (aggregate symbols per file pair)
    local pair_symbols = {}
    for _, e in ipairs(edges) do
      local key = e.from_file .. "|" .. e.to_file
      if not pair_symbols[key] then
        pair_symbols[key] = { from = e.from_file, to = e.to_file, symbols = {} }
      end
      pair_symbols[key].symbols[#pair_symbols[key].symbols + 1] = e.symbol
    end

    for _, p in pairs(pair_symbols) do
      local label = table.concat(p.symbols, ", ")
      if #label > 40 then
        label = string.format("%d symbols", #p.symbols)
      end
      lines[#lines+1] = string.format('  "%s" -> "%s" [label="%s"];', p.from, p.to, label)
    end

    lines[#lines+1] = "}"
  end

  return sen.ok(table.concat(lines, "\n"))
end)

-- ================================================================
-- List-parsers route
-- ================================================================
app:route("list-parsers", function()
  local parsers = __rustlib.list_parsers()
  local lines = {}
  lines[#lines+1] = string.format("%-15s  %s", "Language", "Extensions")
  lines[#lines+1] = string.rep("-", 35)
  for _, p in ipairs(parsers) do
    lines[#lines+1] = string.format("%-15s  %s", p.name, table.concat(p.extensions, ", "))
  end
  return sen.ok(table.concat(lines, "\n"))
end)

-- ================================================================
-- Config-init route (auto-detects Cargo workspace)
-- ================================================================

--- Parse Cargo.toml workspace members with layer comments.
--- Returns list of { name=string, path=string, layer=string|nil }
local function detect_workspace_members()
  if not __rustlib.fs.file_exists("Cargo.toml") then return {} end
  local content = __rustlib.fs.read_file("Cargo.toml")

  -- Find [workspace] members section
  local members_block = content:match("%[workspace%].-members%s*=%s*%[(.-)%]")
  if not members_block then return {} end

  local members = {}
  local pending_layer = nil
  for line in members_block:gmatch("[^\n]+") do
    -- Check for layer comment: # Layer Name
    local layer_comment = line:match("^%s*#%s*(.+)$")
    if layer_comment then
      -- Strip trailing "Layer" and whitespace
      pending_layer = layer_comment:match("^(.-)%s*[Ll]ayer%s*%(.*%)%s*$")
        or layer_comment:match("^(.-)%s*[Ll]ayer%s*$")
        or layer_comment:gsub("%s+$", "")
    end

    -- Check for member path: "crates/foo",
    local member_path = line:match('"([^"]+)"')
    if member_path then
      local name = member_path:match("([^/]+)$")
      members[#members + 1] = {
        name = name,
        path = member_path,
        layer = pending_layer,
      }
    end
  end
  return members
end

app:route("config-init", function()
  local path = ".codedash.lua"

  if __rustlib.fs.file_exists(path) then
    return sen.err(".codedash.lua already exists in current directory")
  end

  local members = detect_workspace_members()

  local lines = {}
  lines[#lines+1] = '--[['
  lines[#lines+1] = '  .codedash.lua — Project configuration for codedash'
  lines[#lines+1] = ''
  lines[#lines+1] = '  extends: inherit bindings from a preset ("recommended")'
  lines[#lines+1] = '  domains: group nodes by file path patterns (vertical slice)'
  lines[#lines+1] = '  layers: group domains by architectural role (horizontal slice)'
  lines[#lines+1] = ']]'
  lines[#lines+1] = ''
  lines[#lines+1] = 'return {'
  lines[#lines+1] = '  extends = "recommended",'

  if #members > 0 then
    -- Domains: each crate = one domain
    lines[#lines+1] = ''
    lines[#lines+1] = '  -- Auto-detected from Cargo.toml workspace'
    lines[#lines+1] = '  domains = {'
    for _, m in ipairs(members) do
      lines[#lines+1] = string.format('    { name = "%s", patterns = { "^%s/" } },', m.name, m.path)
    end
    lines[#lines+1] = '  },'

    -- Layers: group domains by Cargo.toml comments
    local layer_map = {}
    local layer_order = {}
    for _, m in ipairs(members) do
      local layer = m.layer
      if layer then
        if not layer_map[layer] then
          layer_map[layer] = {}
          layer_order[#layer_order + 1] = layer
        end
        layer_map[layer][#layer_map[layer] + 1] = m.name
      end
    end

    if #layer_order > 0 then
      lines[#lines+1] = ''
      lines[#lines+1] = '  -- Layers: group domains by architectural role'
      lines[#lines+1] = '  layers = {'
      for _, layer in ipairs(layer_order) do
        local doms = layer_map[layer]
        local quoted = {}
        for _, d in ipairs(doms) do
          quoted[#quoted + 1] = string.format('"%s"', d)
        end
        lines[#lines+1] = string.format('    { name = "%s", domains = { %s } },',
          layer, table.concat(quoted, ", "))
      end
      lines[#lines+1] = '  },'
    end

    lines[#lines+1] = '  fallback = "other",'
  else
    lines[#lines+1] = ''
    lines[#lines+1] = '  -- Example: domain classification (uncomment to enable)'
    lines[#lines+1] = '  -- domains = {'
    lines[#lines+1] = '  --   { name = "core",  patterns = { "^src/core/", "^src/domain/" } },'
    lines[#lines+1] = '  --   { name = "infra", patterns = { "^src/infra/", "^src/db/" } },'
    lines[#lines+1] = '  --   { name = "api",   patterns = { "^src/api/", "^src/routes/" } },'
    lines[#lines+1] = '  -- },'
    lines[#lines+1] = '  -- layers = {'
    lines[#lines+1] = '  --   { name = "Core",  domains = { "core" } },'
    lines[#lines+1] = '  --   { name = "Infra", domains = { "infra" } },'
    lines[#lines+1] = '  --   { name = "API",   domains = { "api" } },'
    lines[#lines+1] = '  -- },'
    lines[#lines+1] = '  -- fallback = "other",'
  end

  lines[#lines+1] = '}'
  lines[#lines+1] = ''

  local template = table.concat(lines, "\n")

  local f, err = io.open(path, "w")
  if not f then
    return sen.err(string.format("Failed to write %s: %s", path, err))
  end
  f:write(template)
  f:close()

  local msg = "Created .codedash.lua"
  if #members > 0 then
    msg = msg .. string.format(" (detected %d workspace members)", #members)
  end
  return sen.ok(msg)
end)

-- ================================================================
-- View route (interactive HTML dependency map)
-- ================================================================
app:route("view", function(ctx)
  local path = ctx.args.path or "."
  local lang = ctx.args.lang or "rust"
  local out_path = ctx.args.out or "codedash-view.html"
  local config_path = ctx.args.config

  -- Step 1: Rust-side parse + enrich (full pipeline for metrics)
  local enriched_json = __rustlib.analyze(path, lang)

  -- Step 2: Lua-side eval
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

  -- Step 3: Count unique modules (files)
  local file_set = {}
  for _, entry in ipairs(r.entries) do
    file_set[entry.node.file] = true
  end
  local module_count = 0
  for _ in pairs(file_set) do module_count = module_count + 1 end

  -- Step 4: Build view data (aggregate by module + edges)
  local view = require("codedash.view")
  local data_json = view.build_data(r, ast_data.edges or {}, resolved.bindings, domain_map, resolved.layers)

  -- Step 5: Generate HTML and write to file
  local html = view.generate_html(data_json)
  local f, err = io.open(out_path, "w")
  if not f then
    return sen.err(string.format("Failed to write %s: %s", out_path, err))
  end
  f:write(html)
  f:close()

  return sen.ok(string.format("Wrote %s (%d modules, %d code units, %d edges)",
    out_path, module_count, #r.entries, #(ast_data.edges or {})))
end)

-- ================================================================
-- Check-health route
-- ================================================================
app:route("check-health", function(ctx)
  local path = ctx.args.path or "."
  local lang = ctx.args.lang or "rust"
  local lines = {}

  lines[#lines+1] = "codedash health check"
  lines[#lines+1] = string.rep("=", 40)

  -- 1. Git repository
  local git = __rustlib.check_git()
  if git.ok then
    lines[#lines+1] = string.format("[OK]   Git repository: %s", git.path)
  else
    lines[#lines+1] = string.format("[WARN] Git repository: %s (%s)", git.path, git.error or "unknown error")
  end

  -- 2. Parser availability
  local parsers = __rustlib.list_parsers()
  local found_lang = false
  for _, p in ipairs(parsers) do
    if p.name == lang then
      found_lang = true
      lines[#lines+1] = string.format("[OK]   Parser '%s': extensions %s", p.name, table.concat(p.extensions, ", "))
      break
    end
  end
  if not found_lang then
    lines[#lines+1] = string.format("[FAIL] Parser '%s': not found", lang)
    local available = {}
    for _, p in ipairs(parsers) do
      available[#available+1] = p.name
    end
    lines[#lines+1] = string.format("       Available: %s", table.concat(available, ", "))
  end

  -- 3. Config file
  if __rustlib.fs.file_exists(".codedash.lua") then
    local cfg, err = load_config_file(".codedash.lua")
    if cfg then
      lines[#lines+1] = "[OK]   Config: .codedash.lua loaded"
      if cfg.extends then
        lines[#lines+1] = string.format("       Extends: %s", cfg.extends)
      end
    else
      lines[#lines+1] = string.format("[FAIL] Config: .codedash.lua parse error: %s", err)
    end
  else
    lines[#lines+1] = "[INFO] Config: no .codedash.lua (using recommended preset)"
  end

  -- 4. Parse test
  local ok, result = pcall(__rustlib.parse_only, path, lang)
  if ok then
    local ast = __rustlib.json.decode(result)
    local file_count = #ast.files
    local node_count = 0
    for _, f in ipairs(ast.files) do
      node_count = node_count + #f.nodes
    end
    local edge_count = #(ast.edges or {})
    lines[#lines+1] = string.format("[OK]   Parse '%s': %d files, %d nodes, %d edges", path, file_count, node_count, edge_count)
  else
    lines[#lines+1] = string.format("[FAIL] Parse '%s': %s", path, tostring(result))
  end

  return sen.ok(table.concat(lines, "\n"))
end)

return app:build()
