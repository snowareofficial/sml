// test_codes.cpp — 「触发条件 → 期望码」回归（W10，C++ 侧）
//
// 职责不是「C++ 报了错」，而是钉住**报的是哪个码**：码是跨端稳定契约，文案不是
// （见 errors/README.md 与 errors/codes.sml）。期望值一律用 ../c/sml_codes.h 的宏，
// 不手打字符串 —— 顺带保证「宏真的能被 C++ 测试 include 到」。
//
// 覆盖范围：C++ 侧**真的会报**的码。errors/README.md 的「该报错却静默通过」清单里
// 属于 C++ 的那几条（未闭合块注释、未知转义）**不在这里** —— 它们在本实现里根本没有
// 这个错误，给它们编用例等于给不存在的东西发护照；那是 W16「静默清单逐条判定」的对象。
//
// 三个码**无法经公开 API 构造**，只在此说明、不写用例（写了会误导人）：
//   - E-PARSE-012（解析失败兜底）：C++ 侧没有这个兜底出口。
//   - E-INTERNAL-001：C++ API 用 std::string* 而非裸缓冲，没有空指针入参面。
//   - E-LIMIT-010（内存分配失败）：要让它真的触发，得让 std::make_shared 抛
//     std::bad_alloc —— 在测试里无法可靠构造（人为制造 OOM 会拖垮整个测试进程）。
//     实现已接住它（Parser::parse / apply_contract 的薄包装，见 sml.hpp 的
//     「错误码约定」：「没有例外：连内存分配失败也带码」）。这是本文件里唯一
//     「有实现、无用例」的码，显式登记在此，免得后来者以为它没做。
//
// 已知与 Rust 的**行为差异**（不在此断言，免得把差异写死成契约）：
//   - `k: "\u12"`：Rust 报 E-LEX-005（位数不足），本实现接受任意位十六进制。
//     故 E-LEX-005 改从「空码点 / 缺闭合 / 码点溢出」三侧构造同一语义条件。
//   - `k: "\uD800"`：Rust 报 E-LEX-005（代理区非法码点），本实现按码点编码，不校验代理区。
//   - 未闭合块注释：本实现静默接受 —— `E-LEX-002`/`E-LEX-003` 的 `impls` 仅列 rust，
//     note 也写明「其余四端静默接受」，属**已登记**的静默点，这里不假装有码。
//     （注意：**未知转义**不属此类 —— `E-LEX-004` 的 `impls` 明列 cpp，本实现此前
//     静默放行属「声明与实现不符」，现已补上，见下面的用例。）
//   - 数值字面量的**归类**（值模型层）：关键格子已在下面的 [value model] 组钉住，
//     但跨端仍有差异，逐格记清（这几格极易搞反）：
//       超 i64 的**纯整数**：Rust = **Str**（B10：round-trip 安全、零精度损失）、
//         C++ = Str ✓（**别改成 Float** —— 那是照 c_abi.rs 的 JSON 桥猜出来的错方向，
//         两者不是同一入口）、C = Int（被 strtoll 夹到 LLONG_MAX，属"错值"类）。
//       `1e400`：Rust = Float(inf)、C = Float(inf)、C++ 原先 = Str（唯一异类）→ 已修。
//       **仍不一致、但不在本轮批准范围**：前导零纯数字 `007`（Rust 保留为 Str 以免
//         权限位被改写，C++ 现为 Int 7）；Rust 的 Float 另带 raw 以还原书写形式
//         （`1e10` 不写成 `10000000000.0`），C++ 值模型没有 raw 字段。
//   - `k: "abc\`（转义符后**直接**遇文件结尾、无换行）：Rust 经 parse() 报 E-LEX-004，
//     本实现报 E-LEX-001（未闭合字符串）。带换行的同一写法两端都是 E-LEX-004
//     （已由「未知转义」用例覆盖）—— 只剩无换行这一格不同，待判定。
//   - include 的**触发写法**：本实现只认 `@include`；裸 `include "x"` 会被当成普通键
//     （不做包含、也不报错）。而 Rust 的 sml-include 是**文本级**预处理、认裸
//     `include` / `import` 行，JS 的文档也写裸 `include`，C 两种都认 —— 本实现是
//     唯一只认 `@` 别名的。另：`include_dir` 为空时 `@include` 整段被静默忽略。
//     补裸 `include` 是**加语法**而非补码，不在此顺手做。
//   - ⚠️ **`@include` 的成功展开本身是坏的（既有缺陷，HEAD 版同样复现，非本次引入）**：
//     实测 a.sml = `@include "b.sml"` + `from_a: 1` 解析后**只剩一个垃圾键
//     `include = "include"`** —— includer 自己的 `from_a` 与被包含文件的字段一起丢失。
//     根因：把目标文件的 token **插到路径 token 之前**后又 `st.i++`，恰好跳过插入段的
//     首个 token（也就是嵌套的 `@`），于是 `include` 退化成裸块键、把后续字段全当参数吞掉。
//     **警告**：别把它"修"成单纯的索引修正 —— 那个 off-by-one 恰好压住了无限展开，
//     只改索引会让 a↔b 变成真正的死循环（因为循环检测用的栈是 push 完立刻 pop，
//     永远为空）。要修必须同时给出环检测/展开上限。故本文件**只测**越界与缺失，
//     不写"展开成功"的正向用例（见 `_w10_inc_chain_run.py` 的复现）。
//
// 编译运行：
//     g++ -std=c++17 -Wall -Wextra -I. test_codes.cpp sml.cpp -o t_codes.exe && ./t_codes.exe
// 或走 `python cpp/build_verify.py`（本文件是其中的 CODES target）。
//
// SPDX-License-Identifier: MulanPSL-2.0
#include "sml.hpp"
#include "../c/sml_codes.h"

#include <cmath>
#include <cstring>
#include <iostream>
#include <string>

static int failures = 0;

// err 以码开头、码后接一个空格（或字符串结束）。码内不含空格。
// 不用 compare(0,n,want)==0 单独判定：那样 "E-CONTRACT-0011" 也会误命中 "E-CONTRACT-001"。
static bool code_is(const std::string& err, const char* want) {
    std::size_t n = std::strlen(want);
    if (err.compare(0, n, want) != 0) return false;
    return err.size() == n || err[n] == ' ';
}

static void report(const std::string& tag, const char* want,
                   const std::string& err, bool ok) {
    if (ok) {
        std::cout << "  ok: " << tag << " -> " << want << "\n";
    } else {
        std::cout << "FAIL: " << tag << " want " << want << ", got \"" << err << "\"\n";
        failures++;
    }
}

// 内存文本：失败（返回空指针）且码相符。dir 非空时启用 include 展开与越界校验。
static void expect_code(const std::string& tag, const std::string& src,
                        const char* want, const std::string& dir = "") {
    std::string err;
    sml::ValuePtr v = sml::Parser::parse(src, &err, dir);
    if (v) {
        std::cout << "FAIL: " << tag << " parsed OK but should fail\n";
        failures++;
        return;
    }
    report(tag, want, err, code_is(err, want));
}

// 正向对照：这些输入**必须通过**。它们防的是「把码改成一报到底」——
// 只测失败路径的套件，在整个判定被反转（例如所有未声明字段都报错、
// 连 `loose` 也报）时依然会全绿。
static void expect_ok(const std::string& tag, const std::string& src) {
    std::string err;
    sml::ValuePtr v = sml::Parser::parse(src, &err);
    if (!v) {
        std::cout << "FAIL: " << tag << " should parse OK, got \"" << err << "\"\n";
        failures++;
        return;
    }
    std::cout << "  ok: " << tag << " -> (no error)\n";
}

// ------------------------------------------------------------------
// 词法：E-LEX-*
// ------------------------------------------------------------------
static void test_lex_codes() {
    std::cout << "[LEX]\n";

    // 字符串未闭合（缺少结束引号）
    expect_code("unterminated string", "k: \"abc\n", SML_E_LEX_001);

    // 未知转义 → E-LEX-004（codes.sml 的 impls 明列 cpp，note 是「严格策略：未知转义
    // 即失败，避免路径与正则被静默损坏」；此前本实现把 `\` 与字符双双塞回、静默放行）
    expect_code("unknown escape", "k: \"\\z\"\n", SML_E_LEX_004);
    // 接受集收窄到 Rust 的严格集：C 风格的 \a \b \f \v \' 同样报此码（Rust 也拒）
    expect_code("unknown escape (C-style \\a)", "k: \"\\a\"\n", SML_E_LEX_004);
    expect_code("unknown escape (C-style \\')", "k: \"\\'\"\n", SML_E_LEX_004);
    // 反斜杠 + 换行：既不是合法转义，也不能被当成续行 —— 同一条码
    expect_code("unknown escape (backslash-newline)",
                "k: \"abc\\\n\n", SML_E_LEX_004);

    // Unicode 转义非法：\u{} 没有码点 / \u{12 缺少闭合花括号 / 码点大到溢出的十六进制。
    // 注：Rust 用 "k: \"\\u12\"" 触发同一条码（位数不足）；本实现在非花括号形式下
    // 接受任意位十六进制，故改从另外三侧构造同一个语义条件。
    expect_code("bad unicode escape (empty)", "k: \"\\u{}\"\n", SML_E_LEX_005);
    expect_code("bad unicode escape (unclosed)", "k: \"\\u{12\"\n", SML_E_LEX_005);
    expect_code("bad unicode escape (overflow)",
                "k: \"\\u{FFFFFFFFFFFFFFFFFFFFFFFF}\"\n", SML_E_LEX_005);
}

// ------------------------------------------------------------------
// 语法：E-PARSE-*（契约定义内的字段规格）
// ------------------------------------------------------------------
static void test_parse_codes() {
    std::cout << "[PARSE]\n";

    // `min` / `max` 的边界取值缺失
    expect_code("min without value",
                "@contract C { a: int min }\n", SML_E_PARSE_022);
    expect_code("max without value",
                "@contract C { a: int max }\n", SML_E_PARSE_022);

    // `default` 修饰符后缺少取值
    expect_code("default without value",
                "@contract C { a: int default }\n", SML_E_PARSE_021);

    // `enum` 后不是数组
    expect_code("enum without array",
                "@contract C { a: enum }\n", SML_E_PARSE_023);

    // 数组 / 枚举缺少闭合、数组元素类型为空 —— 同一条码的三个子分支
    expect_code("array missing ]",
                "@contract C { a: [ int }\n", SML_E_PARSE_024);
    expect_code("enum missing ]",
                "@contract C { a: enum [ x }\n", SML_E_PARSE_024);
    expect_code("array element type empty",
                "@contract C { a: [ ] }\n", SML_E_PARSE_024);

    // 多余的结束符号（块里出现 `]`）
    expect_code("stray closing bracket",
                "x { ] }\n", SML_E_PARSE_003);
}

// ------------------------------------------------------------------
// 契约：E-CONTRACT-*
// ------------------------------------------------------------------
static void test_contract_codes() {
    std::cout << "[CONTRACT]\n";

    // 引用了未定义的契约（块级 @is）
    expect_code("unknown contract (@is)",
                "server { @is Nope }\n", SML_E_CONTRACT_001);

    // 引用了未定义的契约（字段类型是契约引用）
    expect_code("unknown contract (field ref)",
                "@contract S { inner: Missing }\n"
                "server {\n  @is S\n  inner { y: 1 }\n}\n",
                SML_E_CONTRACT_001);

    // 字段类型不符
    expect_code("field type mismatch",
                "@contract S { port: int }\n"
                "server {\n  @is S\n  port: oops\n}\n",
                SML_E_CONTRACT_002);

    // 同一码的其余子分支：bool / num / str / array 各一条。
    // 分四条写而不是合并：这四条的判定出口在 check_value 里各是一处，合并后
    // 只要有一处漏改码就测不出来。
    expect_code("field expects bool",
                "@contract S { flag: bool }\n"
                "server {\n  @is S\n  flag: 5\n}\n",
                SML_E_CONTRACT_002);
    expect_code("field expects num",
                "@contract S { ratio: num }\n"
                "server {\n  @is S\n  ratio: xyz\n}\n",
                SML_E_CONTRACT_002);
    expect_code("field expects str",
                "@contract S { host: str }\n"
                "server {\n  @is S\n  host: 42\n}\n",
                SML_E_CONTRACT_002);
    expect_code("field expects array",
                "@contract S { tags: [str] }\n"
                "server {\n  @is S\n  tags: 5\n}\n",
                SML_E_CONTRACT_002);

    // 必填字段缺失
    expect_code("missing required field",
                "@contract S { port: int }\n"
                "server { @is S }\n",
                SML_E_CONTRACT_003);

    // 未声明字段（严格模式）
    expect_code("undeclared field (strict)",
                "@contract S { port: int }\n"
                "server {\n  @is S\n  port: 1\n  extra: 2\n}\n",
                SML_E_CONTRACT_004);

    // 数值越界
    expect_code("numeric out of range",
                "@contract S { ratio: num min 0 max 1 }\n"
                "server {\n  @is S\n  ratio: 2\n}\n",
                SML_E_CONTRACT_005);

    // 枚举取值不在列表内 —— 与「类型不符」是两条码，不能混用
    expect_code("enum value not listed",
                "@contract S { st: enum [ a b ] }\n"
                "server {\n  @is S\n  st: c\n}\n",
                SML_E_CONTRACT_006);

    // 枚举的另外两个子分支：整数按标量比对 / 取值既非字符串也非整数
    expect_code("enum int not listed",
                "@contract S { st: enum [ a b ] }\n"
                "server {\n  @is S\n  st: 9\n}\n",
                SML_E_CONTRACT_006);
    expect_code("enum expects scalar",
                "@contract S { st: enum [ a b ] }\n"
                "server {\n  @is S\n  st { }\n}\n",
                SML_E_CONTRACT_006);

    // 组合字段应为块，实际给了标量
    expect_code("composite field not a block",
                "@contract I { x: int }\n"
                "@contract O { inner: I }\n"
                "server {\n  @is O\n  inner: 5\n}\n",
                SML_E_CONTRACT_008);
}

// ------------------------------------------------------------------
// 上限与 include：E-LIMIT-001 / E-INCLUDE-003
// ------------------------------------------------------------------
// ------------------------------------------------------------------
// min/max 边界字面量：E-PARSE-022 / E-CONTRACT-010 / E-CONTRACT-005
//
// 这一组直接钉住「边界必须按 f64 生效」：修复前 C++ 用 std::stoll，会把
// `min 0.5` 截成 0 —— 于是 `ratio: 0.2` 本该报越界却**通过**（下面那条
// "fractional lower bound enforced" 就是修复前会失败的判别用例）。
// ------------------------------------------------------------------
static void test_bound_literals() {
    std::cout << "[bound literals]\n";

    // 边界取值非数字 → E-PARSE-022（codes.sml 该条 note 正指此条件）
    expect_code("bound not a number",
                "@contract C { a: int min abc }\n", SML_E_PARSE_022);

    // std::stod 的宽松前缀会接受 "1abc"（返回 1.0），必须被闸门挡住 ——
    // 否则 C++ 会「悄悄用 1 当边界」，而 Rust 报 E-PARSE-022。
    expect_code("bound with trailing junk",
                "@contract C { a: int max 1abc }\n", SML_E_PARSE_022);

    // 边界为非有限数 → E-CONTRACT-010（NaN 的一切比较均为假，会静默绕过边界）
    expect_code("bound is nan",
                "@contract C { a: num min nan }\n", SML_E_CONTRACT_010);
    // 超出 double 的字面量（Rust 的 parse::<f64>() 给 inf）→ 同一条码
    expect_code("bound overflows double",
                "@contract C { a: num max 1e400 }\n", SML_E_CONTRACT_010);

    // 小数边界必须**真的生效**。下界这条是**判别用例**：修复前 `min 0.5` 被截成 0，
    // `0.2` 会错误通过（实测：把两行改回 std::stoll 后此条 FAIL）。
    // 上界那条不具判别性 —— 修复前 `max 0.5` 截成 0，`0.6 > 0` 恰好也报同一码；
    // 留着是为了钉住「上界也要被检查」，不是用来区分实现。
    expect_code("fractional lower bound enforced",
                "@contract C { ratio: num min 0.5 }\n"
                "x { @is C\n  ratio: 0.2 }\n",
                SML_E_CONTRACT_005);
    expect_code("fractional upper bound enforced",
                "@contract C { ratio: num max 0.5 }\n"
                "x { @is C\n  ratio: 0.6 }\n",
                SML_E_CONTRACT_005);

    // 正向对照：边界上的值不该被误判（修复前 `max 0.5` 被截成 0，0.5 会被误报）
    expect_ok("value exactly at fractional max",
              "@contract C { ratio: num min 0.5 max 0.5 }\n"
              "x { @is C\n  ratio: 0.5 }\n");
}

static void test_limit_and_include() {
    std::cout << "[LIMIT / INCLUDE]\n";

    // 深嵌套必须报错返回，而不是打穿栈（200 层 > 上限 128）。
    // 两条入口都要测：块嵌套走 parse_block_nested、数组嵌套走 parse_value —— 它们
    // 历史上曾各自绕过守卫（纯块嵌套能打穿栈），只测一条会漏掉另一条。
    std::string deep;
    for (int i = 0; i < 200; i++) deep += "a { ";
    expect_code("block nesting too deep (200)", deep, SML_E_LIMIT_001);

    std::string deep_arr = "k: ";
    for (int i = 0; i < 200; i++) deep_arr += "[ ";
    expect_code("array nesting too deep (200)", deep_arr, SML_E_LIMIT_001);

    // include 越界：目标不在基准目录内，已拒绝
    expect_code("include escapes base dir",
                "@include \"../sml.cpp\"\n", SML_E_INCLUDE_003, ".");

    // include 目标不存在 → E-INCLUDE-001。
    // codes.sml 该条的 impls 明列 cpp（[rust c cpp js lua]），此前却是静默跳过。
    // 这两条**必须成对**：只加下面这条而不加「越界」那条时，若实现里忘了用
    // `inside` 设闸，E-INCLUDE-003 会被这条的码覆盖 —— 单测一条测不出来。
    expect_code("include target missing",
                "@include \"_no_such_include.sml\"\n", SML_E_INCLUDE_001, ".");
}

// ------------------------------------------------------------------
// 文件结尾未闭合（E-PARSE-001）与键位置非法记号（E-PARSE-006）
//
// 这两条原先都是**静默成功**：`a {` 解析成 {a:{}}、`a: [ 1` 解析成只含 1 的数组、
// `a { { x } }` 把多余的 `{` 跳过。都属于「用户拿到了错的/残缺的文档却没有报错」。
// ------------------------------------------------------------------
static void test_eof_and_key_position() {
    std::cout << "[EOF / key position]\n";

    // 由 `{` 开启的块走到文件结尾 → E-PARSE-001
    expect_code("unclosed block", "a {\n", SML_E_PARSE_001);
    expect_code("unclosed braced top level", "{ a: 1\n", SML_E_PARSE_001);
    expect_code("unclosed nested block", "a { b {\n", SML_E_PARSE_001);
    // 未闭合数组（含顶层数组）
    expect_code("unclosed array", "a: [ 1\n", SML_E_PARSE_001);
    expect_code("unclosed top-level array", "[ 1\n", SML_E_PARSE_001);

    // 键位置的结构记号 → E-PARSE-006（Rust 同位置同码）
    expect_code("stray brace in key position", "a { { x } }\n", SML_E_PARSE_006);
    expect_code("stray bracket in key position", "a { [ 1 ] }\n", SML_E_PARSE_006);
    expect_code("stray colon in key position", "a { : 1 }\n", SML_E_PARSE_006);
}

// ------------------------------------------------------------------
// 值模型：断言"解析成了什么类型/值"，不是"报不报码"。
// 这几格是三端表里最容易漂的地方（Rust 的 B10 审计专门修过），故一并钉住。
// ------------------------------------------------------------------
static const char* tag_shape(const sml::ValuePtr& x) {
    if (!x) return "missing";
    switch (x->tag) {
        case sml::Value::Tag::Float: return "Float";
        case sml::Value::Tag::Int:   return "Int";
        case sml::Value::Tag::Str:   return "Str";
        case sml::Value::Tag::Bool:  return "Bool";
        case sml::Value::Tag::Null:  return "Null";
        default:                     return "Arr/Obj";
    }
}

// 某个键必须解析成 Float 且为 ±inf（浮点溢出）
static void expect_float_inf(const std::string& tag, const std::string& src,
                             const std::string& key, bool positive) {
    std::string err;
    sml::ValuePtr v = sml::Parser::parse(src, &err);
    if (!v) { std::cout << "FAIL: " << tag << " should parse OK, got \"" << err << "\"\n"; failures++; return; }
    sml::ValuePtr x = v->get(key);
    if (x && x->tag == sml::Value::Tag::Float && std::isinf(x->f) &&
        (positive ? x->f > 0 : x->f < 0)) {
        std::cout << "  ok: " << tag << " -> Float(" << (positive ? "+inf" : "-inf") << ")\n";
        return;
    }
    std::cout << "FAIL: " << tag << " want Float(" << (positive ? "+inf" : "-inf")
              << "), got " << tag_shape(x) << "\n";
    failures++;
}

// 某个键必须解析成 Float 且为 0（浮点下溢）
static void expect_float_zero(const std::string& tag, const std::string& src,
                              const std::string& key) {
    std::string err;
    sml::ValuePtr v = sml::Parser::parse(src, &err);
    if (!v) { std::cout << "FAIL: " << tag << " should parse OK, got \"" << err << "\"\n"; failures++; return; }
    sml::ValuePtr x = v->get(key);
    if (x && x->tag == sml::Value::Tag::Float && x->f == 0.0) {
        std::cout << "  ok: " << tag << " -> Float(0)\n";
        return;
    }
    std::cout << "FAIL: " << tag << " want Float(0), got " << tag_shape(x) << "\n";
    failures++;
}

// 某个键必须解析成 Str 且原文一字不差（超出 i64 的纯整数走这条）
static void expect_str_kept(const std::string& tag, const std::string& src,
                            const std::string& key, const std::string& want) {
    std::string err;
    sml::ValuePtr v = sml::Parser::parse(src, &err);
    if (!v) { std::cout << "FAIL: " << tag << " should parse OK, got \"" << err << "\"\n"; failures++; return; }
    sml::ValuePtr x = v->get(key);
    if (x && x->tag == sml::Value::Tag::Str && x->s == want) {
        std::cout << "  ok: " << tag << " -> Str(\"" << want << "\")\n";
        return;
    }
    std::cout << "FAIL: " << tag << " want Str(\"" << want << "\"), got " << tag_shape(x) << "\n";
    failures++;
}

static void test_value_model() {
    std::cout << "[value model]\n";

    // 浮点溢出：Rust 的 parse::<f64>() 给 ±inf（不是错误，也不是 Str）
    expect_float_inf("1e400 overflows to +inf", "x: 1e400\n", "x", true);
    expect_float_inf("-1e400 overflows to -inf", "x: -1e400\n", "x", false);
    /* 下溢必须给 0 而不是 inf —— 这条是**判别用例**：若实现改用 std::stod 并在
       catch 里一律返回 inf（stod 对下溢同样抛 out_of_range），这里会 FAIL。 */
    expect_float_zero("1e-400 underflows to 0", "x: 1e-400\n", "x");

    /* 超出 i64 的**纯整数**必须保留为 Str（Rust B10：round-trip 安全、零精度损失）。
       这条防的是"顺手把它改成 Float"——那是被 `c_abi.rs` 的 JSON 桥误导过的方向。 */
    expect_str_kept("big integer kept as Str",
                    "x: 99999999999999999999999\n", "x", "99999999999999999999999");
}

static void test_positive_controls() {
    std::cout << "[positive controls]\n";

    // 契约声明齐、类型对、取值在界内 —— 不该报任何码
    expect_ok("contract satisfied",
              "@contract S { port: int }\nserver {\n  @is S\n  port: 8080\n}\n");

    // 枚举取值在列表内
    expect_ok("enum value listed",
              "@contract S { st: enum [ a b ] }\nserver {\n  @is S\n  st: a\n}\n");

    // 契约带 loose 时，未声明字段**不该**报 E-CONTRACT-004
    expect_ok("loose allows undeclared",
              "@contract S loose { port: int }\n"
              "server {\n  @is S\n  port: 1\n  extra: 2\n}\n");

    // 可选字段缺席 —— 不该报 E-CONTRACT-003
    expect_ok("optional may be absent",
              "@contract S { note: str optional }\nserver { @is S }\n");

    // 顶层**裸块**（没有外层花括号）到文件结尾收尾是合法的 —— 不许报 E-PARSE-001
    expect_ok("top-level bare block ends at EOF", "a: 1\nb: 2\n");

    // 数组元素是对象：`{` 由 parse_value_inner 消费后必须仍能解析，
    // 否则 E-PARSE-006 会误伤这个合法写法（这是本次改动最容易被碰坏的地方）
    expect_ok("array of objects", "x: [ { a: 1 } { b: 2 } ]\n");

    // 裸块与片段体仍要正常闭合
    expect_ok("bare block closed", "pool greeter { max: 10 }\n");
    expect_ok("fragment body closed", "@f type X name y { a: 1 }\n");

    // 回归（errno 串味）：同一文档里**先**出现溢出字面量，后面的普通整数不得
    // 被误判成 Float —— 否则声明为 int 的合法字段会报出不该报的 E-CONTRACT-002。
    // 这两条在 errno 未清零的实现下会 FAIL（判别实验验证过）。
    expect_ok("int field after big-int literal",
              "@contract C loose { n: int }\n"
              "x {\n  @is C\n  big: 99999999999999999999999\n  n: 123\n}\n");
    expect_ok("int field after inf literal",
              "@contract C loose { n: int }\n"
              "x {\n  @is C\n  big: 1e400\n  n: 123\n}\n");
}

// err 允许为 nullptr：只断言**不崩**。
//
// 这里刻意**不**断言返回值：err 传 NULL 时，除「深度超限」（走 st.aborted，
// 与 err 无关）之外的失败路径只把消息写进 err —— 于是调用方 `sml::parse(text)`
// 会拿到一棵被截断的对象而不知道出了错。那是既有 API 的设计取舍，不是本次
// 改造引入的，写成断言会把一个可疑行为固化成契约；故只钉「不崩」。
static void test_err_null_safe() {
    std::cout << "[err=NULL]\n";
    sml::ValuePtr v = sml::Parser::parse("@contract C { a: int min }\n", nullptr);
    (void)v;
    v = sml::Parser::parse("k: \"abc\n", nullptr);
    (void)v;
    std::cout << "  ok: err=NULL does not crash\n";
}

int main() {
    test_lex_codes();
    test_parse_codes();
    test_contract_codes();
    test_bound_literals();
    test_eof_and_key_position();
    test_value_model();
    test_limit_and_include();
    test_positive_controls();
    test_err_null_safe();

    if (failures == 0) {
        std::cout << "ALL CODE TESTS PASSED\n";
        return 0;
    }
    std::cout << failures << " FAILURES\n";
    return 1;
}
