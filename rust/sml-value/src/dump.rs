//! SML 文本序列化（`Value` → SML）。
//!
//! 与 `Value` 同 crate 的原因见 [`crate`] 文档：`Display` impl 需要它。
//! 本模块整体由 feature `sml` 门控，只需解析的场景不参与编译。
//!
//! # 三个写入口 / Three emit entries
//!
//! | 函数 | 键序 | 深度超限时 |
//! |---|---|---|
//! | [`to_sml`] / `Display for Value` | 按值的 `Map` 态 | 不失败，写 `/* …深度超限… */ null`（块是 `… {}`） |
//! | [`to_sml_canonical`] | 递归按键 UTF-8 字节序 | 同 `to_sml`（静默写占位） |
//! | [`to_sml_checked`] | 按值的 `Map` 态 | 返回 `E-LIMIT-004` |
//!
//! 键序由值自身的 `Map::Unordered` / `Map::Ordered` 决定，与编译开关无关；
//! `to_sml_canonical` 的「同值同字节」用于哈希 / 签名 / 去重 / 二进制，与保序是两个独立开关（§6.2）。
//! ｜ Key order is a property of the value, not of the build.
//!
//! ## 超深静默降级 / Over-depth degradation
//!
//! 上限 [`crate::MAX_VALUE_DEPTH`]（128，根算第 0 层），守卫在 `dump_value` /
//! `dump_element` / `dump_block` / `dump_inline` 各写一遍。超限节点被换成占位文本，
//! 看着合法、回读却是 `null`，**且不报任何错**。`impl fmt::Display for Value` 就是
//! `to_sml`，`println!("{v}")` 同样有此风险；CLI 与 C-ABI 的 `sml_dump` 走
//! [`to_sml_checked`] 要错误语义。
//! ｜ Over-deep nodes are replaced silently; use `to_sml_checked` when failure must be loud.
//!
//! ## `$env.` 无法无损往返 / `$env.` cannot round-trip
//!
//! 即使加引号，再解析仍会被当作环境变量内联展开（词法阶段即解码 `\u{24}`，与未转义形式
//! 不可区分）。例如 `Value::Str("$env.PATH")` 写出为 `"$env.PATH"`，读回变成 `PATH` 的实际值。
//! 解析不可信文档时应显式禁用 `env` 特性（`sml_parse::parse_with_features` +
//! `FeatureSet::without(Feature::Env)`）；此处是代码字体不是链接 —— `sml-value` 不能依赖
//! `sml-parse`，否则成环。
//! ｜ Values starting with `$env.` cannot round-trip; disable `env` for untrusted input.
//!
//! ## 元数据块 / Metadata blocks
//!
//! 含 `__type` / `__name` 的块原样输出所有键，保证元数据可完整往返。
//! ｜ Blocks with `__type` / `__name` keep all keys so metadata round-trips.

use crate::{Map, MAX_VALUE_DEPTH, Value};

// Text-atom rules (bare words / quoting / escaping, float spelling) now live in `text_rules`:
// there are three consumers (`to_sml`, `ser_text`, `canonical`), and the latter two cannot
// reach inside this module (gated behind the `sml` feature), so each would have copied the
// rules — the one road that always drifts. Same names are re-exported here, so neither
// `ser_text.rs` nor the call sites below need a single edit.
// 「文本原子」规则已搬到 `text_rules`（三个消费者，复制一份必漂移）；此处保留同名
// re-export，既有调用一行不改。
//
// 注：确定性字节**不**靠 `to_sml` 保序实现 —— 那是 `to_sml_canonical`（sort_keys）
// 的职责，两者是独立开关（§6.2）。保序买到的是「键序 == 源序」，不是同值同字节。
pub(crate) use crate::text_rules::{format_float, quote_if_needed};

/// 容器是否「扁平」：**直接子项全是标量**（不含对象 / 数组）。
///
/// 这是 [`dump_element`] 决定「一行写完」还是「展开多行」的唯一判据：
/// `{ type: home }`、`{ offset: "0x00" size: "0x400" }` 扁平 → 紧凑一行；
/// `{ fields: { field: [ … ] } }` 里嵌了容器 → 展开多行。
///
/// **只看一层，不递归**。递归版（「子孙全是标量」）是个恒真判据 ——
/// 任何对象的子孙最终都会落到标量，于是所有东西都被判成扁平、排版分毫不改。
/// 写这段时真踩了这个坑（编译与测试全绿，只是完全没生效）。
/// 非递归顺带去掉了一处递归，程序构造的超深 `Value` 也撑不爆栈。
#[cfg(feature = "sml")]
fn is_flat(v: &Value) -> bool {
    fn scalar(v: &Value) -> bool {
        !matches!(v, Value::Object(_) | Value::Array(_))
    }
    match v {
        Value::Object(m) => m.values().all(scalar),
        Value::Array(a) => a.iter().all(scalar),
        _ => true,
    }
}

/// 值是否要紧跟在 `键:` 之后、**同一行**开始写。
///
/// 非空对象由 [`dump_block`] 先写换行再写 `{`，此时若一律在键后补空格，
/// 行尾就会留下一个看不见的空格 —— 既有输出里每处「键后接块」都这样
/// （实测 SVD 产物 3 行行尾带空白）。新版面会成倍放大这个问题，故按值类型决定。
#[cfg(feature = "sml")]
fn starts_inline(v: &Value) -> bool {
    !matches!(v, Value::Object(m) if !m.is_empty())
}

/// 写对象体：**不含**开头的 `{`（由调用方写），负责逐键换行与收尾 `}`。
///
/// 抽出来是为了让「块里的对象」（缩进后跟 `\n{`，见 [`dump_block`]）与
/// 「数组元素里的对象」（`{` 跟在同一行）共用同一套键渲染。
#[cfg(feature = "sml")]
fn dump_object_body(m: &Map, indent: usize, depth: usize, out: &mut String) {
    for (k, val) in m {
        out.push_str(&format!(
            "\n{}{}:",
            "  ".repeat(indent + 1),
            quote_if_needed(k)
        ));
        if starts_inline(val) {
            out.push(' ');
        }
        dump_value(val, indent + 1, depth + 1, out);
    }
    out.push_str(&format!("\n{}}}", "  ".repeat(indent)));
}

/// 输出一个块。含 `__type` / `__name` 的块也按普通块原样输出所有键，
/// 保证元数据（枚举带数据变体的 `__type` 标记等）可完整往返。
/// SML 的裸块 `type [name] { ... }` 解析后正是 `__type` / `__name` 键。
#[cfg(feature = "sml")]
fn dump_block(m: &Map, indent: usize, depth: usize, out: &mut String) {
    if depth > MAX_VALUE_DEPTH {
        out.push_str("/* …深度超限… */ {}");
        return;
    }
    if m.is_empty() {
        out.push_str("{}");
        return;
    }
    out.push_str(&format!("\n{}{{", "  ".repeat(indent)));
    dump_object_body(m, indent, depth, out);
}

/// 写一个「元素」：扁平的走单行（[`dump_inline`]），含结构的展开成多行。
///
/// 调用方负责**已**写好本元素开头的换行与缩进（`indent` 即该缩进级别），
/// 因此这里不在开头补缩进，展开出来的续行由各自递归负责对齐。
#[cfg(feature = "sml")]
fn dump_element(v: &Value, indent: usize, depth: usize, out: &mut String) {
    // 深度守卫必须在最前：下面的 `Value::Array` 分支会**直接递归自己**
    // （`dump_inline` 那条路自带守卫，这条没有），少了它，深层嵌套数组
    // （如 5 万层 `[[[[…]]]]`）会把栈写爆 —— 实测触发 0xC00000FD。
    if depth > MAX_VALUE_DEPTH {
        out.push_str("/* …深度超限… */ null");
        return;
    }
    if is_flat(v) {
        out.push_str(&dump_inline(v, depth));
        return;
    }
    match v {
        Value::Object(m) => {
            out.push('{');
            dump_object_body(m, indent, depth, out);
        }
        Value::Array(a) => {
            out.push('[');
            for e in a {
                out.push('\n');
                out.push_str(&"  ".repeat(indent + 1));
                dump_element(e, indent + 1, depth + 1, out);
            }
            out.push_str(&format!("\n{}]", "  ".repeat(indent)));
        }
        // `is_flat` 为假只可能是容器；标量一律扁平。真到这儿也退化成单行，
        // 不 panic（本模块对外的承诺是「总能写出一份文本」）。
        _ => out.push_str(&dump_inline(v, depth)),
    }
}

fn dump_value(v: &Value, indent: usize, depth: usize, out: &mut String) {
    if depth > MAX_VALUE_DEPTH {
        out.push_str("/* …深度超限… */ null");
        return;
    }
    let pad = "  ".repeat(indent);
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Float(f, raw) => out.push_str(&format_float(*f, raw.as_deref())),
        Value::Str(s) => out.push_str(&quote_if_needed(s)),
        Value::Array(a) => {
            if a.is_empty() {
                out.push_str("[]");
            } else {
                out.push('[');
                for e in a {
                    out.push('\n');
                    out.push_str(&"  ".repeat(indent + 1));
                    dump_element(e, indent + 1, depth + 1, out);
                }
                out.push_str(&format!("\n{}]", pad));
            }
        }
        Value::Object(m) => dump_block(m, indent, depth + 1, out),
    }
}

#[cfg(feature = "sml")]
fn dump_scalar(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f, raw) => format_float(*f, raw.as_deref()),
        Value::Str(s) => quote_if_needed(s).into_owned(),
        _ => "".into(),
    }
}

#[cfg(feature = "sml")]
fn dump_inline(v: &Value, depth: usize) -> String {
    if depth > MAX_VALUE_DEPTH {
        return "/* …深度超限… */ null".to_string();
    }
    match v {
        Value::Object(m) => {
            // 含 __type/__name 的块原样输出所有键，保证元数据可往返；
            // 嵌套对象的键也必须 quote_if_needed（如 "x y"），否则无法解析回去
            let parts: Vec<String> = m
                .iter()
                .map(|(k, val)| format!("{}: {}", quote_if_needed(k), dump_inline(val, depth + 1)))
                .collect();
            format!("{{ {} }}", parts.join(", "))
        }
        Value::Array(a) => {
            let parts: Vec<String> = a.iter().map(|e| dump_inline(e, depth + 1)).collect();
            format!("[ {} ]", parts.join(", "))
        }
        other => dump_scalar(other),
    }
}

/// 序列化回 SML 文本：键序按值的 `Map` 态，超深节点静默写占位文本。
/// ｜ Serialize back to SML text; over-deep nodes become placeholders.
///
/// 要错误语义用 [`to_sml_checked`]，要同值同字节用 [`to_sml_canonical`]；
/// 限制与理由见模块文档。 ｜ See the module docs for limits and rationale.
///
/// ```
/// use sml_value::{to_sml, to_sml_checked, Value, MAX_VALUE_DEPTH};
///
/// assert_eq!(to_sml(&Value::Int(1)), "1");
///
/// // 深度超限：本函数不报错，而是写下「看着合法」的占位内容
/// let mut v = Value::Int(1);
/// for _ in 0..=MAX_VALUE_DEPTH {
///     v = Value::Array(vec![v]);
/// }
/// assert!(to_sml(&v).contains("深度超限"));
/// assert!(to_sml_checked(&v).is_err());
/// ```
#[cfg(feature = "sml")]
pub fn to_sml(v: &Value) -> String {
    // 预分配输出：扁平对象每行约 "k1234: value-1234"（取 32 B 的保守值）。
    // 从 `String::new()` 起要反复扩容并搬移整段文本；估算只影响容量，不影响一个字节。
    // ｜ Capacity hint only; nested documents still grow naturally, bytes are unchanged.
    let guess = match v {
        Value::Object(m) => m.len().saturating_mul(32).saturating_add(64),
        _ => 64,
    };
    let mut out = String::with_capacity(guess);
    if let Value::Object(m) = v {
        if m.contains_key("__type") {
            dump_block(m, 0, 0, &mut out);
        } else {
            for (k, val) in m {
                // 同 [`starts_inline`]：块值自己会换行，别在行尾留空格
                // 直接 push 而不是 `format!("{}:", ...)`：每键少一次临时 String 分配。
                out.push_str(&quote_if_needed(k));
                out.push(':');
                if starts_inline(val) {
                    out.push(' ');
                }
                dump_value(val, 0, 0, &mut out);
                out.push('\n');
            }
        }
    } else {
        // 顶层非对象：与数组元素同一套规则（扁平单行 / 含结构展开）
        dump_element(v, 0, 0, &mut out);
    }
    out
}

/// canonical 序列化：**递归**按键字节序输出，同值必同字节。
/// ｜ Recursive key byte-order output; equal values always give identical bytes.
///
/// 键序是值自身的事（`Map::Unordered` / `Map::Ordered`），与编译开关无关；
/// 本函数只决定写的时候按不按字节序输出：
/// - `Map::Unordered` 态 + 本函数 == 该态的 `to_sml`（`BTreeMap` 本来就有序）；
/// - `Map::Ordered` 态 + 本函数 == 同值同字节（即使源序不同）。
///
/// ｜ Key order is a property of the value, not of the build.
///
/// # 示例
/// ```
/// use sml_value::{Map, Value, to_sml_canonical};
/// let mut a = Map::new();
/// a.insert("z".to_string(), Value::Int(1));
/// a.insert("a".to_string(), Value::Int(2));
/// let mut b = Map::new();
/// b.insert("a".to_string(), Value::Int(2));
/// b.insert("z".to_string(), Value::Int(1));
/// assert_eq!(to_sml_canonical(&Value::Object(a)), to_sml_canonical(&Value::Object(b)));
/// ```
pub fn to_sml_canonical(v: &Value) -> String {
    to_sml(&v.sorted_by_key())
}

/// 值树的最大嵌套深度。**迭代实现（显式栈）**，故意不递归。
///
/// 为什么不能递归：这个函数的用途正是「在递归写之前判断会不会太深」——
/// 5 万层的值用递归实现会**自己**先把栈写爆（Rust 栈溢出是 abort，接不住），
/// 与 `dump_element` 注释里记的那次 `0xC00000FD` 是同一类。
///
/// **口径：根 = 0**（与解析器 `Parser::depth`、各 emit 后端的 `depth` 形参一致）。
/// 改前这里把根算作 1，于是「解析允许 128 层、序列化却说 129 超限」——
/// 恰好 128 层的文档变成「能解析、`--to json` 能出、`to_sml` 却拒」（TODO 雷 9）。
#[cfg(feature = "sml")]
fn max_value_depth(v: &Value) -> usize {
    let mut stack: Vec<(&Value, usize)> = vec![(v, 0)];
    let mut max = 0usize;
    while let Some((cur, d)) = stack.pop() {
        if d > max {
            max = d;
        }
        match cur {
            Value::Object(m) => {
                for val in m.values() {
                    stack.push((val, d + 1));
                }
            }
            Value::Array(a) => {
                for val in a {
                    stack.push((val, d + 1));
                }
            }
            _ => {}
        }
    }
    max
}

/// Same as [`to_sml`], but **returns an error when the depth limit is exceeded** instead of silently writing placeholder text (W16). ｜ 与 [`to_sml`] 相同，但**深度超限返回错误**而不是静默写占位文本（W16）。
///
/// 改前：超过 [`crate::MAX_VALUE_DEPTH`] 时各处守卫会写下
/// `/* …深度超限… */ null` 之类的占位内容 —— 产物看着像合法 SML、回读却是 `null`，
/// **数据被悄悄改掉且不报错**。现在把这条静默降级变成显式失败。
///
/// 为什么错误类型是 `String` 而不是 `sml_codes::SmlError`：本 crate 刻意**零依赖**
/// （连 `sml-codes` 都不引，见 Cargo.toml 的说明），而 serde 桥同理只能带消息。
/// 故这里按仓库对这类层的既有约定，把码写成 **`文案 [E-LIMIT-004]`** 后缀；
/// 门面 crate 的 `sml::to_sml_checked` 会把它转成结构化的 `SmlError`。
///
/// 注：`to_sml` 本身**保持不失败**（超限仍写占位文本）—— 它被 40+ 处以
/// 「总能给你一份文本」的契约使用（含 `Display`），改签名是破坏性变更；
/// 需要错误语义的调用方（CLI、C-ABI）走本函数。
#[cfg(feature = "sml")]
pub fn to_sml_checked(v: &Value) -> Result<String, String> {
    let d = max_value_depth(v);
    if d > MAX_VALUE_DEPTH {
        return Err(format!(
            "序列化深度 {d} 超过上限 {MAX_VALUE_DEPTH}，拒绝输出占位文本 [E-LIMIT-004]"
        ));
    }
    Ok(to_sml(v))
}

/// `to_sml` 排版规则（扁平才留一行 / 含结构展开）与深度守卫的回归测试。
///
/// 这两件事都**只靠测试通过是测不出来的**：判据写错成恒真时，226 个测试照样全绿、
/// 产物却一个字节没变。所以这里直接对**输出的行**下断言（行宽、行内容）。
#[cfg(all(test, feature = "sml"))]
mod tests {
    use super::*;

    fn obj(pairs: Vec<(&str, Value)>) -> Value {
        let mut m = Map::new();
        for (k, v) in pairs {
            m.insert(k.to_string(), v);
        }
        Value::Object(m)
    }

    fn s(x: &str) -> Value {
        Value::Str(x.to_string())
    }

    #[test]
    fn flat_container_stays_on_one_line() {
        let v = obj(vec![(
            "phoneNumbers",
            Value::Array(vec![
                obj(vec![("type", s("home"))]),
                obj(vec![("type", s("office"))]),
            ]),
        )]);
        let out = to_sml(&v);
        assert!(out.contains("{ type: home }"), "扁平容器应一行写完：{out}");
        assert!(out.contains("{ type: office }"), "{out}");
        assert!(out.lines().all(|l| l.len() < 40), "不该出现换行展开：{out}");
    }

    /// 雷5 B 组（Rust 侧）：整数值浮点必须保留 `.0`，否则回读会退化成 Int。
    #[test]
    fn integral_float_keeps_dot_zero() {
        assert_eq!(format_float(1.0, None), "1.0");
        assert_eq!(format_float(1.0, Some("1.0")), "1.0");
        assert_eq!(format_float(1e10, None), "10000000000.0");
        assert_eq!(format_float(-2.0, None), "-2.0");
    }

    #[test]
    fn structured_element_expands_across_lines() {
        let reg = obj(vec![
            ("name", s("CTLR")),
            (
                "registers",
                obj(vec![(
                    "register",
                    Value::Array(vec![obj(vec![("name", s("CTLR"))])]),
                )]),
            ),
        ]);
        let v = obj(vec![("peripheral", Value::Array(vec![reg]))]);
        let out = to_sml(&v);
        // 含容器的元素展开：`registers` 另起一行（注意键有缩进，故 trim 后再比）
        assert!(
            out.lines().any(|l| l.trim() == "registers:"),
            "含容器的键应换行：{out}"
        );
        // 但最内层仍是扁平的 → 保持一行
        assert!(
            out.lines().any(|l| l.trim() == "{ name: CTLR }"),
            "扁平叶子应仍在一行：{out}"
        );
        assert!(
            out.lines().all(|l| l.len() < 60),
            "展开后不该再有超长行：{out}"
        );
        // 键后接块时不该在行尾留空格（既有输出里每处都留了一个）
        assert!(
            out.lines().all(|l| l == l.trim_end()),
            "不该出现行尾空白：{out}"
        );
    }

    /// 深层嵌套数组必须被深度守卫截住，而不是把栈写爆。
    ///
    /// 实测过：`dump_element` 的数组分支会直接递归自己，漏掉守卫时这一例触发
    /// `0xC00000FD`（STATUS_STACK_OVERFLOW）。
    #[test]
    fn deep_nested_array_is_guarded_not_overflowing() {
        let mut v = Value::Array(vec![]);
        for _ in 0..5000 {
            v = Value::Array(vec![v]);
        }
        let out = to_sml(&v);
        assert!(out.contains("深度超限"), "应被深度守卫截住：{}", &out[..80.min(out.len())]);
    }

    /// W16：`to_sml_checked` 把「深度超限」变成**可报的错误**（`E-LIMIT-004`），
    /// 而 `to_sml` 仍按旧契约写占位文本（上面那个用例钉着）—— 两者刻意共存：
    /// `to_sml` 有 40+ 处调用方依赖「总能给你一份文本」，改签名是破坏性变更。
    ///
    /// 本 crate 零依赖（不引 `sml-codes`），故码以 `[E-LIMIT-004]` 后缀出现在文案里。
    #[test]
    fn checked_variant_reports_depth_instead_of_placeholder() {
        let mut v = Value::Array(vec![]);
        for _ in 0..5000 {
            v = Value::Array(vec![v]);
        }
        let e = to_sml_checked(&v).expect_err("超深值应报错，而不是给占位文本");
        assert!(e.contains("E-LIMIT-004"), "实得：{e}");
        assert!(e.contains("深度"), "实得：{e}");
        // 正对照：不超限的值照常序列化，且与 `to_sml` 的输出**逐字节一致**
        // （checked 只是多一道预检，不改变排版）
        let ok = obj(vec![("a", Value::Int(1)), ("b", Value::Str("x".into()))]);
        assert_eq!(to_sml_checked(&ok).unwrap(), to_sml(&ok));
    }

    #[test]
    fn empty_containers_and_scalars_unchanged() {
        let v = obj(vec![
            ("a", Value::Array(vec![])),
            ("b", obj(vec![])),
            ("c", Value::Int(1)),
            ("d", Value::Bool(true)),
        ]);
        assert_eq!(to_sml(&v), "a: []\nb: {}\nc: 1\nd: true\n");
    }
}
