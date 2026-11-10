-- SPDX-License-Identifier: MulanPSL-2.0
-- Lua 端 SCHEMA/TYPED 动态渲染（D45 / §25.8）对拍。
--
-- Run / 运行：
--   lua lua/test_ambr_typed.lua
--   luajit lua/test_ambr_typed.lua
--
-- Fixture 与 JS / C / C++ 共用（hex 由 Rust Writer 生成）。

local here = (debug.getinfo(1, "S").source:gsub("^@", "")):match("^(.*)[/\\]") or "."
package.path = here .. "/lib/?.lua;" .. here .. "/?.lua;" .. package.path
local Sml = require("sml")
local f = assert(io.open(here .. "/../c/tests/typed_fixture.h", "r"))
local src = f:read("*a")
f:close()

local hex = assert(src:match('"([0-9a-f]+)"'), "fixture hex not found")
local bytes = hex:gsub("%x%x", function(h) return string.char(tonumber(h, 16)) end)

local vars, err = Sml.ambr_load_typed(bytes)
if not vars then
  print("FAIL  load_typed: " .. tostring(err))
  os.exit(1)
end

local bad = 0
local function check(ok, what)
  if not ok then
    print("FAIL  " .. what)
    bad = bad + 1
  end
end

check(type(vars) == "table" and #vars == 1, "one variable")
local item = vars[1]
check(item and item.id == 1, "var id = 1")
check(item and item.name == "User", "type name = User")
local v = item and item.value
check(type(v) == "table", "value is object")
check(v and v.id == 42, "field id = 42")
check(v and v.name == "ada", "field name = ada")
check(v and type(v.tags) == "table" and #v.tags == 2 and v.tags[1] == "a" and v.tags[2] == "b",
      "tags = [a, b]")
check(v and v.note == nil, "note = null")

if bad > 0 then os.exit(1) end
print("ok  1/1 typed render")
