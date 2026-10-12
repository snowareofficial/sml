//! 契约映射：解析期记录「**哪个实体应用了哪些契约**」。
//!
//! 回答的是用户（TASK-agent-w24）的原始问题：「库提供 API 获取一个数据实体
//! 包含哪些 contracts」。真值来源是解析期 —— `@is` 与块级标注在那一刻
//! 就知道应用了谁，顺手记一张旁路表即可；事后对 `Value` 做结构匹配
//! 会被默认值回填、strict/loose 搅浑，那是另一件事，不是本表。
//!
//! 三条铁规矩（见 TASK-agent-w24 §2 D5）：
//! 1. **opt-in**：不开 `trace_contracts` 时解析结果与旧版**逐字一致**（`contract_map: None`）；
//! 2. **交换表示就是一份 SML**（[`ContractMap::to_sml`]）：五端共享一份表示、
//!    可再解析、可 golden、C-ABI 一个 `char*` 就能跨边界；
//! 3. **输出确定**：按路径排序、无时间戳，同输入必须逐字节相同。
//!
//! 路径段的设计（D5）：`Key` = 对象键，`Index` = 数组下标 —— 二者靠 SML 自己的
//! 类型自识别区分（`0` 是下标，`"0"` 是键），不另造点分字符串 DSL
//! （键名含 `.` / `[` 时点分写法有歧义）。

use std::fmt::Write as _;

/// 实体在**结果树**里的位置段。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PathSeg {
    /// 对象键（如 `database`）。
    Key(String),
    /// 数组下标（如 `items` 的第 0 个元素）。
    Index(usize),
}

/// 一条记录：某实体（路径）应用了的契约名（**resolve 后**的名字，与
/// `contracts` 表的键一致，下游可直接反查；多个契约按应用顺序排列）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedContract {
    /// 实体路径；顶层实体为 `[]`。
    pub path: Vec<PathSeg>,
    /// 应用的契约名，按应用顺序（叠加 / 多条 `@is` 时为多项）。
    pub names: Vec<String>,
}

/// 契约映射：`路径 → 契约名列表` 的旁路表。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContractMap {
    entries: Vec<AppliedContract>,
}

impl ContractMap {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 记一条。同一实体的多次应用（多条 `@is`、块级 + `@is` 并存）合并为
    /// 一条、按应用顺序去重追加 —— 同名契约应用两次不产生两个名字。
    pub(crate) fn record(&mut self, path: Vec<PathSeg>, name: String) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.path == path) {
            if !e.names.contains(&name) {
                e.names.push(name);
            }
        } else {
            self.entries.push(AppliedContract {
                path,
                names: vec![name],
            });
        }
    }

    /// 冻结：按路径排序，保证输出与书写 / 应用顺序无关（D5 的确定性要求）。
    /// 在解析**出口**调用一次；之后只读。
    pub(crate) fn freeze(&mut self) {
        self.entries.sort_by(|a, b| a.path.cmp(&b.path));
    }

    /// 某实体的契约名列表；**没有应用任何契约 ⇒ 空切片**
    /// （而不是「不存在」与「空」两种状态 —— 未记录的实体不该出现在映射里）。
    pub fn names_at(&self, path: &[PathSeg]) -> &[String] {
        self.entries
            .iter()
            .find(|e| e.path == path)
            .map(|e| e.names.as_slice())
            .unwrap_or(&[])
    }

    /// 全部记录（已按路径排序，前提是出口处调过 [`Self::freeze`]）。
    pub fn entries(&self) -> &[AppliedContract] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 交换表示：一份 SML 文档 —— 顶层数组，每条
    /// `{ path: [ … ] contracts: [ … ] }`；顶层实体 `path: []`。
    ///
    /// 裸词 vs 引号的判据见 [`word_or_quoted`]：`path` 段与契约名共用同一判据，
    /// `database` / `Server` 保持裸词（可读），`"123"` / `"true"` 这类键自动加引号
    /// （否则读回来会变成数组下标或布尔 —— 那正是「类型自识别」的另一面）。
    pub fn to_sml(&self) -> String {
        if self.entries.is_empty() {
            return "[]\n".to_string();
        }
        let mut out = String::from("[\n");
        for e in &self.entries {
            out.push_str("  { path: ");
            if e.path.is_empty() {
                out.push_str("[]");
            } else {
                out.push_str("[ ");
                for (i, seg) in e.path.iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    match seg {
                        PathSeg::Key(k) => out.push_str(&word_or_quoted(k)),
                        PathSeg::Index(n) => {
                            let _ = write!(out, "{n}");
                        }
                    }
                }
                out.push_str(" ]");
            }
            out.push_str(" contracts: [ ");
            for (i, n) in e.names.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                out.push_str(&word_or_quoted(n));
            }
            out.push_str(" ] }\n");
        }
        out.push_str("]\n");
        out
    }
}

/// 裸词安全判据：全由「词字符」（字母 / 数字 / `_` `@` `.` `-`，含 CJK）组成、
/// 不以 ASCII 数字开头（否则读回是数）、且不是三个保留字 —— 满足则裸写，
/// 否则加引号并转义。
fn word_or_quoted(s: &str) -> String {
    let safe = !s.is_empty()
        && !s.chars().next().unwrap().is_ascii_digit()
        && !matches!(s, "true" | "false" | "null")
        && s.chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '@' | '.' | '-'));
    if safe {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(s: &str) -> PathSeg {
        PathSeg::Key(s.into())
    }

    /// 同一实体多次应用合并为一条、按序去重。
    #[test]
    fn repeated_application_merges_into_one_entry() {
        let mut m = ContractMap::new();
        m.record(vec![k("a")], "X".into());
        m.record(vec![k("a")], "Y".into());
        m.record(vec![k("a")], "X".into()); // 重复应用同名 ⇒ 不重复记
        assert_eq!(m.len(), 1);
        assert_eq!(m.names_at(&[k("a")]), ["X", "Y"]);
    }

    /// 不同实体各占一条。
    #[test]
    fn distinct_paths_are_distinct_entries() {
        let mut m = ContractMap::new();
        m.record(vec![k("a")], "X".into());
        m.record(vec![k("b")], "X".into());
        assert_eq!(m.len(), 2);
        assert_eq!(m.names_at(&[k("b")]), ["X"]);
        assert!(m.names_at(&[k("c")]).is_empty(), "未记录的实体 ⇒ 空切片");
    }

    /// 排序与确定性：`Index` 与 `Key` 的比较固定，输出与记录顺序无关。
    #[test]
    fn freeze_sorts_by_path() {
        let mut m = ContractMap::new();
        m.record(vec![k("b")], "X".into());
        m.record(vec![], "T".into());
        m.record(vec![k("a"), PathSeg::Index(0)], "Y".into());
        m.freeze();
        let paths: Vec<_> = m.entries().iter().map(|e| e.path.clone()).collect();
        assert_eq!(
            paths,
            vec![
                vec![],
                vec![k("a"), PathSeg::Index(0)],
                vec![k("b")],
            ]
        );
    }

    /// to_sml golden：裸词保持裸词、数字键加引号、顶层实体 `path: []`、
    /// 空表输出 `[]`。同输入必须逐字节相同。
    #[test]
    fn to_sml_golden() {
        assert_eq!(ContractMap::new().to_sml(), "[]\n");

        let mut m = ContractMap::new();
        m.record(vec![k("database"), k("primary")], "Server".into());
        m.record(vec![], "Top".into());
        m.record(vec![k("items"), PathSeg::Index(0)], "Item".into());
        m.record(vec![k("123")], "Numeric".into()); // 数字键必须加引号
        m.freeze();
        // 排序 = 路径逐段比较（`Key` 段按字典序，故 `"123" < database < items`；
        // 同位置 `Key` 在 `Index` 前 —— 枚举声明序即比较序，固定且与记录顺序无关）
        assert_eq!(
            m.to_sml(),
            "[\n  { path: [] contracts: [ Top ] }\n  \
                  { path: [ \"123\" ] contracts: [ Numeric ] }\n  \
                  { path: [ database primary ] contracts: [ Server ] }\n  \
                  { path: [ items 0 ] contracts: [ Item ] }\n]\n"
        );
        // 同输入两次输出逐字节相同（确定性）
        assert_eq!(m.to_sml(), m.to_sml());
    }

    /// 特殊字符键：引号 + 转义，读回不失真。
    #[test]
    fn to_sml_escapes_special_keys() {
        let mut m = ContractMap::new();
        m.record(vec![k("a b")], "X".into());
        m.record(vec![k("q\"k")], "X".into());
        m.freeze();
        let s = m.to_sml();
        assert!(s.contains("\"a b\""), "{s:?}");
        assert!(s.contains("\"q\\\"k\""), "{s:?}");
    }
}
