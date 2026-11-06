// SPDX-License-Identifier: MulanPSL-2.0
//! # smlkit-easy
//!
//! 读一份 SML 只需要 **4 个概念**：文档 → 字段 → 值 → 错误。入门（L0 / L1）只暴露这 4 个；
//! 要诊断和逐后端输出再多两个：诊断（[`Doc::diagnostics`]）与输出（[`out`]）。
//!
//! ```no_run
//! // ⚠️ 本 crate 尚未发布，下面是发布后的用法；现在试跑请看本仓库的 `tests/quickstart.rs`。
//! use smlkit_easy::Doc;
//!
//! let doc = Doc::read("server { host: localhost port: 8080 }")?;
//! assert_eq!(doc.str("server.host"), Some("localhost"));
//! assert_eq!(doc.int("server.port"), Some(8080));
//! # Ok::<(), smlkit_easy::Error>(())
//! ```
//!
//! ## 为什么单独一个 crate
//!
//! `swsml` 的根命名空间有 **90 多个**名字（13 个 `parse_*`、31 个 C-ABI 函数、include 的
//! 内部件、三个 `MAX_*`…）。那是给「要把 SML 嵌进自己产品、需要控制每个旋钮」的人准备的；
//! 只想读个配置的人不该先读完那些。
//!
//! 所以这一层**不在 `swsml` 里做成模块**：模块解决不了「`use sml::*` 时把 31 个 C 函数
//! 一起拉进命名空间」这种事，而且会继续挤占根命名空间。独立 crate 才能做到 ——
//! **这里的根只有 4 个名字：`Doc`、`Value`、`Error`、`Contracts`。**
//!
//! > 顺带记下本仓最容易踩的命名陷阱：**包名是 `swsml`、库名是 `sml`**
//! > （crates.io 上 `sml-rs` 被无关项目占了，只能换包名）。
//! > 于是 `Cargo.toml` 里写 `swsml = "0.6"`，代码里却要 `use sml::…`。
//! > 本 crate 的 `Cargo.toml` 用 `sml = { package = "swsml", … }` 把这件事显式化了。
//!
//! `Value` 出现是因为 [`Doc::list`] 与 [`Doc::raw`] 的返回类型就是它（否则调用方
//! 连类型都写不出来）；它是 swsml 的值类型本身，**不是**本 crate 另造的一层包装。
//!
//! ## 这一层故意不做什么
//!
//! | 不做 | 想要就去用 |
//! |---|---|
//! | 契约**内容**（定义、类型校验实现、`contract_map` / `errors` 专家字段） | [`sml`]（`parse_with` / `ParseOutput`） |
//! | 逐后端输出的**旋钮**（`math` / `standalone` / 自定义模板…） | [`sml::emit`] 的 Options |
//! | 特性开关、版本范围限制 | [`sml`] 的 `parse_with_features` / `parse_allowed` |
//! | 二进制持久化、查询 | 待 I-18（`.ambr` / `smldb`）落地 |
//!
//! 本层给的三个「进阶」能力，每个都只有一个概念：
//!
//! - [`Contracts`]：文档里写 `@is <契约名>` 时认不认；
//! - [`Doc::read_with_report`] + [`Doc::diagnostics`]：把非致命诊断挂到文档上；
//! - [`out`]：一个后端一枚函数（`out::markdown` / `out::html` / `out::latex` …），
//!   各由同名 `emit-*` feature 门控 —— 关掉就**整个不存在**，不是运行时错误。
//!
//! 再往下（按名字应用契约、`contract_map`、增量 emit）就回 [`sml`]。
//!
//! ## 字段路径
//!
//! 访问器接受 **`.` 分隔的路径**（`"db.host"`），与 [`sml::Value::get`] 同源：
//!
//! - **只能穿对象**：`items.0.name` 这类**永远取不到**（路径语法里没有下标），
//!   数组先用 [`Doc::list`] 取出来再自己索引；
//! - **键名自带 `.` 的字段取不到**（分段后每段当完整键名匹配）；
//! - 取不到一律 `None` / `false`，**不报错** —— 想区分「不存在」和「值不对」时，
//!   那已经超出这一层的范围，请用 [`Doc::raw`] 回去看 [`sml::Value`]。

use std::path::Path;

/// 统一错误类型：就是 [`sml::SmlError`]，这里只换个短名字。
///
/// 换名而不换类型是刻意的 —— [`Error::code`]（原 `SmlError::code`）返回的错误码
/// （`E-PARSE-006` 之类）是**跨五端稳定**的契约，包一层自己的错误类型就会把它丢掉。
pub use sml::SmlError as Error;

/// 值类型：**就是 [`sml::Value`]**，不是本 crate 另造的包装。
///
/// 之所以要再导出： [`Doc::list`] 返回 `&[Value]`、[`Doc::raw`] 返回 `&Value`，
/// 调用方得能写出这个类型名。
pub use sml::Value;

/// 文档内 `@is` 契约的开关 —— 本 crate 唯一的解析开关。
///
/// 默认 [`Contracts::On`]，与 [`Doc::read`] / [`Doc::read_file`] 一致；
/// 只有想「用自己的结构体当契约来源」时才需要 [`Contracts::Off`]。
///
/// ```
/// use smlkit_easy::{Contracts, Doc};
///
/// // 文档里引用了不存在的契约：默认直接报错
/// let err = Doc::read("@version v1\ndoc {\n  @is NoSuch\n  x: 1\n}\n").unwrap_err();
/// assert_eq!(err.code(), "E-CONTRACT-001");
///
/// // 关掉后放行，且不回填契约里的默认值
/// let doc = Doc::read_with("@version v1\ndoc {\n  @is NoSuch\n  x: 1\n}\n", Contracts::Off).unwrap();
/// assert_eq!(doc.int("doc.x"), Some(1));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Contracts {
    /// 生效（默认）：校验类型、回填默认值，未知契约报 `E-CONTRACT-001`。
    #[default]
    On,
    /// 忽略：不校验、不填默认值、未知契约也不报错。
    ///
    /// `@contract` 定义本身仍照常收集 —— 它**两种模式都不进结果树**。
    Off,
}

impl Contracts {
    /// 折成 `ParseOptions::apply_contracts` 的布尔值。
    fn enabled(self) -> bool {
        matches!(self, Contracts::On)
    }

    /// 本层唯一会设置的选项。
    fn options(self) -> sml::ParseOptions {
        sml::ParseOptions::new().apply_contracts(self.enabled())
    }
}

/// 一份已解析的 SML 文档。
///
/// 只持有值树（外加诊断，见 [`Doc::diagnostics`]），不持有原文 —— 需要「原样写回去」时
/// 用 [`Doc::to_sml`]（会重新排版，不保证与原文逐字节相同）。
#[derive(Debug, Clone)]
pub struct Doc {
    value: Value,
    /// 非致命诊断。只有 `*_with_report` 入口会填；其余入口恒为空。
    diagnostics: Vec<sml::Diagnostic>,
}

impl Doc {
    /// 解析一段 SML 文本。
    ///
    /// ```
    /// let doc = smlkit_easy::Doc::read("name: demo tags: [ a b ]").unwrap();
    /// assert_eq!(doc.str("name"), Some("demo"));
    /// assert!(doc.has("tags"));
    /// ```
    ///
    /// 失败时拿到的是**带码**的错误（码是跨端契约，可直接用于判断分支）：
    ///
    /// ```
    /// let err = smlkit_easy::Doc::read("42").unwrap_err();
    /// assert_eq!(err.code(), "E-PARSE-008"); // 顶层须为容器，单个标量无法往返
    /// ```
    pub fn read(text: &str) -> Result<Self, Error> {
        sml::parse(text).map(|value| Self {
            value,
            diagnostics: Vec::new(),
        })
    }

    /// 解析一个文件，**并展开 `include`**（基准目录 = 该文件所在目录）。
    ///
    /// 与 [`Doc::read`] 的差别只有这一点：`read` 只解析你给的这段文本，
    /// 里面的 `include "x.sml"` 会原样留在值树里；`read_file` 会真的把 `x.sml` 并进来。
    pub fn read_file(path: impl AsRef<Path>) -> Result<Self, Error> {
        sml::parse_file(path).map(|value| Self {
            value,
            diagnostics: Vec::new(),
        })
    }

    /// 解析文本，并显式指定文档内 `@is` 契约是否生效。
    ///
    /// [`Doc::read`] 等价于 `read_with(text, Contracts::On)`；这里只为把开关
    /// 摆到明面上（典型场景：契约由调用方自己的结构体提供，文件里的 `@is` 应当忽略）。
    ///
    /// ```
    /// use smlkit_easy::{Contracts, Doc};
    ///
    /// let text = "@version v1\n@contract P { n: str role: str default guest }\ndoc {\n  @is P\n  n: 42\n}\n";
    /// // 契约开启：类型不符直接报错
    /// assert!(Doc::read(text).is_err());
    /// // 契约关闭：放行，且**不回填** `role` 的默认值
    /// let doc = Doc::read_with(text, Contracts::Off).unwrap();
    /// assert_eq!(doc.int("doc.n"), Some(42));
    /// assert!(!doc.has("doc.role"));
    /// ```
    pub fn read_with(text: &str, contracts: Contracts) -> Result<Self, Error> {
        sml::parse_with(text, contracts.options()).map(|out| Self {
            value: out.value,
            diagnostics: out.diagnostics,
        })
    }

    /// 解析文件（**会展开 `include`**），并显式指定文档内 `@is` 契约是否生效。
    ///
    /// 这是本层唯一同时做到「展开 include」与「关掉文档内契约」的入口 ——
    /// [`Doc::read_file`] 关不掉，[`Doc::read_with`] 不展开 include。
    pub fn read_file_with(path: impl AsRef<Path>, contracts: Contracts) -> Result<Self, Error> {
        sml::parse_file_with(path, contracts.options()).map(|out| Self {
            value: out.value,
            diagnostics: out.diagnostics,
        })
    }

    /// 解析文本，**并保留诊断**（重复键、片段覆盖等非致命问题）。
    ///
    /// 结果仍是 [`Doc`] —— 诊断挂在文档上，用 [`Doc::diagnostics`] 取；
    /// 没有另造 `Report` 类型，复用的就是 [`sml::ParseOutput`] 的 `diagnostics` 字段。
    ///
    /// ```
    /// let doc = smlkit_easy::Doc::read_with_report("a: 1\na: 2").unwrap();
    /// assert_eq!(doc.diagnostics().len(), 1);        // 重复键被提升为数组，但不是错误
    /// assert!(doc.has("a"));
    /// ```
    ///
    /// ⚠️ 这一档用的是**默认契约开关**（等价 [`Contracts::On`]）。要「关掉文档内 `@is`」
    /// 又「拿诊断」，请回 [`sml::parse_with`]（专家层，`ParseOutput` 三个字段都在）。
    pub fn read_with_report(text: &str) -> Result<Self, Error> {
        sml::parse_with_warnings(text).map(|(value, diagnostics)| Self {
            value,
            diagnostics,
        })
    }

    /// 解析文件（**会展开 `include`**），并保留诊断。语义同 [`Doc::read_with_report`]。
    pub fn read_file_with_report(path: impl AsRef<Path>) -> Result<Self, Error> {
        sml::parse_file_with_warnings(path).map(|(value, diagnostics)| Self {
            value,
            diagnostics,
        })
    }

    /// 解析期产生的**非致命诊断**（空切片表示没有任何警告）。
    ///
    /// 只有 [`Doc::read_with_report`] / [`Doc::read_file_with_report`] / [`Doc::read_with`] /
    /// [`Doc::read_file_with`] 会填；[`Doc::read`] / [`Doc::read_file`] 恒为空
    /// （它们走的是丢掉诊断的既有入口）。
    pub fn diagnostics(&self) -> &[sml::Diagnostic] {
        &self.diagnostics
    }

    /// 是否有任何诊断（等价 `!diagnostics().is_empty()`）。
    pub fn has_diagnostics(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    /// 逃生口：拿到底层的 [`sml::Value`]。
    ///
    /// 这一层只覆盖「读字段」；遇到数组遍历、类型判断、`describe_value` 之类，
    /// 直接从这儿下去，不必重新 `parse` 一遍。
    pub fn raw(&self) -> &Value {
        &self.value
    }

    /// 该路径是否存在（**不判断类型**：存在但是块/数组也返回 `true`）。
    pub fn has(&self, path: &str) -> bool {
        self.value.get(path).is_some()
    }

    /// 取字符串。值不是字符串时返回 `None`。
    ///
    /// ```
    /// let doc = smlkit_easy::Doc::read("a: hi b: 1").unwrap();
    /// assert_eq!(doc.str("a"), Some("hi"));
    /// assert_eq!(doc.str("b"), None); // 是数字，不是字符串
    /// ```
    pub fn str(&self, path: &str) -> Option<&str> {
        self.value.get(path)?.as_str()
    }

    /// 取整数。**只认整数类型的值**，不做字符串到数字的转换。
    ///
    /// ```
    /// let doc = smlkit_easy::Doc::read("port: 8080 mode: 0755").unwrap();
    /// assert_eq!(doc.int("port"), Some(8080));
    /// // ⚠️ 前导零是**字符串**（保住权限位语义），所以这里取不到：
    /// assert_eq!(doc.int("mode"), None);
    /// ```
    ///
    /// ⚠️ 同一个坑还有更大的一种：超出 `i64` 的整数（`9223372036854775808`）也是**字符串**。
    /// 判类型请以本函数的返回值为准，别假设「看起来是数字就是整数」。
    pub fn int(&self, path: &str) -> Option<i64> {
        self.value.get(path)?.as_i64()
    }

    /// 取浮点。整数能被安全转成 `f64` 时也会返回（`1` ⇒ `Some(1.0)`）。
    pub fn float(&self, path: &str) -> Option<f64> {
        self.value.get(path)?.as_float()
    }

    /// 取布尔。
    pub fn bool(&self, path: &str) -> Option<bool> {
        self.value.get(path)?.as_bool()
    }

    /// 取数组（**只读切片**）。想要下标访问就从这里拿：
    ///
    /// ```
    /// let doc = smlkit_easy::Doc::read("tags: [ a b ]").unwrap();
    /// let tags = doc.list("tags").unwrap();
    /// assert_eq!(tags.len(), 2);
    /// assert_eq!(tags[1].as_str(), Some("b"));
    /// ```
    ///
    /// 注意：路径语法里**没有下标**（`"tags.0"` 取不到），数组只能这样取出来再自己索引。
    pub fn list(&self, path: &str) -> Option<&[Value]> {
        Some(self.value.get(path)?.as_array()?.as_slice())
    }

    /// 重新序列化为 SML 文本（会重新排版，不保证与原文逐字节相同）。
    ///
    /// 值超深（超过 [`sml::MAX_VALUE_DEPTH`] 层）时**不报错**，而是写出占位文本 ——
    /// 要失败语义请用 [`sml::to_sml_checked`]。
    pub fn to_sml(&self) -> String {
        sml::to_sml(&self.value)
    }
}

impl From<Doc> for Value {
    fn from(doc: Doc) -> Self {
        doc.value
    }
}

impl std::fmt::Display for Doc {
    /// 与 [`Doc::to_sml`] 同一个输出。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_sml())
    }
}

/// 逐后端输出（L2）：一个后端一枚函数，各由**同名 feature** 门控。
///
/// 关掉某个 `emit-*` feature ⇒ 对应函数**整个不存在**（编译期就不在），
/// 而不是运行时返回错误 —— 这样关闭后端也不必为它付出构建成本。
///
/// 每枚函数都是薄转发，用的都是该后端的 `Default` 选项；要调 `math` / `standalone`
/// 这类旋钮，请直接去 [`sml::emit`]（专家层）。
pub mod out {
    // 关掉全部 emit 后端时本模块为空（连 import 都不需要），故这两个 use 也按同一批 feature 门控。
    #[cfg(any(
        feature = "emit-markdown",
        feature = "emit-html",
        feature = "emit-latex",
        feature = "emit-xml",
        feature = "emit-svg",
        feature = "emit-slint"
    ))]
    use crate::Error;
    #[cfg(any(
        feature = "emit-markdown",
        feature = "emit-html",
        feature = "emit-latex",
        feature = "emit-xml",
        feature = "emit-svg",
        feature = "emit-slint"
    ))]
    use super::Doc;

    /// Markdown 文本（`emit-markdown`）。
    #[cfg(feature = "emit-markdown")]
    pub fn markdown(doc: &Doc) -> Result<String, Error> {
        sml::emit::to_markdown(doc.raw(), &Default::default())
    }

    /// 独立 HTML 文档（`emit-html`）。
    #[cfg(feature = "emit-html")]
    pub fn html(doc: &Doc) -> Result<String, Error> {
        sml::emit::to_html(doc.raw(), &Default::default())
    }

    /// LaTeX 文档（`emit-latex`）。
    #[cfg(feature = "emit-latex")]
    pub fn latex(doc: &Doc) -> Result<String, Error> {
        sml::emit::to_latex(doc.raw(), &Default::default())
    }

    /// XML（`emit-xml`）。
    #[cfg(feature = "emit-xml")]
    pub fn xml(doc: &Doc) -> Result<String, Error> {
        sml::emit::to_xml(doc.raw(), &Default::default())
    }

    /// SVG（`emit-svg`）。
    #[cfg(feature = "emit-svg")]
    pub fn svg(doc: &Doc) -> Result<String, Error> {
        sml::emit::to_svg(doc.raw(), &Default::default())
    }

    /// Slint 界面代码（`emit-slint`）。
    #[cfg(feature = "emit-slint")]
    pub fn slint(doc: &Doc) -> Result<String, Error> {
        sml::emit::to_slint(doc.raw(), &Default::default())
    }
}
