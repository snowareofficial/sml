-- SPDX-License-Identifier: MulanPSL-2.0
--- SML 文本工具：数字形态判定、是否必须加引号、引号内转义。
---
--- 从 `lib/sml.lua` 抽出（纯 stdlib 依赖，不碰模块状态）。判据与 Rust
--- `sml-value::text_rules` 一致：空串 / true·false·null·inf·nan / 纯数字 /
--- 注释前缀 / 破坏语法的字符 / 首字符非字母下划线 / 含 [A-Za-z0-9_.-] 之外的字符。

local M = {}

------------------------------------------------------------------------------
-- 序列化：dump（round-trip 友好）
------------------------------------------------------------------------------
-- 字符串是否需要加引号：空白（%s 含空格/制表/换行/回车）、" 与 \ 都会破坏裸词语法。
-- #12（换行）、#17（引号）的根因：原只看空白，含 `"`/`\` 原样写出是非法 SML。
-- 纯数字形态（会被 coerce 字面量化）。`0x` / `0b` 前缀不算 —— 与 Rust
-- `f64::parse` 不认十六进制对齐（Lua 的 tonumber 认 `0x20`，不挡住就会与 Rust 分叉）。
function M.looks_number(s)
  local q = s
  local c1 = string.sub(q, 1, 1)
  if c1 == "+" or c1 == "-" then q = string.sub(q, 2) end
  if string.match(q, "^0[xXbB]") then return false end
  if not string.match(q, "^[%d%.]") then return false end
  return tonumber(s) ~= nil
end

-- 是否需要给字符串加引号 —— 与 Rust `sml-value::text_rules::needs_quote` 同判据：
-- 空串 / true·false·null·inf·nan / 纯数字 / 注释前缀 / 破坏语法的字符 /
-- 首字符非字母下划线 / 含 [A-Za-z0-9_.-] 之外的字符。
-- 改前只判空白与 `"` `\` ⇒ `"42"`、`"true"`、`"007"` 全写成裸词，
-- 自己 dump 的文本自己读回来就换了类型（台账 S-05）。
function M.needs_quote(s)
  if s == "" then return true end
  if s == "true" or s == "false" or s == "null" or s == "inf" or s == "nan" then
    return true
  end
  if M.looks_number(s) then return true end
  local h2 = string.sub(s, 1, 2)
  if h2 == "--" or h2 == "//" or h2 == "/*" or h2 == "*/" or h2 == "_*"
     or string.sub(s, 1, 1) == "*" then
    return true
  end
  if string.find(s, '[%s:#{},%[%]"\\/%*]') then return true end
  local first = string.byte(s, 1)
  if not (string.match(string.sub(s, 1, 1), "^[%a_]$") or first >= 0x80) then
    return true
  end
  for k = 1, #s do
    local b = string.byte(s, k)
    if not (string.match(string.sub(s, k, k), "^[%w_%-%.]$") or b >= 0x80) then
      return true
    end
  end
  return false
end

-- 加引号**同时**要转义：`\\ \" \n \t \r \0`，其余控制字符走 `\u00XX`
-- —— 与 Rust `quote_if_needed` 的转义集逐条一致。改前只转义 `\` 与 `"`，
-- 真换行 / 真 NUL 会以裸字节落进文本（台账 S-06）。
function M.escape(s)
  local out = {}
  for k = 1, #s do
    local b = string.byte(s, k)
    if b == 92 then out[#out + 1] = "\\\\"
    elseif b == 34 then out[#out + 1] = '\\"'
    elseif b == 10 then out[#out + 1] = "\\n"
    elseif b == 9 then out[#out + 1] = "\\t"
    elseif b == 13 then out[#out + 1] = "\\r"
    elseif b == 0 then out[#out + 1] = "\\0"
    elseif b < 32 or b == 127 then out[#out + 1] = string.format("\\u%04X", b)
    else out[#out + 1] = string.sub(s, k, k) end
  end
  return table.concat(out)
end

return M
