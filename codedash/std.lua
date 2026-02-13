--[[
  std.lua — Rust-Lua stdlib shim

  Contract defined in rustlib.json.
  Prototype mode: uses lua-cjson and io.open.
  Production mode: Rust injects via mlua before this module loads.

  Usage:
    local std = require("codedash.std")
    local data = std.json.decode(str)
    local content = std.fs.read_file("path.json")

  mlua injection (Rust side):
    lua.globals().set("__rustlib", injected_table)?;
    -- Then codedash.std picks it up automatically.
]]

local M = {}

-- ============================================================
-- Check for Rust-injected stdlib (mlua production mode)
-- ============================================================
local injected = rawget(_G, "__rustlib")

-- ============================================================
-- json
-- ============================================================
if injected and injected.json then
  M.json = injected.json
else
  local ok, cjson = pcall(require, "cjson")
  if not ok then
    error("std.json: lua-cjson not found. Install via: luarocks install lua-cjson")
  end

  local cjson_null = cjson.null

  local function sanitize_null(v)
    if v == cjson_null then return nil end
    if type(v) == "table" then
      local clean = {}
      for k, val in pairs(v) do
        clean[k] = sanitize_null(val)
      end
      return clean
    end
    return v
  end

  M.json = {
    decode = function(str)
      return sanitize_null(cjson.decode(str))
    end,
    encode = function(tbl)
      return cjson.encode(tbl)
    end,
  }
end

-- ============================================================
-- fs
-- ============================================================
if injected and injected.fs then
  M.fs = injected.fs
else
  M.fs = {
    read_file = function(path)
      local f, err = io.open(path, "r")
      if not f then error(string.format("std.fs.read_file: %s", err), 2) end
      local content = f:read("*a")
      f:close()
      return content
    end,
    file_exists = function(path)
      local f = io.open(path, "r")
      if f then f:close(); return true end
      return false
    end,
  }
end

return M
