//! `weak*` 三件（`@weakbind` / `@weakdata` / `@weakdoc`）的**护栏用例**。
//!
//! 背景：这三件在 Rust 侧早已实现（`parser.rs`），但此前**零测试覆盖**（`docs/open-work.md` §9）。
//! 本文件把它们规范里写死的硬约束逐条钉住 —— 尤其是每次都要验的那条**核心判据**：
//!
//! > 有无 `@weak*` 指令，**主树与 `contract_map` 必须逐字节相同**。
//!
//! 三者的门控都是 `@feature enable type.weak`（默认关 ⇒ 关掉即回到既有行为）。
use sml::{parse, parse_with, ParseOptions, Value};

/// 打开弱指令特性的文档头。
const ON: &str = "@feature enable type.weak\n";

fn tree_dbg(v: &Value) -> String {
    format!("{v:?}")
}

// ---------------------------------------------------------------------------
// @weakdoc —— 只登记 profile 名，库不读文件系统
// ---------------------------------------------------------------------------

#[test]
fn weakdoc_registers_name_only() {
    let out = parse_with(&format!("{ON}@weakdoc tls\nweb {{ a: 1 }}\n"), ParseOptions::new()).unwrap();
    assert_eq!(out.weak_docs.len(), 1, "应登记一条 profile 引用");
    assert_eq!(out.weak_docs[0].name, "tls");
    // 名字只是名字：主树里既没有 `tls` 键，也没有 `__weak*` 痕迹
    assert!(out.value.get("tls").is_none(), "profile 名不得进主树：{:?}", out.value);
    assert!(out.value.get("web").is_some());
}

#[test]
fn weakdoc_duplicate_is_rejected() {
    // 文档级判重（不看块路径）：同一个 profile 重复挂就是笔误
    let err = parse(&format!("{ON}@weakdoc tls\n@weakdoc tls\nweb {{ a: 1 }}\n")).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("E-PARSE-030"), "重复挂应报 E-PARSE-030：{msg}");
}

#[test]
fn weakdoc_rejects_payload_block() {
    // 载荷是 `@weakdata` 的形态；混用会让人以为 profile 能内联
    let err = parse(&format!("{ON}@weakdoc tls {{ a: 1 }}\n")).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("E-PARSE-019"), "载荷块应被拒：{msg}");
}

// ---------------------------------------------------------------------------
// @weakdata —— 载荷根本不进树
// ---------------------------------------------------------------------------

#[test]
fn weakdata_payload_stays_out_of_tree() {
    let out = parse_with(
        &format!("{ON}@weakdata ui {{ theme: dark }}\nweb {{ a: 1 }}\n"),
        ParseOptions::new(),
    )
    .unwrap();
    assert_eq!(out.weak_data.len(), 1);
    let wd = &out.weak_data[0];
    assert_eq!(wd.name, "ui");
    assert_eq!(
        wd.value.get("theme").and_then(|x| x.as_str()),
        Some("dark"),
        "载荷内容应在旁路表里：{:?}",
        wd.value
    );
    // 主树里查不到同名键 —— 这是「有无 @weakdata，主树逐字节相同」的落点
    assert!(out.value.get("ui").is_none(), "载荷不得进主树：{:?}", out.value);
}

#[test]
fn weakdata_last_wins() {
    // 同一 (path, name) 只留最后一条
    let out = parse_with(
        &format!("{ON}@weakdata ui {{ theme: dark }}\n@weakdata ui {{ theme: light }}\n"),
        ParseOptions::new(),
    )
    .unwrap();
    assert_eq!(out.weak_data.len(), 1, "同名同路径应只留一条");
    assert_eq!(
        out.weak_data[0].value.get("theme").and_then(|x| x.as_str()),
        Some("light"),
        "应是最后一次生效"
    );
}

#[test]
fn weakdata_requires_payload_block() {
    let err = parse(&format!("{ON}@weakdata ui\n")).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("E-PARSE-019"), "缺载荷块应报 E-PARSE-019：{msg}");
}

// ---------------------------------------------------------------------------
// @weakbind —— 只作用于紧邻的下一个块；数据仍在树里，只是库不校验
// ---------------------------------------------------------------------------

#[test]
fn weakbind_targets_the_next_block() {
    let out = parse_with(
        &format!("{ON}@weakbind tls\nweb {{ a: 1 }}\n"),
        ParseOptions::new(),
    )
    .unwrap();
    assert_eq!(out.weak_binds.len(), 1);
    let wb = &out.weak_binds[0];
    assert_eq!(wb.name, "tls", "契约名按书写原样保留（库不认识它）");
    assert!(
        format!("{:?}", wb.path).contains("web"),
        "应指向紧邻的下一个块，实际路径：{:?}",
        wb.path
    );
    assert!(out.value.get("web").is_some());
}

#[test]
fn weakbind_accepts_field_body() {
    let out = parse_with(
        &format!("{ON}@weakbind tls {{ host: str }}\nweb {{ a: 1 }}\n"),
        ParseOptions::new(),
    )
    .unwrap();
    let body = out.weak_binds[0].body.as_ref().expect("应解析出字段体");
    assert!(body.contains_key("host"), "字段体应有 host：{body:?}");
}

#[test]
fn weakbind_twice_is_rejected() {
    let err = parse(&format!("{ON}@weakbind tls\n@weakbind tls\nweb {{ a: 1 }}\n")).unwrap_err();
    assert!(
        format!("{err}").contains("E-PARSE-029"),
        "连续两条应被拒（E-PARSE-029）：{err}"
    );
}

#[test]
fn weakbind_before_scalar_is_rejected() {
    // 后面跟标量 ⇒ target-not-block
    let err = parse(&format!("{ON}@weakbind tls\nx: 1\n")).unwrap_err();
    assert!(
        format!("{err}").contains("E-PARSE-029"),
        "指向标量应被拒（E-PARSE-029）：{err}"
    );
}

#[test]
fn weakbind_dangling_is_rejected() {
    // 写在块末尾、后面没有块 ⇒ dangling
    let err = parse(&format!("{ON}web {{ a: 1 }}\n@weakbind tls\n")).unwrap_err();
    assert!(
        format!("{err}").contains("E-PARSE-029"),
        "悬空应被拒（E-PARSE-029）：{err}"
    );
}

#[test]
fn weakbind_soft_check_warns_on_mismatch() {
    // 先读回一份内置契约表（宿主注入的标准做法），再用与实际不符的声明挂上去
    let builtin = "@contract tls { host: str }\n";
    let captured = parse_with(
        builtin,
        ParseOptions::new().capture_contracts(true),
    )
    .unwrap();
    let table = captured.contracts.expect("应捕获到契约表");

    let out = parse_with(
        &format!("{ON}@weakbind tls {{ host: int }}\nweb {{ a: 1 }}\n"),
        ParseOptions::new().builtin_contracts(table),
    )
    .unwrap();

    assert!(
        out.diagnostics.iter().any(|d| format!("{d:?}").contains("WeakBindMismatch")),
        "类型对不上应产出弱绑定诊断：{:?}",
        out.diagnostics
    );
    // 软校验只读 ⇒ 主树不受影响
    assert!(out.value.get("web").is_some());
}

// ---------------------------------------------------------------------------
// 核心判据：有没有 @weak*，主树必须逐字节相同
// ---------------------------------------------------------------------------

#[test]
fn main_tree_is_byte_identical_with_or_without_weak_directives() {
    let base = format!("{ON}web {{ a: 1 b: [ 1 2 ] }}\n");
    let with_doc = format!("{ON}@weakdoc tls\nweb {{ a: 1 b: [ 1 2 ] }}\n");
    let with_data = format!("{ON}@weakdata ui {{ theme: dark }}\nweb {{ a: 1 b: [ 1 2 ] }}\n");
    let with_bind = format!("{ON}@weakbind tls\nweb {{ a: 1 b: [ 1 2 ] }}\n");

    let expected = tree_dbg(&parse(&base).unwrap());
    for (label, src) in [
        ("@weakdoc", with_doc),
        ("@weakdata", with_data),
        ("@weakbind", with_bind),
    ] {
        let got = tree_dbg(&parse(&src).unwrap());
        assert_eq!(got, expected, "`{label}` 不得改变主树");
    }
}

// ---------------------------------------------------------------------------
// 门控：默认关
// ---------------------------------------------------------------------------

#[test]
fn weak_directives_are_opt_in() {
    for (label, src) in [
        ("@weakdoc", "@weakdoc tls\nweb { a: 1 }\n"),
        ("@weakdata", "@weakdata ui { theme: dark }\nweb { a: 1 }\n"),
        ("@weakbind", "@weakbind tls\nweb { a: 1 }\n"),
    ] {
        let err = parse(src).unwrap_err();
        assert!(
            format!("{err}").contains("E-FEATURE-001"),
            "`{label}` 未开 `type.weak` 应报 E-FEATURE-001：{err}"
        );
    }
}
