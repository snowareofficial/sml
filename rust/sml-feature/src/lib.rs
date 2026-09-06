// SPDX-License-Identifier: MulanPSL-2.0
//! SML 版本与特性系统 —— **纯数据层**，不依赖词法。
//!
//! 只定义三件事：
//! - [`Version`]：语法版本（`v1`..`v4`）；
//! - [`Feature`] / [`FeatureSet`]：能力位与位掩码集合；
//! - `FEATURES` / `feature_names`：名字 ↔ 位的注册表。
//!
//! 刻意**不**包含 `@version`/`@feature` 的**扫描**：那部分需要 `Tok`
//! （在 `sml-lex`），若放进来会形成 feature → lex → feature 的循环依赖，
//! 故置于 `sml-parse::scan`。

use std::fmt;

/// SML 语法版本
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
    /// 当前实现支持的最新版本
    pub const CURRENT: Version = Version::V4;

    /// 是否要求字符串显式引号（v2 / v3 为严格模式）
    pub fn strict_strings(self) -> bool {
        self >= Version::V2
    }

    /// 解析版本字面量（`v1`/`1`、`v2`/`2`、`v3`/`3`、`v4`/`4`）
    pub fn from_word(w: &str) -> Option<Version> {
        match w {
            "v1" | "1" => Some(Version::V1),
            "v2" | "2" => Some(Version::V2),
            "v3" | "3" => Some(Version::V3),
            "v4" | "4" => Some(Version::V4),
            _ => None,
        }
    }

    /// 版本名（用于错误信息与序列化回显）
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

/// 单个特性标识。与 [`FEATURES`] 表一一对应；改表即改全端。
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
}

/// 返回全部已注册特性的名字，顺序与 [`FEATURES`]（即特性位序）一致。
///
/// C-ABI 的 `sml_feature_name(bit)` 依赖此顺序，测试中有对应守护用例。
pub fn feature_names() -> Vec<&'static str> {
    FEATURES.iter().map(|(n, _)| *n).collect()
}

/// 特性名 → 枚举 的注册表。所有端共用同一组名字，保证跨语言一致。
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
];

impl Feature {
    /// 按名字查特性；未知名字返回 None（调用方据此报错，杜绝静默 typo）。
    pub fn from_name(name: &str) -> Option<Feature> {
        FEATURES.iter().find(|(n, _)| *n == name).map(|(_, f)| *f)
    }

    /// 特性名（用于报错 / 序列化回显）
    pub fn name(self) -> &'static str {
        FEATURES
            .iter()
            .find(|(_, f)| *f == self)
            .map(|(n, _)| *n)
            .unwrap_or("<unknown>")
    }
}

/// 位掩码形式的特性集合。
///
/// 设计哲学：从极简到丰富、功能可裁剪。默认基线（`baseline()`）只开极简三件套
/// （`include` + `namespace` + `implicit-ns`），复杂能力（多目标 / glob / 正则 /
/// 扩展名重写）必须显式 `@feature enable` 才生效，避免重蹈 YAML 过度复杂的覆辙。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeatureSet(u64);

impl FeatureSet {
    /// 全部特性位（含所有 opt-in 能力）。用于「调用方允许全集」与版本基线，
    /// 实际默认并不开启这些——见 [`FeatureSet::baseline`]。
    pub fn all() -> FeatureSet {
        let mut m = 0u64;
        for (_, f) in FEATURES {
            m |= 1 << (*f as u8);
        }
        FeatureSet(m)
    }

    /// 极简默认集（SML 核心可用能力）。这是 `parse_file` 的默认允许集；
    /// 仅「多目标 / glob / 正则 / 扩展名重写」等高级能力需文档内
    /// `@feature enable` 显式开启（避免重蹈 YAML 覆辙）。
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

    /// 空集合
    pub fn none() -> FeatureSet {
        FeatureSet(0)
    }

    /// 按版本基线构造默认特性集：v1 极简默认（baseline）+ 裸词字符串；
    /// v2/v3 关闭裸词字符串（须引号）。复杂能力（glob/regex/multi...）仍默认关闭，
    /// 需文档 `@feature enable` 显式开启。
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

    /// 是否包含某特性
    pub fn has(self, f: Feature) -> bool {
        (self.0 & (1 << (f as u8))) != 0
    }

    /// 返回开启 `f` 后的副本
    pub fn with(self, f: Feature) -> FeatureSet {
        FeatureSet(self.0 | (1 << (f as u8)))
    }

    /// 返回关闭 `f` 后的副本
    pub fn without(self, f: Feature) -> FeatureSet {
        FeatureSet(self.0 & !(1 << (f as u8)))
    }

    /// 与另一集合取交集（用于「文档裁剪 ∩ 调用方允许」）
    pub fn intersection(self, other: FeatureSet) -> FeatureSet {
        FeatureSet(self.0 & other.0)
    }

    /// 是否无任何特性
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

/// `@feature` 解析模式：白名单（仅启用列出的）/ 黑名单（禁用列出的）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureMode {
    Default,
    Whitelist,
    Blacklist,
}
