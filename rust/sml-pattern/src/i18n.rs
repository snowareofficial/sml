//! 关键字国际化：把「书写形式」与「内部语义」解耦。
//!
//! # 为什么不是硬编码别名
//!
//! 若在解析器里写死 `if k == "字面" || k == "lit"`，每加一种语言都要改核心代码，
//! 且第三方无法自带方言。这里抽象为 [`KeywordTable`] trait：
//!
//! - 内置三张表：[`CHINESE`] / [`ENGLISH`] / [`BILINGUAL`]（默认，两种写法等价）
//! - 用户可实现 trait 自带任何语言，或构造 [`StaticTable`]
//!
//! 注意「机翻等价」的界定：**只替换关键字本身，不动结构标记**
//! （`@` `:` `{}` `[]` `&` 原样保留），因此语法结构在所有语言下一致。

/// 与语言无关的**内部语义**。任何书写形式都先映射到它，再由它驱动编译。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Concept {
    // —— 模式元素 ——
    /// 字面量：`字面` / `lit`
    Lit,
    /// 字符类：`类` / `class`
    Class,
    /// 选择：`任一` / `alt`
    Alt,
    /// 内联组：`组` / `group`
    Group,
    /// 序列：`序列` / `seq`
    Seq,
    /// 引用另一规则：`用` / `use`
    Use,
    /// 推进直到：`直到` / `until`
    Until,
    /// 量词：`次` / `times`
    Times,
    /// 可选：`可选` / `optional`
    Optional,
    /// 命名捕获：`名` / `name`
    Name,
    /// regex 逃生舱：`正则` / `regex`
    Regex,

    // —— 量词写成对象时的字段 ——
    /// `最小` / `min`
    Min,
    /// `最大` / `max`
    Max,

    // —— 字符类的取值 ——
    /// `数字` / `digit`
    Digit,
    /// `字母` / `alpha`（含汉字）
    Alpha,
    /// `空白` / `space`
    Space,
    /// `字` / `word`
    Word,
    /// `任意` / `any`
    Any,

    // —— 特殊终止符 ——
    /// `行尾` / `eol`
    Eol,
}

/// 关键字表：书写形式 → 内部语义。
///
/// 实现此 trait 即可接入任意语言，无需改动编译逻辑。
pub trait KeywordTable {
    fn lookup(&self, word: &str) -> Option<Concept>;
}

/// 静态表：由「书写形式 → 语义」的常量切片构造。
pub struct StaticTable {
    entries: &'static [(&'static str, Concept)],
}

impl StaticTable {
    pub const fn new(entries: &'static [(&'static str, Concept)]) -> Self {
        Self { entries }
    }
}

impl KeywordTable for StaticTable {
    fn lookup(&self, word: &str) -> Option<Concept> {
        self.entries
            .iter()
            .find(|(w, _)| *w == word)
            .map(|(_, c)| *c)
    }
}

use Concept::*;

/// 中文表
pub const CHINESE: StaticTable = StaticTable::new(&[
    ("字面", Lit),
    ("类", Class),
    ("任一", Alt),
    ("组", Group),
    ("序列", Seq),
    ("用", Use),
    ("直到", Until),
    ("次", Times),
    ("可选", Optional),
    ("名", Name),
    ("正则", Regex),
    ("最小", Min),
    ("最大", Max),
    ("数字", Digit),
    ("字母", Alpha),
    ("空白", Space),
    ("字", Word),
    ("任意", Any),
    ("行尾", Eol),
    ("末尾", Eol),
]);

/// 英文表
pub const ENGLISH: StaticTable = StaticTable::new(&[
    ("lit", Lit),
    ("literal", Lit),
    ("class", Class),
    ("alt", Alt),
    ("any-of", Alt),
    ("group", Group),
    ("seq", Seq),
    ("use", Use),
    ("until", Until),
    ("times", Times),
    ("repeat", Times),
    ("optional", Optional),
    ("name", Name),
    ("regex", Regex),
    ("re", Regex),
    ("min", Min),
    ("max", Max),
    ("digit", Digit),
    ("alpha", Alpha),
    ("space", Space),
    ("word", Word),
    ("any", Any),
    ("eol", Eol),
    ("end", Eol),
]);

/// 双语表（默认）：同一份文档里中英写法等价，可混用。
///
/// 这是 SML「中文关键字 = 机翻等价」的直接体现：不发明新语法，
/// 只是同一个语义有两种书写形式。
pub const BILINGUAL: StaticTable = StaticTable::new(&[
    // 中文
    ("字面", Lit),
    ("类", Class),
    ("任一", Alt),
    ("组", Group),
    ("序列", Seq),
    ("用", Use),
    ("直到", Until),
    ("次", Times),
    ("可选", Optional),
    ("名", Name),
    ("正则", Regex),
    ("最小", Min),
    ("最大", Max),
    ("数字", Digit),
    ("字母", Alpha),
    ("空白", Space),
    ("字", Word),
    ("任意", Any),
    ("行尾", Eol),
    ("末尾", Eol),
    // 英文
    ("lit", Lit),
    ("literal", Lit),
    ("class", Class),
    ("alt", Alt),
    ("any-of", Alt),
    ("group", Group),
    ("seq", Seq),
    ("use", Use),
    ("until", Until),
    ("times", Times),
    ("repeat", Times),
    ("optional", Optional),
    ("name", Name),
    ("regex", Regex),
    ("re", Regex),
    ("min", Min),
    ("max", Max),
    ("digit", Digit),
    ("alpha", Alpha),
    ("space", Space),
    ("word", Word),
    ("any", Any),
    ("eol", Eol),
    ("end", Eol),
]);
