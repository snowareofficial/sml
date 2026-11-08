// SPDX-License-Identifier: MulanPSL-2.0
//! Path syntax shared by **readers and writers** ｜ **读与写共用**的路径语法。
//!
//! S6（featag 落点）要用同一套语法「写」值，故解析器独立成模块，读只是它的第一个用户。
//! S6 uses the same grammar to *write* values; reading is merely its first caller.
//!
//! # 语法（唯一权威，改这里就是改语法）
//!
//! ```text
//! path  := seg ( "." seg )*        -- 至少一段
//! seg   := key index*  |  index+   -- 键后跟任意个下标；也可以只有下标
//! key   := 1*( 除 "." "[" "]" 与空白/控制字符之外的任意字符 )
//! index := "[" 1*DIGIT "]"         -- usize，0 起；不做负号、不做区间
//! ```
//!
//! | 写法 | 含义 |
//! |---|---|
//! | `a.b.c` | 块套块取键 |
//! | `a[0]` | 键 `a` 后取数组第 0 个元素 |
//! | `a[0].b` | 先取元素再取键（可任意交替） |
//! | `[0][1]` | 只写下标：对当前值连续下钻 |
//! | `a.[0]` | 同上，键为空时等价于「当前值」——为兼容旧 `Value::get` 而保留 |
//!
//! # 为什么不用 `Value::get`
//!
//! [`Value::get`] 是**旧的可访问器**，语义被历史钉住了两条：
//!
//! 1. 失败一律返回 `None` —— 分不清「键不存在」「越界」「类型不符」；
//! 2. **字面键优先**：先拿整段原文当键查一次，故键名本身含 `[` 的文档（如键就叫 `a[0]`）
//!    仍能取到，不会因为新增下标语法而从 `Some` 变 `None`。
//!
//! 这两条对「写落点」都是**有害**的（写下去必须知道究竟失败在哪一步，也不能因为键名
//! 长得像路径就把数据写到别处），所以这里另立一条干净的路：
//! [`Path::parse`] + [`Value::get_path`]，失败带**原因**与**段号**。
//! 旧 `Value::get` 原样保留，不动它的一根毫毛（有回归钉子 `bracket_in_key_name_is_not_rewritten`）。
//!
//! # 示例
//!
//! ```
//! use sml_value::{Path, Value};
//!
//! let doc = Value::Object(
//!     [(
//!         "a".to_string(),
//!         Value::Object(
//!             [(
//!                 "b".to_string(),
//!                 Value::Array(vec![Value::Int(10), Value::Int(20)]),
//!             )]
//!             .into(),
//!         ),
//!     )]
//!     .into(),
//! );
//!
//! let p = Path::parse("a.b[1]").unwrap();
//! assert_eq!(doc.get_path(&p).map(|v| v.as_i64()), Ok(Some(20)));
//!
//! // 越界不再是一句 `None`：说清是哪一段、越了多少
//! let bad = Path::parse("a.b[9]").unwrap();
//! let err = doc.get_path(&bad).unwrap_err();
//! assert!(format!("{err}").contains("越界"), "{err}");
//! # use sml_value::Map;
//! ```

use std::fmt;
use std::str::FromStr;

use crate::value::Value;

/// One step of a path ｜ 路径的一段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seg {
    /// Object key ｜ 对象键。
    Key(String),
    /// Array index (`[n]`, 0-based) ｜ 数组下标（`[n]`，0 起）。
    Index(usize),
}

/// A parsed path ｜ 已解析的路径。
///
/// 只能由 [`Path::parse`]（或 [`Path::new`]）得到 —— 不存在「语法非法但仍是 `Path`」的中间态。
/// Only obtainable via [`Path::parse`] / [`Path::new`]: no "parsed but invalid" state exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    segs: Vec<Seg>,
}

/// Why a path failed to parse, or failed to resolve ｜ 路径的解析 / 查找失败原因。
///
/// 语法错误由 [`Path::parse`] 产生，查找错误由 [`Value::get_path`] 产生；
/// 用 [`PathError::is_syntax`] 区分两阶段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    // ---------------- 语法（parse 阶段） ----------------
    /// 空路径 ｜ the path is empty.
    Empty,
    /// 段为空：`.a` / `a..b` / `a.` ｜ an empty segment.
    EmptyKey { at: usize },
    /// 下标不是非负整数 ｜ the index is not a non-negative integer.
    BadIndex { text: String, at: usize },
    /// `[` 未闭合 ｜ the `[` is never closed.
    UnclosedIndex { at: usize },
    /// 非法字符（空白 / 控制字符 / 多余的 `]` 等）｜ an illegal character.
    Stray { ch: char, at: usize },

    // ---------------- 查找（get_path 阶段） ----------------
    /// 键不存在 ｜ the key does not exist.
    NotFound { at: usize, key: String },
    /// 数组越界 ｜ the index is out of range.
    OutOfRange { at: usize, index: usize, len: usize },
    /// 该段是键，但当前值不是块 ｜ a key segment applied to a non-object.
    NotAnObject { at: usize, actual: &'static str },
    /// 该段是下标，但当前值不是数组 ｜ an index segment applied to a non-array.
    NotAnArray { at: usize, actual: &'static str },
}

impl PathError {
    /// 是否发生在**解析**阶段（`false` = 查找阶段）｜ whether this is a syntax error.
    pub fn is_syntax(&self) -> bool {
        matches!(
            self,
            PathError::Empty
                | PathError::EmptyKey { .. }
                | PathError::BadIndex { .. }
                | PathError::UnclosedIndex { .. }
                | PathError::Stray { .. }
        )
    }
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathError::Empty => write!(f, "路径为空"),
            PathError::EmptyKey { at } => write!(
                f,
                "路径第 {at} 个字符处的段为空（连续的 `.`、或首尾多余的 `.`）"
            ),
            PathError::BadIndex { text, at } => write!(
                f,
                "数组下标须为非负整数：`[{text}]`（第 {at} 个字符处）"
            ),
            PathError::UnclosedIndex { at } => {
                write!(f, "数组下标未闭合：第 {at} 个字符处的 `[` 缺少 `]`")
            }
            PathError::Stray { ch, at } => write!(
                f,
                "路径第 {at} 个字符处出现非法字符 `{ch}`（键名不得含空白、控制字符或 `.` `[` `]`）"
            ),
            PathError::NotFound { at, key } => {
                write!(f, "路径第 {at} 段 `{key}`：键不存在")
            }
            PathError::OutOfRange { at, index, len } => write!(
                f,
                "路径第 {at} 段 `[{index}]`：越界（该数组只有 {len} 个元素）"
            ),
            PathError::NotAnObject { at, actual } => {
                write!(f, "路径第 {at} 段是键名，但当前值是{actual}，不是块")
            }
            PathError::NotAnArray { at, actual } => {
                write!(f, "路径第 {at} 段是下标，但当前值是{actual}，不是数组")
            }
        }
    }
}

impl std::error::Error for PathError {}

/// 简短的类型名（用于错误文案；**不带值内容**，避免把数据漏进日志）。
fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "布尔值",
        Value::Int(_) => "整数",
        Value::Float(..) => "浮点数",
        Value::Str(_) => "字符串",
        Value::Array(_) => "数组",
        Value::Object(_) => "块",
    }
}

impl Path {
    /// 直接由段构造（写落点时往往是程序拼的，不必再过一遍字符串）｜ build from segments.
    ///
    /// 空 `segs` 是**合法**的「当前值」路径（解析为它只会返回原值），
    /// 与 [`Path::parse`] 拒绝空串不同 —— 后者是「用户写了空路径」，前者是「无需下钻」。
    pub fn new(segs: Vec<Seg>) -> Path {
        Path { segs }
    }

    /// Parse a path ｜ 解析路径。
    ///
    /// # Errors
    /// 见 [`PathError`] 的语法分支：空路径、空段、下标非法、下标未闭合、非法字符。
    pub fn parse(s: &str) -> Result<Path, PathError> {
        if s.is_empty() {
            return Err(PathError::Empty);
        }
        let ch: Vec<char> = s.chars().collect();
        let mut segs: Vec<Seg> = Vec::new();
        let mut i = 0usize;

        loop {
            // ---- 1) 键名：吃到 `.` / `[` 为止；`]`、空白、控制字符当场报错 ----
            let mut key = String::new();
            while i < ch.len() {
                let c = ch[i];
                match c {
                    '.' | '[' => break,
                    ']' => return Err(PathError::Stray { ch: c, at: i + 1 }),
                    c if c.is_whitespace() || c.is_control() => {
                        return Err(PathError::Stray { ch: c, at: i + 1 })
                    }
                    c => {
                        key.push(c);
                        i += 1;
                    }
                }
            }

            // ---- 2) 下标：`[` DIGIT+ `]`，可连续多个 ----
            let mut idxs: Vec<usize> = Vec::new();
            while i < ch.len() && ch[i] == '[' {
                let open = i;
                i += 1;
                let ds = i;
                while i < ch.len() && ch[i].is_ascii_digit() {
                    i += 1;
                }
                if i == ds {
                    // `[]` / `[x]` / `[-1]`：把到 `]` 为止的原文抄进文案，便于定位
                    let mut text = String::new();
                    let mut j = ds;
                    while j < ch.len() && ch[j] != ']' {
                        text.push(ch[j]);
                        j += 1;
                    }
                    return Err(PathError::BadIndex { text, at: ds + 1 });
                }
                if i >= ch.len() {
                    return Err(PathError::UnclosedIndex { at: open + 1 });
                }
                if ch[i] != ']' {
                    // `a[0.5]` / `a[0x1]`
                    let mut text = String::new();
                    let mut j = ds;
                    while j < ch.len() && ch[j] != ']' {
                        text.push(ch[j]);
                        j += 1;
                    }
                    return Err(PathError::BadIndex { text, at: ds + 1 });
                }
                let num: String = ch[ds..i].iter().collect();
                let n = num
                    .parse::<usize>()
                    .map_err(|_| PathError::BadIndex { text: num.clone(), at: ds + 1 })?;
                idxs.push(n);
                i += 1; // 越过 ']'
            }

            // ---- 3) 本段必须有内容（键或至少一个下标） ----
            if key.is_empty() && idxs.is_empty() {
                return Err(PathError::EmptyKey { at: i + 1 });
            }
            if !key.is_empty() {
                segs.push(Seg::Key(key));
            }
            for n in idxs {
                segs.push(Seg::Index(n));
            }

            // ---- 4) 段间分隔：`.` 或结尾 ----
            if i >= ch.len() {
                break;
            }
            if ch[i] != '.' {
                return Err(PathError::Stray { ch: ch[i], at: i + 1 });
            }
            i += 1;
        }

        Ok(Path { segs })
    }

    /// The segments, in order ｜ 按顺序的段。
    pub fn segments(&self) -> &[Seg] {
        &self.segs
    }

    /// 段数（`Path::new(vec![])` 为 0）｜ number of segments.
    pub fn len(&self) -> usize {
        self.segs.len()
    }

    /// 是否为空路径（只看「当前值」）｜ whether the path has no segments.
    pub fn is_empty(&self) -> bool {
        self.segs.is_empty()
    }

    /// 最后一段（写落点时用来决定「在父里放什么名字」）｜ the last segment.
    pub fn last(&self) -> Option<&Seg> {
        self.segs.last()
    }
}

impl fmt::Display for Path {
    /// 规范化回显：`a.b[0].c`（下标紧跟其键，不额外加点）。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for seg in &self.segs {
            match seg {
                Seg::Key(k) => {
                    if !first {
                        f.write_str(".")?;
                    }
                    f.write_str(k)?;
                }
                Seg::Index(n) => write!(f, "[{n}]")?,
            }
            first = false;
        }
        Ok(())
    }
}

impl FromStr for Path {
    type Err = PathError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Path::parse(s)
    }
}

impl Value {
    /// Resolve a parsed path, reporting **why** it failed ｜ 按路径取值，失败带原因。
    ///
    /// 与 [`Value::get`] 不同：不吃「字面键优先」，失败返回 [`PathError`]（带 1 起段号）而非 `None`。
    /// ｜ Unlike `Value::get`, failures carry a reason and a 1-based segment index.
    ///
    /// ```
    /// use sml_value::{Path, Value};
    /// let v = Value::Array(vec![Value::Int(1)]);
    /// let p = Path::parse("[3]").unwrap();
    /// assert_eq!(
    ///     v.get_path(&p).unwrap_err().to_string(),
    ///     "路径第 1 段 `[3]`：越界（该数组只有 1 个元素）"
    /// );
    /// ```
    pub fn get_path(&self, path: &Path) -> Result<&Value, PathError> {
        let mut cur = self;
        for (i, seg) in path.segments().iter().enumerate() {
            let at = i + 1;
            match seg {
                Seg::Key(k) => match cur {
                    Value::Object(m) => {
                        cur = m
                            .get(k)
                            .ok_or_else(|| PathError::NotFound { at, key: k.clone() })?;
                    }
                    other => {
                        return Err(PathError::NotAnObject { at, actual: type_name(other) });
                    }
                },
                Seg::Index(n) => match cur {
                    Value::Array(a) => {
                        cur = a.get(*n).ok_or(PathError::OutOfRange {
                            at,
                            index: *n,
                            len: a.len(),
                        })?;
                    }
                    other => {
                        return Err(PathError::NotAnArray { at, actual: type_name(other) });
                    }
                },
            }
        }
        Ok(cur)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Map;

    fn doc() -> Value {
        Value::Object(
            [
                (
                    "a".to_string(),
                    Value::Object(
                        [(
                            "b".to_string(),
                            Value::Array(vec![
                                Value::Object([("c".to_string(), Value::Int(10))].into()),
                                Value::Object([("c".to_string(), Value::Int(20))].into()),
                            ]),
                        )]
                        .into(),
                    ),
                ),
                ("s".to_string(), Value::Str("hi".into())),
            ]
            .into(),
        )
    }

    #[test]
    fn parses_all_documented_forms() {
        assert_eq!(
            Path::parse("a.b[0].c").unwrap().segments(),
            &[
                Seg::Key("a".into()),
                Seg::Key("b".into()),
                Seg::Index(0),
                Seg::Key("c".into())
            ]
        );
        // 连续下标
        assert_eq!(
            Path::parse("[0][1]").unwrap().segments(),
            &[Seg::Index(0), Seg::Index(1)]
        );
        // 空键 + 下标（兼容旧 `get` 的 `a.[0]`）
        assert_eq!(
            Path::parse("a.[0]").unwrap().segments(),
            &[Seg::Key("a".into()), Seg::Index(0)]
        );
        // 前导零与超大下标：前者照收，后者按溢出报错
        assert_eq!(Path::parse("a[007]").unwrap().segments()[1], Seg::Index(7));
        assert!(Path::parse("a[99999999999999999999999999]").is_err());
    }

    #[test]
    fn syntax_errors_are_specific() {
        assert_eq!(Path::parse("").unwrap_err(), PathError::Empty);
        assert_eq!(Path::parse(".a").unwrap_err(), PathError::EmptyKey { at: 1 });
        assert_eq!(Path::parse("a..b").unwrap_err(), PathError::EmptyKey { at: 3 });
        assert_eq!(Path::parse("a.").unwrap_err(), PathError::EmptyKey { at: 3 });
        assert_eq!(
            Path::parse("a[x]").unwrap_err(),
            PathError::BadIndex { text: "x".into(), at: 3 }
        );
        assert_eq!(
            Path::parse("a[-1]").unwrap_err(),
            PathError::BadIndex { text: "-1".into(), at: 3 }
        );
        assert_eq!(
            Path::parse("a[]").unwrap_err(),
            PathError::BadIndex { text: "".into(), at: 3 }
        );
        assert_eq!(
            Path::parse("a[0").unwrap_err(),
            PathError::UnclosedIndex { at: 2 }
        );
        // 空白 / 控制字符不是键名的一部分（S1b 起标识符层同样拒绝）
        assert!(matches!(
            Path::parse("a b").unwrap_err(),
            PathError::Stray { ch: ' ', at: 2 }
        ));
        // 多余的 `]` / 段后直接跟字符
        assert!(matches!(
            Path::parse("a[0]]").unwrap_err(),
            PathError::Stray { ch: ']', at: 5 }
        ));
        assert!(Path::parse("a[0]b").is_err());
        // 两阶段用 is_syntax 区分
        assert!(Path::parse("a..b").unwrap_err().is_syntax());
        assert!(!PathError::OutOfRange { at: 1, index: 9, len: 3 }.is_syntax());
    }

    #[test]
    fn resolves_dotted_and_indexed() {
        let d = doc();
        for (p, want) in [("a.b[0].c", 10), ("a.b[1].c", 20)] {
            let path = Path::parse(p).unwrap();
            assert_eq!(
                d.get_path(&path).map(|v| v.as_i64()),
                Ok(Some(want)),
                "{p}"
            );
        }
    }

    #[test]
    fn lookup_errors_carry_position_and_reason() {
        let d = doc();
        let e = |p: &str| d.get_path(&Path::parse(p).unwrap()).unwrap_err();

        assert_eq!(
            e("a.b[9].c"),
            PathError::OutOfRange { at: 3, index: 9, len: 2 }
        );
        assert_eq!(
            e("a.z"),
            PathError::NotFound { at: 2, key: "z".into() }
        );
        assert_eq!(
            e("s.x"),
            PathError::NotAnObject { at: 2, actual: "字符串" }
        );
        assert_eq!(
            e("a.b.c"),
            PathError::NotAnObject { at: 3, actual: "数组" }
        );
        assert_eq!(
            e("a.b[0][0]"),
            PathError::NotAnArray { at: 4, actual: "块" }
        );
        // 文案里不许出现数据内容（避免日志泄漏）
        assert!(!e("s.x").to_string().contains("hi"));
    }

    #[test]
    fn empty_segment_list_is_the_identity() {
        let d = doc();
        let p = Path::new(Vec::new());
        assert!(p.is_empty());
        assert_eq!(d.get_path(&p).unwrap(), &d);
        assert_eq!(p.to_string(), "");
    }

    #[test]
    fn display_round_trips() {
        for s in ["a.b[0].c", "[0][1]", "a.[0]"] {
            let p = Path::parse(s).unwrap();
            let rendered = p.to_string();
            // `a.[0]` 这种空键写法规范化后是 `a[0]`：语义相同（都只下钻，不取键）
            if s == "a.[0]" {
                assert_eq!(rendered, "a[0]");
            } else {
                assert_eq!(rendered, s);
            }
            assert_eq!(Path::parse(&rendered).unwrap(), p, "{s}");
        }
        // 前导零是**输入**别名，规范回显不带零（`007` ⇒ `7`）
        assert_eq!(Path::parse("a[007]").unwrap().to_string(), "a[7]");
    }

    #[test]
    fn strict_path_does_not_do_literal_key_first() {
        // 键名本身像路径：旧 `get` 能取，严格路径**取不到**（S1b 起这种键名会被直接拒绝）
        let d = Value::Object([("a[0]".to_string(), Value::Int(1))].into());
        assert_eq!(d.get("a[0]").and_then(|v| v.as_i64()), Some(1));
        assert_eq!(
            d.get_path(&Path::parse("a[0]").unwrap()),
            Err(PathError::NotFound { at: 1, key: "a".into() })
        );
    }

    #[test]
    fn works_with_preserve_order_map() {
        // `Map` 在两种档位下都是同一套 API，这条用例保证路径层没有写死 `BTreeMap`
        let m: Map = [("k".to_string(), Value::Int(1))].into();
        let d = Value::Object(m);
        assert_eq!(
            d.get_path(&Path::parse("k").unwrap()).map(|v| v.as_i64()),
            Ok(Some(1))
        );
    }
}
