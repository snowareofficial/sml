// SPDX-License-Identifier: MulanPSL-2.0
//! 「**同因同码**」回归：错误码是跨端契约，改文案不该改码。
//!
//! 这一组用例的职责不是「Rust 报了错」（那是别的测试的活），而是钉住**报的是哪个码**：
//!
//! 1. 同一触发条件在五端必须给同一个码（`errors/README.md` 的纪律）；
//! 2. 文案可以改，码不能改 —— 用户脚本与统计都建立在码上；
//! 3. 生成的常量表（`sml-codes`）与唯一事实来源 `errors/codes.sml` 不许漂移。
//!
//! 第 3 条尤其要紧：`sml-codes` 是**生成物**，如果只改生成脚本不重跑、
//! 或手改了生成物，这里会当场拦住。

use sml::parse;

/// 跑一段源码，返回它的错误码；解析成功则 panic（用例写错了）。
fn code_of(src: &str) -> &'static str {
    match parse(src) {
        Ok(_) => panic!("应当解析失败，实际成功了：{src:?}"),
        Err(e) => e.code(),
    }
}

fn assert_code(src: &str, want: &str) {
    let got = code_of(src);
    assert_eq!(got, want, "源码 {src:?} 的错误码不符");
}

/// 词法层：`E-LEX-*`。
#[test]
fn lex_codes() {
    assert_code("k: \"未闭合\n", "E-LEX-001"); // 字符串未闭合
    assert_code("/* 没有收尾\nk: 1\n", "E-LEX-002"); // /* ... */ 未闭合
    assert_code("_* 没有收尾\nk: 1\n", "E-LEX-003"); // _* ... *_ 未闭合
    assert_code("k: \"\\z\"\n", "E-LEX-004"); // 未知转义
    assert_code("k: \"\\u12\"\n", "E-LEX-005"); // \u 位数不足
    assert_code("k: \"\\uD800\"\n", "E-LEX-005"); // 代理区码点非法
}

/// `E-LEX-006`（转义符后遇文件结尾）只有**直接走词法层**才看得到。
///
/// 走 `parse()` 时看不到：它先把正文按行剥离 `@version`/`@feature` 再重建，
/// 文件结尾那个位置已经被补上一个换行，于是同一条输入变成「未知转义」（E-LEX-004）。
/// 这不是缺陷，是「按行重建」的必然结果 —— 记下来，免得下次有人以为码丢了。
#[test]
fn lex_escape_at_eof_needs_the_lexer() {
    match sml_lex::tokenize("k: \"abc\\") {
        Ok(_) => panic!("应当失败"),
        Err(e) => assert_eq!(e.code(), "E-LEX-006"),
    }
    // 走 parse() 则是另一个码：同一个触发位置，但不是同一个条件
    assert_code("k: \"abc\\", "E-LEX-004");
}

/// 顶层标量**不可往返** ⇒ `E-PARSE-008`（W16 接线）。
///
/// 改之前这里是**静默造键**：`42` 被 `parse_block(None)` 当成「键即值」的裸词键，
/// 解析成 `{"42": 42}`，重新序列化得到 `"42": 42` ≠ `42` —— 数据形状被悄悄改掉。
/// 而 `E-PARSE-008` 一直躺在 `sml-codes` 里（名字就叫「顶层标量不可往返」）、
/// **从未被接线**：全仓只有常量定义与 doctest 引用。
///
/// 判据（已实测定死）：**顶层恰好一个标量 token**。故 `hello world`（两 token，
/// 得 `{"hello":"world"}`、值可往返）**不算**；带指令的顶层标量（token 数 > 1）
/// 也**不报** —— 有意保守，宁漏不误伤。
#[test]
fn top_level_scalar_needs_a_container() {
    assert_code("42\n", "E-PARSE-008");
    assert_code("hello\n", "E-PARSE-008");
    assert_code("\"hi\"\n", "E-PARSE-008");
    assert_code("true\n", "E-PARSE-008");

    // 正对照：合法顶层形态一律不受影响（这些值都能往返）
    for ok in [
        "a: 1\n",
        "a: 1\nb: 2\n",
        "[1, 2]\n",
        "{ a: 1 }\n",
        "42: x\n",
        "hello world\n",
    ] {
        assert!(parse(ok).is_ok(), "应当仍然解析成功：{ok:?}");
    }
}

/// 语法层：`E-PARSE-*`。
#[test]
fn parse_codes() {
    assert_code("a {\n", "E-PARSE-001"); // 块未闭合
    assert_code("a: 1\n}\n", "E-PARSE-003"); // 多余的结束符号
    assert_code("@\nk: 1\n", "E-PARSE-004"); // 孤立的 `@`
    assert_code("a: 1, 2\n", "E-PARSE-007"); // 裸词中不可含逗号
}

/// 契约层：`E-CONTRACT-*`。同一份契约在五端必须给出同一个码。
#[test]
fn contract_codes() {
    // 引用了未定义的契约
    assert_code("server { @is Nope }\n", "E-CONTRACT-001");
    // 字段类型不符（`port` 要 int，给了裸词 → str）
    assert_code(
        "@contract S { port: int }\nserver {\n  @is S\n  port: oops\n}\n",
        "E-CONTRACT-002",
    );
    // 必填字段缺失
    assert_code(
        "@contract S { port: int }\nserver { @is S }\n",
        "E-CONTRACT-003",
    );
    // 未声明字段（严格模式）
    assert_code(
        "@contract S { port: int }\nserver {\n  @is S\n  port: 1\n  extra: 2\n}\n",
        "E-CONTRACT-004",
    );
    // 数值越界
    assert_code(
        "@contract S { ratio: num min 0 max 1 }\nserver {\n  @is S\n  ratio: 2\n}\n",
        "E-CONTRACT-005",
    );
    // 枚举取值不在列表内 —— 与「类型不符」是**两条码**，不能混用
    assert_code(
        "@contract S { st: enum [ a b ] }\nserver {\n  @is S\n  st: c\n}\n",
        "E-CONTRACT-006",
    );
}

/// 片段与 include：`E-INCLUDE-*`。
#[test]
fn include_codes() {
    // 未定义的片段引用（拼错片段名不该静默变成字符串）
    assert_code("k: &nope\n", "E-INCLUDE-006");
    // 键列表语法非法（空键列表）见下面的 include_key_list_codes —— 它走不到 `parse()`。
}

/// `E-INCLUDE-005`（键列表语法非法：空键列表）只有**直接走 include 指令解析**才看得到。
///
/// 理由与上面 `lex_escape_at_eof_needs_the_lexer` 相同：`parse()` 不做 include 展开
/// （那是 `sml-include` 的活），这条分支在 `parse_include_line` 里。
/// JS 侧是在解析器内部处理 include，所以 `js/probe-error-codes.mjs` 能用
/// `include "x.sml" as w { }` 走全量 `parse()` 触发它。**同一个语义条件（空键列表），
/// 两端入口不同，码必须相同** —— 这正是本文件要钉的关系；少了这一条，
/// 「probe 与 error_codes.rs 同条件同码」那句话就不成立了。
///
/// （W14 带出来的：JS 那条分支原先抛宿主 `ReferenceError`，走 `parseSafe` 更是被
/// 静默吞成 `ok:false` + 无码。修完才有这条跨端对应关系。)
#[test]
fn include_key_list_codes() {
    match sml_include::parse_include_line("import \"m.sml\" { }", sml_feature::FeatureSet::all()) {
        Ok(_) => panic!("空键列表应当失败"),
        Err(e) => assert_eq!(e.code(), "E-INCLUDE-005"),
    }
}

/// 特性与版本：`E-FEATURE-*`。
#[test]
fn feature_codes() {
    assert_code("@version v9\nk: 1\n", "E-FEATURE-004"); // 不支持的版本
    assert_code("@feature enable nosuch\nk: 1\n", "E-FEATURE-003"); // 未知特性名
    assert_code("@feature nosuch\nk: 1\n", "E-FEATURE-006"); // 未知子命令（与上一条是两条码）
    assert_code("@feature\nk: 1\n", "E-FEATURE-007"); // @feature 缺参数
    assert_code("@feature mode nope\nk: 1\n", "E-FEATURE-008"); // mode 取值非法
}

/// 上限：`E-LIMIT-*`。深嵌套必须**报错返回**而不是打穿栈。
#[test]
fn limit_codes() {
    let src = "a { ".repeat(200);
    let got = code_of(&src);
    assert_eq!(got, "E-LIMIT-001", "深嵌套应报 E-LIMIT-001");
    // 边界要**逐格**钉住，不能只测 200（200 在两种口径下都报，测不出差一格）。
    // 口径：128 层放行、第 129 层报此码 —— 这是**实测**出来的（五端闭合嵌套扫过一遍）：
    //   Rust 127 / C 127 / C++ 128 / JS 128 / Lua 128（改前）→ 统一为 128 / 129。
    //   Rust 与 C 原先用 `>=`，128 层就报，而文案写的是「**超过** 128 层」，自相矛盾。
    // ⚠️ 必须用**闭合**输入：不闭合会被 E-PARSE-001（未闭合块）先抓住，测到的不是深度。
    let nest = |n: usize| "a { ".repeat(n) + &"} ".repeat(n);
    assert!(parse(&nest(128)).is_ok(), "128 层应放行");
    assert_code(&nest(129), "E-LIMIT-001");
    // 数组入口必须同口径 —— 两处守卫历史上就各自绕过过（只补一条会漏另一条）。
    let arr = |n: usize| "k: ".to_string() + &"[ ".repeat(n) + &"] ".repeat(n);
    assert!(parse(&arr(128)).is_ok(), "128 层数组应放行");
    assert_code(&arr(129), "E-LIMIT-001");
}

/// `Display` 要把码缀在文案之后 —— 用户看到的每一句话都能拿去查码表。
#[test]
fn display_carries_the_code() {
    let e = parse("k: &nope\n").unwrap_err();
    let shown = e.to_string();
    assert!(
        shown.ends_with("[E-INCLUDE-006]"),
        "Display 应以 `[码]` 结尾，实际：{shown}"
    );
    assert!(!e.message().contains('['), "message() 不应含码：{}", e.message());
    // 兼容通道：转成 String 时只取文案（既有 `Result<_, String>` 接口行为不变）
    let s: String = e.into();
    assert_eq!(s, "sml: 未定义的片段引用 `&nope`");
}

/// 生成的常量表与唯一事实来源 `errors/codes.sml` 必须一致。
///
/// `sml-codes` 是生成物（`python errors/gen_codes.py`）：只改生成脚本不重跑、
/// 或手改了生成物，都会在这里被抓住。
#[test]
fn generated_codes_match_registry() {
    let registry_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../errors/codes.sml");
    let registry = std::fs::read_to_string(registry_path)
        .unwrap_or_else(|e| panic!("读不到 {registry_path}：{e}"));
    let mut missing = Vec::new();
    for code in sml_codes::ALL {
        if !registry.contains(code) {
            missing.push(*code);
        }
    }
    assert!(
        missing.is_empty(),
        "这些码由 gen_codes.py 生成，却不在 errors/codes.sml 里：{missing:?}"
    );
    assert_eq!(
        sml_codes::ALL.len(),
        sml_codes::COUNT,
        "ALL 与 COUNT 不一致（生成物被手改过？）"
    );
}
