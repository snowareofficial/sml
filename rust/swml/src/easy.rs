// SPDX-License-Identifier: MulanPSL-2.0
//! # swml::easy
//!
//! Reading SML takes **four concepts**: document → field → value → error. This module exposes
//! only those four; diagnostics ([`Doc::diagnostics`]) and per-backend output ([`out`]) are the
//! two optional extras. ｜ 读一份 SML 只需要**四个概念**：文档 → 字段 → 值 → 错误。
//! 入门只暴露这四个；诊断与逐后端输出是可有可无的两个附加项。
//!
//! ```no_run
//! use swml::easy::Doc;
//!
//! let doc = Doc::read("server { host: localhost port: 8080 }")?;
//! assert_eq!(doc.str("server.host"), Some("localhost"));
//! assert_eq!(doc.int("server.port"), Some(8080));
//! # Ok::<(), swml::easy::Error>(())
//! ```
//!
//! ## Why a module, not a separate crate ｜ 为什么做成模块而不是独立 crate
//!
//! The beginner path lives **inside `swml`**, the facade crate. `swml` already re-exports all
//! of `swsml` at its root, so a separate crate would buy nothing but another version to keep
//! in sync — and the whole point of the facade is one install for both audiences.
//! ｜ 入门层放在**门面 crate `swml` 里**。`swml` 的根已经原样再导出了 `swsml`，
//! 再拆一个独立 crate 只会多一条要同步的版本线；门面的意义本来就是「一次安装，两种人用」。
//!
//! L0 只需四个概念：`Doc`、`Value`、`Error`、`Contracts`。
//! 完整公开面还包括 `pub mod out`（六个 emit 后端）与 `Doc` 上的访问器方法。
//! ｜ L0 needs four concepts; the full surface also has `pub mod out` and `Doc` accessors.
//!
//! > Naming trap worth remembering ｜ 值得记住的命名陷阱：the package is `swsml`, the lib is
//! > `sml` (crates.io already had `sml-rs`). So `Cargo.toml` says `swsml` while code says
//! > `use sml::…`. `swml/Cargo.toml` makes that explicit with `sml = { package = "swsml" }`.
//!
//! `Value` appears because [`Doc::list`] and [`Doc::raw`] return it; it is `swsml`'s value type
//! itself, **not** a wrapper. ｜ `Value` 出现是因为两个访问器的返回类型就是它；
//! 它是 `swsml` 的值类型本身，不是包装。
//!
//! ## What this layer deliberately omits ｜ 这一层故意不做什么
//!
//! | Not here ｜ 不做 | Use instead ｜ 想要就去用 |
//! |---|---|
//! | Contract **contents** (definitions, type checks, `contract_map` / `errors`) | [`sml`] (`parse_with` / `ParseOutput`) |
//! | Per-backend **knobs** (`math` / `standalone` / custom templates…) | [`sml::emit`] options |
//! | Feature switches, version allow-lists | [`sml`] `parse_with_features` / `parse_allowed` |
//! | Binary persistence, queries | I-18 (`.ambr` / `smldb`), still in progress |
//!
//! Three optional extras, one concept each ｜ 三个附加能力，每个只有一个概念：
//!
//! - [`Contracts`] — whether `@is <name>` inside the document is enforced ｜ 文档里的 `@is` 认不认；
//! - [`Doc::read_with_report`] + [`Doc::diagnostics`] — attach non-fatal diagnostics to the doc ｜
//!   把非致命诊断挂到文档上；
//! - [`out`] — one function per backend, gated by the matching `emit-*` feature: turning it off
//!   makes the function **not exist** rather than fail at runtime ｜ 关掉即函数不存在，不是运行时错误。
//!
//! Anything below that (apply contracts by name, `contract_map`, incremental emit) goes back to
//! [`sml`] ｜ 再往下就回专家层。
//!
//! ## Field paths ｜ 字段路径
//!
//! Accessors take **dot-separated paths** (`"db.host"`), same source as [`sml::Value::get`] ｜
//! 访问器接受 `.` 分隔的路径，与 `Value::get` 同源：
//!
//! - **Objects only** ｜ 只能穿对象：`items.0.name` never resolves (no index syntax in paths);
//!   pull the array out with [`Doc::list`] and index it yourself.
//! - A key that itself contains `.` is unreachable ｜ 键名自带 `.` 的字段取不到。
//! - Missing → `None` / `false`, **never an error** ｜ 取不到一律静默，用 [`Doc::raw`] 区分「不存在」与「类型不对」。

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
/// use swml::easy::{Contracts, Doc};
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
    /// let doc = swml::easy::Doc::read("name: demo tags: [ a b ]").unwrap();
    /// assert_eq!(doc.str("name"), Some("demo"));
    /// assert!(doc.has("tags"));
    /// ```
    ///
    /// 失败时拿到的是**带码**的错误（码是跨端契约，可直接用于判断分支）：
    ///
    /// ```
    /// let err = swml::easy::Doc::read("42").unwrap_err();
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
    /// use swml::easy::{Contracts, Doc};
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
    /// let doc = swml::easy::Doc::read_with_report("a: 1\na: 2").unwrap();
    /// assert_eq!(doc.diagnostics().len(), 1);        // 重复键被提升为数组，但不是错误
    /// assert!(doc.has("a"));
    /// ```
    ///
    /// 这一档用的是**默认契约开关**（等价 [`Contracts::On`]）。要「关掉文档内 `@is`」
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
    /// let doc = swml::easy::Doc::read("a: hi b: 1").unwrap();
    /// assert_eq!(doc.str("a"), Some("hi"));
    /// assert_eq!(doc.str("b"), None); // 是数字，不是字符串
    /// ```
    pub fn str(&self, path: &str) -> Option<&str> {
        self.value.get(path)?.as_str()
    }

    /// 取整数。**只认整数类型的值**，不做字符串到数字的转换。
    ///
    /// ```
    /// let doc = swml::easy::Doc::read("port: 8080 mode: 0755").unwrap();
    /// assert_eq!(doc.int("port"), Some(8080));
    /// // 前导零是**字符串**（保住权限位语义），所以这里取不到：
    /// assert_eq!(doc.int("mode"), None);
    /// ```
    ///
    /// 同一个坑还有更大的一种：超出 `i64` 的整数（`9223372036854775808`）也是**字符串**。
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
    /// let doc = swml::easy::Doc::read("tags: [ a b ]").unwrap();
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
    use super::Error;
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
