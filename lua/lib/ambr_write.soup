-- SPDX-License-Identifier: MulanPSL-2.0
-- ambr_write.soup — optional `.ambr` **writer** for Lua (LuaJIT FFI → amberice_ffi).
-- ambr_write.soup —— Lua 端可选启用的 `.ambr` **写入器**（LuaJIT FFI 调 amberice_ffi）。
--
-- Why FFI and not a pure-Lua encoder / 为什么不用纯 Lua 编码器:
--   `sml.soup` already requires LuaJIT's `ffi` to decode f64 exactly, and its varint
--   reader accumulates in a **double** — a pure-Lua encoder cannot represent i64
--   (the vectors include `9223372036854775807`). Writing through the same C ABI is the
--   only byte-exact route, and it matches the dependency the decoder already has.
--   ｜ 现有解码器的 varint 用 double 累加，纯 Lua 写不了 i64（向量里有 i64 极值）；
--   走同一条 C ABI 才能逐字节精确，且与解码器已有的 LuaJIT 依赖一致。
--
-- This is a **separate module**: `require("ambr_write")` only when you need writes.
-- Without LuaJIT it loads but every call returns a clear error instead of guessing.
-- ｜ 独立模块：需要写入时才 require；没有 LuaJIT 时调用会返回明确错误，不猜。
--
-- Usage / 用法:
--   local W = require("ambr_write")
--   local caps = W.capabilities()          -- bit 0 = read, bit 1 = write
--   local bytes, err = W.pack_text("a: 1\n")
--
-- Library path / 库路径: env `AMBR_LIB` (full name) wins; otherwise `amberice_ffi`
-- on the loader path. Build it with `cargo build -p amberice-ffi --features sml`.

local M = {}

M.CAP_READ = 1
M.CAP_WRITE = 2

local has_ffi, ffi = pcall(require, "ffi")
if not has_ffi then
  -- Keep the module shape; fail at the call site with a message that names the cause.
  -- 保持模块形状；调用时才失败，并说明原因。
  local msg = "AMBR: LuaJIT ffi unavailable — writes need LuaJIT / 写入需要 LuaJIT 的 ffi"
  function M.capabilities() return 0, msg end
  function M.pack_text() return nil, msg end
  function M.pack_text_file() return nil, msg end
  return M
end

pcall(ffi.cdef, [[
unsigned ambr_caps(void);
unsigned char *ambr_pack_text(const char *text, size_t len, size_t *out_len,
                              char *err, size_t errsz);
void ambr_free_bytes(unsigned char *p, size_t len);
]])

local _lib = nil

local function lib()
  if _lib then return _lib end
  local name = os.getenv("AMBR_LIB")
  if name == nil or name == "" then name = "amberice_ffi" end
  local ok, l = pcall(ffi.load, name)
  if not ok then
    return nil, "AMBR: cannot load libamberice_ffi / 加载失败: " .. tostring(l)
  end
  _lib = l
  return _lib
end

-- Capability bits of the loaded cdylib. Returns `0, err` when it cannot even load.
-- 已加载 cdylib 的能力位；连库都加载不了时返回 `0, err`。
function M.capabilities()
  local l, err = lib()
  if not l then return 0, err end
  return tonumber(l.ambr_caps()), nil
end

-- Pack SML source text into `.ambr` bytes. Returns `bytes, nil` or `nil, err`.
-- Objects keep source order; a lone top-level scalar round-trips too.
-- ｜ 把 SML 源文本打成 `.ambr`；对象保源序；顶层标量也可往返。
function M.pack_text(text)
  local l, err = lib()
  if not l then return nil, err end

  local caps = tonumber(l.ambr_caps())
  -- Lua 5.1 has no bitwise ops: `caps & CAP_WRITE` as floor-division.
  if math.floor(caps / M.CAP_WRITE) % 2 == 0 then
    return nil, "AMBR: this build is read-only — rebuild amberice-ffi with --features sml"
  end

  local errbuf = ffi.new("char[256]")
  local out_len = ffi.new("size_t[1]")
  local p = l.ambr_pack_text(text, #text, out_len, errbuf, 256)
  if p == nil then
    return nil, ffi.string(errbuf)
  end

  local n = tonumber(out_len[0])
  local bytes = ffi.string(p, n)
  l.ambr_free_bytes(p, n)
  return bytes, nil
end

-- Pack and write to a file; returns `byte_count, nil` or `nil, err`.
-- 打包并写入文件；返回字节数或错误。
function M.pack_text_file(text, path)
  local bytes, err = M.pack_text(text)
  if not bytes then return nil, err end

  local f, ferr = io.open(path, "wb")
  if not f then return nil, "AMBR: cannot write " .. path .. ": " .. tostring(ferr) end
  f:write(bytes)
  f:close()
  return #bytes, nil
end

return M
