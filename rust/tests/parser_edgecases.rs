//! 解析器边界行为回归测试（对应审计报告 P0/P1/P2 静默缺陷）。
//!
//! 核心原则：错误必须显式化，绝不能静默吞掉数据。
//! - P0-1：@contract 块未闭合必须报错（否则主文档被静默清空）
//! - P0-2：单独 `@` / 非指令 `@name` 必须报错（否则吞掉后续行/块）
//! - P1-3：裸块多词参数全部保留（__name + __args），不再静默丢弃
//! - P1-4：前导零裸词保留为字符串（mode: 0755 不变成 755）
//! - P1-5：未知字符串转义必须报错（\U \d \z 不再静默丢反斜杠）
//! - P2-7：裸词含逗号必须报错（不再凭空拆出第二个键）
//!
//! 另有一条**已登记口径**的回归点：键位置上的裸 `:`（`a: B::C`、`a: http://x`）。
//! Rust 报错是**有意为之**（见 `errors/codes.sml` 的 `E-PARSE-006` 备注：W10 已把
//! Lua 的「静默当成键名」补掉），JS 至今仍静默接受、C 仍静默丢字段 ——
//! 欠账在那两端，不在 Rust。见文件末尾
//! `colon_in_bareword_is_rejected_by_design_and_message_is_the_gap`；
//! 真正可改的是**报错文案**（它不提 `::`、位置还落到下一行）。

use sml::parse;

#[test]
fn p0_1_unclosed_contract_must_error() {
    let r = parse("@contract C { a: str\nreal_key: 1");
    assert!(r.is_err(), "契约体未闭合应报错，实际 ok: {:?}", r);
    let r = parse("@contract C { a: str\nk1: 1\nk2: 2\nk3: 3");
    assert!(r.is_err(), "契约体未闭合（多行）应报错，实际 ok: {:?}", r);
}

#[test]
fn p0_1_well_formed_contract_ok() {
    // 合法契约定义仍应正常，不进主树
    let r = parse("@contract C { a: str }");
    assert!(r.is_ok(), "合法 @contract 应 ok，实际 err: {:?}", r);
}

#[test]
fn p0_2_lone_at_must_error() {
    // 单独 `@` 后换行：吞掉后续内容
    let r = parse("@\nk: 1");
    assert!(r.is_err(), "单独 @ 应报错，实际 ok: {:?}", r);
    // 块内单独 `@`：清空块内容
    let r = parse("o {\n@\nk: 1\n}");
    assert!(r.is_err(), "块内单独 @ 应报错，实际 ok: {:?}", r);
    // `@name` 后既非已知指令又无片段体 `{`：静默吞数据的兜底报错
    let r = parse("@notacommand");
    assert!(r.is_err(), "非指令 @name 无片段体应报错，实际 ok: {:?}", r);
}

#[test]
fn p0_2_valid_fragment_still_ok() {
    // `@name { ... }` 是合法片段定义（即使名字不像片段），保持 ok
    let r = parse("@base { x: 1 }\nuse: &base");
    assert!(r.is_ok(), "合法片段定义应 ok，实际 err: {:?}", r);
    // 形式合法的片段定义（@name { }）不报错，仅当缺少片段体或非指令才报错
    let r = parse("@blk { x: 1 }");
    assert!(r.is_ok(), "合法片段语法 @blk {{}} 应 ok，实际 err: {:?}", r);
}

#[test]
fn p1_3_bare_block_keeps_all_args() {
    // 三词裸块：web/prod 必须保留，__name 取首个
    let r = parse("server web prod { port: 80 host: h }");
    assert!(r.is_ok(), "裸块应 ok，实际 err: {:?}", r);
    let v = r.unwrap();
    if let sml::Value::Object(m) = &v {
        let server = m.get("server").unwrap();
        if let sml::Value::Object(sm) = server {
            assert_eq!(sm.get("__type"), Some(&sml::Value::Str("server".into())));
            assert_eq!(sm.get("__name"), Some(&sml::Value::Str("web".into())));
            // prod 必须在 __args 里，不可丢失
            match sm.get("__args") {
                Some(sml::Value::Array(a)) => {
                    assert!(a.iter().any(|x| x == &sml::Value::Str("prod".into())));
                }
                other => panic!("prod 应进入 __args 数组，实际 {:?}", other),
            }
        } else {
            panic!("server 应为对象，实际 {:?}", server);
        }
    } else {
        panic!("顶层应为对象，实际 {:?}", v);
    }
}

#[test]
fn p1_4_leading_zero_kept_as_str() {
    let r = parse("mode: 0755");
    assert_eq!(r.unwrap().get("mode"), Some(&sml::Value::Str("0755".into())));
    let r = parse("id: 007");
    assert_eq!(r.unwrap().get("id"), Some(&sml::Value::Str("007".into())));
    // 0xFF 显式十六进制前缀：保留为字符串（非十进制，符合既有语义）
    let r = parse("x: 0xFF");
    assert_eq!(r.unwrap().get("x"), Some(&sml::Value::Str("0xFF".into())));
    // 普通数字不应受影响
    let r = parse("n: 123");
    assert_eq!(r.unwrap().get("n"), Some(&sml::Value::Int(123)));
}

#[test]
fn p1_5_unknown_escape_must_error() {
    let r = parse("p: \"C:\\Users\"");
    assert!(r.is_err(), "未知转义 \\U 应报错，实际 ok: {:?}", r);
    let r = parse("re: \"\\d+\"");
    assert!(r.is_err(), "未知转义 \\d 应报错，实际 ok: {:?}", r);
    let r = parse("x: \"\\z\"");
    assert!(r.is_err(), "未知转义 \\z 应报错，实际 ok: {:?}", r);
    // 合法转义仍正常
    let r = parse("p: \"a\\nb\"");
    assert_eq!(r.unwrap().get("p"), Some(&sml::Value::Str("a\nb".into())));
}

#[test]
fn p2_7_comma_in_bareword_must_error() {
    let r = parse("a: x,y");
    assert!(r.is_err(), "裸词含逗号应报错，实际 ok: {:?}", r);
    // 显式数组语法合法
    let r = parse("a: [x, y]");
    assert!(r.is_ok(), "数组语法应 ok，实际 err: {:?}", r);
    // 加引号合法
    let r = parse("a: \"x,y\"");
    assert_eq!(r.unwrap().get("a"), Some(&sml::Value::Str("x,y".into())));
}

#[test]
fn p2_6_comment_no_leading_space_still_ok() {
    // 注释符无需前置空格（实测已正确）
    let r = parse("a: 1#c");
    assert_eq!(r.unwrap().get("a"), Some(&sml::Value::Int(1)));
    let r = parse("a: x#c");
    assert_eq!(r.unwrap().get("a"), Some(&sml::Value::Str("x".into())));
    let r = parse("port: 465# 注释");
    assert_eq!(r.unwrap().get("port"), Some(&sml::Value::Int(465)));
}

// ---------------------------------------------------------------------------
// 裸词里的连续冒号（`a: B::C`）：**这是已登记的口径选择，不是待修缺陷**
//
// 先读 `errors/codes.sml` 的 `E-PARSE-006` 备注，那里已经写清楚了：
// 「**Lua 原先静默**（`:` 会被当成键名、多余的 `{` 会被当成键），W10 期间补齐
// （探针实测 `: 1` 解析出键名为 `:` 的树）」——即「键位置上的裸 `:` 应当报错」
// 是本仓库**有记录的取舍**：谁把它变成键名，谁才算静默。
//
// 同一份输入 `X { a: B::C }` 各端现状（实测，2026-10-10）：
//
// | 端 | 结果 | 与本仓库口径 |
// |---|---|---|
// | Rust | `ERR E-PARSE-006 期望键, 得 Some(Colon)` | ✅ 符合 |
// | JS   | `ok` ⇒ `{"X":{"a":"B",":":"C"}}`（裸 `:` 成键名） | ❌ 即上面那条已补过的静默 |
// | C    | `ok` ⇒ `{"X":{"a":"B"}}`（**丢掉**第二个字段） | ❌ 静默，且比 JS 更糟（JS 至少保住了数据） |
// | C++  | 包 Rust，随 Rust | ✅ |
//
// ⇒ **本测试不主张改行为**，它主张两件事：
//   1. **Rust 侧不要动摇**：报错是有意为之，改成「接受」会退回到已补过的静默缺陷；
//   2. **JS / C 那两格才是欠账**（去 TODO.md 的 P1，别在 Rust 单边改）。
//
// 真正可改的是**文案**，那是本轮的原始起因：下游把 `ty: Ty::U32`（想写字符串、
// 忘了引号）写进一份 `.sml`，得到的全部信息就是「期望键, 得 Some(Colon)」——
// **没有一个字提到 `::`**，而且词法已把 `::` 切成两半，报错点落在**后一行**。
// 用户只能靠二分找。建议（不做在本次提交里，因为它要动 `sml-codes` 的码表 +
// 五端同步）：给一个专码，文案里必须出现 `::` 与「含 `:` 的标量请加引号」。
// 注意 `a: http://x` 是同一机制，所以这不是「只有类型名会踩」的边角问题。
#[test]
fn colon_in_bareword_is_rejected_by_design_and_message_is_the_gap() {
    // 1. Rust 侧现状：显式报错，与 E-PARSE-006 的已登记口径一致。
    //    若这里开始 ok，说明有人把它改成了「接受裸 `:` 当键」—— 那是退回静默缺陷，
    //    应当先改 errors/codes.sml 的口径，并同步 JS / C。
    let r = parse("X {\n  a: B::C\n}\n");
    match r {
        Err(e) => assert_eq!(
            e.code(),
            "E-PARSE-006",
            "口径若真要变，改这里之前先改 errors/codes.sml 与 JS/C"
        ),
        Ok(v) => panic!(
            "Rust 开始接受键位置上的裸 `:` 了（形状：{v:?}）—— \
             这正是 E-PARSE-006 备注里说的、W10 已经从 Lua 补掉的静默行为"
        ),
    }

    // 2. URL 是同一个机制，单独钉住，因为它看起来完全无害。
    let r = parse("X {\n  a: http://x\n}\n");
    assert!(r.is_err(), "`http://x` 与 `B::C` 同机制；它开始 ok 说明 `::` 语义被改过");

    // 3. 加引号是五端一致且稳定的写法。
    let r = parse("X {\n  a: \"B::C\"\n}\n").expect("引号形式五端一致，必须 ok");
    assert_eq!(
        r.get("X").and_then(|x| x.get("a")),
        Some(&sml::Value::Str("B::C".into()))
    );
}

