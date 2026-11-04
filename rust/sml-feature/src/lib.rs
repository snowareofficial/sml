// SPDX-License-Identifier: MulanPSL-2.0
//! SML version & feature system — a **pure data layer**, no dependency on lexing. ｜ SML 版本与特性系统 —— **纯数据层**，不依赖词法。
//!
//! Defines exactly three things: ｜ 只定义三件事：
//! - [`Version`] — grammar version (`v1`..`v4`); ｜ 语法版本；
//! - [`Feature`] / [`FeatureSet`] — capability bits and bitmask sets; ｜ 能力位与位掩码集合；
//! - `FEATURES` / `feature_names` — the name ↔ bit registry. ｜ 名字 ↔ 位的注册表。
//!
//! 刻意**不**包含 `@version`/`@feature` 的**扫描**：那部分需要 `Tok`
//! （在 `sml-lex`），若放进来会形成 feature → lex → feature 的循环依赖，
//! 故置于 `sml-parse::scan`。

use std::fmt;

/// SML syntax version ｜ SML 语法版本。
///
/// SML 源于 eclog，演进中通过 `@version` 声明文档遵循的语法版本，
/// 使解析器能在将来引入 v2 不兼容语法时仍正确读取旧文档。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Version {
    /// v1：初始公开版本。字符串可裸写（`name: John`），自动识别类型。
    V1,
    /// v2：草案版，引入「字符串必须显式引号」的不兼容语法（与 v3 同语义）。
    V2,
    /// v3：正式版。取消自动字符串无引号，自由文本必须写作 `"..."`；
    ///     数字 / bool / null / 片段引用 `&x` / 环境变量 `$env.X` 仍为裸词。
    V3,
    /// v4：片段定义的 `type` / `name` 参数改为**显式**关键字形式：
    ///     `@f type: Server name: prod { .. }`；
    ///     废弃 v3 的位置参数形式（`@f Server prod { .. }`）。
    ///
    /// 动机：位置参数使「拼错的指令 `@nosuch Word { .. }`」与
    /// 「片段定义 + type 参数」在 token 流上完全同形，无法判别，
    /// 导致块被当作片段体消费、内容静默丢失且不报错。
    /// 语法与 v3 其余部分完全兼容（字符串引号、标量裸词等规则不变）。
    V4,
}

impl Version {
    /// Newest version this implementation supports ｜ 当前实现支持的最新版本。
    pub const CURRENT: Version = Version::V4;

    /// Whether strings must be explicitly quoted (`v2` onward; `v1` is the relaxed one) ｜ 是否要求字符串显式引号（v2 起为严格模式）。
    pub fn strict_strings(self) -> bool {
        self >= Version::V2
    }

    /// Parse a version literal: `v1`/`1` … `v4`/`4`; anything else is `None` ｜ 解析版本字面量（`v1`/`1`、`v2`/`2`、`v3`/`3`、`v4`/`4`）。
    pub fn from_word(w: &str) -> Option<Version> {
        match w {
            "v1" | "1" => Some(Version::V1),
            "v2" | "2" => Some(Version::V2),
            "v3" | "3" => Some(Version::V3),
            "v4" | "4" => Some(Version::V4),
            _ => None,
        }
    }

    /// Version name, i.e. `"v1"`…`"v4"` (for error messages / round-trip echo) ｜ 版本名（用于错误信息与序列化回显）。
    pub fn name(self) -> &'static str {
        match self {
            Version::V1 => "v1",
            Version::V2 => "v2",
            Version::V3 => "v3",
            Version::V4 => "v4",
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ===========================================================================
// 特性集 (FeatureSet)
//
// 文档可通过 `@feature` 指令在「版本基线」之上做**裁剪**（窄化），调用方也可
// 通过 `parse_with_features` / `parse_allowed` 限制接受的子集。文档不能扩宽
// 调用方给出的范围——否则 `@feature` 就成了绕过限制的后门。
//
// 为保证五端（Rust/C/JS/C++/Lua）实现一致且易于维护，特性名与位定义集中
// 在此（见 [`FEATURES`] 表）。新增特性只需在表中加一行，并在对应 parser 处
// 用 `ps.features.has(Feature::Xxx)` 判定即可，无需散落大量 if。
// ===========================================================================

/// One feature flag, mirrored 1:1 by [`FEATURES`] — change the table and every binding follows ｜ 单个特性标识。与 [`FEATURES`] 表一一对应；改表即改全端。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    /// 裸词即字符串（v1 行为）。v2/v3 关闭后字符串必须加引号。
    BarewordStr,
    /// `include "x.sml"` 文件包含。
    Include,
    /// `$env.VAR` 环境变量内插。
    Env,
    /// `@contract` / `@is` 契约系统。
    Contract,
    /// `&frag` / `@frag` 片段复用。
    Fragment,
    /// 顶层裸数组 `[ ... ]`（无键）。
    TopArray,
    /// `include "x.sml" as ns` 命名空间包含（高优先级前缀）。
    Namespace,
    /// 无扩展名的 `include "foo"` 默认等价于 `include "foo.sml" as foo`。
    ImplicitNs,
    /// 逗号分隔的多目标 `include "a", "b" as y` 与 `import` 别名。
    MultiInclude,
    /// 通配 `include "dir/*.sml"`（glob）。
    GlobInclude,
    /// 正则匹配 `include /re/`（需 `regex-include`）。
    RegexInclude,
    /// 扩展名重写 `include "x.conf" -> "x.sml"`（将非 sml 当 sml 解析）。
    ExtRewrite,
    /// `@when <条件>` 条件裁剪（作用于紧邻的下一个字段/块）。
    ///
    /// **opt-in**：不在 `baseline()` 中，需文档显式 `@feature enable when`。
    /// 条件只支持 `$env.NAME` 与 `==` / `!=` 比较这一**闭集**形式，
    /// 不引入通用表达式求值器（无沙箱/无 IO/无注入面）。
    ///
    /// 追加在末尾以保证既有特性位序不变（C-ABI `sml_feature_name(bit)`
    /// 与该位序绑定，见 `feature_names` 的守护用例）。
    When,
    /// `@for var in a b c { ... }` 有界循环展开（作用于值位置，生成数组）。
    ///
    /// **opt-in**：不在 `baseline()` 中，需文档显式 `@feature enable for`。
    /// 刻意**不引入通用表达式求值器，也不是图灵完备的**：
    /// 1. **循环有界**：只遍历有限列表（`in` 后的显式枚举），无 `while`；
    /// 2. **变量只读**：`${var}` 是只读绑定，循环体不能修改它或列表；
    /// 3. **无递归**：模板不能引用自身。
    ///
    /// 有界循环属于 LOOP 语言（原始递归），算不了 Ackermann 函数；一旦引入
    /// `while`/递归就必须配套沙箱与资源配额——那与「SML 是纯数据格式」冲突。
    ///
    /// 追加在 `When` 之后以保位序稳定。
    For,
    /// 块级类型标注：`<契约名> <块名> { .. }`。
    ///
    /// 裸块的首词若命中已定义的契约，则自动把该契约应用到这个块
    /// （等价于在块内首行写 `@is 契约名`）。这是 `@is` 更自然的替代写法：
    /// 类型标注与数据定义合一，嵌套块也能各自带约束。
    ///
    /// 与既有裸块语法 `type [name...] { }` **完全同形**，故不引入任何新 token
    /// 或新语法；未开启时首词仅作 `__type` 元数据，既有文档零影响。
    ///
    /// **opt-in**：不在 `baseline()` 中，需文档显式 `@feature enable typed-block`。
    ///
    /// 追加在 `For` 之后以保位序稳定（C-ABI 位序绑定，见 `feature_names` 守护用例）。
    TypedBlock,
    /// 契约名查找**首字母大小写不敏感**（opt-in）。
    ///
    /// 默认（未开启）时契约名严格区分大小写：契约声明为 `Rule`，而块级标注写成
    /// `rule ssh { .. }`，首词不会命中契约表 ⇒ 契约**静默不生效**（不校验、不填
    /// 默认值、**也不报错**）。这是当前最容易踩的坑 —— 配置里写错首字母大小写，
    /// 等于校验形同虚设且不报错。
    ///
    /// 开启后：精确匹配失败时，再按「首字母小写后相等」做一次兜底查找，于是
    /// `rule` 能命中 `Rule`（反向同样成立）。**只影响查找**：契约的声明名、
    /// `__type` / `__name` 元数据、错误信息里的契约名，一律保持书写原样。
    ///
    /// **opt-in**：不在 `baseline()` 中，需文档显式 `@feature enable case-insensitive`。
    ///
    /// 追加在 `TypedBlock` 之后以保位序稳定（C-ABI 位序绑定，见 `feature_names` 守护用例）。
    CaseInsensitive,
    /// 块级标注（typed-block）里「首个未命中契约表的词」可否当作**块名**（opt-in）。
    ///
    /// 默认（未开启）时，typed-block 模式下前缀里的每个词都必须是契约名。这拆掉了一颗雷：
    /// `Rule Securty ssh { .. }` 里拼错的 `Securty` 原先会**静默变成块名**、真正的块名
    /// `ssh` 被挤进 `__args`，全程不报错 —— 契约写错等于静默失效。
    /// 开启本特性后恢复「首个非契约词 = 块名，其余进 `__args`」的行为。
    ///
    /// ⚠️ **只作用于 typed-block 模式**（首词命中了契约表）。普通裸块
    /// `server web prod { }` 的既有行为不受影响（`__type`/`__name`/`__args` 回归红线）。
    ///
    /// **opt-in**：不在 `baseline()` 中，需文档显式 `@feature enable block-name`；
    /// 与 typed-block 合用需两个特性都开。
    ///
    /// 追加在 `CaseInsensitive` 之后以保位序稳定（C-ABI 位序绑定）。
    BlockName,
}

/// All registered feature names, in [`FEATURES`] order (i.e. bit order) ｜ 返回全部已注册特性的名字，顺序与 [`FEATURES`]（即特性位序）一致。
///
/// C-ABI 的 `sml_feature_name(bit)` 依赖此顺序，测试中有对应守护用例。
pub fn feature_names() -> Vec<&'static str> {
    FEATURES.iter().map(|(n, _)| *n).collect()
}

/// Name → enum registry. All bindings share one set of names, so the wire form matches ｜ 特性名 → 枚举 的注册表。所有端共用同一组名字，保证跨语言一致。
pub static FEATURES: &[(&str, Feature)] = &[
    ("bareword-string", Feature::BarewordStr),
    ("include", Feature::Include),
    ("env", Feature::Env),
    ("contract", Feature::Contract),
    ("fragment", Feature::Fragment),
    ("top-level-array", Feature::TopArray),
    ("namespace", Feature::Namespace),
    ("implicit-ns", Feature::ImplicitNs),
    ("multi-include", Feature::MultiInclude),
    ("glob-include", Feature::GlobInclude),
    ("regex-include", Feature::RegexInclude),
    ("ext-rewrite", Feature::ExtRewrite),
    ("when", Feature::When),
    ("for", Feature::For),
    ("typed-block", Feature::TypedBlock),
    ("case-insensitive", Feature::CaseInsensitive),
    ("block-name", Feature::BlockName),
];

impl Feature {
    /// Look a feature up by name; `None` for unknown names so callers can error instead of silently accepting a typo ｜ 按名字查特性；未知名字返回 None（调用方据此报错，杜绝静默 typo）。
    pub fn from_name(name: &str) -> Option<Feature> {
        FEATURES.iter().find(|(n, _)| *n == name).map(|(_, f)| *f)
    }

    /// Feature name as written in documents (for errors / round-trip echo) ｜ 特性名（用于报错 / 序列化回显）。
    pub fn name(self) -> &'static str {
        FEATURES
            .iter()
            .find(|(_, f)| *f == self)
            .map(|(n, _)| *n)
            .unwrap_or("<unknown>")
    }
}

/// A set of features as a bitmask ｜ 位掩码形式的特性集合。
///
/// 设计哲学：从极简到丰富、功能可裁剪。默认基线（`baseline()`）只开核心八项
/// （`bareword-string` / `include` / `env` / `contract` / `fragment` / `top-level-array` /
/// `namespace` / `implicit-ns`），复杂能力（多目标 / glob / 正则 /
/// 扩展名重写）必须显式 `@feature enable` 才生效，避免重蹈 YAML 过度复杂的覆辙。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeatureSet(u64);

impl FeatureSet {
    /// Every feature bit, including all opt-in abilities. This is the **whole** universe
    /// (`caller-allows-everything`); it is not the default — see [`FeatureSet::baseline`]. ｜ 全部特性位（含所有 opt-in 能力）。
    ///
    /// 用途是「调用方允许全集」，真正的默认集是 [`FeatureSet::baseline`]。
    pub fn all() -> FeatureSet {
        let mut m = 0u64;
        for (_, f) in FEATURES {
            m |= 1 << (*f as u8);
        }
        FeatureSet(m)
    }

    /// The minimal default set (core SML) — **eight** bits, see below. This is `parse_file`'s default allow-set ｜ 极简默认集（SML 核心可用能力）。
    ///
    /// 具体八项：`bareword-string` / `include` / `env` / `contract` / `fragment` /
    /// `top-level-array` / `namespace` / `implicit-ns`。
    /// 仅「多目标 / glob / 正则 / 扩展名重写 / when / for / typed-block / case-insensitive /
    /// block-name」等高级能力需文档内 `@feature enable` 显式开启（避免重蹈 YAML 覆辙）。
    /// ⚠️ 早先的文档写「极简三件套」，与实现不符（`baseline` 实际有 8 位），2026-11-02 更正。
    pub fn baseline() -> FeatureSet {
        FeatureSet::none()
            .with(Feature::BarewordStr)
            .with(Feature::Include)
            .with(Feature::Env)
            .with(Feature::Contract)
            .with(Feature::Fragment)
            .with(Feature::TopArray)
            .with(Feature::Namespace)
            .with(Feature::ImplicitNs)
    }

    /// The empty set — every feature off ｜ 空集合（所有特性都关）。
    pub fn none() -> FeatureSet {
        FeatureSet(0)
    }

    /// Build the default feature set for a version: [`FeatureSet::baseline`] plus/minus bareword strings ｜ 按版本基线构造默认特性集。
    ///
    /// 具体：`v1` = baseline + 裸词字符串；`v2` 起关闭裸词字符串（须引号）。
    /// 复杂能力（glob / regex / multi…）仍默认关闭，需文档 `@feature enable` 显式开启。
    pub fn for_version(v: Version) -> FeatureSet {
        let mut s = FeatureSet::baseline();
        // 严格模式（v2/v3）关闭裸词字符串；非严格（v1）开启。
        // 显式设置该位，确保与 baseline 默认值无关。
        if v.strict_strings() {
            s = s.without(Feature::BarewordStr);
        } else {
            s = s.with(Feature::BarewordStr);
        }
        s
    }

    /// Whether feature `f` is enabled ｜ 是否包含某特性。
    pub fn has(self, f: Feature) -> bool {
        (self.0 & (1 << (f as u8))) != 0
    }

    /// Copy with `f` turned **on** (takes self by value; `FeatureSet` is `Copy`) ｜ 返回开启 `f` 后的副本。
    pub fn with(self, f: Feature) -> FeatureSet {
        FeatureSet(self.0 | (1 << (f as u8)))
    }

    /// Copy with `f` turned **off** ｜ 返回关闭 `f` 后的副本。
    pub fn without(self, f: Feature) -> FeatureSet {
        FeatureSet(self.0 & !(1 << (f as u8)))
    }

    /// Intersect with another set — `document-declared ∩ caller-allowed` ｜ 与另一集合取交集（用于「文档裁剪 ∩ 调用方允许」）。
    pub fn intersection(self, other: FeatureSet) -> FeatureSet {
        FeatureSet(self.0 & other.0)
    }

    /// Whether nothing is enabled ｜ 是否无任何特性。
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl fmt::Display for FeatureSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for (n, feat) in FEATURES {
            if self.has(*feat) {
                if !first {
                    f.write_str(",")?;
                }
                f.write_str(n)?;
                first = false;
            }
        }
        if first {
            f.write_str("<none>")?;
        }
        Ok(())
    }
}

/// `@feature` parse mode: whitelist (only the listed ones on) / blacklist (the listed ones off) ｜ `@feature` 解析模式：白名单（仅启用列出的）/ 黑名单（禁用列出的）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureMode {
    Default,
    Whitelist,
    Blacklist,
}

// ===========================================================================
// Edition —— 版本 = 功能集的名字（edition 模型）
//
// 设计：版本之间**没有大小关系**，判据是「这个特性在不在这套集合里」。
// 见 `docs/version-naming.md`（v1.0 定稿）。
//
// ⚠️ 本段是**纯加法**：既有的 [`Version`] 枚举与 `for_version` 一律不动，
// 供解析层逐步切换（切片 S0a）。等解析层切完再删 `Version`。
// ===========================================================================

/// 一代版本集的状态 ｜ lifecycle status of one edition.
///
/// - `Dev`：开发中，文档必须写 `dev` 标记；
/// - `Supported`：已发布，**永久向后兼容**（语义与默认特性位都不许再变）；
/// - `Removed`：曾存在、已被移除（只可能出现在开发代号上）⇒ 报错 + 指向迁移工具。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditionStatus {
    Dev,
    Supported,
    Removed,
}

/// 一代版本：[大版本].[小版本] ｜ one edition, identified by `major.minor`.
///
/// 数字只是**名字**的一部分，不表示大小关系（不做 `>=` 比较）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Edition {
    pub major: u16,
    pub minor: u16,
}

/// 版本登记表的一行：数字 + 代号 + 状态 ｜ one row of the edition registry.
pub struct EditionRow {
    pub edition: Edition,
    /// 开发代号（如 `camelia`）；与数字**永久双向映射**。
    pub codename: Option<&'static str>,
    pub status: EditionStatus,
}

/// 已登记的全部版本集（顺序即「代」的顺序，不参与比较）｜ the edition registry.
///
/// ⚠️ 新增一行即可；**不许改既有行的数字或代号**（已发布版本永久兼容）。
pub static EDITIONS: &[EditionRow] = &[
    EditionRow { edition: Edition { major: 1, minor: 0 }, codename: None, status: EditionStatus::Supported },
    EditionRow { edition: Edition { major: 2, minor: 0 }, codename: None, status: EditionStatus::Supported },
    EditionRow { edition: Edition { major: 3, minor: 0 }, codename: None, status: EditionStatus::Supported },
    EditionRow { edition: Edition { major: 4, minor: 0 }, codename: None, status: EditionStatus::Supported },
    EditionRow { edition: Edition { major: 5, minor: 0 }, codename: Some("camelia"), status: EditionStatus::Dev },
];

impl Edition {
    /// 构造（const，便于在静态表与常量里用）｜ constructors usable in `const` context.
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// 解析版本字面量 ｜ parse a version literal.
    ///
    /// 接受的全部形式（都是**别名**，指向同一代）：
    /// - 旧写法：`v1`..`v9` / `1`..`9` → 对应 `N.0`
    /// - 连写：`10` / `40` / `50` → `1.0` / `4.0` / `5.0`（**两段各一位**）
    /// - 点号：`1.0` / `5.0`
    ///
    /// 连写只支持「各大版本/小版本各一位」——`100` 这类三位的必须写点号，
    /// 否则无法区分 `10.0` 与 `1.00`。非法输入返回 `None`（由调用方映射成错误码）。
    pub fn parse(lit: &str) -> Option<Edition> {
        let s = lit.trim();
        let s = s.strip_prefix('v').or_else(|| s.strip_prefix('V')).unwrap_or(s);
        if s.is_empty() {
            return None;
        }
        if let Some((a, b)) = s.split_once('.') {
            if a.is_empty() || b.is_empty() || a.len() > 5 || b.len() > 5 {
                return None;
            }
            let major: u16 = a.parse().ok()?;
            let minor: u16 = b.parse().ok()?;
            return Some(Edition { major, minor });
        }
        if !s.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        match s.len() {
            1 => Some(Edition { major: s.parse().ok()?, minor: 0 }),
            2 => {
                let mut it = s.chars();
                let major = it.next()?.to_digit(10)? as u16;
                let minor = it.next()?.to_digit(10)? as u16;
                Some(Edition { major, minor })
            }
            // 三位及以上必须写点号（`100` 有歧义）
            _ => None,
        }
    }

    /// 规范名字（`5.0`）｜ canonical name.
    pub fn name(&self) -> String {
        format!("{}.{}", self.major, self.minor)
    }

    /// 登记行（未登记的版本返回 `None`）｜ registry row, `None` if not registered.
    pub fn row(&self) -> Option<&'static EditionRow> {
        EDITIONS.iter().find(|r| r.edition == *self)
    }

    /// 代号（未登记或无代号返回 `None`）｜ codename, if any.
    pub fn codename(&self) -> Option<&'static str> {
        self.row().and_then(|r| r.codename)
    }

    /// 生命周期状态（未登记返回 `None`）｜ lifecycle status, `None` if not registered.
    pub fn status(&self) -> Option<EditionStatus> {
        self.row().map(|r| r.status)
    }

    /// 是否处于开发中（`Dev`）｜ whether this edition is still in development.
    pub fn is_dev(&self) -> bool {
        self.status() == Some(EditionStatus::Dev)
    }

    /// 是否可长期使用（`Supported`：已发布、永久兼容）｜ long-term usable.
    pub fn is_supported(&self) -> bool {
        self.status() == Some(EditionStatus::Supported)
    }

    /// 从 [`Version`] 旧枚举转过来（迁移期用）｜ conversion from the legacy enum.
    pub fn from_version(v: Version) -> Edition {
        match v {
            Version::V1 => Edition::new(1, 0),
            Version::V2 => Edition::new(2, 0),
            Version::V3 => Edition::new(3, 0),
            Version::V4 => Edition::new(4, 0),
        }
    }
}

/// 按代号查登记行（`"camelia"` → 5.0）｜ look an edition up by codename.
pub fn edition_by_codename(name: &str) -> Option<&'static EditionRow> {
    EDITIONS.iter().find(|r| r.codename == Some(name))
}

impl fmt::Display for Edition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name())
    }
}

#[cfg(test)]
mod edition_tests {
    use super::*;

    #[test]
    fn aliases_are_the_same_edition() {
        let one = Edition::new(1, 0);
        for lit in ["1", "v1", "10", "1.0", "V1"] {
            assert_eq!(Edition::parse(lit), Some(one), "字面量 {lit}");
        }
        let four = Edition::new(4, 0);
        for lit in ["4", "v4", "40", "4.0"] {
            assert_eq!(Edition::parse(lit), Some(four), "字面量 {lit}");
        }
    }

    #[test]
    fn dev_edition_parses_like_any_other() {
        assert_eq!(Edition::parse("50"), Some(Edition::new(5, 0)));
        assert_eq!(Edition::parse("5.0"), Some(Edition::new(5, 0)));
        assert_eq!(Edition::parse("v5"), Some(Edition::new(5, 0)));
    }

    #[test]
    fn three_digits_need_a_dot() {
        // `100` 有歧义（10.0 还是 1.00）⇒ 必须写点号
        assert_eq!(Edition::parse("100"), None);
        assert_eq!(Edition::parse("10.0"), Some(Edition::new(10, 0)));
    }

    #[test]
    fn rejects_garbage() {
        for lit in ["", "v", "abc", "1.", ".0", "1.2.3", "-1"] {
            assert_eq!(Edition::parse(lit), None, "字面量 {lit:?} 不该被接受");
        }
    }

    #[test]
    fn registry_status_and_codename() {
        assert_eq!(Edition::new(4, 0).status(), Some(EditionStatus::Supported));
        assert_eq!(Edition::new(4, 0).codename(), None);
        assert_eq!(Edition::new(5, 0).status(), Some(EditionStatus::Dev));
        assert_eq!(Edition::new(5, 0).codename(), Some("camelia"));
        assert!(Edition::new(5, 0).is_dev());
        assert!(Edition::new(4, 0).is_supported());
        assert_eq!(Edition::new(9, 9).status(), None, "未登记");
    }

    #[test]
    fn codename_maps_back_to_edition() {
        let row = edition_by_codename("camelia").expect("camelia 应已登记");
        assert_eq!(row.edition, Edition::new(5, 0));
        assert_eq!(row.status, EditionStatus::Dev);
        assert!(edition_by_codename("nope").is_none());
    }

    #[test]
    fn legacy_version_converts() {
        assert_eq!(Edition::from_version(Version::V4), Edition::new(4, 0));
        assert_eq!(Edition::from_version(Version::V1), Edition::new(1, 0));
    }

    #[test]
    fn display_is_major_dot_minor() {
        assert_eq!(Edition::new(5, 0).to_string(), "5.0");
        assert_eq!(Edition::new(5, 1).to_string(), "5.1");
    }
}
