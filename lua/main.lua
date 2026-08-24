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
  print("sml (Soup Markup Language) v1.0 -- Soup 生态声明式数据格式库")
  print("用法: soupx sml.sar <file.sml>   解析并打印结果")
  print("作为库: local sml = require('lib.sml'); local v,err = sml.load(text)")
  -- 内置自检
  local v = Sml.load("a: 1\nb: { c: 'hi' }")
  print("self-test: a=" .. tostring(v.a) .. " b.c=" .. tostring(v.b.c))
else
  local f = io.open(arg[1], "r")
  if not f then
    io.stderr:write("文件不存在: " .. arg[1] .. "\n")
    return
  end
  local text = f:read("a"); f:close()
  local v, err = Sml.load(text)
  if err then
    io.stderr:write("解析失败: " .. err .. "\n")
  else
    print(Sml.dump(v))
  end
end
