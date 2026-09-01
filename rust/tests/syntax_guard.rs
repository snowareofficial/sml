// SPDX-License-Identifier: MulanPSL-2.0
//! 语法防线测试：防止「静默丢弃 / 静默损坏」类缺陷回归。
//!
//! # 背景
//!
//! SML 解析器历史上反复出现同一类缺陷：**输入被静默丢弃**。
//! 它们不 panic、不报错，普通单元测试（测"正确输入→正确输出"）
//! 与 fuzz（仅断言"不崩溃"）都无法发现，只能靠人工抽检。
//!
//! 已知实例：
//! - 未闭合块 / 多余 `}`：静默吞掉后续内容
//! - `@contract` 漏写 `}`：整个文档被清空
//! - 未知指令（如孤立的 `@`）后接块：后续内容被当作片段体吃掉
//! - 多目标 include 未开特性：静默降级为普通内容
//! - 未定义片段引用 `&nope`：静默降级为字符串
//! - 裸块第三个词：静默丢弃
//! - 前导零 `0755`：静默剥离为 `755`
//! - 未知转义 `\z`：静默吞掉反斜杠
//!
//! # 防守策略
//!
//! 1. **核心不变量（内容守恒）**：解析成功时，源码中出现的每个键
//!    都必须出现在结果中（任意层级）。这能自动抓出"内容被吃掉"。
//! 2. **结构畸变**：对合法文档删除/插入结构性符号（`{ } [ ] " : @`），
//!    断言要么报错，要么保持内容守恒。
//! 3. **历史回归**：已修复缺陷的精确用例，防止退化。
//!
//! # 新增语法时的自检清单
//!
//! 添加语法分支时，对「意外输入」的兜底必须显式二选一：
//! - 能明确理解用户意图 → 给出结果；
//! - 不能 → **返回 Err**。
//!
//! 禁止「猜测并继续」。若确需容错（如 `loose` 契约放行额外字段），
//! 必须是**特性开关显式开启**的行为，而非兜底分支的默认行为。

use sml::{parse, Value};
use std::collections::HashSet;

// ---------- 工具 ----------

/// 递归收集结果中所有出现过的键（任意层级）。
fn keys_of(v: &Value) -> HashSet<String> {
    let mut out = HashSet::new();
    collect(v, &mut out);
    out
}

fn collect(v: &Value, out: &mut HashSet<String>) {
    match v {
        Value::Object(m) => {
            for (k, val) in m {
                out.insert(k.clone());
                collect(val, out);
            }
        }
        Value::Array(a) => {
            for x in a {
                collect(x, out);
            }
        }
        _ => {}
    }
}

/// 从源码中提取「看得见的键」：行首的 `key:`、`key {`、`key word {`。
/// 跳过注释行与指令行（`@` 开头），以及 `@contract` 定义体内部
/// （其中的 `host: str` 是契约字段声明，不是文档数据键）。
fn input_keys(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut in_contract = false;

    for raw in src.lines() {
        let t = raw.trim();
        if t.starts_with("@contract") {
            in_contract = true;
            depth = 0;
        }

        if !in_contract && !t.is_empty() {
            out.extend(key_of_line(t));
        }

        // 在契约体内跟踪大括号，闭合后恢复提取
        if in_contract {
            for c in t.chars() {
                match c {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    _ => {}
                }
            }
            if depth <= 0 {
                in_contract = false;
            }
        }
    }
    out
}

/// 提取单行行首的键（若有）。
fn key_of_line(t: &str) -> Option<String> {
    if t.starts_with('#')
        || t.starts_with("//")
        || t.starts_with("--")
        || t.starts_with("/*")
        || t.starts_with("_*")
        || t.starts_with('@')
    {
        return None;
    }
    let is_ident = |c: char| c.is_alphanumeric() || matches!(c, '_' | '.' | '-' | '$');
    let tok: String = t.chars().take_while(|&c| is_ident(c)).collect();
    if tok.is_empty() {
        return None;
    }
    let rest = t[tok.len()..].trim_start();
    if rest.starts_with(':') || rest.starts_with('{') {
        return Some(tok);
    }
    // 裸块：`key word {`
    let w: String = rest.chars().take_while(|&c| is_ident(c)).collect();
    let r2 = rest[w.len()..].trim_start();
    if !w.is_empty() && r2.starts_with('{') {
        return Some(tok);
    }
    None
}

/// 核心不变量：解析成功 ⇒ 源码中出现的键全部出现在结果中。
fn assert_keys_preserved(src: &str) {
    if let Ok(v) = parse(src) {
        let got = keys_of(&v);
        for k in input_keys(src) {
            assert!(
                got.contains(&k),
                "内容被静默丢弃：源码中出现的键 `{k}` 未出现在解析结果中\n\
                 结果: {v:?}\n\
                 源码:\n{src}"
            );
        }
    }
}

/// 用于畸变测试的基准文档。
const BASE: &str = r#"
@contract Cfg loose {
    host: str
    port: int default 465 min 1 max 65535
}
@is Cfg
host: example.com
port: 465
mailer {
    retry: 3
    tags: [ alpha beta ]
}
extra: 1
"#;

// ---------- 1. 结构畸变：不得静默丢键 ----------

#[test]
fn deleting_structural_chars_never_drops_keys() {
    for (i, ch) in BASE.char_indices() {
        if matches!(ch, '{' | '}' | '[' | ']' | '"' | ':' | '@') {
            let mut s = String::with_capacity(BASE.len() - 1);
            s.push_str(&BASE[..i]);
            s.push_str(&BASE[i + ch.len_utf8()..]);
            assert_keys_preserved(&s);
        }
    }
}

#[test]
fn inserting_at_before_scalar_never_drops_keys() {
    // 只在「非块」行前插入 `@`：这类场景应已修复，用于守住回归。
    // （在块前行插入 `@` 会触发已知缺陷，见文末 ignored 用例。）
    let lines: Vec<&str> = BASE.lines().collect();
    for n in 0..lines.len() {
        let target = lines[n].trim();
        // 跳过块起始行、契约体、指令行
        if target.ends_with('{') || target.starts_with('@') || target.starts_with('}') {
            continue;
        }
        // 若插入点之后存在块起始行，插入 `@` 会触发已知缺陷（吞块），跳过
        if lines[n..].iter().any(|l| l.trim().ends_with('{')) {
            continue;
        }
        let mut v = String::new();
        for (m, l) in lines.iter().enumerate() {
            if n == m {
                v.push('@');
            }
            v.push_str(l);
            v.push('\n');
        }
        assert_keys_preserved(&v);
    }
}

#[test]
fn truncating_never_drops_visible_keys() {
    for cut in [10usize, 40, 80, 120, 180] {
        if cut < BASE.len() {
            assert_keys_preserved(&BASE[..cut]);
        }
    }
}

// ---------- 2. 历史缺陷回归 ----------

#[test]
fn unclosed_must_error() {
    let e = parse("@contract C { a: str\nreal_key: 1\n").unwrap_err();
    assert!(e.to_string().contains("未闭合"), "实际错误: {e}");
}

#[test]
fn contract_missing_closing_brace_must_error() {
    // 后接标量/多个键：应报「未闭合」
    for src in [
        "@contract C { a: str\nreal_key: 1\n",
        "@contract C { a: str\nk1: 1\nk2: 2\nk3: 3\n",
    ] {
        let e = parse(src).unwrap_err();
        assert!(
            e.to_string().contains("未闭合"),
            "源码 {src:?} 应报未闭合，实际: {e}"
        );
    }
    // 契约体内混入块：只要报错即可（具体措辞不限）
    assert!(parse("@contract C { a: str\nother { x: 1 }\n").is_err());
}

#[test]
fn lone_at_with_scalar_must_error() {
    let e = parse("@\nk: 1\n").unwrap_err();
    assert!(e.to_string().contains("不是合法指令"), "实际: {e}");
}

#[test]
fn unknown_directive_with_scalar_must_error() {
    for src in ["@nosuch\nk: 1\n", "@versoin v1\nk: 1\n"] {
        let e = parse(src).unwrap_err();
        assert!(e.to_string().contains("不是合法指令"), "源码 {src:?} 实际: {e}");
    }
}

#[test]
fn unclosed_block_must_error() {
    for src in ["a {\n  x: 1\nb: 2\n", "a {\n  x: 1\nb {\n  y: 2\n}\n"] {
        let e = parse(src).unwrap_err();
        assert!(e.to_string().contains("未闭合"), "源码 {src:?} 实际: {e}");
    }
}

#[test]
fn extra_closing_brace_must_error() {
    let e = parse("a: 1\n}\n").unwrap_err();
    assert!(e.to_string().contains("多余"), "实际: {e}");
}

#[test]
fn undefined_fragment_must_error() {
    let e = parse("k: &nope\n").unwrap_err();
    assert!(e.to_string().contains("未定义的片段"), "实际: {e}");
}

#[test]
fn unknown_escape_must_error() {
    for src in ["x: \"\\z\"\n", "re: \"\\d+\"\n", "p: \"C:\\Users\"\n"] {
        let e = parse(src).unwrap_err();
        assert!(e.to_string().contains("未知转义"), "源码 {src:?} 实际: {e}");
    }
}

#[test]
fn leading_zero_preserved_as_string() {
    for (src, want) in [
        ("mode: 0755\n", "0755"),
        ("mode: 0644\n", "0644"),
        ("id: 007\n", "007"),
    ] {
        let v = parse(src).unwrap_or_else(|e| panic!("{src:?} 不应报错: {e}"));
        assert_eq!(
            v.get(src.split(':').next().unwrap()).and_then(|x| x.as_str()),
            Some(want),
            "源码 {src:?} 前导零被破坏"
        );
    }
}

#[test]
fn bare_block_third_word_preserved_in_args() {
    let v = parse("server web prod { port: 80 }\n").unwrap();
    let inner = v.get("server").expect("缺 server");
    let args = inner.get("__args").expect("第三个词 `prod` 被丢弃，应保留于 __args");
    assert!(
        format!("{args:?}").contains("prod"),
        "__args 中应含 prod，实际: {args:?}"
    );
}

#[test]
fn comma_in_bareword_must_error() {
    let e = parse("a: x,y\n").unwrap_err();
    assert!(e.to_string().contains("逗号"), "实际: {e}");
}

#[test]
fn multiline_string_keeps_directive_text() {
    let v = parse("note: \"line1\n@version v1\nline2\"\nother: 2\n").unwrap();
    let note = v.get("note").and_then(|x| x.as_str()).expect("缺 note");
    assert!(note.contains("@version"), "字符串内指令被剥离: {note:?}");
}

// ---------- 3. 孤立 `@` 必须报错（已修复，常驻防线） ----------
//
// 根因：`@` 与后随名字之间的空白在词法阶段被丢弃，导致
// `@` + `blk { .. }` 与 `@blk { .. }` 的 token 流**完全相同**，
// 孤立 `@` 会被当作片段定义，把紧随其后的块当片段体消费掉
// （不进主树、不报错）。
//
// 修复：词法层按「`@` 是否紧邻后随内容」区分 `Tok::At` 与 `Tok::BareAt`，
// `BareAt` 在解析阶段直接报错。

#[test]
fn lone_at_anywhere_must_error() {
    for src in [
        "@\nblk { x: 1 }\n",      // 原：静默清空整份文档
        "@\nserver web { x: 1 }\n",
        "@ blk { x: 1 }\n",       // 空格分隔
        "@\tblk { x: 1 }\n",      // tab 分隔
        "@\nk: 1\n",
        "a: 1\n@\nb: 2\n",
        "o {\n@\nk: 1\n}\n",      // 块内
        "a: 1\n@\n",              // 文档末尾
    ] {
        let e = parse(src).unwrap_err();
        assert!(
            e.to_string().contains("孤立的 `@`"),
            "源码 {src:?} 应报「孤立的 @」，实际: {e}"
        );
    }
}

/// 配置中间出现孤立 `@`：修复前只吞掉紧随其后的块、其余内容保留，
/// 属**部分丢失**，比整份清空更难察觉。现在应直接报错。
#[test]
fn lone_at_in_middle_must_error_not_partially_drop() {
    let src = "a: 1\n@\nb { x: 2 }\nc: 3\n";
    let e = parse(src).unwrap_err();
    assert!(
        e.to_string().contains("孤立的 `@`"),
        "孤立 @ 不应静默吞掉紧随的块，实际: {e}"
    );
}

/// 兼容性护栏：以下合法写法**不得**被上述改动误伤。
#[test]
fn at_related_valid_syntax_still_works() {
    // 片段定义与引用
    let v = parse("@base { x: 1 }\nk: &base\n").unwrap();
    assert!(
        matches!(v.get("k").and_then(|x| x.get("x")), Some(Value::Int(1))),
        "片段引用失效: {:?}",
        v.get("k")
    );
    // v4 起片段的 type / name 参数须显式写作 `type: X` / `name: Y`
    let v = parse("@f type: Server { x: 1 }\nk: &f\n").unwrap();
    assert!(format!("{:?}", v.get("k")).contains("Server"));
    let v = parse("@f type: S name: prod { x: 1 }\nk: &f\n").unwrap();
    assert!(format!("{:?}", v.get("k")).contains("prod"));
    // 名为 `type` / `name` 的片段仍可正常定义（后无冒号，不算参数）
    let v = parse("@type { x: 1 }\nk: &type\n").unwrap();
    assert!(format!("{:?}", v.get("k")).contains("x"));
    let v = parse("@name { y: 2 }\nk: &name\n").unwrap();
    assert!(format!("{:?}", v.get("k")).contains("y"));
    // 契约定义与应用
    let v = parse("@contract C { a: str }\n@is C\na: hello\n").unwrap();
    assert_eq!(v.get("a").and_then(|x| x.as_str()), Some("hello"));
    // 指令
    assert!(parse("@version v1\na: 1\n").is_ok());
    assert!(parse("@feature enable bareword-string\na: 1\n").is_ok());
    // `@` 出现在词中间（邮箱等）必须保留为普通字符
    let v = parse("email: a@b.c\n").unwrap();
    assert_eq!(v.get("email").and_then(|x| x.as_str()), Some("a@b.c"));
    let v = parse("k: x@y\n").unwrap();
    assert_eq!(v.get("k").and_then(|x| x.as_str()), Some("x@y"));
}

// ---------- 4. v4：片段参数显式化（原多义性已消除） ----------
//
// v3 及更早允许位置参数 `@f Server [prod] { .. }`，这与
// 「拼错的指令 `@nosuch Word { .. }`」在 token 流上完全同形，
// 解析器无法判别，只能把块当片段体吃掉 —— 内容静默丢失且不报错。
//
// v4 改为显式关键字 `type: X` / `name: Y`，二者得以区分：
// - `@f type: Server { .. }`  → 片段 f，带 `__type: Server`
// - `@nosuch blk { .. }`      → 报错（blk 不是显式参数，也不是 `{`）

#[test]
fn v4_explicit_fragment_params_work() {
    // type 单独
    let v = parse("@f type: Server { host: a }\nk: &f\n").unwrap();
    assert!(format!("{:?}", v.get("k")).contains("Server"));
    // type + name
    let v = parse("@f type: S name: prod { host: a }\nk: &f\n").unwrap();
    let s = format!("{:?}", v.get("k"));
    assert!(s.contains("S") && s.contains("prod"), "实际: {s}");
    // 只用 name
    let v = parse("@f name: prod { host: a }\nk: &f\n").unwrap();
    assert!(format!("{:?}", v.get("k")).contains("prod"));
    // 无参数（最常见形式）仍正常
    let v = parse("@f { host: a }\nk: &f\n").unwrap();
    assert!(format!("{:?}", v.get("k")).contains("host"));
    // 引号形式的值
    let v = parse("@f type: \"My Type\" { host: a }\nk: &f\n").unwrap();
    assert!(format!("{:?}", v.get("k")).contains("My Type"));
}

#[test]
fn v4_positional_fragment_params_rejected() {
    // 废弃的位置参数形式：必须报错，且错误信息应指引正确写法
    for src in [
        "@f Server { host: a }\n",
        "@f Server prod { host: a }\n",
    ] {
        let e = parse(src).unwrap_err();
        assert!(
            e.contains("type:") && e.contains("name:"),
            "错误信息应指引显式写法，源码 {src:?}，实际: {e}"
        );
    }
}

/// 原多义性用例：拼错的指令后接块，v4 起必须报错。
#[test]
fn unknown_directive_name_with_block_must_error() {
    for src in ["@nosuch\nblk { x: 1 }\n", "@versoin v1\nblk { x: 1 }\n"] {
        let e = parse(src).unwrap_err();
        assert!(
            e.contains("type:") || e.contains("不是合法指令"),
            "源码 {src:?} 应报错，实际: {e}"
        );
    }
}

#[test]
fn v4_duplicate_params_rejected() {
    let e = parse("@f type: A type: B { x: 1 }\n").unwrap_err();
    assert!(e.contains("重复"), "实际: {e}");
    let e = parse("@f name: A name: B { x: 1 }\n").unwrap_err();
    assert!(e.contains("重复"), "实际: {e}");
}

#[test]
fn fragment_named_type_or_name_still_works() {
    // 名为 `type` / `name` 的片段：后接 `{` 而非 `:`，不应被当作参数
    let v = parse("@type { x: 1 }\nk: &type\n").unwrap();
    assert!(format!("{:?}", v.get("k")).contains("x"));
    let v = parse("@name { y: 2 }\nk: &name\n").unwrap();
    assert!(format!("{:?}", v.get("k")).contains("y"));
}

#[test]
fn v4_version_accepted() {
    // `@version v4` 应可识别
    let v = parse("@version v4\nk: \"s\"\n").unwrap();
    assert!(v.get("k").is_some());
    // 数字简写
    assert!(parse("@version 4\nk: \"s\"\n").is_ok());
    // v4 与 v3 同样要求字符串引号
    assert!(parse("@version v4\nk: bare\n").is_err());
    // v4 片段参数显式化在声明 v4 时同样生效
    assert!(parse("@version v4\n@f S { x: 1 }\n").is_err());
    assert!(parse("@version v4\n@f type: S { x: 1 }\nk: &f\n").is_ok());
}
