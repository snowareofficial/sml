-- SPDX-License-Identifier: MulanPSL-2.0
-- `.ambr` writer in pure Lua 5.3+ — no FFI, no LuaJIT required.
--
-- 纯 Lua 5.3+ 的 `.ambr` 写入器 —— 不需要 FFI，也不需要 LuaJIT。
--
-- Scope / 范围:
--   * Uncompressed, unencrypted containers only (`comp = 0`, `enc = 0`, no DIR/TYPED).
--     / 只写未压缩、未加密的容器（`comp = 0`、`enc = 0`，不含 DIR/TYPED）。
--   * Produces the same bytes as Rust `amberice::dump` for the same value.
--     / 与 Rust `amberice::dump` 对同一值产出相同字节。
--
-- Why 5.3+: the encoder needs integer arithmetic (i64 varint), bitwise ops and
-- `string.pack`. Lua 5.1 / LuaJIT only have doubles, so `i64` above 2^53 cannot be
-- written exactly; `supports()` reports that instead of silently truncating.
-- ｜ 编码器需要整数运算、位运算与 `string.pack`。5.1/LuaJIT 只有 double，
-- i64 超过 2^53 无法精确写；`supports()` 会明确报告，而不是静默截断。
--
-- Byte layout (spec §19.2) / 字节布局:
--   canonical_bytes = count:varint ‖ strtab ‖ node_tree
--   strtab          = (len:varint ‖ utf8)*            -- dedup, id = first-seen order
--   node            = tag:u8 ‖ flags:u8 ‖ subtree_len:u32le ‖ payload
--     Null/False/True → empty
--     Int             → zigzag:varint
--     Float           → f64 little-endian, 8 B
--     Str             → str_id:varint
--     Arr             → count:varint ‖ node*
--     Obj             → count:varint ‖ (key_id:varint ‖ node)*   -- source order
--
-- Key order / 键序:
--   Lua tables do not preserve insertion order, so the writer needs an explicit order.
--   Pass an `order` map: `order[obj] = { "a", "b" }` (the array of keys in source order).
--   Without it, keys fall back to sorted order **and a warning flag is set** — never a
--   silent guess about what the source looked like.
--   ｜ Lua 表不保序，写入需要显式键序：`order[obj] = { "a", "b" }`。
--   不给就退回字典序，并置警告标志 —— 不静默猜测源序。

local M = {}

M.VERSION = "0.1.0"

-- Capability probe: the encoder needs integer arithmetic + bitwise ops.
-- 能力探测：需要整数类型与位运算。
local function has_int_ops()
    local ok, v = pcall(function() return 1 << 1 end)
    return ok and v == 2 and math.maxinteger ~= nil
end

M.HAS_INT = has_int_ops()

--- Whether this runtime can write `.ambr` exactly.
--- 当前运行环境能否**精确**写入 `.ambr`。
function M.supports()
    return M.HAS_INT
end

------------------------------------------------------------------------------
-- varint / zigzag
------------------------------------------------------------------------------

local function uvarint(n)
    local out = {}
    repeat
        local b = n & 0x7f
        n = n >> 7
        if n ~= 0 then b = b | 0x80 end
        out[#out + 1] = string.char(b)
    until n == 0
    return table.concat(out)
end

-- ZigZag: 0 → 0, -1 → 1, 1 → 2, -2 → 3 …
local function zigzag(n)
    return n >= 0 and (n << 1) or ~(n << 1)
end

------------------------------------------------------------------------------
-- SHA-256 (pure Lua) — needed for every segment digest
------------------------------------------------------------------------------

local K = {
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
}

local function ror(x, n)
    return ((x >> n) | (x << (32 - n))) & 0xffffffff
end

local function sha256(msg)
    local h0, h1, h2, h3 = 0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a
    local h4, h5, h6, h7 = 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19

    local len = #msg
    local bits = len * 8
    local padded = msg .. "\128" .. string.rep("\0", (55 - len) % 64)
    -- length as 64-bit big-endian (upper 32 bits are 0 for any realistic segment)
    padded = padded .. string.pack(">I4>I4", math.floor(bits / 4294967296), bits & 0xffffffff)

    local w = {}
    for i = 1, #padded, 64 do
        local chunk = string.sub(padded, i, i + 63)
        for j = 1, 16 do
            w[j] = string.unpack(">I4", chunk, (j - 1) * 4 + 1)
        end
        for j = 17, 64 do
            local s0 = ror(w[j - 15], 7) ~ ror(w[j - 15], 18) ~ (w[j - 15] >> 3)
            local s1 = ror(w[j - 2], 17) ~ ror(w[j - 2], 19) ~ (w[j - 2] >> 10)
            w[j] = (w[j - 16] + s0 + w[j - 7] + s1) & 0xffffffff
        end

        local a, b, c, d, e, f, g, h = h0, h1, h2, h3, h4, h5, h6, h7
        for j = 1, 64 do
            local S1 = ror(e, 6) ~ ror(e, 11) ~ ror(e, 25)
            local ch = (e & f) ~ (~e & g)
            local t1 = (h + S1 + ch + K[j] + w[j]) & 0xffffffff
            local S0 = ror(a, 2) ~ ror(a, 13) ~ ror(a, 22)
            local maj = (a & b) ~ (a & c) ~ (b & c)
            local t2 = (S0 + maj) & 0xffffffff

            h, g, f, e = g, f, e, (d + t1) & 0xffffffff
            d, c, b, a = c, b, a, (t1 + t2) & 0xffffffff
        end

        h0 = (h0 + a) & 0xffffffff
        h1 = (h1 + b) & 0xffffffff
        h2 = (h2 + c) & 0xffffffff
        h3 = (h3 + d) & 0xffffffff
        h4 = (h4 + e) & 0xffffffff
        h5 = (h5 + f) & 0xffffffff
        h6 = (h6 + g) & 0xffffffff
        h7 = (h7 + h) & 0xffffffff
    end

    return string.pack(">I4>I4>I4>I4>I4>I4>I4>I4", h0, h1, h2, h3, h4, h5, h6, h7)
end

M.sha256 = sha256

------------------------------------------------------------------------------
-- Value → canonical blocks
------------------------------------------------------------------------------

local NULL, FALSE, TRUE, INT, FLOAT, STR, ARR, OBJ = 0, 1, 2, 3, 4, 5, 6, 7

-- `{}` is ambiguous in Lua: it is both an empty object and an empty array, and canonical
-- bytes tell them apart (tag 7 vs 6). Callers mark an array with `setmetatable(t, ARRAY_MT)`
-- or `M.array()`; without the marker an empty table is an **empty object**.
-- ｜ `{}` 在 Lua 里无法区分空对象与空数组；用 `M.array()` 标记。
local ARRAY_MT = {}
M.ARRAY_MT = ARRAY_MT

function M.array(...)
    return setmetatable({ ... }, ARRAY_MT)
end

-- `sml.lua` 的空数组用**它自己的** ARRAY_MT 标记；两边各 new 一个就永远认不出对方。
-- 懒加载，避免模块循环依赖。
-- ｜ `sml.lua` marks empty arrays with its own metatable; share it lazily.
local sml_marker = nil   -- nil = 未查；false = 查过但没有
local function shared_array_marker()
    if sml_marker ~= nil then return sml_marker or nil end
    local ok, S = pcall(require, "sml")
    sml_marker = (ok and S.ARRAY_MT) or false
    return sml_marker or nil
end

local function is_array(t)
    if getmetatable(t) == ARRAY_MT then return true end
    local shared = shared_array_marker()
    if shared and getmetatable(t) == shared then return true end
    local n = 0
    for k in pairs(t) do
        if type(k) ~= "number" or k ~= math.floor(k) or k < 1 then return false end
        if k > n then n = k end
    end
    for i = 1, n do if t[i] == nil then return false end end
    return n > 0
end

local function sorted_keys(t)
    local keys = {}
    for k in pairs(t) do keys[#keys + 1] = k end
    table.sort(keys)
    return keys
end

-- String table: dedup, id = first-seen order.
local function new_strtab()
    return { list = {}, map = {} }
end

local function str_id(st, s)
    local id = st.map[s]
    if id then return id end
    id = #st.list
    st.list[#st.list + 1] = s
    st.map[s] = id
    return id
end

-- Encode one node. `order_of(t)` returns the key array for an object.
local function enc_node(v, order_of, st)
    local tv = type(v)

    if v == nil then
        return string.pack("<BBI4", NULL, 0, 0)
    elseif tv == "boolean" then
        return string.pack("<BBI4", v and TRUE or FALSE, 0, 0)
    elseif tv == "number" then
        if math.type(v) == "integer" then
            local p = uvarint(zigzag(v))
            return string.pack("<BBI4", INT, 0, #p) .. p
        end
        local p = string.pack("<d", v)          -- f64 little-endian
        return string.pack("<BBI4", FLOAT, 0, 8) .. p
    elseif tv == "string" then
        local p = uvarint(str_id(st, v))
        return string.pack("<BBI4", STR, 0, #p) .. p
    elseif tv == "table" then
        if is_array(v) then
            local parts = {}
            for i = 1, #v do parts[i] = enc_node(v[i], order_of, st) end
            local p = uvarint(#v) .. table.concat(parts)
            return string.pack("<BBI4", ARR, 0, #p) .. p
        end
        local keys = order_of(v)
        local parts = {}
        local n = 0
        for i = 1, #keys do
            local k = keys[i]
            local val = v[k]
            -- 跳过不存在的键：Lua 里 `nil` 既表示「没有这个键」也表示「值为 null」，
            -- 而 canonical 的对象只编码**实际存在**的键。要写 null 请用 `M.NULL`。
            -- ｜ Skip absent keys; use `M.NULL` for a real null value.
            if val ~= nil then
                n = n + 1
                parts[n] = uvarint(str_id(st, k)) .. enc_node(val, order_of, st)
            end
        end
        local p = uvarint(n) .. table.concat(parts)
        -- flags bit 0 = FLAG_HAS_TYPE：对象含 `__type` 键时置位（Rust `encode_node`）。
        -- ｜ Bit 0 is FLAG_HAS_TYPE, set when the object carries a `__type` key.
        local flags = (v["__type"] ~= nil) and 1 or 0
        return string.pack("<BBI4", OBJ, flags, #p) .. p
    end
    error("ambr_pack: unsupported value type " .. tv, 2)
end

--- Encode a value into (STRTAB, VALUE) segment bodies.
--- 把一个值编码成 (STRTAB, VALUE) 两个段体。
--- @param v any
--- @param order table|nil   order[obj] = { "k1", "k2" }  (source order)
--- @return string strtab, string value, boolean guessed_order
function M.encode_blocks(v, order)
    local st = new_strtab()
    local guessed = false

    local function order_of(t)
        if order and order[t] then return order[t] end
        -- `Sml.load` 把源序记在元表的 `__order` 上（不进值树）
        local mt = getmetatable(t)
        if mt and mt.__order then
            local o = mt.__order
            -- 字节侧是**源序**：`__type` / `__name` 在解析树里是后加的，所以排在末尾；
            -- 文本侧把它们提到最前是 C10 的规定（§18.2 vs §19.2 的已知差异）。
            -- ｜ Byte side keeps metadata keys last (source order), unlike canonical text.
            if t["__type"] ~= nil or t["__featags"] ~= nil then
                local out = {}
                for i = 1, #o do
                    local k = o[i]
                    if k ~= "__type" and k ~= "__name" and k ~= "__featags" then
                        out[#out + 1] = k
                    end
                end
                if t["__featags"] ~= nil then out[#out + 1] = "__featags" end
                if t["__type"] ~= nil then out[#out + 1] = "__type" end
                if t["__name"] ~= nil then out[#out + 1] = "__name" end
                return out
            end
            return o
        end
        guessed = true
        return sorted_keys(t)
    end

    local nodes = enc_node(v, order_of, st)

    local parts = {}
    for i = 1, #st.list do
        local s = st.list[i]
        parts[i] = uvarint(#s) .. s
    end
    return table.concat(parts), nodes, guessed
end

------------------------------------------------------------------------------
-- Manifest (canonical-encoded object, fixed field order)
------------------------------------------------------------------------------

local MANIFEST_ORDER = {
    primary = { "primary", "hash_id", "segments", "payloads", "crypto", "caps", "patch", "defaults", "created", "tool" },
    segment = { "id", "comp", "enc", "len", "sha256" },
    crypto  = { "enc", "sig", "kem", "kdf", "kdf_m", "kdf_t", "kdf_p", "salt" },
    caps    = { "required", "optional" },
    patch   = { "strategy" },
    defaults= { "dir_depth", "crypto_block", "pad_quantum" },
}

-- 逐类精确匹配（不靠启发式）：按已知字段集判断
local function manifest_order(t)
    if t.primary ~= nil then return MANIFEST_ORDER.primary end
    if t.id ~= nil and t.len ~= nil then return MANIFEST_ORDER.segment end
    if t.enc ~= nil and t.kem ~= nil then return MANIFEST_ORDER.crypto end
    if t.required ~= nil then return MANIFEST_ORDER.caps end
    if t.strategy ~= nil then return MANIFEST_ORDER.patch end
    if t.dir_depth ~= nil then return MANIFEST_ORDER.defaults end
    error("ambr_pack: unknown manifest object shape", 2)
end

local function to_hex(s)
    return (s:gsub(".", function(c) return string.format("%02x", string.byte(c)) end))
end

local function build_manifest(segments)
    local segs = {}
    for i = 1, #segments do
        local s = segments[i]
        segs[i] = {
            id = s.id,
            comp = 0,
            enc = 0,
            len = s.len,
            -- manifest 里是 **hex 字符串**（64 字符），不是 32 字节原始摘要。
            -- ｜ The manifest stores the digest as 64-char hex, not raw bytes.
            sha256 = to_hex(s.sha256),
        }
    end

    local m = {
        primary = 1,
        hash_id = 48,                    -- SHA-256 的注册 id（不是序号 1）
        segments = segs,
        crypto = { enc = 0, sig = M.array(), kem = 0 },
        caps = { required = M.array(), optional = M.array() },
        patch = { strategy = "override" },
        defaults = { dir_depth = 4, crypto_block = 4096, pad_quantum = 64 },
    }
    return m
end

------------------------------------------------------------------------------
-- Container assembly
------------------------------------------------------------------------------

local MAGIC = "AMBR"
local EDITION = 0x0004
local FMT = 0
local HEADER_LEN = 40
local SEG_HEADER_LEN = 41
local SEG_VALUE, SEG_STRTAB, SEG_MANIFEST = 0x01, 0x02, 0x04

local function align_up(x, granule)
    local r = x % granule
    if r == 0 then return x end
    return x + (granule - r)
end

--- Pack a value into `.ambr` bytes (uncompressed, unencrypted).
--- 把值打成 `.ambr` 字节（未压缩、未加密）。
--- @param v any
--- @param order table|nil
--- @return string bytes, boolean guessed_order
function M.pack(v, order)
    if not M.HAS_INT then
        error("ambr_pack: needs Lua 5.3+ (integer subtype + bitwise ops) / 需要 Lua 5.3+", 2)
    end

    local strtab, value, guessed = M.encode_blocks(v, order)

    -- 内容段（manifest 的 segments[] 只列它们，不含 manifest 自己）
    local segs = { { id = SEG_VALUE, body = value } }
    if #strtab > 0 then segs[#segs + 1] = { id = SEG_STRTAB, body = strtab } end

    local listed = {}
    for i = 1, #segs do
        listed[i] = {
            id = segs[i].id,
            len = #segs[i].body,
            sha256 = sha256(segs[i].body),
        }
    end

    local manifest_body = (function()
        local st = new_strtab()
        local nodes = enc_node(build_manifest(listed), manifest_order, st)
        local parts = {}
        for i = 1, #st.list do
            local s = st.list[i]
            parts[i] = uvarint(#s) .. s
        end
        -- manifest 段是独立的 canonical_bytes（带 count 前缀）
        return uvarint(#st.list) .. table.concat(parts) .. nodes
    end)()

    segs[#segs + 1] = { id = SEG_MANIFEST, body = manifest_body }
    table.sort(segs, function(a, b) return a.id < b.id end)

    local table_off = HEADER_LEN
    local table_end = table_off + #segs * SEG_HEADER_LEN
    local data_start = align_up(table_end, 8)          -- 未加密：8 B 对齐
    local out = { string.rep("\0", data_start) }

    -- header
    local h = {}
    h[#h + 1] = MAGIC
    h[#h + 1] = string.pack("<I2", EDITION)
    h[#h + 1] = string.char(FMT)
    h[#h + 1] = string.char(HEADER_LEN)
    h[#h + 1] = string.pack("<I8", table_off)
    h[#h + 1] = string.pack("<I4", #segs)
    h[#h + 1] = string.pack("<I2", 0)                  -- flags
    h[#h + 1] = string.pack("<I2", 0)                  -- reserved
    h[#h + 1] = string.pack("<I8", 0)                  -- tail_off
    h[#h + 1] = string.pack("<I8", 0)                  -- reserved
    local header = table.concat(h)

    local table_parts = {}
    local data_parts = {}
    local data_off = data_start
    local cursor = data_start

    for i = 1, #segs do
        local s = segs[i]
        table_parts[i] = string.char(s.id, 0, 0)       -- id, comp=0, enc=0
            .. string.pack("<I2", 0)                    -- seg_flags
            .. string.pack("<I4", #s.body)
            .. sha256(s.body)
    end

    for i = 1, #segs do
        local s = segs[i]
        if cursor < data_off then
            data_parts[#data_parts + 1] = string.rep("\0", data_off - cursor)
        end
        data_parts[#data_parts + 1] = s.body
        data_off = data_off + #s.body
        cursor = data_off
        data_off = align_up(data_off, 8)
    end

    return header .. table.concat(table_parts) .. string.rep("\0", data_start - table_end)
        .. table.concat(data_parts), guessed
end

--- Pack SML **source text** into `.ambr` bytes.
--- 把 SML **源文本**打成 `.ambr` 字节。
---
--- Requires `sml.lua` (the parser records source key order in the value's metatable).
--- Returns `nil, err` on a parse failure.
--- ｜ 依赖 `sml.lua`（解析器把源序记在元表里）；解析失败返回 `nil, err`。
function M.pack_text(text)
    local Sml = require("sml")
    -- 第三个参数是 `allow_top_scalar`：canonical 文本可能是单个标量（`42\n`）。
    local ok, v, err = pcall(Sml.load, text, nil, true)
    if not ok then return nil, tostring(v) end
    if v == nil and err ~= nil then return nil, err end
    return M.pack(v)
end

return M
