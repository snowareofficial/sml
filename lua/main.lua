-- SPDX-License-Identifier: MulanPSL-2.0
-- sml-lib 入口（.sar 归档入口）
-- 设置 package.path 使其 lib/ 可被 require，并暴露 sml API。
-- 直接运行 soupx sml.sar 时打印版本并演示一次解析。

local src = debug.getinfo(1, "S").source
local self = src:match("^@(.+)$") or src
local dir = self:match("^(.+)[/\\]") or "."
local libdir = dir .. "/?.soup;" .. dir .. "/?/init.soup"
package.path = libdir .. ";" .. package.path

local Sml = require("lib.sml")

if not arg or #arg == 0 then
  print("sml (SNOWARE Markup Language) v1.0 -- SNOWARE 生态声明式数据格式库")
  print("用法: soupx sml.sar <file.sml>   解析并打印结果")
  print("作为库: local sml = require('lib.sml'); local v,err = sml.load(text)")
  -- 内置自检
  local v = Sml.load("a: 1\nb: { c: 'hi' }")
  print("self-test: a=" .. tostring(v.a) .. " b.c=" .. tostring(v.b.c))
else
  local f = io.open(arg[1], "r")
  if not f then
    -- 入口文件读不出来 → E-IO-001（与 C 的 sml_parse_file fopen 失败同码）。
    -- 取码：err:match("^(%S+)") —— 码是稳定契约，文案不是（见 errors/codes.sml）。
    io.stderr:write("E-IO-001 sml: 读取失败，文件不存在或不可读: " .. arg[1] .. "\n")
    return
  end
  local text = f:read("a"); f:close()
  -- include 沙箱根 = **输入文件所在目录**（W20 第二阶段）。
  -- 不给 base 就等同于「include 关闭」（与改动前一致）；给了才展开，
  -- 且 `@include "../x"` 这类越界会被拒。无目录部分时退化为当前目录 "."。
  local dir = string.match(arg[1], "^(.+)[/\\]") or "."
  local v, err = Sml.load(text, dir)
  if err then
    io.stderr:write("解析失败: " .. err .. "\n")
  else
    print(Sml.dump(v))
  end
end
