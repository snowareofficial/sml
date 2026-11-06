-- SPDX-License-Identifier: MulanPSL-2.0
-- test_ambr.lua — `.ambr` 解码器对拍容器向量（T7 / N03，Lua 端）。
--
-- 判据：fixture 里 21 个 `ambr_hex` 全部解码成功 + 若干形状逐值正确。
-- 运行：LUA_PATH="lua/?.soup;lua/?.lua;;" luajit lua/test_ambr.lua [fixture.json]
--
-- 先用本目录的 sml.soup；再从仓库根解析 fixture 路径，便于在任意 cwd 下跑。

local here = (arg and arg[0] or ""):match("^(.*)[/\\][^/\\]*$") or "."
package.path = table.concat({
  here .. "/lib/?.soup", here .. "/lib/?.lua", -- sml.soup
  here .. "/?.lua",                            -- sml_errors.lua（生成物在 lua/ 下）
  "lua/lib/?.soup", "lua/lib/?.lua", "lua/?.lua",
  package.path,
}, ";")
local Sml = require("sml")

local path = (arg and arg[1]) or (here .. "/../tests/vectors/amberice-container.v1.json")

local function read_all(p)
  local f = assert(io.open(p, "rb"), "打不开 " .. p)
  local s = f:read("*a")
  f:close()
  return s
end

local function hex_to_bytes(h)
  local out = {}
  for i = 1, #h, 2 do
    out[#out + 1] = string.char(tonumber(string.sub(h, i, i + 1), 16))
  end
  return table.concat(out)
end

local function take_quoted(text, from, needle)
  local at = string.find(text, needle, from, true)
  if not at then return nil end
  local s = at + #needle
  local e = string.find(text, '"', s, true)
  if not e then return nil end
  return string.sub(text, s, e - 1), e + 1
end

local json = read_all(path)
local failures, cases = 0, 0
local pos = 1

local function ok(id) print("  ok: " .. id) end
local function bad(id, msg)
  print(string.format("FAIL: %s — %s", id, msg))
  failures = failures + 1
end

while true do
  local id, after_id = take_quoted(json, pos, '"id": "')
  if not id then break end
  local hex, after_hex = take_quoted(json, after_id, '"ambr_hex": "')
  if not hex then break end

  local v, err = Sml.ambr_load(hex_to_bytes(hex))
  if err then
    bad(id, err)
  elseif id == "scalar-int" then
    if v == 1 then ok(id) else bad(id, "期望整数 1") end
  elseif id == "flat-array" then
    if type(v) == "table" and #v == 3 and v[1] == 1 and v[3] == 3 then ok(id)
    else bad(id, "期望 [1 2 3]") end
  elseif id == "strtab-dedup" then
    if type(v) == "table" and v.k == "k" and v.other == "k" then ok(id)
    else bad(id, "期望 k/other 都是 k") end
  elseif id == "nested-containers" then
    if type(v) == "table" and type(v.config) == "table" and v.config.port == 8080
       and type(v.tags) == "table" and v.tags[1] == "x" then ok(id)
    else bad(id, "期望 config.port=8080 / tags=[x]") end
  elseif id == "typed-block" then
    if type(v) == "table" and v.__name == "ssh" and v.action == "allow" then ok(id)
    else bad(id, "期望 __name=ssh / action=allow") end
  end

  cases = cases + 1
  pos = after_hex
end

print(string.format("[ambr] %d 例，%d 个失败", cases, failures))
if cases ~= 21 then
  print(string.format("FAIL: 期望 21 例，实得 %d", cases))
  failures = failures + 1
end
print(failures == 0 and "[ambr] ALL OK" or "[ambr] FAILED")
os.exit(failures == 0 and 0 or 1)
