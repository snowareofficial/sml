// SPDX-License-Identifier: MulanPSL-2.0
//! # smlkit-easy
//!
//! 读一份 SML 只需要 **4 个概念**：文档 → 字段 → 值 → 错误。这个 crate 只暴露这 4 个。
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
//! **这里的根只有 3 个名字：`Doc`、`Value`、`Error`。**
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
//! | 契约校验、诊断、`@is` | [`sml`]（`parse_with_warnings` / `ParseOutput`） |
//! | 逐后端输出（md / html / svg / latex / slint…） | [`sml`] 的 `emit-*` |
//! | 特性开关、版本范围限制 | [`sml`] 的 `parse_with_features` / `parse_allowed` |
//! | 二进制持久化、查询 | 待 I-18（`.smlb` / `smldb`）落地 |
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

/// 一份已解析的 SML 文档。
///
/// 只持有值树，不持有原文 —— 需要「原样写回去」时用 [`Doc::to_sml`]（会重新排版，
/// 不保证与原文逐字节相同）。
#[derive(Debug, Clone)]
pub struct Doc {
    value: Value,
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
        sml::parse(text).map(|value| Self { value })
    }

    /// 解析一个文件，**并展开 `include`**（基准目录 = 该文件所在目录）。
    ///
    /// 与 [`Doc::read`] 的差别只有这一点：`read` 只解析你给的这段文本，
    /// 里面的 `include "x.sml"` 会原样留在值树里；`read_file` 会真的把 `x.sml` 并进来。
    pub fn read_file(path: impl AsRef<Path>) -> Result<Self, Error> {
        sml::parse_file(path).map(|value| Self { value })
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
