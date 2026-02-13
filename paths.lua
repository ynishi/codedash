-- paths.lua — Package path setup for codedash (development use)
-- When installed via LuaRocks, this file is not needed.
local info = debug.getinfo(1, "S")
local dir = info.source:match("^@(.*/)") or "./"

package.path = package.path
  .. ";" .. dir .. "?.lua"
  .. ";" .. dir .. "?/init.lua"
