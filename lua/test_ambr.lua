-- SPDX-License-Identifier: MulanPSL-2.0
-- test_ambr.lua — `.ambr` 解码器对拍容器向量（T7 / N03，Lua 端）。
--
-- 判据：fixture 里 22 个 `ambr_hex` 全部解码成功 + 若干形状逐值正确。
-- 运行：LUA_PATH="lua/?.lua;lua/?.lua;;" luajit lua/test_ambr.lua [fixture.json]
--
-- 先用本目录的 sml.lua；再从仓库根解析 fixture 路径，便于在任意 cwd 下跑。

local here = (arg and arg[0] or ""):match("^(.*)[/\\][^/\\]*$") or "."
package.path = table.concat({
  here .. "/lib/?.lua", here .. "/lib/?.lua", -- sml.lua
  here .. "/?.lua",                            -- sml_errors.lua（生成物在 lua/ 下）
  "lua/lib/?.lua", "lua/lib/?.lua", "lua/?.lua",
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
  elseif id == "scalar-int-i64-min" then
    -- 5.3+ 有 integer 子类型，必须精确到 math.mininteger；5.1 / LuaJIT 只有 double，
    -- 只要求不崩，不要求精确（这是运行时能力，不是解码器的错）。
    -- ｜ 5.3+ must be exact; 5.1 / LuaJIT can only degrade.
    if math.maxinteger then
      if math.type(v) == "integer" and v == math.mininteger then ok(id)
      else bad(id, "期望 math.mininteger，实得 " .. tostring(v) .. " (" .. tostring(math.type(v)) .. ")") end
    else
      ok(id .. " (double runtime, exactness not required)")
    end
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


-- hash_id 分派（§20）：读 tests/vectors/amberice-hash-algos.v1.hex（SM3 / BLAKE3 各一份），
-- 断言能解码且值正确；再改坏第一份的段摘要，断言必须拒绝。
-- ｜ Verify hash_id dispatch against the Rust-generated fixtures, plus a tampered-digest case.
local function run_hash_fixtures()
  local fpath = here .. "/../tests/vectors/amberice-hash-algos.v1.hex"
  local f = io.open(fpath, "rb")
  if not f then
    print("FAIL: hash fixtures -> " .. fpath)
    failures = failures + 1
    return
  end
  local cases2 = 0
  for line in f:lines() do
    if line ~= "" and string.sub(line, 1, 1) ~= "#" then
      local id, hid, hx = string.match(line, "^(%S+)%s+(%d+)%s+(%S+)$")
      if not id then
        print("FAIL: hash fixture format -> " .. line)
        failures = failures + 1
      else
        local bytes = hex_to_bytes(hx)
        local v, err = Sml.ambr_load(bytes)
        if not v then
          print("FAIL: " .. id .. " -> " .. tostring(err))
          failures = failures + 1
        elseif type(v) ~= "table" or v.who ~= "u1" or v.n ~= 7 then
          print("FAIL: " .. id .. " -> value mismatch")
          failures = failures + 1
        else
          print(string.format("  ok: hash_id 0x%02x decoded (%s)", tonumber(hid), id))
          -- 反例：改坏段表首项摘要 ⇒ 必须拒绝
          if cases2 == 0 then
            local table_off = 0
            for i = 8, 1, -1 do
              table_off = table_off * 256 + string.byte(bytes, 9 + i - 1)
            end
            local at = table_off + 9
            if at <= #bytes then
              local c = string.byte(bytes, at)
              local flipped = string.char((c + 1) % 256)
              local bad_bytes = string.sub(bytes, 1, at - 1) .. flipped .. string.sub(bytes, at + 1)
              local v2 = Sml.ambr_load(bad_bytes)
              if v2 then
                print("FAIL: " .. id .. " -> tampered digest still decodes")
                failures = failures + 1
              else
                print("  ok: tampered digest -> rejected (" .. id .. ")")
              end
            end
          end
        end
      end
      cases2 = cases2 + 1
    end
  end
  f:close()
  if cases2 ~= 2 then
    print(string.format("FAIL: hash fixtures expected 2, got %d", cases2))
    failures = failures + 1
  end
end

run_hash_fixtures()

print(string.format("[ambr] %d 例，%d 个失败", cases, failures))
if cases ~= 22 then
  print(string.format("FAIL: 期望 22 例，实得 %d", cases))
  failures = failures + 1
end
print(failures == 0 and "[ambr] ALL OK" or "[ambr] FAILED")
os.exit(failures == 0 and 0 or 1)
