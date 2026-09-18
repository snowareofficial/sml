-- test_codes.lua — 「触发条件 → 期望码」回归（W10，Lua 侧）
--
-- 职责不是「Lua 报了错」，而是钉住**报的是哪个码**：码是跨端稳定契约，文案不是
-- （见 errors/README.md 与 errors/codes.sml）。期望值直接写码字面量，这些字面量
-- 会被 `errors/gen_codes.py` 的反向校验扫到（`lua/lib/sml.soup` 在它的清单里，
-- 本文件不在 —— 本文件的期望值就是抄码表，抄错会被下面的断言当场抓住）。
--
-- 覆盖范围：Lua 侧**真的会报**的码。故意不测的（给不存在的东西编用例等于发护照）：
--   * （W20 第二阶段**作废**了原先这条）~~E-INCLUDE-001：Lua 没有 include 语法~~
--     —— Lua 现在有 include（`include "x"` 与 `@include "x"` 两种都认），本条已有用例；
--     include 的高级写法（`as ns` / glob / regex / 多目标 / 部分引用）显式报
--     E-FEATURE-001，也已有用例。故这两条不再属于「故意不测」。
--   * E-LEX-001（未闭合字符串）、E-LEX-002/003（未闭合块注释）、顶层标量：
--     属 errors/README.md「静默清单」里**已登记**的静默点，归 W16 判定。
--     本文件把它们放在「不许一报到底」组里**反向钉住**：现在不该发码。
--   * E-PARSE-012（解析阶段兜底）：Lua 侧它是 pcall 的兜底出口，**构造不出来** ——
--     与 C++ 的 E-LIMIT-010 同类，属「有实现、无可用例」，显式登记在此，
--     免得后来者以为它没做。
--   * 「纯数组嵌套」`a: [[[ … ]]]`：Lua 的 parse_array **不递归处理 `[`**（数组元素位置的
--     `[` 被当裸词 coerce），所以它既不递归、也无从触发深度上限 —— 属**数据正确性**
--     问题（与 C 已登记的「嵌套数组被静默丢弃」同类），不在本组范围。数组入口的
--     深度上限改用「块 / 数组交替」形式测（`a { x: [ … ] }`，见 test_limit_codes）。
--   * E-CONTRACT-007 / 009 / 011（外置类型、模式类型、外置修饰符）：Lua 没有 `@type`
--     与扩展注册点，属另一件事（W20 报告已登记），故不测；E-PARSE-024 是 C++ 把契约
--     解析与取值校验并入同一层后自有的码，本端不可达。E-CONTRACT-012 只有
--     「数组元素类型未知」这一条可达分支（本端没有外置类型）。
--
-- W20 起**契约（@contract / @is）已在 Lua 落地**：见文件后半的 test_contract_codes
-- 与 test_contract_positive。口径逐条对照 rust/sml-parse/src/parser.rs 的
-- parse_field_spec 与 rust/sml-contract/src/lib.rs 的 apply_contract。
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
  -- ⚠️ 必须把解析结果**返回**：契约组的正向用例要在上面「断言填出来的值」，
  --    原先这里没有 return，于是 `local v = expect_ok(...)` 恒为 nil、
  --    那些断言会被整段跳过 —— 套件照样全绿，却没有证明任何默认值。
  return v
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
-- 深度上限：E-LIMIT-001（W15，上限 128 层，与 Rust/C/C++ 同口径）
--
-- 判别实验（改动前实测）：
--   * 129 / 200 层块嵌套 → **静默解析成功**（拿到的是被套了 200 层的树，无任何提示）
--   * 块/数组交替 65 / 100 层 → 同样静默成功
--   * 20000 层 → 报的是 `E-PARSE-012 ... stack overflow`（pcall 兜底码），
--     即「靠宿主栈溢出偶然报错」，不是本码 —— 码表里声明的 lua 实现等于没做
-- 修完这三格都必须变成 E-LIMIT-001。
--
-- ⚠️ 两个方向都要钉：
--   * 超限 → E-LIMIT-001 且**返回 nil**（error 会沿 pcall 退到 Sml.load，不许留下
--     半截的树当成功返回）；
--   * 128 层以内（含 128）必须照常解析成功 —— 只测失败路径的套件，在上限被写成
--     0 或 1 时会照样全绿。
-- ------------------------------------------------------------------
local function nest_blocks(n)
  -- `a { a { … } }`：纯块嵌套，输入**是闭合的**（不闭合会先被 E-PARSE-001 抓走，
  -- 测到的就是另一条码了）。
  return string.rep("a { ", n) .. string.rep("} ", n)
end

local function nest_block_array(n)
  -- `a { x: [ a { x: [ … ] } ] }`：块与数组交替 —— 每轮同时走 parse_block 与
  -- parse_array 两个递归入口（只给一条入口加守卫，这里会漏）。
  -- 每轮长 2 层，故 64 轮 = 128 层。
  return string.rep("a { x: [ ", n) .. string.rep("] } ", n)
end

local function test_limit_codes()
  io.write("[LIMIT]\n")

  -- 超限：块入口
  expect_code("block nesting 129 layers", nest_blocks(129), "E-LIMIT-001")
  expect_code("block nesting 200 layers", nest_blocks(200), "E-LIMIT-001")
  -- 超限：数组入口（块/数组交替，两个递归入口都会被走到）
  expect_code("block/array nesting 130 layers", nest_block_array(65), "E-LIMIT-001")
  expect_code("block/array nesting 200 layers", nest_block_array(100), "E-LIMIT-001")
  -- 改前报的是 E-PARSE-012（stack overflow 兜底），改后必须是深度码
  expect_code("block nesting 20000 layers (was stack overflow)",
              nest_blocks(20000), "E-LIMIT-001")

  -- 反向对照：上限内必须照常解析（含正好 128 层这一格 —— 别把上限算错一格）
  expect_ok("block nesting 127 layers", nest_blocks(127))
  expect_ok("block nesting 128 layers (at the limit)", nest_blocks(128))
  expect_ok("block/array nesting 128 layers (at the limit)", nest_block_array(64))

  -- 正向对照加强版：128 层不只是「不报错」，还得是**完整的树**（叶子能走到）。
  -- 防的是「超限时提前 return 半截树、却被当成功」那类修法。
  local deep, derr = Sml.load(string.rep("a { ", 128) .. "leaf: 1" .. string.rep("} ", 128))
  local ok_deep = deep ~= nil
  local cur = deep
  for _ = 1, 128 do
    if type(cur) ~= "table" then ok_deep = false; break end
    cur = cur.a
  end
  if ok_deep then ok_deep = (type(cur) == "table" and cur.leaf == 1) end
  report("128 layers fully parsed (leaf reachable)", "(no error)", derr, ok_deep)
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
-- 契约：E-CONTRACT-* / E-PARSE-019/021/022/023（W20）
--
-- 判别实验（改动前实测，报告里贴了原始输出）：
--   * Lua 原先**完全没有契约概念** —— `@contract` 与 `@is` 都被当成片段定义，
--     `@is Service` 还会把紧随的 `name` 当「类型/名字」参数吃掉。本组用例
--     改动前 **29 / 33 是红的**（大多报 E-PARSE-006 这个局部症状，或
--     「解析成功但树是错的」），改动后每一条都落在下面写死的码上。
--
-- 口径来源（逐条对照源码，不是按码表反推）：
--   * 类型 / 修饰符的解析：rust/sml-parse/src/parser.rs 的 parse_field_spec
--   * 校验与默认值填充：  rust/sml-contract/src/lib.rs 的 apply_contract / check_type
--   * 码：                rust/tests/error_codes.rs 的 contract_codes
--
-- ⚠️ 本组只覆盖 Lua **真的会报**的码：E-CONTRACT-007 / 009 / 011 需要 `@type`
--    或扩展注册点（Lua 没有），E-PARSE-024 是 C++ 独有的码 —— 见文件头说明。
-- ------------------------------------------------------------------
local function test_contract_codes()
  io.write("[CONTRACT]\n")

  -- E-CONTRACT-001 引用了未定义的契约（含组合字段引用的嵌套情形）
  expect_code("unknown contract (@is)", "server { @is Nope }\n", "E-CONTRACT-001")
  expect_code("unknown contract (composition)",
    "@contract S { a: Nope }\nsrv {\n  @is S\n  a { x: 1 }\n}\n", "E-CONTRACT-001")
  -- E-CONTRACT-002 字段类型不符
  expect_code("type mismatch int (bareword)",
    "@contract S { port: int }\nserver {\n  @is S\n  port: oops\n}\n", "E-CONTRACT-002")
  expect_code("type mismatch str (number given)",
    "@contract S { host: str }\nserver {\n  @is S\n  host: 42\n}\n", "E-CONTRACT-002")
  expect_code("type mismatch array element",
    "@contract S { tags: [str] }\nx { @is S tags: [ a 2 ] }\n", "E-CONTRACT-002")
  -- E-CONTRACT-003 必填字段缺失（非 optional 且无 default）
  expect_code("missing required field",
    "@contract S { port: int }\nserver { @is S }\n", "E-CONTRACT-003")
  -- E-CONTRACT-004 未声明字段（严格模式是默认；loose 见正向组）
  expect_code("undeclared field (strict)",
    "@contract S { port: int }\nserver {\n  @is S\n  port: 1\n  extra: 2\n}\n", "E-CONTRACT-004")
  -- 未声明字段：**语法元数据也不豁免**。裸块带参数（`addr Sub { … }`）时子块含
  -- __type/__name，Rust 实测同样报 E-CONTRACT-004（smltools：`addr.__name` 未声明）；
  -- C++ 侧显式跳过这两个键 —— 本端以 Rust 为准（W20 报告已登记该差异）。
  expect_code("meta keys count as undeclared (strict)",
    "@contract A { city: str }\n@contract S { addr: A }\n" ..
    "x {\n  @is S\n  addr Sub { city: Beijing }\n}\n",
    "E-CONTRACT-004")
  -- E-CONTRACT-005 数值越界（下界 / 上界各一格）
  expect_code("below min",
    "@contract S { ratio: num min 0 max 1 }\nsrv {\n  @is S\n  ratio: -1\n}\n", "E-CONTRACT-005")
  expect_code("above max",
    "@contract S { ratio: num min 0 max 1 }\nsrv {\n  @is S\n  ratio: 2\n}\n", "E-CONTRACT-005")
  -- E-CONTRACT-006 枚举取值不在列表内（**不能**与 E-CONTRACT-002 混用）
  expect_code("enum value not in list",
    "@contract S { st: enum [ a b ] }\nsrv {\n  @is S\n  st: c\n}\n", "E-CONTRACT-006")
  -- E-CONTRACT-008 组合字段应为块却给了标量
  expect_code("composition field is scalar",
    "@contract A { city: str }\n@contract S { addr: A }\nsrv {\n  @is S\n  addr: nope\n}\n",
    "E-CONTRACT-008")
  -- E-CONTRACT-010 非有限数：既覆盖**边界字面量**（nan / inf），也覆盖**值**（1e400）
  expect_code("min bound nan", "@contract S { n: num min nan }\nx { @is S n: 5 }\n",
              "E-CONTRACT-010")
  expect_code("max bound inf", "@contract S { n: num max inf }\nx { @is S n: 5 }\n",
              "E-CONTRACT-010")
  expect_code("non-finite value vs bounds",
    "@contract S { ratio: num min 0 max 1 }\nx { @is S ratio: 1e400 }\n", "E-CONTRACT-010")
  -- E-PARSE-022 边界取值非数字（与上面的非有限数**分属两条码**）
  expect_code("min bound not a number",
    "@contract S { n: num min abc }\nx { @is S n: 1 }\n", "E-PARSE-022")
  expect_code("max bound not a number",
    "@contract S { n: num max xyz }\nx { @is S n: 1 }\n", "E-PARSE-022")
  -- E-PARSE-023 enum 后须为数组
  expect_code("enum needs array", "@contract S { st: enum a b }\nx { @is S st: a }\n",
              "E-PARSE-023")
  -- E-CONTRACT-012 数组元素类型未知（本端只有这一条可达分支）
  expect_code("unknown array element type",
    "@contract S { xs: [nope] }\nx { @is S xs: [ 1 ] }\n", "E-CONTRACT-012")
  -- E-PARSE-019 指令头语法非法（@contract / @is 缺名字或缺体）
  expect_code("@contract without name", "@contract { a: str }\nx: 1\n", "E-PARSE-019")
  expect_code("@contract without body", "@contract S\nx: 1\n", "E-PARSE-019")
  expect_code("@is without name", "x { @is }\n", "E-PARSE-019")
  -- E-PARSE-021 default 修饰符后缺取值
  expect_code("default without value",
    "@contract S { a: str default }\nx { @is S }\n", "E-PARSE-021")
  -- E-PARSE-001 契约体未闭合（缺 `}` 遇到文件结尾）
  expect_code("unclosed contract body", "@contract S { a: str\nx: 1\n", "E-PARSE-001")
  -- E-PARSE-006 契约体里字段名 / 冒号位置非法
  expect_code("contract field without colon", "@contract S { a str }\nx: 1\n", "E-PARSE-006")
  expect_code("contract field key is structural", "@contract S { : str }\nx: 1\n", "E-PARSE-006")
end

-- ------------------------------------------------------------------
-- 契约正向对照：**断言填出来的值**，不只断言「没报错」。
-- 只测失败路径的套件，在「@is 被整条忽略」时依然会全绿 —— 这组就是拦它的。
-- ------------------------------------------------------------------
local function test_contract_positive()
  io.write("[CONTRACT positive controls]\n")

  -- 默认值必须**真的填进结果树**
  local v = expect_ok("defaults filled",
    "@contract S { host: str  port: int default 8080  tls: bool default true }\n" ..
    "server {\n  @is S\n  host: db1\n}\n")
  if v then
    report("  -> server.port == 8080", "8080", tostring(v.server and v.server.port),
           v.server ~= nil and v.server.port == 8080)
    report("  -> server.tls == true", "true", tostring(v.server and v.server.tls),
           v.server ~= nil and v.server.tls == true)
    report("  -> server.host 保留", "db1", tostring(v.server and v.server.host),
           v.server ~= nil and v.server.host == "db1")
  end

  -- optional 且无默认值 -> 字段不出现，也不报错
  local v2 = expect_ok("optional may be absent",
    "@contract S { note: str optional }\nx { @is S }\n")
  if v2 then
    report("  -> x.note 不出现", "nil", tostring(v2.x and v2.x.note),
           v2.x ~= nil and v2.x.note == nil)
  end

  -- loose 显式放宽未声明字段（同样输入在严格模式报 E-CONTRACT-004，见上组）
  local v3 = expect_ok("loose allows extra",
    "@contract S loose { host: str }\nx {\n  @is S\n  host: h\n  extra: 1\n}\n")
  if v3 then
    report("  -> loose 下 extra 保留", "1", tostring(v3.x and v3.x.extra),
           v3.x ~= nil and v3.x.extra == 1)
  end

  -- 组合：递归校验 + 把被引用契约的 default 填进**子块**
  local v4 = expect_ok("composition + nested default",
    "@contract A { city: str  country: str default CN }\n" ..
    "@contract S { addr: A }\nx {\n  @is S\n  addr { city: Beijing }\n}\n")
  if v4 then
    report("  -> x.addr.country == CN", "CN",
           tostring(v4.x and v4.x.addr and v4.x.addr.country),
           v4.x ~= nil and v4.x.addr ~= nil and v4.x.addr.country == "CN")
  end

  -- 契约定义本身不进主树（否则 `@contract S` 会污染数据）
  local v5 = expect_ok("contract definition not in tree", "@contract S { a: str }\nx: 1\n")
  if v5 then
    report("  -> 结果里无 contract / S 键", "x=1", tostring(v5.x),
           v5.contract == nil and v5.S == nil and v5.x == 1)
  end

  -- 同一契约可应用到多个块（默认值各自独立填充）
  local v6 = expect_ok("contract applies to multiple blocks",
    "@contract S { port: int default 80 }\na { @is S }\nb { @is S port: 9090 }\n")
  if v6 then
    report("  -> a.port==80 且 b.port==9090", "80/9090",
           tostring(v6.a and v6.a.port) .. "/" .. tostring(v6.b and v6.b.port),
           v6.a ~= nil and v6.a.port == 80 and v6.b ~= nil and v6.b.port == 9090)
  end

  -- 各类型的正向：数组、枚举（含逗号列表）、num/bool/any/int、引号串
  expect_ok("enum accepts declared value",
    "@contract S { st: enum [ active retired ] }\nx { @is S st: active }\n")
  expect_ok("enum accepts comma list",
    "@contract S { st: enum [ a, b ] }\nx { @is S st: b }\n")
  expect_ok("array of str accepts barewords",
    "@contract S { tags: [str] }\nx { @is S tags: [ a b ] }\n")
  expect_ok("num / bool / any / int accept right kinds",
    "@contract S { r: num  b: bool  z: any  i: int }\n" ..
    "x { @is S r: 1.5 b: true z: whatever i: 7 }\n")
  expect_ok("str accepts quoted value",
    "@contract S { host: str }\nx { @is S host: \"a b\" }\n")
  expect_ok("explicit required keyword",
    "@contract S { host: str required }\nx { @is S host: h }\n")

  -- 指令（`@`）不得被当作前一个键的**裸块参数**吞掉（W20 第二阶段修的那格）。
  -- 背景：`common.sml` 的嵌套块注释残留一个无害的 `key: */`，旧写法随即把紧随的
  -- `@contract Service loose { … }` 整块吞掉、报出无关的 E-PARSE-003，
  -- 连带 include 了它的 examples/app.sml 也无法解析。
  -- 最小复现（改动前 E-PARSE-003，Rust 实跑 rc=0 得 {"g":"*/"}）。
  local v7 = expect_ok("directive not swallowed as bare-block arg",
    'g */\n@contract C { env: enum [ a b ] }\nkept: 1\n')
  if v7 then
    report("  -> 指令未被吞、后续字段可见", "kept=1", tostring(v7.kept),
           v7.kept == 1 and v7.C == nil and v7.contract == nil)
  end
end

-- ------------------------------------------------------------------
-- include：E-INCLUDE-001/002/003/004/010/012 + E-LIMIT-003（W20 第二阶段）
--
-- 判别实验（改动前实测）：include 完全是**惰性**的 —— `include "x.sml"` 被当成
-- 裸块键，把后面到第一个 `{` 的内容全吞进片段体，于是「解析成功但树是错的」；
-- W10 给键位置加码后改成明确报 E-PARSE-006（本阶段的红基线）。
-- 判据用真实文件：沙箱建在**系统临时目录**里、用完删掉（不往仓库写任何东西）。
--
-- 口径：Rust `sml-include` 与 C `resolve_includes`。
--   * 存在判定在越界判定**之前** ⇒「越界且不存在」报 E-INCLUDE-001；
--   * 环检测用**链栈** ⇒ 菱形包含合法（重复键按 SML 规则合并成数组）；
--   * 深度 32 / 展开 10000，「文档根不计层」；v4 那格用文件链实测边界。
-- ⚠️ E-INCLUDE-011（include 预处理词法失败）在本端**不可达**：Lua 的 tokenize
--    刻意宽松、从不抛异常（未闭合字符串/块注释归 W16 的静默清单），故不编用例。
-- ------------------------------------------------------------------
local function test_include_codes()
  io.write("[INCLUDE]\n")

  -- ---- 沙箱：系统临时目录，用完清理 ----
  local win = package.config:sub(1, 1) == "\\"
  local stem = os.tmpname()
  local root = stem .. "_inc"
  local inside = root .. "/in"
  local function mkdir(p)
    if win then os.execute('mkdir "' .. p .. '" 2>nul')
    else os.execute('mkdir -p "' .. p .. '"') end
  end
  local function rmdir(p)
    if win then os.execute('rmdir "' .. p .. '" 2>nul')
    else os.execute('rmdir "' .. p .. '" 2>/dev/null') end
  end
  mkdir(root); mkdir(inside)
  local written = {}
  local function put(dir, name, text)
    local p = dir .. "/" .. name
    local f = assert(io.open(p, "wb"))
    f:write(text); f:close()
    written[#written + 1] = p
    return p
  end
  -- 沙箱外的既有文件（用于「越界且**存在**」那一格）
  put(root, "outside.sml", "outside: 1\n")

  local function ierr(tag, src, base, want)
    local v, err = Sml.load(src, base)
    if v ~= nil then
      failures = failures + 1
      io.write(string.format("FAIL: %s parsed OK but should fail\n", tag))
      return
    end
    report(tag, want, err, code_is(err, want))
  end
  local function iok(tag, src, base)
    local v, err = Sml.load(src, base)
    if v == nil then
      failures = failures + 1
      io.write(string.format("FAIL: %s should parse OK, got \"%s\"\n", tag, tostring(err)))
      return nil
    end
    passed = passed + 1
    io.write(string.format("  ok: %s -> (no error)\n", tag))
    return v
  end

  -- E-INCLUDE-012 路径写法非法（未加引号 / 引号未闭合 / 多余字符）
  ierr("include unquoted path", "include nope.sml\n", inside, "E-INCLUDE-012")
  ierr("@include unquoted path", "@include nope.sml\n", inside, "E-INCLUDE-012")
  ierr("include unclosed quote", 'include "nope.sml\n', inside, "E-INCLUDE-012")
  -- E-INCLUDE-001 目标不存在
  ierr("include target missing", 'include "nope.sml"\n', inside, "E-INCLUDE-001")
  -- 越界的**且不存在** → 存在判定在前，报 001（不是 003）
  ierr("escaped AND missing -> 001 (existence first)",
       'include "../also_missing_xyz.sml"\n', inside, "E-INCLUDE-001")
  -- E-INCLUDE-003 越界（目标存在、但在沙箱之外）
  ierr("escape the sandbox", 'include "../outside.sml"\n', inside, "E-INCLUDE-003")
  ierr("escape the sandbox (@include form)", '@include "../outside.sml"\n', inside, "E-INCLUDE-003")
  -- E-INCLUDE-010 基准目录不可解析（fail-closed）
  ierr("base dir does not exist", 'include "x.sml"\n', root .. "_no_such_base", "E-INCLUDE-010")
  ierr("base is empty string", 'include "x.sml"\n', "", "E-INCLUDE-010")
  -- E-INCLUDE-002 环（互包含）；自包含同样成环
  put(inside, "cyc_a.sml", 'include "cyc_b.sml"\n')
  put(inside, "cyc_b.sml", 'include "cyc_a.sml"\n')
  ierr("include cycle (mutual)", 'include "cyc_a.sml"\n', inside, "E-INCLUDE-002")
  put(inside, "selfinc.sml", 'include "selfinc.sml"\n')
  ierr("include cycle (self)", 'include "selfinc.sml"\n', inside, "E-INCLUDE-002")

  -- E-INCLUDE-004 嵌套超 32 层：**文档根不计层** ⇒ 先钉住边界（31 层放行 / 32 层报）
  local function chain(prefix, n)
    for k = 1, n do
      local body = (k < n) and ('include "' .. prefix .. (k + 1) .. '.sml"\n') or "leaf: 1\n"
      put(inside, prefix .. k .. ".sml", body)
    end
    return 'include "' .. prefix .. '1.sml"\n'
  end
  local v31 = iok("include nesting 31 layers (at/below the limit)", chain("deep", 31), inside)
  if v31 then
    report("  -> 最深层字段可达", "1", tostring(v31.leaf), v31.leaf == 1)
  end
  ierr("include nesting 32 layers (> limit)", chain("deep2", 32), inside, "E-INCLUDE-004")

  -- E-LIMIT-003 展开次数超 10000（**全局**计数，防菱形 2^N 膨胀）
  -- 14 层「每层把下一层包含两次」⇒ 无界时 2^1+…+2^14 = 2^15-2 = 32766 次 > 10000
  -- （少一层只有 2^14-2 = 16382… 实测过：13 层时 2^13-1 = 8191 < 10000，**不够**）
  for k = 1, 14 do
    local nxt = (k < 14)
      and ('include "dia_' .. (k + 1) .. '.sml"\ninclude "dia_' .. (k + 1) .. '.sml"\n')
      or "leaf: 1\n"
    put(inside, "dia_" .. k .. ".sml", nxt)
  end
  ierr("expansions exceed 10000 (diamond blow-up)",
       'include "dia_1.sml"\n', inside, "E-LIMIT-003")

  -- 正向：菱形包含**合法**（不是环）；重复键按 SML 规则合并成数组
  put(inside, "dl.sml", "k: 1\n")
  local vd = iok("diamond include is legal (not a cycle)",
                 'include "dl.sml"\ninclude "dl.sml"\n', inside)
  if vd then
    report("  -> 重复键合并为数组", "1,1",
           type(vd.k) == "table" and (tostring(vd.k[1]) .. "," .. tostring(vd.k[2])) or tostring(vd.k),
           type(vd.k) == "table" and vd.k[1] == 1 and vd.k[2] == 1)
  end
  -- 正向：嵌套 include（两侧字段都在）
  put(inside, "ch_b.sml", "from_b: 2\n")
  put(inside, "ch_a.sml", 'include "ch_b.sml"\nfrom_a: 1\n')
  local vc = iok("nested include keeps both sides' fields", 'include "ch_a.sml"\n', inside)
  if vc then
    report("  -> from_a 与 from_b 都在", "1/2",
           tostring(vc.from_a) .. "/" .. tostring(vc.from_b),
           vc.from_a == 1 and vc.from_b == 2)
  end
  -- 正向：@include 形式等价
  local va2 = iok("@include form works", '@include "ch_b.sml"\n', inside)
  if va2 then report("  -> @include 展开出字段", "2", tostring(va2.from_b), va2.from_b == 2) end
  -- 正向：子文件的 @version/@feature 行会被剥离（与 Rust expand_file_tokens 同口径）
  put(inside, "meta.sml", "@version v4\nmv: 1\n")
  local vm = iok("child @version line is stripped", 'include "meta.sml"\n', inside)
  if vm then report("  -> 子文件字段仍在", "1", tostring(vm.mv), vm.mv == 1) end
  -- 正向：base 不给 ⇒ include 关闭，**不读文件**（缺文件也不报错）
  local vn = iok("base omitted => include disabled (no file read)",
                 'include "definitely_missing_xyz.sml"\n')
  if vn then report("  -> 未展开也不报错", "-", "-", true) end

  -- 本阶段不做：遇到即**显式报错**，绝不静默
  ierr("namespace (as) unsupported", 'include "x.sml" as ns\n', inside, "E-FEATURE-001")
  ierr("glob unsupported", 'include "*.sml"\n', inside, "E-FEATURE-001")
  ierr("regex unsupported", 'include "re:widget_.*"\n', inside, "E-FEATURE-001")
  ierr("multi-target unsupported", 'include "a.sml", "b.sml"\n', inside, "E-FEATURE-001")
  ierr("partial reference unsupported", 'include "x.sml" { a, b }\n', inside, "E-FEATURE-001")

  -- 指令检测只看**行首**；且不展开块注释/多行字符串里的 include（否则会去读文件）
  local vk = iok("key named include: with colon is not a directive", "include: 5\n", inside)
  if vk then report("  -> include 作为普通键", "5", tostring(vk.include), vk.include == 5) end
  iok('include inside /* */ block comment is not expanded',
      '/* head\ninclude "definitely_missing_xyz.sml"\n*/\nk: 1\n', inside)
  iok("include inside multi-line string is not expanded",
      's: "head\ninclude "definitely_missing_xyz.sml"\ntail"\nk: 1\n', inside)

  -- 端到端：仓库里的 examples/app.sml（include + 片段 + 契约 + $env 同时用上）
  local root_dir = self:match("^(.+)[/\\]") or "."
  local app_path = root_dir .. "/../examples/app.sml"
  local af = io.open(app_path, "rb")
  if af == nil then
    failures = failures + 1
    io.write("FAIL: cannot open " .. app_path .. "\n")
  else
    local atext = af:read("*a"); af:close()
    local vapp = iok("examples/app.sml end-to-end (include + fragment + contract)",
                     atext, root_dir .. "/../examples")
    if vapp then
      report("  -> api.name 来自契约块", "gateway", tostring(vapp.api and vapp.api.name),
             vapp.api ~= nil and vapp.api.name == "gateway")
      report("  -> network.region 来自被 include 的片段", "cn-north-1",
             tostring(vapp.network and vapp.network.region),
             vapp.network ~= nil and vapp.network.region == "cn-north-1")
    end
  end

  -- ---- 清理沙箱 ----
  for k = 1, #written do os.remove(written[k]) end
  rmdir(inside); rmdir(root)
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
test_limit_codes()
test_feature_codes()
test_host_codes()
test_contract_codes()
test_contract_positive()
test_include_codes()
test_positive_controls()

io.write(string.format("\n%d 通过, %d 失败\n", passed, failures))
if failures == 0 then
  io.write("ALL CODE TESTS PASSED\n")
  os.exit(0)
end
io.write(failures .. " FAILURES\n")
os.exit(1)
