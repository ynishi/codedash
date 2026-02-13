--[[
  examples/basic_run.lua — Domain classification example

  Demonstrates codedash with domain classification.
  Uses the sample auth-service dataset by default.

  Usage:
    lua examples/basic_run.lua                              -- sample data
    lua examples/basic_run.lua path/to/enriched.json        -- custom data
]]

-- Setup paths (resolve from project root)
local info = debug.getinfo(1, "S")
local example_dir = info.source:match("^@(.*/)") or "./"
package.path = package.path
  .. ";" .. example_dir .. "../?.lua"
  .. ";" .. example_dir .. "../?/init.lua"

local codedash = require("codedash")

-- ================================================================
-- Config with domain classification
-- ================================================================
local config = {
  extends = "recommended",

  domains = {
    { name = "auth",    patterns = { "auth" } },
    { name = "crypto",  patterns = { "crypto" } },
    { name = "data",    patterns = { "repository" } },
    { name = "token",   patterns = { "token" } },
    { name = "errors",  patterns = { "errors" } },
  },
}

-- ================================================================
-- Run
-- ================================================================
local source = arg[1] or (example_dir .. "sample_enriched.json")
local instance = codedash.init(source, config)

print(string.format("Loaded %d nodes\n", instance:count()))

local r = instance:run()
local report = codedash.report

-- Summary
print(report.summary(r))
print()

-- Top 10
print("--- Top 10 ---")
local top = report.top(r, 10)
for _, entry in ipairs(top) do
  print("  " .. report.format_entry(entry, r.bindings))
end
print()

-- Domain groups
if #r.groups > 0 then
  print("--- Domains ---")
  for _, g in ipairs(r.groups) do
    print(string.format("  %-15s  %d nodes (%.1f%%)", g.name, g.count, g.pct))
  end
end
