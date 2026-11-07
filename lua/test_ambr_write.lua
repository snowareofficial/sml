-- SPDX-License-Identifier: MulanPSL-2.0
-- test_ambr_write.lua — optional `.ambr` writer against the container vectors.
-- test_ambr_write.lua —— Lua 端可选写入器对拍容器向量。
--
-- Judgement / 判据:
--   1. `pack_text(canonical_text)` succeeds for all 21 vectors;
--   2. `ambr_load` of the packed bytes succeeds for all 21;
--   3. the five shapes already asserted by the decoder test keep their values.
--   Byte-for-byte canonical parity is asserted by the ends that own a canonical emitter
--   (Rust / JS). Lua reads values, so it checks semantics here.
--   ｜ 逐字节对拍由有 canonical 发射器的端负责；Lua 验语义。
--
-- Run / 运行:
--   AMBR_LIB=... LUA_PATH="lua/lib/?.lua;lua/?.lua;;" luajit lua/test_ambr_write.lua

local here = (arg and arg[0] or ""):match("^(.*)[/\\][^/\\]*$") or "."
package.path = table.concat({
  here .. "/lib/?.lua", here .. "/lib/?.lua",
  here .. "/?.lua",
  "lua/lib/?.lua", "lua/lib/?.lua", "lua/?.lua",
  package.path,
}, ";")

local Sml = require("sml")
local W = require("ambr_write")

local path = (arg and arg[1]) or (here .. "/../tests/vectors/amberice-container.v1.json")

local function read_all(p)
  local f = assert(io.open(p, "rb"), "打不开 " .. p)
  local s = f:read("*a")
  f:close()
  return s
end

-- Read a JSON string body, skipping escaped quotes. Returns the **raw** (still escaped)
-- body and the next search position. ｜ 取 JSON 字符串体，跳过 \"；返回原始（未反转义）内容。
local function take_json_string(text, from, needle)
  local at = string.find(text, needle, from, true)
  if not at then return nil end
  local i = at + #needle
  local out = {}
  while i <= #text do
    local c = string.sub(text, i, i)
    if c == "\\" then
      out[#out + 1] = string.sub(text, i, i + 1)
      i = i + 2
    elseif c == '"' then
      return table.concat(out), i + 1
    else
      out[#out + 1] = c
      i = i + 1
    end
  end
  return nil
end

local function json_unescape(s)
  return (s:gsub("\\(.)", function(c)
    if c == "n" then return "\n" end
    if c == "t" then return "\t" end
    if c == "r" then return "\r" end
    if c == "b" then return "\b" end
    if c == "f" then return "\f" end
    return c -- covers \" \\ \/ and leaves unknown escapes as their literal char
  end))
end

local caps, caps_err = W.capabilities()
if caps == 0 then
  print("[ambr-write] SKIP: " .. tostring(caps_err))
  os.exit(0)
end

print(string.format("[ambr-write] capabilities=%d (write=%s)",
  caps, tostring(math.floor(caps / W.CAP_WRITE) % 2 == 1)))
if math.floor(caps / W.CAP_WRITE) % 2 == 0 then
  print("[ambr-write] FAIL: read-only cdylib / 只读构建")
  os.exit(1)
end

local json = read_all(path)
local failures, cases, packed_ok = 0, 0, 0
local pos = 1

local function bad(id, msg)
  print(string.format("FAIL: %s — %s", id, msg))
  failures = failures + 1
end

while true do
  local id, after_id = take_json_string(json, pos, '"id": "')
  if not id then break end
  local raw, after_text = take_json_string(json, after_id, '"canonical_text": "')
  if not raw then break end

  local text = json_unescape(raw)
  local bytes, perr = W.pack_text(text)
  if not bytes then
    bad(id, "pack: " .. tostring(perr))
  else
    packed_ok = packed_ok + 1
    local v, lerr = Sml.ambr_load(bytes)
    if lerr then
      bad(id, "load: " .. tostring(lerr))
    elseif id == "scalar-int" then
      if v == 1 then print("  ok: " .. id) else bad(id, "期望整数 1") end
    elseif id == "flat-array" then
      if type(v) == "table" and #v == 3 and v[1] == 1 and v[3] == 3 then print("  ok: " .. id)
      else bad(id, "期望 [1 2 3]") end
    elseif id == "strtab-dedup" then
      if type(v) == "table" and v.k == "k" and v.other == "k" then print("  ok: " .. id)
      else bad(id, "期望 k/other 都是 k") end
    elseif id == "nested-containers" then
      if type(v) == "table" and type(v.config) == "table" and v.config.port == 8080
         and type(v.tags) == "table" and v.tags[1] == "x" then print("  ok: " .. id)
      else bad(id, "期望 config.port=8080 / tags=[x]") end
    elseif id == "typed-block" then
      if type(v) == "table" and v.__name == "ssh" and v.action == "allow" then print("  ok: " .. id)
      else bad(id, "期望 __name=ssh / action=allow") end
    end
  end

  cases = cases + 1
  pos = after_text
end

print(string.format("[ambr-write] %d 例，%d 个 pack 成功，%d 个失败", cases, packed_ok, failures))
if cases ~= 21 then
  print(string.format("FAIL: 期望 21 例，实得 %d", cases))
  failures = failures + 1
end
print(failures == 0 and "[ambr-write] ALL OK" or "[ambr-write] FAILED")
os.exit(failures == 0 and 0 or 1)
