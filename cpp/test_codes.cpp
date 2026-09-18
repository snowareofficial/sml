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
//   ⚠️ 以下三条**已随 W16 末段对齐**：旧注释称「本实现静默接受」是 W16 之前的过时描述，已更正。
//   - `k: "\u12"`（位数不足）/ `k: "\uD800"`（代理区）/ 未闭合块注释 `* … */` 与 `_* … *_`
//     现在**都报码**（E-LEX-005 / E-LEX-005 / E-LEX-002 / E-LEX-003），与 Rust 同码；
//     见下 `test_w16_codes()` 的 lex 组断言。E-LEX-004（未知转义）此前静默放行属
//     「声明与实现不符」，现已补上（见下面的用例）。
//   其余仍属真实的端间差异（数值归类、include 触发写法等）记在下方。
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
//   - ⚠️ **`@include` 的成功展开曾经是坏的（W18，已修）**：修复前 a.sml =
//     `@include "b.sml"` + `from_a: 1` 解析后只剩一个垃圾键 `include = "include"`
//     —— includer 自己的 `from_a` 与被包含文件的字段一起丢失。根因是「边解析边插
//     token」：把目标文件的 token 插到路径 token 之前后又 `st.i++`，恰好跳过插入段的
//     首个 token。现在 include 挪到**解析之前**的 `expand_includes` 里递归展开，
//     链栈只装当前路径（与 Rust/C 同架构）。用例见下面 [INCLUDE expansion / W18] 组。
//     ⚠️ 这里记着**当时为什么不能只修索引**：那个 off-by-one 恰好压住了无限展开
//     （首 token 被跳过 ⇒ 嵌套的 `@include` 永远不被当指令），而环检测的栈是 push 完
//     立刻 pop、**永远为空**，自包含根本拦不住。故修索引必须同时给出环检测 + 上限，
//     否则 a↔b 会变成真死循环。本文件的正向用例（链式/菱形）正是钉住这一点。
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
#include <filesystem>
#include <fstream>
#include <iostream>
#include <sstream>
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
    expect_code("stray closing bracket (mismatch: block expects '}', got ']')",
                "x { ] }\n", SML_E_PARSE_002);
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
    std::cout << "[LIMIT]\n";

    // 深嵌套必须报错返回，而不是打穿栈（200 层 > 上限 128）。
    // 两条入口都要测：块嵌套走 parse_block_nested、数组嵌套走 parse_value —— 它们
    // 历史上曾各自绕过守卫（纯块嵌套能打穿栈），只测一条会漏掉另一条。
    std::string deep;
    for (int i = 0; i < 200; i++) deep += "a { ";
    expect_code("block nesting too deep (200)", deep, SML_E_LIMIT_001);

    std::string deep_arr = "k: ";
    for (int i = 0; i < 200; i++) deep_arr += "[ ";
    expect_code("array nesting too deep (200)", deep_arr, SML_E_LIMIT_001);

    // include 的用例已移到下面的 [INCLUDE expansion / W18] 组：那里用真实临时目录，
    // 越界那条改成**目标真实存在**（Rust/C 都是先 canonicalize 再比前缀，故目标不
    // 存在时先报 E-INCLUDE-001，用不存在的路径测越界是测不出越界的）。
}

// ------------------------------------------------------------------
// include 展开（W18）：E-INCLUDE-001/002/003/004/010/011、E-LIMIT-003
//
// 这一组是 W18 的验收用例，**同时是判别实验**：
//   - 链式包含两侧字段都在：修复前只剩垃圾键 `include = "include"`，用例直接红。
//   - 自包含 / 互包含 → E-INCLUDE-002：修复前环检测的栈 push 完立刻 pop、永远为空，
//     自包含根本拦不住（旧的 off-by-one 恰好压住了无限展开，只改索引就变死循环）。
//   - **菱形包含必须合法**：这条是反例 —— 把环检测写成「见过即拒」（集合而非链栈）
//     会把菱形包含误判成环，这条用例专门挡住那种"修法"。
//   - 子目录链式包含：子文件的基准目录是**它自己的**所在目录，修复前一律相对根目录。
//
// 需要真实文件，故用系统临时目录建临时工程，跑完删掉。
// ------------------------------------------------------------------
namespace fs = std::filesystem;

static std::string fresh_dir(const std::string& tag) {
    std::error_code ec;
    fs::path d = fs::temp_directory_path(ec) / ("sml_w18_" + tag);
    fs::remove_all(d, ec);
    fs::create_directories(d, ec);
    return d.string();
}

static void write_in(const std::string& dir, const std::string& name, const std::string& body) {
    std::error_code ec;
    fs::path p = fs::path(dir) / name;
    fs::create_directories(p.parent_path(), ec);
    std::ofstream f(p, std::ios::binary);
    f << body;
}

// 以 dir 为基准目录解析 dir/name 的内容（读文件而不是内联，免去转义引号的噪音）
static sml::ValuePtr parse_in(const std::string& dir, const std::string& name, std::string* err) {
    std::ifstream f(fs::path(dir) / name, std::ios::binary);
    std::stringstream ss; ss << f.rdbuf();
    return sml::Parser::parse(ss.str(), err, dir);
}

static void expect_int(const std::string& tag, const sml::ValuePtr& v,
                       const std::string& key, long long want) {
    sml::ValuePtr got = v ? v->get(key) : nullptr;
    if (got && got->tag == sml::Value::Tag::Int && got->i == want) {
        std::cout << "  ok: " << tag << " -> " << key << " = " << want << "\n";
    } else {
        std::cout << "FAIL: " << tag << " -> " << key << " expected " << want << "\n";
        failures++;
    }
}

// 重复键在 SML 里**合并成数组**（set_field_local）：菱形包含会让叶子的字段在 token
// 流里出现两次，故断言它是 [4, 4] —— 这比断言"值为 4"更贴事实，也顺带钉住
// 「同一个文件被包含两次 ≠ 环」这个结论。
static void expect_int_arr(const std::string& tag, const sml::ValuePtr& v,
                           const std::string& key, const std::vector<long long>& want) {
    sml::ValuePtr got = v ? v->get(key) : nullptr;
    bool ok = got && got->tag == sml::Value::Tag::Arr && got->arr.size() == want.size();
    for (std::size_t i = 0; ok && i < want.size(); i++) {
        sml::ValuePtr e = got->arr[i];
        ok = e && e->tag == sml::Value::Tag::Int && e->i == want[i];
    }
    if (ok) {
        std::cout << "  ok: " << tag << " -> " << key << " = [";
        for (std::size_t i = 0; i < want.size(); i++)
            std::cout << (i ? ", " : "") << want[i];
        std::cout << "]\n";
    } else {
        std::cout << "FAIL: " << tag << " -> " << key << " expected an array of "
                  << want.size() << " int(s)\n";
        failures++;
    }
}

static void expect_code_in_dir(const std::string& tag, const std::string& dir,
                               const std::string& name, const char* want) {
    std::string err;
    sml::ValuePtr v = parse_in(dir, name, &err);
    if (v) {
        std::cout << "FAIL: " << tag << " parsed OK but should fail\n";
        failures++;
        return;
    }
    report(tag, want, err, code_is(err, want));
}

static void test_include_expansion() {
    std::cout << "[INCLUDE expansion / W18]\n";

    // ① 链式包含：includer 与被包含文件的字段**都在**（W18 的验收条件之一）
    {
        std::string d = fresh_dir("chain");
        write_in(d, "b.sml", "from_b: 2\n");
        write_in(d, "a.sml", "@include \"b.sml\"\nfrom_a: 1\n");
        std::string err;
        sml::ValuePtr v = parse_in(d, "a.sml", &err);
        if (!v) {
            std::cout << "FAIL: chained include should parse, got \"" << err << "\"\n";
            failures++;
        } else {
            expect_int("chained include: includer's own field", v, "from_a", 1);
            expect_int("chained include: included file's field", v, "from_b", 2);
        }
        fs::remove_all(d);
    }

    // ② 嵌套 include 相对**父文件所在目录**解析（子目录里的链）
    {
        std::string d = fresh_dir("subdir");
        write_in(d, "sub/c.sml", "from_c: 3\n");
        write_in(d, "sub/b.sml", "@include \"c.sml\"\nfrom_b: 2\n");
        write_in(d, "a.sml", "@include \"sub/b.sml\"\nfrom_a: 1\n");
        std::string err;
        sml::ValuePtr v = parse_in(d, "a.sml", &err);
        if (!v) {
            std::cout << "FAIL: subdir chained include should parse, got \"" << err << "\"\n";
            failures++;
        } else {
            expect_int("subdir include: root field", v, "from_a", 1);
            expect_int("subdir include: middle field", v, "from_b", 2);
            expect_int("subdir include: grandchild field", v, "from_c", 3);
        }
        fs::remove_all(d);
    }

    // ③ 自包含 → E-INCLUDE-002
    {
        std::string d = fresh_dir("self");
        write_in(d, "a.sml", "@include \"a.sml\"\nfrom_a: 1\n");
        expect_code_in_dir("self include", d, "a.sml", SML_E_INCLUDE_002);
        fs::remove_all(d);
    }

    // ④ 互包含（a→b→a）→ E-INCLUDE-002
    {
        std::string d = fresh_dir("mutual");
        write_in(d, "a.sml", "@include \"b.sml\"\n");
        write_in(d, "b.sml", "@include \"a.sml\"\n");
        expect_code_in_dir("mutual include", d, "a.sml", SML_E_INCLUDE_002);
        fs::remove_all(d);
    }

    // ⑤ 菱形包含（a→b、a→c、b 与 c 都→d）**必须合法**：反例见组头的说明
    {
        std::string d = fresh_dir("diamond");
        write_in(d, "d.sml", "from_d: 4\n");
        write_in(d, "b.sml", "@include \"d.sml\"\nfrom_b: 2\n");
        write_in(d, "c.sml", "@include \"d.sml\"\nfrom_c: 3\n");
        write_in(d, "a.sml", "@include \"b.sml\"\n@include \"c.sml\"\nfrom_a: 1\n");
        std::string err;
        sml::ValuePtr v = parse_in(d, "a.sml", &err);
        if (!v) {
            std::cout << "FAIL: diamond include must be legal, got \"" << err << "\"\n";
            failures++;
        } else {
            expect_int("diamond include: root", v, "from_a", 1);
            expect_int("diamond include: left branch", v, "from_b", 2);
            expect_int("diamond include: right branch", v, "from_c", 3);
            expect_int_arr("diamond include: shared leaf (包含两次 → 键合并成数组)",
                           v, "from_d", {4, 4});
        }
        fs::remove_all(d);
    }

    // ⑥ 嵌套超过 32 层 → E-INCLUDE-004。
    //    修复前这里是**静默跳过**（栈恒空 ⇒ `size() < 32` 恒真，其实连 32 都不生效），
    //    字段凭空消失且没有任何提示。
    {
        std::string d = fresh_dir("deep");
        for (int i = 0; i < 40; i++) {
            if (i == 39) write_in(d, "f39.sml", "leaf: 1\n");
            else write_in(d, "f" + std::to_string(i) + ".sml",
                          "@include \"f" + std::to_string(i + 1) + ".sml\"\n");
        }
        expect_code_in_dir("include nesting deeper than 32", d, "f0.sml", SML_E_INCLUDE_004);
        fs::remove_all(d);
    }

    // ⑦ 菱形膨胀（每层把下一层包含两次 → 2^20 次读取）→ E-LIMIT-003。
    //    修复前 C++ **完全没有**这个闸（Rust/C 都有）：深度上限挡不住菱形膨胀。
    {
        std::string d = fresh_dir("bomb");
        const int N = 20;
        for (int i = 0; i < N; i++) {
            std::string body;
            if (i == N - 1) {
                body = "leaf: 1\n";
            } else {
                std::string inc = "@include \"l" + std::to_string(i + 1) + ".sml\"\n";
                body = inc + inc;
            }
            write_in(d, "l" + std::to_string(i) + ".sml", body);
        }
        expect_code_in_dir("diamond expansion bomb (2^20)", d, "l0.sml", SML_E_LIMIT_003);
        fs::remove_all(d);
    }

    // ⑧ 越界：目标**真实存在**但在基准目录之外 → E-INCLUDE-003。
    //    必须用存在的文件测：与 Rust/C 同序（先 canonicalize，失败即 E-INCLUDE-001），
    //    拿一个不存在的路径测越界，测到的其实是「读不到」。
    {
        std::string a = fresh_dir("escape_in");
        std::string b = fresh_dir("escape_out");
        write_in(b, "target.sml", "secret: 1\n");
        write_in(a, "a.sml", "@include \"../" + fs::path(b).filename().string() + "/target.sml\"\n");
        expect_code_in_dir("include escapes base dir (target exists)", a, "a.sml", SML_E_INCLUDE_003);
        fs::remove_all(a);
        fs::remove_all(b);
    }

    // ⑨ 目标不存在 → E-INCLUDE-001
    {
        std::string d = fresh_dir("missing");
        write_in(d, "a.sml", "@include \"_no_such_include.sml\"\n");
        expect_code_in_dir("include target missing", d, "a.sml", SML_E_INCLUDE_001);
        fs::remove_all(d);
    }

    // ⑩ 基准目录本身不可解析 → E-INCLUDE-010（fail-closed）。
    //    修复前这里被报成 E-INCLUDE-003：根因是用了 weakly_canonical —— 它只做词法
    //    规范化，一个**不存在**的目录也会"成功"规范化，于是「基准目录不可解析」被
    //    降级成「目录里没这个文件」，报出另一个码（错码比静默更坏）。现改用严格 canonical。
    {
        std::string d = fresh_dir("nobase");
        write_in(d, "a.sml", "@include \"x.sml\"\n");
        std::string err;
        sml::ValuePtr v = sml::Parser::parse("@include \"x.sml\"\n", &err, d + "_no_such_base");
        report("base dir unresolvable", SML_E_INCLUDE_010, err,
               (!v && code_is(err, SML_E_INCLUDE_010)));
        fs::remove_all(d);
    }

    // ⑪ 子文件里的词法失败 → E-INCLUDE-011。
    //    修复前**丢掉了子文件的词法错误**、把残缺 token 段插进去 —— 未闭合字符串会
    //    变成静默截断的文档（这正是 W18 那一类「文档被悄悄毁掉」的毛病）。
    {
        std::string d = fresh_dir("lex");
        write_in(d, "bad.sml", "k: \"unterminated\n");
        write_in(d, "a.sml", "@include \"bad.sml\"\n");
        expect_code_in_dir("lexical error inside included file", d, "a.sml", SML_E_INCLUDE_011);
        fs::remove_all(d);
    }

    // ⑫ include_dir 为空 = 按 sml.hpp 的约定关闭 include：指令被跳过，文档其余部分照常解析
    {
        std::string err;
        sml::ValuePtr v = sml::Parser::parse("@include \"whatever.sml\"\nfrom_a: 1\n", &err);
        if (!v) {
            std::cout << "FAIL: include disabled should not error, got \"" << err << "\"\n";
            failures++;
        } else {
            expect_int("include disabled (empty dir): directive skipped", v, "from_a", 1);
        }
    }
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
    expect_ok("fragment body closed", "@f type: X name: y { a: 1 }\n");

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

// ------------------------------------------------------------------
// W16：把静默改成报错的一批（见 TASK-hy3.md / c/test_codes.c）。
// 每条严格性都配**正对照**：把静默改成报错，最大的风险是误伤合法文档，
// 只测「应当失败」的一半会看不出误伤。期望值与 Rust / JS / Lua / C 逐字同码。
static void test_w16_codes() {
    std::cout << "[W16: lex]\n";
    expect_code("W16 unterminated string", "k: \"abc\n", SML_E_LEX_001);
    expect_code("W16 unclosed /* comment", "k: 1\n/* abc\n", SML_E_LEX_002);
    expect_code("W16 unclosed _* comment", "k: 1\n_* abc\n", SML_E_LEX_003);
    expect_code("W16 unknown escape", "k: \"a\\qb\"\n", SML_E_LEX_004);
    expect_code("W16 \\u too few digits", "k: \"\\u12\"\n", SML_E_LEX_005);

    expect_ok("W16 legal escapes", "k: \"a\\nb\\tc\"\n");
    expect_ok("W16 \\u fixed 4 digits", "k: \"\\u4e2d\"\n");
    expect_ok("W16 \\u braced", "k: \"\\u{1F680}\"\n");
    expect_ok("W16 /* comment closed", "k: 1\n/* ok */\n");
    expect_ok("W16 _* comment closed", "k: 1\n_* ok *_ \n");

    std::cout << "[W16: parse/include]\n";
    expect_code("W16 stray } in array", "m: [ } ]\n", SML_E_PARSE_003);
    expect_code("W16 closing mismatch a { ] }", "a { ] }\n", SML_E_PARSE_002);
    expect_code("W16 stray } at top", "k: 1\n}\n", SML_E_PARSE_003);
    expect_code("W16 stray ] at top", "k: 1\n]\n", SML_E_PARSE_003);
    expect_code("W16 unregistered directive (positional+body)", "@foo bar { x: 1 }\n", SML_E_PARSE_005);
    expect_code("W16 unregistered directive (positional, no body)", "@foo bar\n", SML_E_PARSE_005);
    expect_code("W16 undefined fragment ref", "x: &nosuchfrag\n", SML_E_INCLUDE_006);
    expect_code("W16 top-level scalar (bareword)", "42\n", SML_E_PARSE_008);
    expect_code("W16 top-level scalar (quoted)", "\"42\"\n", SML_E_PARSE_008);

    expect_ok("W16 fragment explicit params", "@foo type: Server name: p { x: 1 }\n");
    expect_ok("W16 fragment no params", "@foo { x: 1 }\n");
    expect_ok("W16 fragment def + ref", "@foo { x: 1 }\ny: &foo\n");
    expect_ok("W16 two-token bare key pair", "hello world\n");
    expect_ok("W16 key-value block", "42: x\n");
    expect_ok("W16 object block", "{ a: 1 }\n");
    expect_ok("W16 top-level array", "[1, 2]\n");
    expect_ok("W16 nested array", "m: [ 1, [2, 3], 4 ]\n");
    expect_ok("W16 top-level scalar with directive", "@version v1\n42\n");
    expect_ok("W16 empty input", "");
    expect_ok("W16 comment only", "# c\n");
}

int main() {
    test_lex_codes();
    test_parse_codes();
    test_contract_codes();
    test_bound_literals();
    test_eof_and_key_position();
    test_value_model();
    test_limit_and_include();
    test_include_expansion();
    test_positive_controls();
    test_w16_codes();
    test_err_null_safe();

    if (failures == 0) {
        std::cout << "ALL CODE TESTS PASSED\n";
        return 0;
    }
    std::cout << failures << " FAILURES\n";
    return 1;
}
