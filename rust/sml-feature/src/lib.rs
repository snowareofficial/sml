// SPDX-License-Identifier: MulanPSL-2.0
//! SML edition & feature system — a **pure data layer**, no dependency on lexing. ｜ SML 版本集与特性系统 —— **纯数据层**，不依赖词法。
//!
//! Defines exactly three things: ｜ 只定义三件事：
//! - [`Edition`] — 版本集（`1.0`..`4.0` 已发布、`5.0` 开发中）；旧名 [`Version`] 保留为**类型别名**；
//! - [`Feature`] / [`FeatureSet`] — capability bits and bitmask sets; ｜ 能力位与位掩码集合；
//! - [`FEATURES`] / [`EDITIONS`] — 特性表与版本登记表（一行同时承载数字 / 代号 / 状态 / **功能集**）。
//!
//! ⚠️ **版本之间没有大小关系**（`docs/version-naming.md` v1.0）：版本号只是「一整套默认特性位」的名字，
//! 判据永远是「这个特性在不在这套功能集里」，所以本类型**不实现 `Ord`**。
//!
//! 刻意**不**包含 `@version`/`@feature` 的**扫描**：那部分需要 `Tok`
//! （在 `sml-lex`），若放进来会形成 feature → lex → feature 的循环依赖，
//! 故置于 `sml-parse::scan`。

use std::fmt;

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
    /// `!featag` 语法（`wea !critical obey[1,2] { }`）—— **第一个带域名**的特性。
    ///
    /// 文档里写作 `@feature enable type.featag`（`<域>.<名>`，最多两段，见
    /// `docs/feature-namespace-plan.md` §3 / N4）。老特性一律保持平铺，**不给** `type.xxx` 别名（N2）。
    ///
    /// **当前状态（2026-11-04）**：位已注册（默认关、不在 `baseline()`、不进任何 [`Edition`]
    /// 的功能集）。S2 接**块头语法识别**、S3 接**数据落盘**：开启后
    /// `wea !critical obey[1, 2], name { }` 会产出
    /// `{ wea: { __featags: [ { name: "critical" }, { name: "obey", args: [1, 2] } ], … } }`
    /// （保序数组）；三条歧义各报一码（`E-PARSE-026/027/028`）。
    /// ⚠️ 仍**不做声明校验**（未声明的 featag、参数类型不符都放行）—— 那是 S4/S5。
    ///
    /// 追加在 `BlockName` 之后以保位序稳定（C-ABI 位序绑定，见 `feature_names` 守护用例）。
    Featag,
    /// **标识符校验**（S1b，第二个带域名的特性：`lex.ident`）—— opt-in。
    ///
    /// 开启后，**键名**必须满足标识符白名单（`docs/featag-impl-slices.md` §2）：
    /// ASCII 字母数字 + `_` `-` + CJK 汉字 + 假名 + 谚文；拒绝 `.` `[` `]`（路径语法保留）、
    /// 空白 / 控制字符、零宽 / 双向控制（Cf）、组合标记（⇒ 天然 NFC）、
    /// 西里尔 / 希腊等同形字母、全角与兼容区字符。
    ///
    /// **为什么单列一个位、而不是塞进 `type.featag`**：标识符规则与 `!` 记号毫无关系，
    /// 若共用一个位，「开 featag 就顺手改了所有键名」⇒ 回滚粒度变粗、诊断也说不清是谁的规则。
    /// （`docs/featag-impl-slices.md` 的三条跨片原则写的是「都在 `type.featag` 后面」，
    /// 这里**有意偏离**并在施工计划里登记了理由。）
    ///
    /// ⚠️ 「每契约单一语种」不在这里 —— 那是**契约级**规则（同契约多个标识符之间比较），
    /// 跟声明收集（S4）一起做。
    Ident,
    /// **弱声明族**（`weakbind` / `weakdata` / `weakdoc`）—— 第三个带域名的特性：`type.weak`。
    ///
    /// 与 `type.featag` 同域：都挂在「类型与契约」上。`weak` 是**机制级**开关，
    /// 不是单指令开关 —— 这一族的分工见 `docs/weakbind-spec-draft.md` §2.0：
    /// `weakbind` = 编辑器/插件按契约解释、库不参与；`weakdata` = 旁路数据；
    /// `weakdoc` = 项目级 profile 引用。三者共用本开关（用户 2026-11-04 定）。
    ///
    /// **命名必须带域**：`docs/feature-namespace-plan.md` §3 规则 1 —— 新增特性一律
    /// `<域>.<名>`，老名保持平铺。故文档里写 `@feature enable type.weak`，
    /// **不是**裸 `weak`（裸 `weak` 会被当成未知特性名）。
    ///
    /// **opt-in**：不在 `baseline()` 中，不进任何 [`Edition`] 的功能集。
    ///
    /// 追加在 `Ident` 之后以保位序稳定（C-ABI 位序绑定，见 `feature_names` 守护用例）。
    Weak,
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
    // ⚠️ 第一个**带域**的名字：`type.featag`。老名一律保持平铺（见 FEATURE_DOMAINS）。
    ("type.featag", Feature::Featag),
    // 第二个带域名：`lex.ident`（标识符白名单，S1b）
    ("lex.ident", Feature::Ident),
    // 第三个带域名：`type.weak`（弱声明族，见 `docs/weakbind-spec-draft.md`）
    ("type.weak", Feature::Weak),
];

/// 特性名的**封闭域白名单**（`docs/feature-namespace-plan.md` §3，用户 2026-11-03 定死四个域）。
///
/// 规则（N1 / N2 / N4，都是定稿）：
/// - **只有新增特性带域**，写作 `<域>.<名>`；老名保持平铺 —— 故判据很便宜：**名字里有点 = 新特性**；
/// - **只在第一个点切一次**，三段及以上直接拒（用户要求「层数不能过多」）；
/// - **不做别名**：不给 `type.contract` 这种第二名字（一个特性两个名字会让诊断、`disable`、
///   C-ABI 回显出现「同一件事两种写法」）；
/// - 新增域必须改这张表，不许随手分层。
pub const FEATURE_DOMAINS: &[&str] = &["lex", "mod", "type", "flow"];

/// 为什么一个特性名不被接受 ｜ why `@feature enable <name>` was rejected.
///
/// 与「特性未启用（`E-FEATURE-001`）」无关：这里全是**名字本身**的问题，报 `E-FEATURE-003`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeatureNameError {
    /// 三段及以上（`a.b.c`）｜ more than two segments.
    TooManySegments,
    /// 域不在 [`FEATURE_DOMAINS`] 里（`foo.bar`）｜ unknown domain.
    BadDomain { domain: String },
    /// 形态合法，但注册表里没有（平名拼错，或对老特性用了 `type.xxx` 别名）｜ not registered.
    Unknown,
}

impl fmt::Display for FeatureNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeatureNameError::TooManySegments => {
                write!(f, "特性名最多两段（`<域>.<名>`），不许三段")
            }
            FeatureNameError::BadDomain { domain } => write!(
                f,
                "未知域 `{domain}`（只允许 {}）",
                FEATURE_DOMAINS.join(" / ")
            ),
            FeatureNameError::Unknown => write!(f, "未注册"),
        }
    }
}

impl Feature {
    /// Look a feature up by name; `None` for unknown names so callers can error instead of silently accepting a typo ｜ 按名字查特性；未知名字返回 None（调用方据此报错，杜绝静默 typo）。
    pub fn from_name(name: &str) -> Option<Feature> {
        FEATURES.iter().find(|(n, _)| *n == name).map(|(_, f)| *f)
    }

    /// Parse a feature name **with the namespace rule applied** ｜ 按命名空间规则解析特性名。
    ///
    /// 与 [`Feature::from_name`] 的分工：`from_name` 是**纯查表**（老调用方用它）；本函数先验形态
    /// 再查表，把「三段名」「未知域」「拼错」三类失败分得开 —— 否则用户只会看到一句
    /// 「未知特性」，改半天才发现是自己多写了一个点。
    ///
    /// ```
    /// use sml_feature::{Feature, FeatureNameError};
    /// assert_eq!(Feature::parse_name("typed-block"), Ok(Feature::TypedBlock));
    /// assert_eq!(Feature::parse_name("type.featag"), Ok(Feature::Featag));
    /// assert_eq!(Feature::parse_name("a.b.c"), Err(FeatureNameError::TooManySegments));
    /// assert!(matches!(
    ///     Feature::parse_name("foo.bar"),
    ///     Err(FeatureNameError::BadDomain { .. })
    /// ));
    /// // 老特性**不补**带域别名（N2）：域合法，但名字没注册
    /// assert_eq!(
    ///     Feature::parse_name("type.typed-block"),
    ///     Err(FeatureNameError::Unknown)
    /// );
    /// ```
    pub fn parse_name(name: &str) -> Result<Feature, FeatureNameError> {
        if let Some((domain, rest)) = name.split_once('.') {
            if rest.contains('.') {
                return Err(FeatureNameError::TooManySegments);
            }
            if !FEATURE_DOMAINS.contains(&domain) {
                return Err(FeatureNameError::BadDomain {
                    domain: domain.to_string(),
                });
            }
        }
        Feature::from_name(name).ok_or(FeatureNameError::Unknown)
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
    /// block-name / type.featag」等高级能力需文档内 `@feature enable` 显式开启（避免重蹈 YAML 覆辙）。
    /// ⚠️ 早先的文档写「极简三件套」，与实现不符（`baseline` 实际有 8 位），2026-11-02 更正。
    ///
    /// `const` 是因为版本登记表 [`EDITIONS`] 要在静态初始化里为每一代**写死**它的功能集
    /// （「版本 = 功能集」，见 `docs/version-naming.md` §2）。
    pub const fn baseline() -> FeatureSet {
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
    pub const fn none() -> FeatureSet {
        FeatureSet(0)
    }

    /// The default feature set of one edition — **a table lookup**, not a comparison ｜ 某一代版本的默认特性集（**查表**，不做大小比较）。
    ///
    /// 数据来源是 [`EDITIONS`] 每一行的 `features` 字段（「版本 = 功能集」，单一事实来源）。
    /// 未登记的标识回落到 [`FeatureSet::baseline`] —— 调用方通常会在更早的地方
    /// （`sml-parse::scan` 的 `resolve_edition`）先报「未知版本集」，走不到这里。
    ///
    /// ```ignore
    /// // 1.0 允许裸词字符串；2.0 起要求显式引号
    /// assert!(FeatureSet::for_edition(Edition::new(1, 0)).has(Feature::BarewordStr));
    /// assert!(!FeatureSet::for_edition(Edition::new(3, 0)).has(Feature::BarewordStr));
    /// ```
    pub fn for_edition(e: Edition) -> FeatureSet {
        e.row().map(|r| r.features).unwrap_or_else(FeatureSet::baseline)
    }

    /// Whether feature `f` is enabled ｜ 是否包含某特性。
    pub const fn has(self, f: Feature) -> bool {
        (self.0 & (1 << (f as u8))) != 0
    }

    /// Copy with `f` turned **on** (takes self by value; `FeatureSet` is `Copy`) ｜ 返回开启 `f` 后的副本。
    pub const fn with(self, f: Feature) -> FeatureSet {
        FeatureSet(self.0 | (1 << (f as u8)))
    }

    /// Copy with `f` turned **off** ｜ 返回关闭 `f` 后的副本。
    pub const fn without(self, f: Feature) -> FeatureSet {
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
// 一行 = { 数字, 代号, 状态, **功能集** }（单一事实来源）：
// 代号映射与默认特性位同处登记，不可能漂。
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
/// 数字只是**名字**的一部分，不表示大小关系（不实现 `Ord`、不做 `>=` 比较）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Edition {
    pub major: u16,
    pub minor: u16,
}

/// 版本登记表的一行：数字 + 代号 + 状态 + **功能集** ｜ one row of the edition registry.
pub struct EditionRow {
    pub edition: Edition,
    /// 开发代号（如 `camelia`）；与数字**永久双向映射**。
    pub codename: Option<&'static str>,
    pub status: EditionStatus,
    /// 这一代的**默认特性集**（「版本 = 功能集」）。`Removed` 行留空即可 ——
    /// 该代根本走不到求特性集那一步（先在 `resolve_edition` 里报错）。
    pub features: FeatureSet,
}

/// 已登记的全部版本集（顺序即「代」的顺序，不参与比较）｜ the edition registry.
///
/// ⚠️ 新增一行即可；**不许改既有行的数字、代号或功能集**（已发布版本永久兼容）。
///
/// | 代 | 别名 | 默认差异 |
/// |---|---|---|
/// | `1.0` | `v1` / `1` / `10` | 允许裸词字符串 |
/// | `2.0` / `3.0` / `4.0` | `vN` / `N` / `N0` | 关掉裸词字符串（须引号）；三者特性位相同，差异在语法层 |
/// | `5.0`（`camelia`，Dev） | `v5` / `50` / `5.0` | 同上；开发期新能力一律 opt-in，定稿才固化进来 |
pub static EDITIONS: &[EditionRow] = &[
    EditionRow {
        edition: Edition::V1,
        codename: None,
        status: EditionStatus::Supported,
        features: FeatureSet::baseline(),
    },
    EditionRow {
        edition: Edition::V2,
        codename: None,
        status: EditionStatus::Supported,
        features: FeatureSet::baseline().without(Feature::BarewordStr),
    },
    EditionRow {
        edition: Edition::V3,
        codename: None,
        status: EditionStatus::Supported,
        features: FeatureSet::baseline().without(Feature::BarewordStr),
    },
    EditionRow {
        edition: Edition::V4,
        codename: None,
        status: EditionStatus::Supported,
        features: FeatureSet::baseline().without(Feature::BarewordStr),
    },
    EditionRow {
        edition: Edition::new(5, 0),
        codename: Some("camelia"),
        status: EditionStatus::Dev,
        features: FeatureSet::baseline().without(Feature::BarewordStr),
    },
];

impl Edition {
    /// 构造（const，便于在静态表与常量里用）｜ constructors usable in `const` context.
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// 旧枚举项保留为**常量**：`Version::V1` ⇔ `Edition::V1` ⇔ `1.0` ｜ legacy names as constants.
    ///
    /// 只做命名（便于旧代码与教程继续写 `Version::V1`），**不带任何序关系** ——
    /// 版本之间不可比较，判据永远查 [`FeatureSet::for_edition`]。
    pub const V1: Edition = Edition::new(1, 0);
    /// 见 [`Edition::V1`]；`2.0` 起要求字符串显式引号。
    pub const V2: Edition = Edition::new(2, 0);
    /// 见 [`Edition::V1`]；与 `2.0` 特性位相同，差异在语法层。
    pub const V3: Edition = Edition::new(3, 0);
    /// 见 [`Edition::V1`]；片段定义改显式 `type:` / `name:` 参数。
    pub const V4: Edition = Edition::new(4, 0);

    /// 最新**已发布**的代（`4.0`）—— 注意**不是**表里最新的那行：`5.0` 还在 Dev。
    ///
    /// 保留这个常量只为不打断既有调用方；新代码请直接写 [`Edition::V4`] 或具体数字。
    pub const CURRENT: Edition = Edition::V4;

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

    /// 是否要求字符串显式引号 ｜ whether strings must be quoted.
    ///
    /// **查表判据**：`true` ⇔ 该代默认功能集里**不含** `bareword-string`。
    /// 旧实现是 `self >= Version::V2` —— 版本之间没有大小关系，那种比较必须消失。
    pub fn strict_strings(&self) -> bool {
        !FeatureSet::for_edition(*self).has(Feature::BarewordStr)
    }
}

/// 旧名字：`Version` 是 [`Edition`] 的**类型别名**（改名零破坏，见 `docs/version-naming.md` V1）。
///
/// 保留它是因为「语法版本」这个词已经写进大量教程与文档；代码侧两者完全同一类型。
pub use Edition as Version;

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
    fn legacy_name_is_the_same_type() {
        // `Version` 只是别名 ⇒ 两边可以互相赋值、常量同名同值
        let v: Version = Edition::V4;
        assert_eq!(v, Edition::new(4, 0));
        assert_eq!(Version::V1, Edition::new(1, 0));
        assert_eq!(Version::CURRENT, Edition::V4);
    }

    #[test]
    fn feature_set_comes_from_the_registry_row() {
        // 1.0 允许裸词；2.0 起要求引号；5.0（Dev）与 2.0–4.0 同特性位
        assert!(FeatureSet::for_edition(Edition::V1).has(Feature::BarewordStr));
        for e in [Edition::V2, Edition::V3, Edition::V4, Edition::new(5, 0)] {
            assert!(
                !FeatureSet::for_edition(e).has(Feature::BarewordStr),
                "{} 应关闭裸词字符串",
                e
            );
        }
        // 未登记 ⇒ 回落 baseline（调用方通常更早报「未知版本集」）
        assert_eq!(FeatureSet::for_edition(Edition::new(9, 9)), FeatureSet::baseline());
        // 严格性判据改成查表，不再是 `>=`
        assert!(!Edition::V1.strict_strings());
        assert!(Edition::V3.strict_strings());
    }

    #[test]
    fn display_is_major_dot_minor() {
        assert_eq!(Edition::new(5, 0).to_string(), "5.0");
        assert_eq!(Edition::new(5, 1).to_string(), "5.1");
    }
}
