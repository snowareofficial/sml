-- SPDX-License-Identifier: MulanPSL-2.0
-- ambr_hash.lua — 零依赖 SM3 / BLAKE3（32 B 摘要），供 `.ambr` 解码器按 manifest 的
-- `hash_id` 校验段摘要（§20）。
--
-- 位运算用 LuaJIT 的 `bit` 库（32 位有符号，统一用 `u32()` 归一回无符号）。
-- 纯 Lua 5.1 没有位运算，故本模块在 **LuaJIT** 下加载；加载失败由调用方明确跳过校验
-- 并标注（绝不退回 SHA-256 假装验过）。
-- ｜ Zero-dependency SM3 / BLAKE3 for `.ambr` segment verification. Uses LuaJIT's `bit`
-- library; callers that cannot load it must **skip verification explicitly** instead of
-- falling back to SHA-256.

local bit = assert(require("bit"), "ambr_hash needs LuaJIT's bit library")

local M = {}

local M32 = 4294967296

local band, bor, bxor, bnot, lshift, rshift, rol, ror =
  bit.band, bit.bor, bit.bxor, bit.bnot, bit.lshift, bit.rshift, bit.rol, bit.ror

--- 把任意整数归一到 [0, 2^32) ｜ normalise to unsigned 32-bit.
local function u32(x) return x % M32 end

local function be32(s, at)
  local a, b, c, d = string.byte(s, at, at + 3)
  return ((a * 256 + b) * 256 + c) * 256 + d
end

local function le32(s, at)
  local a, b, c, d = string.byte(s, at, at + 3)
  return a + b * 256 + c * 65536 + d * 16777216
end

local function b3_words(buf, len)
  local m = {}
  for i = 1, 16 do m[i] = 0 end
  local full = math.floor(len / 4)
  for i = 1, full do m[i] = le32(buf, (i - 1) * 4 + 1) end
  local rem = len % 4
  if rem > 0 then
    local w = 0
    for i = 0, rem - 1 do
      w = w + string.byte(buf, full * 4 + i + 1) * 256 ^ i
    end
    m[full + 1] = u32(w)
  end
  return m
end

local function be_bytes_u32(x, out)
  x = u32(x)
  out[#out + 1] = string.char(math.floor(x / 16777216) % 256)
  out[#out + 1] = string.char(math.floor(x / 65536) % 256)
  out[#out + 1] = string.char(math.floor(x / 256) % 256)
  out[#out + 1] = string.char(x % 256)
end

local function le_bytes_u32(x, out)
  x = u32(x)
  out[#out + 1] = string.char(x % 256)
  out[#out + 1] = string.char(math.floor(x / 256) % 256)
  out[#out + 1] = string.char(math.floor(x / 65536) % 256)
  out[#out + 1] = string.char(math.floor(x / 16777216) % 256)
end

-- ============================ SM3 ============================
local SM3_IV = { 0x7380166f, 0x4914b2b9, 0x172442d7, 0xda8a0600,
                 0xa96f30bc, 0x163138aa, 0xe38dee4d, 0xb0fb0e4e }

local function sm3_p0(x) return u32(bxor(x, bxor(rol(x, 9), rol(x, 17)))) end
local function sm3_p1(x) return u32(bxor(x, bxor(rol(x, 15), rol(x, 23)))) end

--- SM3 of a byte string -> 32-byte string. ｜ 对字节串求 SM3。
function M.sm3(msg)
  local h = { SM3_IV[1], SM3_IV[2], SM3_IV[3], SM3_IV[4],
              SM3_IV[5], SM3_IV[6], SM3_IV[7], SM3_IV[8] }
  local bitlen = #msg * 8
  local body = msg .. "\128"
  while #body % 64 ~= 56 do body = body .. "\0" end
  local hi, lo = math.floor(bitlen / M32), bitlen % M32
  for i = 3, 0, -1 do body = body .. string.char(math.floor(hi / 256 ^ i) % 256) end
  for i = 3, 0, -1 do body = body .. string.char(math.floor(lo / 256 ^ i) % 256) end

  for blk = 1, #body, 64 do
    local w = {}
    for j = 0, 15 do w[j] = be32(body, blk + j * 4) end
    for j = 16, 67 do
      -- W[j] = P1(W[j-16] ^ W[j-9] ^ rol(W[j-3],15)) ^ rol(W[j-13],7) ^ W[j-6]
      w[j] = sm3_p1(u32(bxor(bxor(w[j - 16], w[j - 9]), rol(w[j - 3], 15))))
      w[j] = u32(bxor(bxor(w[j], rol(w[j - 13], 7)), w[j - 6]))
    end
    local w1 = {}
    for j = 0, 63 do w1[j] = u32(bxor(w[j], w[j + 4])) end

    local a, b, c, d = h[1], h[2], h[3], h[4]
    local e, f, g, hh = h[5], h[6], h[7], h[8]
    for j = 0, 63 do
      local tj = (j < 16) and 0x79cc4519 or 0x7a879d8a
      local ss1 = u32(rol(u32(u32(rol(a, 12) + e) + u32(rol(tj, j))), 7))
      local ss2 = u32(bxor(ss1, rol(a, 12)))
      local ff, gg
      if j < 16 then
        ff = u32(bxor(bxor(a, b), c))
        gg = u32(bxor(bxor(e, f), g))
      else
        ff = u32(bor(bor(band(a, b), band(a, c)), band(b, c)))
        gg = u32(bor(band(e, f), band(bnot(e), g)))
      end
      local tt1 = u32(u32(u32(ff + d) + ss2) + w1[j])
      local tt2 = u32(u32(u32(gg + hh) + ss1) + w[j])
      d, c, b, a = c, rol(b, 9), a, tt1
      hh, g, f, e = g, rol(f, 19), e, sm3_p0(tt2)
    end
    h[1] = u32(bxor(h[1], a)); h[2] = u32(bxor(h[2], b))
    h[3] = u32(bxor(h[3], c)); h[4] = u32(bxor(h[4], d))
    h[5] = u32(bxor(h[5], e)); h[6] = u32(bxor(h[6], f))
    h[7] = u32(bxor(h[7], g)); h[8] = u32(bxor(h[8], hh))
  end

  local out = {}
  for i = 1, 8 do be_bytes_u32(h[i], out) end
  return table.concat(out)
end

-- ============================ BLAKE3 ============================
local B3_IV = { 0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19 }

local B3_PERM = { 3, 7, 4, 11, 8, 1, 5, 14, 2, 12, 13, 6, 10, 15, 16, 9 } -- 1-based

local CHUNK_START, CHUNK_END, PARENT, ROOT = 1, 2, 4, 8

local function b3_g(s, a, b, c, d, mx, my)
  s[a] = u32(u32(s[a] + s[b]) + mx)
  s[d] = u32(ror(bxor(s[d], s[a]), 16))
  s[c] = u32(s[c] + s[d])
  s[b] = u32(ror(bxor(s[b], s[c]), 12))
  s[a] = u32(u32(s[a] + s[b]) + my)
  s[d] = u32(ror(bxor(s[d], s[a]), 8))
  s[c] = u32(s[c] + s[d])
  s[b] = u32(ror(bxor(s[b], s[c]), 7))
end

local function b3_round(s, m)
  b3_g(s, 1, 5, 9, 13, m[1], m[2])
  b3_g(s, 2, 6, 10, 14, m[3], m[4])
  b3_g(s, 3, 7, 11, 15, m[5], m[6])
  b3_g(s, 4, 8, 12, 16, m[7], m[8])
  b3_g(s, 1, 6, 11, 16, m[9], m[10])
  b3_g(s, 2, 7, 12, 13, m[11], m[12])
  b3_g(s, 3, 8, 9, 14, m[13], m[14])
  b3_g(s, 4, 5, 10, 15, m[15], m[16])
end

local function b3_permute(m)
  local old = {}
  for i = 1, 16 do old[i] = m[i] end
  for i = 1, 16 do m[i] = old[B3_PERM[i]] end
end

local function b3_compress(cv, blockw, counter, block_len, flags, out)
  local s = { cv[1], cv[2], cv[3], cv[4], cv[5], cv[6], cv[7], cv[8],
              B3_IV[1], B3_IV[2], B3_IV[3], B3_IV[4],
              counter % M32, math.floor(counter / M32) % M32, block_len, flags }
  local m = {}
  for i = 1, 16 do m[i] = blockw[i] end
  for _ = 1, 7 do
    b3_round(s, m)
    b3_permute(m)
  end
  for i = 1, 8 do
    out[i] = u32(bxor(s[i], s[i + 8]))
    out[i + 8] = u32(bxor(s[i + 8], cv[i]))
  end
end

--- BLAKE3 of a byte string -> 32-byte string. ｜ 对字节串求 BLAKE3（32 B）。
function M.blake3(msg)
  local cv = { B3_IV[1], B3_IV[2], B3_IV[3], B3_IV[4],
               B3_IV[5], B3_IV[6], B3_IV[7], B3_IV[8] }
  local counter, buf, buf_len, blocks = 0, "", 0, 0
  local stack, stack_len = {}, 0
  local pos, total = 1, #msg

  local function chunk_len() return 64 * blocks + buf_len end
  local function start_flag() return blocks == 0 and CHUNK_START or 0 end
  local function chunk_output()
    return { cv = cv, words = b3_words(buf, buf_len), counter = counter,
             block_len = buf_len, flags = u32(bor(start_flag(), CHUNK_END)) }
  end
  local function output_cv(o)
    local out = {}
    b3_compress(o.cv, o.words, o.counter, o.block_len, o.flags, out)
    local r = {}
    for i = 1, 8 do r[i] = out[i] end
    return r
  end
  local function parent_output(l, r)
    local w = {}
    for i = 1, 8 do w[i] = l[i]; w[i + 8] = r[i] end
    return { cv = B3_IV, words = w, counter = 0, block_len = 64, flags = PARENT }
  end

  while pos <= total do
    if chunk_len() == 1024 then
      local new_cv = output_cv(chunk_output())
      local tc = counter + 1
      stack_len = stack_len + 1
      stack[stack_len] = new_cv
      while tc % 2 == 0 and stack_len >= 2 do
        local right = stack[stack_len]; stack_len = stack_len - 1
        local left = stack[stack_len]; stack_len = stack_len - 1
        stack_len = stack_len + 1
        stack[stack_len] = output_cv(parent_output(left, right))
        tc = math.floor(tc / 2)
      end
      counter = counter + 1
      cv = { B3_IV[1], B3_IV[2], B3_IV[3], B3_IV[4],
             B3_IV[5], B3_IV[6], B3_IV[7], B3_IV[8] }
      buf, buf_len, blocks = "", 0, 0
    end
    local take = 1024 - chunk_len()
    if take > total - pos + 1 then take = total - pos + 1 end
    local i = 0
    while i < take do
      if buf_len == 64 then
        local out = {}
        b3_compress(cv, b3_words(buf, 64), counter, 64, start_flag(), out)
        local ncv = {}
        for k = 1, 8 do ncv[k] = out[k] end
        cv = ncv
        blocks = blocks + 1
        buf, buf_len = "", 0
      end
      buf = buf .. string.sub(msg, pos, pos)
      pos = pos + 1
      buf_len = buf_len + 1
      i = i + 1
    end
  end

  local o = chunk_output()
  for i = stack_len, 1, -1 do
    o = parent_output(stack[i], output_cv(o))
  end
  local out = {}
  b3_compress(o.cv, o.words, 0, o.block_len, u32(bor(o.flags, ROOT)), out)
  local res = {}
  for i = 1, 8 do le_bytes_u32(out[i], res) end
  return table.concat(res)
end

--- 注入 SHA-256（由 sml.lua 的写侧实现提供，避免重复一份）。
function M.set_sha256(fn) M.sha256 = fn end

--- 按 §20 的 hash_id 取摘要函数；`nil` = 本端算不了（预留号 / 多级元组）。
--- ｜ Dispatch by §20 hash_id; `nil` means "cannot verify".
function M.by_id(hash_id)
  if hash_id == 0x30 then return M.sha256 end
  if hash_id == 0x31 then return M.sm3 end
  if hash_id == 0x32 then return M.blake3 end
  return nil
end

return M
