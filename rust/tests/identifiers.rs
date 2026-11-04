//! 标识符白名单（**S1b**）门面测试。
//!
//! 门控：`@feature enable lex.ident`（默认关 ⇒ 关掉即回到 v4 行为）。
//! 规则权威：`docs/featag-impl-slices.md` §2 的两张表。
use sml::parse;

const ON: &str = "@feature enable lex.ident\n";

fn err_of(src: &str) -> String {
    format!("{}", parse(src).unwrap_err())
}

#[test]
fn whitelisted_keys_are_accepted() {
    let src = format!("{ON}a_b-c: 1\n端口: 2\n設定ファイル: 3\n이름: 4\n");
    let v = parse(&src).expect("白名单内的键都应通过");
    assert_eq!(v.get("端口").and_then(|x| x.as_i64()), Some(2));
}

#[test]
fn path_reserved_chars_in_keys_are_rejected() {
    // 裸写时 `[` / `]` 会被词法当成结构符号（那里根本不是键），故这三类要用引号才能构成「键」。
    for key in ["\"a.b\"", "\"a[0]\"", "\"a]b\""] {
        let err = err_of(&format!("{ON}{key}: 1\n"));
        assert!(err.contains("E-LEX-007"), "`{key}` 应报标识符非法：{err}");
        assert!(
            err.contains("路径语法保留"),
            "文案应说清为什么（路径保留）：{err}"
        );
    }
    // 裸词里带点：词法不按 `.` 切，所以它就是一个含点的键名 ⇒ 同样要拒
    let err = err_of(&format!("{ON}a.b: 1\n"));
    assert!(err.contains("E-LEX-007"), "裸词含点也应拒：{err}");
}

#[test]
fn invisible_and_homoglyph_chars_are_rejected() {
    // 零宽空格 U+200B
    let err = err_of(&format!("{ON}a\u{200B}b: 1\n"));
    assert!(err.contains("E-LEX-007") && err.contains("零宽"), "{err}");
    // 组合重音符 U+0301（非预组合 ⇒ 非 NFC）
    let err = err_of(&format!("{ON}e\u{0301}: 1\n"));
    assert!(err.contains("E-LEX-007") && err.contains("NFC"), "{err}");
    // 西里尔 `а`(U+0430) 与拉丁 `a` 同形
    let err = err_of(&format!("{ON}а: 1\n"));
    assert!(err.contains("E-LEX-007") && err.contains("同形"), "{err}");
    // 全角字母
    let err = err_of(&format!("{ON}ａ: 1\n"));
    assert!(err.contains("E-LEX-007") && err.contains("全角"), "{err}");
    // 含空白的键：裸写会被词法切成两个词（那是另一回事），故用引号构造这个键
    let err = err_of(&format!("{ON}\"a b\": 1\n"));
    assert!(err.contains("E-LEX-007") && err.contains("空白"), "{err}");
}

#[test]
fn position_is_reported_in_characters_not_bytes() {
    // `端口` 两个汉字 = 6 字节，点在第 **3** 个字符
    let err = err_of(&format!("{ON}端口.x: 1\n"));
    assert!(err.contains("第 3 个字符"), "位置应按字符数给：{err}");
}

#[test]
fn gated_off_means_old_behaviour() {
    // 没开 lex.ident ⇒ `a.b` 仍是**一个键**（词法不按点切），一字不改
    let v = parse("a.b: 1\n").expect("未开特性时应照旧");
    let m = v.as_object().expect("顶层是块");
    assert_eq!(
        m.get("a.b").and_then(|x| x.as_i64()),
        Some(1),
        "键名应原样保留：{v:?}"
    );
}

#[test]
fn fragment_merge_keys_are_not_identifiers() {
    // `&base` 是片段引用，不是标识符：开启校验后仍要能合并
    let src = format!("{ON}@base {{ x: 1 }}\nhost: {{ &base }}\n");
    let v = parse(&src).expect("片段合并不应被标识符规则拦住");
    assert_eq!(
        v.get("host.x").and_then(|x| x.as_i64()),
        Some(1),
        "合并结果：{v:?}"
    );
}
