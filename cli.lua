#!/usr/bin/env lua
--[[
  cli.lua — codedash generic runner

  Usage:
    lua cli.lua <source.json>                    -- recommended preset
    lua cli.lua <source.json> --config my.lua    -- custom config
    lua cli.lua <source.json> --top 20           -- show top 20
    lua cli.lua <source.json> --domain ssr       -- filter domain

  Config resolution (first match wins):
    1. --config <path>      explicit config file
    2. .codedash.lua        in current directory
    3. { extends = "recommended" }   built-in fallback
]]

-- Dev path setup (no-op when installed via LuaRocks)
local info = debug.getinfo(1, "S")
local dir = info.source:match("^@(.*/)") or "./"
package.path = package.path
  .. ";" .. dir .. "?.lua"
  .. ";" .. dir .. "?/init.lua"

local codedash = require("codedash")

-- ================================================================
-- Arg parsing
-- ================================================================
local function parse_args(raw)
  local opts = { top = 10 }
  local i = 1
  while i <= #raw do
    local v = raw[i]
    if v == "--config" then
      i = i + 1
      opts.config = raw[i]
    elseif v == "--top" then
      i = i + 1
      opts.top = tonumber(raw[i]) or 10
    elseif v == "--domain" then
      i = i + 1
      opts.domain = raw[i]
    elseif v == "--help" or v == "-h" then
      opts.help = true
    elseif not opts.source and v:sub(1, 1) ~= "-" then
      opts.source = v
    end
    i = i + 1
  end
  return opts
end

local function print_usage()
  print([[
Usage: codedash <source.json> [options]

Options:
  --config <path>   Config file (default: .codedash.lua)
  --top <n>         Show top N entries (default: 10)
  --domain <name>   Filter output to a specific domain
  -h, --help        Show this help]])
end

-- ================================================================
-- Config loader
-- ================================================================
local function load_config_file(path)
  local fn, err = loadfile(path)
  if not fn then return nil, err end
  return fn()
end

local function resolve_config(opts)
  -- 1. Explicit --config
  if opts.config then
    local cfg, err = load_config_file(opts.config)
    if not cfg then
      io.stderr:write(string.format("Error loading config '%s': %s\n", opts.config, err))
      os.exit(1)
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
-- Main
-- ================================================================
local opts = parse_args(arg)

if opts.help then
  print_usage()
  os.exit(0)
end

if not opts.source then
  io.stderr:write("Error: source JSON path required\n\n")
  print_usage()
  os.exit(1)
end

local config = resolve_config(opts)
local instance = codedash.init(opts.source, config)

print(string.format("Loaded %d nodes\n", instance:count()))

local r = instance:run()

-- ================================================================
-- Report
-- ================================================================
local report = codedash.report

print(report.summary(r))
print()

-- Top N
print(string.format("--- Top %d ---", opts.top))
local top = report.top(r, opts.top)
for _, entry in ipairs(top) do
  print("  " .. report.format_entry(entry, r.bindings))
end
print()

-- Domain groups (optionally filtered)
if #r.groups > 0 then
  print("--- Domains ---")
  for _, g in ipairs(r.groups) do
    if not opts.domain or g.name == opts.domain then
      print(string.format("  %-15s  %d nodes (%.1f%%)", g.name, g.count, g.pct))
    end
  end
end
