-- W5：把 .sml 用 **Lua 侧** 解析后 dump 成 SML 文本，供跨实现一致性套件比对。
--
-- 用法（驱动脚本负责设 LUA_PATH）：
--   LUA_PATH="<repo>/lua/?.soup;<repo>/lua/?.lua" luajit dump_lua.lua <输出文件> <file.sml> ...
--
-- 为什么**写到文件**而不是 stdout：Windows 上 Lua 的 stdout 是**文本模式**，
-- `io.write("\n")` 会被翻成 "\r\n"。而本套件要比较的是字符串的**内容** ——
-- 一旦 dump 里出现裸换行（如 `s: "a<换行>b"`），那个被翻译出来的 `\r` 就落进
-- 字符串值里，表现为「Lua 解析出的值多了一个 \r」（实测踩过，纯属载体噪声）。
-- 用 "wb" 打开的输出文件可以逐字节控制，噪声消失。
--
-- 输出协议（与 tools/dump_c.c 同一口径）：
--   "DUMP <路径>\n" + dump 原文       解析成功
--   "ERR  <路径> <错误信息>\n"         解析失败（信息以错误码开头）
--
-- 另注（TODO.md 的「复核方法」写过，别踩第二遍）：Lua 解析失败时进程退出码**仍是 0**，
-- 错误只走返回值 ⇒ 判定必须看输出文本，不能看 rc。
local Sml = require("lib.sml")

local out_path = arg[1]
local out = assert(io.open(out_path, "wb"))

local function read_all(path)
  local f = io.open(path, "rb")
  if not f then return nil end
  local t = f:read("a")
  f:close()
  return t
end

for i = 2, #arg do
  local path = arg[i]
  local text = read_all(path)
  if not text then
    out:write("ERR " .. path .. " E-IO-001 sml: 读取失败\n")
  else
    -- include 沙箱根 = **文件所在目录**（W20）：不给 base 就等于 include 关闭。
    local dir = string.match(path, "^(.+)[/\\]") or "."
    local v, err = Sml.load(text, dir)
    if not v then
      out:write("ERR " .. path .. " " .. tostring(err) .. "\n")
    else
      out:write("DUMP " .. path .. "\n" .. Sml.dump(v) .. "\n")
    end
  end
end

out:close()
