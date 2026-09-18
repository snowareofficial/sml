-- test_codes.lua — 「触发条件 → 期望码」回归（W10，Lua 侧）
--
-- 职责不是「Lua 报了错」，而是钉住**报的是哪个码**：码是跨端稳定契约，文案不是
-- （见 errors/README.md 与 errors/codes.sml）。期望值直接写码字面量，这些字面量
-- 会被 `errors/gen_codes.py` 的反向校验扫到（`lua/lib/sml.soup` 在它的清单里，
-- 本文件不在 —— 本文件的期望值就是抄码表，抄错会被下面的断言当场抓住）。
--
-- 覆盖范围：Lua 侧**真的会报**的码。故意不测的（给不存在的东西编用例等于发护照）：
--   * E-INCLUDE-001：Lua 实现**没有 include 语法**（`include "x"` 与 `@include "x"`
--     都被静默当普通键），本条对它不适用 —— 码表已把 lua 从 impls 移除。
--   * E-LEX-001（未闭合字符串）、E-LEX-002/003（未闭合块注释）、顶层标量：
--     属 errors/README.md「静默清单」里**已登记**的静默点，归 W16 判定。
--     本文件把它们放在「不许一报到底」组里**反向钉住**：现在不该发码。
--   * E-PARSE-012（解析阶段兜底）：Lua 侧它是 pcall 的兜底出口，**构造不出来** ——
--     与 C++ 的 E-LIMIT-010 同类，属「有实现、无可用例」，显式登记在此，
--     免得后来者以为它没做。
--
-- 用法：luajit lua/test_codes.lua      （或走 lua/run_check.py，它会自己找解释器）

local self = arg[0] or "test_codes.lua"
local root = self:match("^(.+)[/\\]") or "."
package.path = root .. "/?.soup;" .. root .. "/?/init.soup;" .. package.path
local Sml = require("lib.sml")

local failures = 0
local passed = 0

-- 码是消息的**第一个空白分隔的字段**（与 c/sml.h、cpp/sml.hpp 同一口径）。
-- 不写 startswith 判定：那样 "E-PARSE-011" 也会命中 "E-PARSE-01"。
local function code_is(err, want)
  if type(err) ~= "string" then return false end
  return err:match("^(%S+)") == want
end

local function report(tag, want, err, ok)
  if ok then
    passed = passed + 1
    io.write(string.format("  ok: %s -> %s\n", tag, want))
  else
    failures = failures + 1
    io.write(string.format("FAIL: %s want %s, got \"%s\"\n", tag, want, tostring(err)))
  end
end

-- 失败（返回 nil）且码相符
local function expect_code(tag, src, want)
  local v, err = Sml.load(src)
  if v ~= nil then
    failures = failures + 1
    io.write(string.format("FAIL: %s parsed OK but should fail\n", tag))
    return
  end
  report(tag, want, err, code_is(err, want))
end

-- 正向对照：这些输入**必须通过**。防的是「把码改成一报到底」——
-- 只测失败路径的套件，在整条判定被反转时依然会全绿。
local function expect_ok(tag, src)
  local v, err = Sml.load(src)
  if v == nil then
    failures = failures + 1
    io.write(string.format("FAIL: %s should parse OK, got \"%s\"\n", tag, tostring(err)))
    return
  end
  passed = passed + 1
  io.write(string.format("  ok: %s -> (no error)\n", tag))
end

-- ------------------------------------------------------------------
-- 语法：E-PARSE-*
-- ------------------------------------------------------------------
local function test_parse_codes()
  io.write("[PARSE]\n")

  -- 未闭合的块 / 数组 → E-PARSE-001。
  -- 修复前这三种全是**静默成功**：`a { b: 1` 得到 {a={b=1}}，用户拿着被截断的文档
  -- 却没有任何提示（探针实测）。这正是本组最要紧的判别用例。
  expect_code("unclosed block", "a { b: 1", "E-PARSE-001")
  expect_code("unclosed nested block", "a { b { c: 1 }", "E-PARSE-001")
  expect_code("unclosed array", "a: [ 1", "E-PARSE-001")
  expect_code("unclosed top-level brace block", "{ a: 1", "E-PARSE-001")

  -- 多余的结束符号 → E-PARSE-003（原先报错但不带码）
  expect_code("stray }", "a: 1\n}", "E-PARSE-003")
  expect_code("stray ]", "a: 1\n]", "E-PARSE-003")

  -- 键位置的结构记号 → E-PARSE-006。
  -- 修复前 `: 1` 是**静默**的（`:` 成了键名！），`a { { x } }` 则报成 E-PARSE-003
  -- （错误的码 —— 真正的原因是键位置给了结构记号）。
  expect_code("key position is :", ": 1", "E-PARSE-006")
  expect_code("key position is {", "a { { x } }", "E-PARSE-006")
  expect_code("key position is [", "a { [ 1 ] }", "E-PARSE-006")
  expect_code("key position is : inside block", "a { : 1 }", "E-PARSE-006")
end

-- ------------------------------------------------------------------
-- 特性：E-FEATURE-004（版本声明）
-- ------------------------------------------------------------------
local function test_feature_codes()
  io.write("[FEATURE]\n")

  expect_code("version v2 (too new)", "@version v2\na: 1\n", "E-FEATURE-004")
  expect_code("version v9 (unknown)", "@version v9\na: 1\n", "E-FEATURE-004")
  expect_code("version 42 (unknown)", "@version 42\na: 1\n", "E-FEATURE-004")
end

-- ------------------------------------------------------------------
-- 宿主绑定：E-INTERNAL-001
-- ------------------------------------------------------------------
local function test_host_codes()
  io.write("[HOST]\n")

  -- 非字符串入参 —— 与 C 的 `sml_parse(NULL)` 同一个格子（探针实测原先返回的是
  -- 不带码的「sml.load: 期望字符串」）。
  local v, err = Sml.load(12345)
  report("non-string argument", "E-INTERNAL-001", err,
         v == nil and code_is(err, "E-INTERNAL-001"))
  local v2, err2 = Sml.load(nil)
  report("nil argument", "E-INTERNAL-001", err2,
         v2 == nil and code_is(err2, "E-INTERNAL-001"))
end

-- ------------------------------------------------------------------
-- 正向对照 + 「不许一报到底」
--
-- 后半段尤其重要：静默清单里那几条（未闭合字符串、未闭合块注释、顶层标量）
-- 是**已登记**的静默点，归 W16 判定。本轮只补「声明了 lua 却不报」的那几条，
-- 如果顺手把它们也改成报错，就是**偷偷扩了范围** —— 这组用例专门拦住这件事。
-- ------------------------------------------------------------------
local function test_positive_controls()
  io.write("[positive controls]\n")

  expect_ok("simple document", "a: 1\nb { c: 2 }\n")
  expect_ok("top-level array", "[ 1 2 3 ]\n")
  expect_ok("top-level brace block closed", "{ a: 1 }\n")
  expect_ok("version v1 accepted", "@version v1\na: 1\n")
  expect_ok("empty input", "")
  expect_ok("fragment define + reference", "@a { x: 1 }\nk: &a\n")

  -- 顶层裸块到文件结尾收尾是**合法**的（这是 E-PARSE-001 最容易误伤的一格：
  -- 把「顶层裸块」也当成未闭合，会让所有没写外层花括号的文档全部报错）。
  expect_ok("top-level bare block ends at EOF", "a: 1\nb: 2\n")

  -- 已登记的静默点：现在**不该**发码
  expect_ok("silent (registered): top-level scalar", "42\n")
  expect_ok("silent (registered): unterminated string", "k: \"abc\n")
  expect_ok("silent (registered): unclosed /* comment", "k: 1\n/* never closed\n")
  expect_ok("silent (registered): unclosed _* comment", "k: 1\n_* never closed\n")
end

io.write("lua/test_codes.lua — 触发条件 → 期望码（W10，Lua 侧）\n")
test_parse_codes()
test_feature_codes()
test_host_codes()
test_positive_controls()

io.write(string.format("\n%d 通过, %d 失败\n", passed, failures))
if failures == 0 then
  io.write("ALL CODE TESTS PASSED\n")
  os.exit(0)
end
io.write(failures .. " FAILURES\n")
os.exit(1)
