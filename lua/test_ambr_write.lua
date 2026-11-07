-- SPDX-License-Identifier: MulanPSL-2.0
-- test_ambr_write.lua — `.ambr` writer against the container vectors.
-- test_ambr_write.lua —— Lua 端写入器对拍容器向量。
--
-- Two backends / 两个后端:
--   1. `ambr_write` — LuaJIT FFI to `amberice_ffi` (needs a cdylib).
--   2. `ambr_pack`  — pure Lua 5.3+ encoder (no FFI, no cdylib).
--   FFI wins when both work; the pure encoder is the fallback, so Lua 5.5 can write too.
--   ｜ 有 FFI 用 FFI；否则用纯 Lua 5.3+ 编码器。
--
-- Judgement / 判据:
--   1. `pack_text(canonical_text)` succeeds for all 21 vectors;
--   2. the produced bytes are **identical** to `ambr_hex` in the vector file;
--   3. `ambr_load` of those bytes succeeds and the five shapes keep their values.
--   ｜ 21 例全部可打包、**逐字节**等于向量、读回后五种形状语义正确。
--
-- Run / 运行:
--   LUA_PATH="lua/lib/?.lua;lua/?.lua;;" lua5.5 lua/test_ambr_write.lua
--   AMBR_LIB=... LUA_PATH="lua/lib/?.lua;lua/?.lua;;" luajit lua/test_ambr_write.lua

local here = (arg and arg[0] or ""):match("^(.*)[/\\][^/\\]*$") or "."
package.path = table.concat({
  here .. "/lib/?.lua",
  here .. "/?.lua",
  "lua/lib/?.lua", "lua/?.lua",
  package.path,
}, ";")

local Sml = require("sml")
local pack_text
local mode

-- 1) FFI backend, when a writable cdylib is present.
do
  local ok, W = pcall(require, "ambr_write")
  if ok and W then
    local caps = W.capabilities and W.capabilities()
    if caps and math.floor(caps / W.CAP_WRITE) % 2 == 1 then
      pack_text = function(t) return W.pack_text(t) end
      mode = "ffi"
    end
  end
end

-- 2) Pure Lua 5.3+ backend.
if not pack_text then
  local ok, P = pcall(require, "ambr_pack")
  if ok and P and P.supports() then
    pack_text = function(t) return P.pack_text(t) end
    mode = "pure"
  end
end

if not pack_text then
  print("[ambr-write] SKIP: no writable cdylib and no Lua 5.3+ (integer subtype)")
  print("[ambr-write] SKIP: 既无可用 cdylib，也不是 Lua 5.3+")
  os.exit(0)
end

print("[ambr-write] backend=" .. mode)

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

local function hex_to_bytes(h)
  return (h:gsub("%x%x", function(x) return string.char(tonumber(x, 16)) end))
end

local json = read_all(path)
local failures, cases, packed_ok, bytes_ok = 0, 0, 0, 0
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
  local hx = string.match(json, '"ambr_hex": "([^"]+)"', after_text)

  local text = json_unescape(raw)
  local bytes, perr = pack_text(text)
  if not bytes then
    bad(id, "pack: " .. tostring(perr))
  else
    packed_ok = packed_ok + 1

    -- Byte-for-byte against the vector: both backends must agree with Rust.
    if hx then
      local want = hex_to_bytes(hx)
      if bytes == want then
        bytes_ok = bytes_ok + 1
      else
        local d = 1
        while d <= math.min(#bytes, #want) and bytes:byte(d) == want:byte(d) do d = d + 1 end
        bad(id, string.format("bytes differ at %d (%02x vs %02x, %d vs %d B)",
          d, bytes:byte(d) or 0, want:byte(d) or 0, #bytes, #want))
      end
    end

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

print(string.format("[ambr-write] %d 例，%d 个 pack 成功，%d 个字节一致，%d 个失败",
  cases, packed_ok, bytes_ok, failures))
if cases ~= 21 then
  print(string.format("FAIL: 期望 21 例，实得 %d", cases))
  failures = failures + 1
end
print(failures == 0 and "[ambr-write] ALL OK" or "[ambr-write] FAILED")
os.exit(failures == 0 and 0 or 1)
