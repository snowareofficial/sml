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
//! 另有一条**跨实现分歧**（不是缺陷，是待收敛项）：
//! `a: B::C` 在 Rust 报错、在 JS 把裸 `:` 当键、在 C 静默截断 —— 见文件末尾
//! `colon_in_bareword_rust_diverges_from_js_and_c` 的实测表。
//! 该测试只钉事实、不改行为；修法要跨五端同步，不能一端单独改。

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
// 裸词里的连续冒号（`a: B::C`）：**五端不一致，Rust 是最吵的那一端**
//
// 这是实测出来的分歧，不是猜测。同一份输入 `X {\n  a: B::C\n}\n`：
//
// | 端 | 结果 |
// |---|---|
// | JS   | ok，`{"X":{"a":"B",":":"C"}}` —— 裸 `:` 在键位置就是键名 `:` |
// | C    | ok，`{"X":{"a":"B"}}` —— **静默丢掉后半段**（最差：无声丢数据） |
// | Rust | **报错** `E-PARSE-006 期望键, 得 Some(Colon)` |
// | C++  | 包 Rust，随 Rust |
//
// 一个真实的踩坑现场：把 `ty: Ty::U32`（想写字符串，忘了引号）写进一份 SML 文档，
// Rust 报「期望键, 得 Some(Colon)」——**这句话里没有一个字提到 `::`**，
// 而且 token 已经被 `::` 切成两半，报错点落在**后一行**，人只能靠二分才找得到。
//
// 本测试**不改行为**，只把事实钉住。原因是修法有分歧且要跨五端同步：
//   * 学 JS（裸 `:` 当键）=> 与 C 不一致，且会继续静默生成奇怪的对象；
//   * 学 C（截断）=> 属于静默丢数据，与本 crate 的核心纪律直接冲突；
//   * 报一个**指名道姓**的错（推荐）=> 五端都要加，且需要 E-* 新码。
// 在协调之前，Rust 保持现状（显式报错优于静默），并把这条写进文档。
// 上游若要收敛，建议：裸词含 `::` 时给出专码 + 指向该行的报错，
// 文案里必须出现 `::` 与「加引号」。
//
// 注意：`a: http://x` 是同一件事（`//x` 被当成第二个键 `:` 的值），
// 所以这不是「只有类型名会踩」的边角问题 —— URL 一样会踩。
#[test]
fn colon_in_bareword_rust_diverges_from_js_and_c() {
    // Rust 侧现状：显式报错，绝不静默
    let r = parse("X {\n  a: B::C\n}\n");
    match r {
        Err(e) => assert_eq!(e.code(), "E-PARSE-006", "分歧点的码，改行为时要同步改这里"),
        Ok(v) => panic!("Rust 若不再报错，说明行为变了：{v:?}"),
    }

    // URL 是同一个机制，值得单独钉住，因为它看起来完全无害
    let r = parse("X {\n  a: http://x\n}\n");
    assert!(
        r.is_err(),
        "`http://x` 与 `B::C` 同机制；若这里开始 ok，说明 `::` 语义被改过"
    );

    // 加引号是当前唯一稳定写法，五端一致
    let r = parse("X {\n  a: \"B::C\"\n}\n").expect("引号形式五端一致，必须 ok");
    assert_eq!(
        r.get("X").and_then(|x| x.get("a")),
        Some(&sml::Value::Str("B::C".into()))
    );
}

