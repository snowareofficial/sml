// SPDX-License-Identifier: MulanPSL-2.0
//! Public parsing API for SML (`parse` / `parse_file` / `loads` etc.) ｜ SML 解析的**公开 API**。
//!
//! 与 [`crate::parser`] 分开：后者是内部实现，这里只做入口组装
//! （特性求取 → 指令剥离 → tokenize → 解析 → include 展开）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sml_codes::{E_FEATURE_001, E_FEATURE_004, E_FEATURE_009, E_IO_001, E_PARSE_008, SmlError};
use sml_feature::{Edition, Feature, FeatureSet};
use sml_include::resolve_includes_spans;
use sml_lex::{Span, Tok, tokenize_spans};
use sml_value::Value;

use crate::contract_map::ContractMap;
use crate::error::ParseError;
use crate::ext::{Diagnostic, ParseOptions, ParseOutput};
use crate::parser::Parser;
use crate::scan::{features_for, strip_features, strip_version};

/// 「正文里不可能有任何 `@` 指令」的**保守**判据（IDEAS I-23 的快路径闸门）。
///
/// 三条缺一不可：
/// - 不以 BOM 开头 —— `strip_version` 除 `@version` 外**只做 BOM 归一化**，跳过它会把
///   BOM 喂进词法器，首个键名被静默改掉；
/// - 不以 `\` 结尾 —— 慢路径「按行重建」会在文件尾补换行，使 `k: "abc\` 报 `E-LEX-004`
///   而不是词法层的 `E-LEX-006`；那是**已登记的跨端「同因不同码」**（见 `TODO.md` §三·十二 H 组），
///   不随性能优化一起改；
/// - 不含 `@` 字节 —— 有 `@` 才可能含 `@version` / `@feature` / `@指令`。
///   `@` 出现在引号串或注释里也会退回慢路径：只多一次 memchr，换「宁慢不误」。
#[inline]
fn no_directives(text: &str) -> bool {
    !text.starts_with('\u{feff}') && !text.ends_with('\\') && !text.as_bytes().contains(&b'@')
}

/// `E-FEATURE-004`：文档声明的版本不在调用方给出的白名单里。
///
/// 抽成函数是为了让 [`parse_allowed`] 的**快路径与慢路径文案逐字一致** ——
/// 两处各写一份 format!，改文案时必漏一处。
fn version_not_allowed(v: Edition, allowed: &[Edition]) -> SmlError {
    SmlError::from_code_title(E_FEATURE_004).with_detail(format!(
        "文档声明版本 {} 不在本库接受的版本范围 {{{}}} 内",
        v.name(),
        allowed
            .iter()
            .map(|x| x.name())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// `E-FEATURE-009`：文档所求的特性集与调用方白名单**无交集**。
fn feature_set_empty(feats: &FeatureSet, allowed: &FeatureSet) -> SmlError {
    SmlError::from_code_msg(E_FEATURE_009).with_detail(format!("{feats} 与 {allowed} 无交集"))
}

/// 两个「特性受限」入口的快路径尾段：求交集 → 空则报错 → 解析。
///
/// 抽出来是为了让 [`parse_with_features`] 与 [`parse_with_features_env`] 的快路径
/// **不会各写一份**（它们的差别只有 `env` 表）。
fn with_effective(
    text: &str,
    feats: FeatureSet,
    allowed: FeatureSet,
    env: BTreeMap<String, String>,
) -> Result<(Value, FeatureSet), SmlError> {
    let effective = feats.intersection(allowed);
    if effective.is_empty() {
        return Err(feature_set_empty(&feats, &allowed));
    }
    Ok((parse_impl(text, effective, env)?, effective))
}

/// Parse SML text and return its declared edition ｜ 解析 SML 文本，并返回其声明的**版本集**。
///
/// 未声明版本时按 `1.0` 处理（裸词即字符串），**既有文档不受影响**；
/// 显式 `@version v3`（等价 `30` / `3.0`）则返回 `3.0`（此时字符串需引号）。
pub fn parse_versioned(text: &str) -> Result<(Value, Edition), SmlError> {
    // 快路径同上（`no_directives` 已排除 BOM 与以 `\` 结尾的输入）
    if no_directives(text) {
        return Ok((
            parse_impl(text, FeatureSet::for_edition(Edition::V1), BTreeMap::new())?,
            Edition::V1,
        ));
    }
    let (rest, declared) = strip_version(text)?;
    let (rest, feats, touched, mode, base, had) = strip_features(&rest)?;
    // 版本优先级：@version 显式声明 > @feature base > 默认 1.0
    let v = declared.or(base).unwrap_or(Edition::V1);
    let feats = features_for(v, feats, touched, mode, had);
    Ok((parse_impl(&rest, feats, BTreeMap::new())?, v))
}

/// Parse an SML file (expand includes) and return its declared edition ｜ 解析 SML 文件：展开 include，并返回其声明的**版本集**
pub fn parse_file_versioned(path: impl AsRef<Path>) -> Result<(Value, Edition), SmlError> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path).map_err(|e| {
        SmlError::from_code_msg(E_IO_001).with_detail(format!("{}: {e}", path.display()))
    })?;
    let base = path
        .parent()
        .map(|p| p.to_path_buf())
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("."));
    let (rest, declared) = strip_version(&text)?;
    let (rest, feats, touched, mode, base_ver, had) = strip_features(&rest)?;
    let allowed = FeatureSet::all().intersection(feats);
    let v = declared.or(base_ver).unwrap_or(Edition::V1);
    let feats = features_for(v, allowed, touched, mode, had);
    let (toks, spans) = resolve_includes_spans(&rest, &base, allowed)?;
    let spans = spans.unwrap_or_default();
    // 无 include ⇒ token 与 `rest` 一一对应，可定位行列；有 include ⇒ 跨文件合并、
    // 无单一原文，`spans` 为空 ⇒ 解析器退回「不带位置」（宁缺勿错）。
    let lines = if spans.is_empty() { Vec::new() } else { token_lines(&rest, &spans) };
    let val = parse_impl_tokens(toks, spans, lines, feats, BTreeMap::new())
        .map_err(|e| with_position(&rest, e))?;
    Ok((val, v))
}

/// Parse an SML file (expand includes) and collect the non-fatal diagnostics ｜ 解析 SML 文件（展开 include），并把**非致命诊断**（键冲突告警）一并交出来。
///
/// 为什么除了 [`parse_with_warnings`] 还要单给一个文件入口：`KeyOverwritten`
/// 那类告警（片段展开覆盖父块已有的键）几乎只发生在真实文档里，而片段又常常
/// 定义在被 include 的子文件中 —— 只有文本入口会漏掉最典型的那一类。
///
/// ⚠️ 文件入口**无 include** 时 token 与原文一一对应 ⇒ 诊断带行号；**有 include** 时
/// token 跨文件合并、没有可映射的单一原文 ⇒ `line` 为 `None`（文案末尾会指引改用
/// `smltools --lint` 拿行号）。
pub fn parse_file_with_warnings(
    path: impl AsRef<Path>,
) -> Result<(Value, Vec<Diagnostic>), SmlError> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path).map_err(|e| {
        SmlError::from_code_msg(E_IO_001).with_detail(format!("{}: {e}", path.display()))
    })?;
    let base = path
        .parent()
        .map(|p| p.to_path_buf())
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("."));
    let (rest, declared) = strip_version(&text)?;
    let (rest, feats, touched, mode, base_ver, had) = strip_features(&rest)?;
    let allowed = FeatureSet::all().intersection(feats);
    let v = declared.or(base_ver).unwrap_or(Edition::V1);
    let feats = features_for(v, allowed, touched, mode, had);
    let (toks, spans) = resolve_includes_spans(&rest, &base, allowed)?;
    let spans = spans.unwrap_or_default();
    let lines = if spans.is_empty() { Vec::new() } else { token_lines(&rest, &spans) };
    let out = parse_impl_tokens_ext(
        toks,
        spans,
        lines,
        feats,
        BTreeMap::new(),
        ParseOptions::new(),
    )
    .map_err(|e| with_position(&rest, e))?;
    Ok((out.value, out.diagnostics))
}

/// 文件入口 + 调用方选项（**展开 include**，与 [`parse_with`] 的文件版对应）｜ file entry with [`ParseOptions`]
///
/// 存在的理由：此前「**文件**」与「**选项**」互斥 —— [`parse_file`] 一族内部把
/// [`ParseOptions::new`] 写死、不接受调用方选项；而 [`parse_with`] 虽收选项却只走
/// `tokenize_spans`、**不展开 include**。于是「读文件（含 include）+ 关掉文档内契约」
/// 这种组合在旧入口下**无路可走**。本入口把两者接上：
///
/// - 相对路径与 include 展开基准 = 该文件自身所在目录（与 [`parse_file`] 同）；
/// - 版本/特性声明照旧从主文件剥离，最终允许集 = `FeatureSet::all() ∩ 文档声明`；
/// - 传入 [`ParseOptions::new`] 时，`value` 与 [`parse_file`] 逐字一致，差别只在
///   额外返回 `diagnostics` / `contract_map` / `errors`（与 [`parse_with`] 同一返回类型）；
/// - 诊断的 `line` 规则同 [`parse_file_with_warnings`]：无 include 时带行号，有 include
///   时为 `None`（跨文件无单一原文，宁缺勿错）。
pub fn parse_file_with(
    path: impl AsRef<Path>,
    opts: ParseOptions,
) -> Result<ParseOutput, SmlError> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path).map_err(|e| {
        SmlError::from_code_msg(E_IO_001).with_detail(format!("{}: {e}", path.display()))
    })?;
    let base = path
        .parent()
        .map(|p| p.to_path_buf())
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("."));
    let (rest, declared) = strip_version(&text)?;
    let (rest, feats, touched, mode, base_ver, had) = strip_features(&rest)?;
    let allowed = FeatureSet::all().intersection(feats);
    let v = declared.or(base_ver).unwrap_or(Edition::V1);
    let feats = features_for(v, allowed, touched, mode, had);
    let (toks, spans) = resolve_includes_spans(&rest, &base, allowed)?;
    let spans = spans.unwrap_or_default();
    let lines = if spans.is_empty() { Vec::new() } else { token_lines(&rest, &spans) };
    parse_impl_tokens_ext(toks, spans, lines, feats, BTreeMap::new(), opts)
        .map_err(|e| with_position(&rest, e))
}

/// Parse SML text ｜ 解析 SML 文本
///
/// 会自动识别并剥离 `@version` / `@feature` 声明（需要版本信息时用
/// [`parse_versioned`]，需要特性裁剪信息时用 [`parse_with_features`]）。
///
/// **向后兼容**：未声明 `@version` 的文档按 `V1` 解析（裸词即字符串），
/// 既有大量 v1 文档不受影响；仅显式 `@version v2|v3` 才启用严格字符串。
pub fn parse(text: &str) -> Result<Value, SmlError> {
    // 快路径（IDEAS I-23「更激进的性能优化」的第一处 A/B）：
    // **没有 `@` 的文档不可能含 `@version` / `@feature`**，于是「去 BOM → 两趟剥离 → 特性求交」
    // 整段都可以跳过，直接用 v1 基线 —— 与走完整链路的结果**逐字节相同**：
    // - `strip_version` 除 `@version` 外只做 BOM 归一化 ⇒ 下面单独把 BOM 排除在快路径外；
    // - `strip_features` 的产物在 `had_feature == false` 时被 `features_for` 直接丢弃，
    //   返回 `FeatureSet::for_edition(1.0)`（见 `scan::features_for` 首个分支）；
    // - `rest` 与原文等长且无指令时内容相同，故位置信息不受影响。
    //
    // 判据见 [`no_directives`]（保守三条件）。命中时**连 `rest` 的全文拷贝一起省掉** ——
    // 慢路径的 `strip_version` / `strip_features` 各自产出一个新的 `String`。
    if no_directives(text) {
        return parse_impl(text, FeatureSet::for_edition(Edition::V1), BTreeMap::new());
    }
    let (rest, declared) = strip_version(text)?;
    let (rest, feats, touched, mode, base, had) = strip_features(&rest)?;
    let v = declared.or(base).unwrap_or(Edition::V1);
    let feats = features_for(v, feats, touched, mode, had);
    parse_impl(&rest, feats, BTreeMap::new())
}

/// Parse SML text with external directive extensions ｜ 带**外置扩展**的解析入口（扩展点见 [`crate::ext`]）。
///
/// 与 [`parse`] 的唯一差别：可传入 [`ParseOptions`]，让下游注册的自定义 `@指令` 生效，
/// 并额外返回非致命诊断（如「位置参数形式已废弃」）。
/// 传入空选项（[`ParseOptions::new`]）时，**解析结果与 [`parse`] 完全一致** ——
/// 差别只在本入口额外返回 `diagnostics`（含键冲突告警，见
/// [`parse_with_warnings`]）；`parse` 那条路把诊断丢掉了。
///
/// 用途：下游要挂「带类型的元数据块，且不进主数据树」（表单描述 / 处理流 / 文章块等）
/// 时，**不必 fork 解析器、也不必把方言名字写进 SML 规范层** —— 方言留在下游仓库。
///
/// ```ignore
/// use sml_parse::ext::{Directive, Outcome, ParseOptions};
/// use sml_value::Value;
///
/// struct Form; // 收集 `@form ... { }` 这类元数据块，不进主树
/// impl Directive for Form {
///     fn name(&self) -> &str { "form" }
///     fn positional(&self) -> bool { true } // 兼容既有的 `@form Name { }` 写法
///     fn call(&self, _arg: Option<&str>, _body: Value) -> Result<Outcome, String> {
///         Ok(Outcome::Discard)
///     }
/// }
///
/// let out = sml_parse::parse_with("name: x\n@form F { a: 1 }\n", ParseOptions::new().directive(Form)?)?;
/// assert_eq!(out.value.get("name"), Some(&Value::Str("x".into())));
/// # Ok::<(), String>(())
/// ```
/// # 语义：只做「文本 → 值」，**不碰文件系统**
///
/// 本函数**不展开 `include`** —— 只有 `parse_file*` 一族会调 `resolve_includes_spans`，
/// 本函数走的是 `tokenize_spans`。所以把
/// `std::fs::read_to_string(p)?` 的结果直接喂进来，等于**悄悄关掉了模块化**。
///
/// 四个入口的分界（最容易搞错的一处）：
///
/// | 入口 | 收 `ParseOptions` | 展开 `include` |
/// |---|---|---|
/// | [`parse`] / [`parse_with_warnings`] | 否 / 否 | 否 |
/// | 本函数 | **是** | 否 |
/// | [`parse_file`] / [`parse_file_with_warnings`] | 否 | 是 |
/// | [`parse_file_with`] | **是** | 是 |
///
/// 选型一句话：**手上是字符串用 `parse_with`，手上是路径用 `parse_file_with`**；
/// 「要选项」和「要 include」必须同时要时，只有 [`parse_file_with`] 一条路。
///
/// # 例子
///
/// ```
/// use sml_parse::{parse_with, ParseOptions};
///
/// let out = parse_with("db { host: db.internal }\n", ParseOptions::new()).unwrap();
/// assert_eq!(out.value.get("db.host").and_then(|v| v.as_str()), Some("db.internal"));
/// ```
///
/// # 陷阱
///
/// ⚠️ **你以为「传空选项就等于 `parse`，随便用哪个都行」—— 其实 [`parse`] 把诊断丢了**。
/// 键冲突（`DuplicateKey`：重复键已**提升为数组**）与片段覆盖（`KeyOverwritten`：父块原值
/// **已丢失**）都会改写数据，而 `parse` 的签名 `Result<Value, _>` 承载不了这些告警
/// （见 [`crate::ext::Diagnostic`]）。生产路径请走本函数或 [`parse_with_warnings`]。
pub fn parse_with(text: &str, opts: ParseOptions) -> Result<ParseOutput, SmlError> {
    // 快路径（见 [`no_directives`]）：省掉两趟全文剥离与 `rest` 的整串拷贝。
    if no_directives(text) {
        return parse_ext(text, FeatureSet::for_edition(Edition::V1), opts);
    }
    let (rest, declared) = strip_version(text)?;
    let (rest, feats, touched, mode, base, had) = strip_features(&rest)?;
    let v = declared.or(base).unwrap_or(Edition::V1);
    let feats = features_for(v, feats, touched, mode, had);
    let (toks, spans) = tokenize_spans(&rest)?;
    let lines = token_lines(&rest, &spans);
    parse_impl_tokens_ext(toks, spans, lines, feats, BTreeMap::new(), opts)
        .map_err(|e| with_position(&rest, e))
}

/// 「词法 → 行号 → 带扩展选项解析」这条固定链（三个入口共用，避免抄三份）。
fn parse_ext(text: &str, feats: FeatureSet, opts: ParseOptions) -> Result<ParseOutput, SmlError> {
    let (toks, spans) = tokenize_spans(text)?;
    let lines = token_lines(text, &spans);
    parse_impl_tokens_ext(toks, spans, lines, feats, BTreeMap::new(), opts)
        .map_err(|e| with_position(text, e))
}

/// Parse into **explicitly source-ordered objects** (方案 2 / B).
/// 解析成**显式保源序**的值：所有对象都是 `Map::Ordered`，可直接交给
/// `smlb::dump` / `canonical_text` / `canonical_bytes`，**无需任何 feature**。
///
/// 与 [`parse`] 的差别只有对象载体：`parse()` 保持历史字节序（BTreeMap 排序），
/// 本函数保留源序。这是「少隐式」的显式入口 —— 要签 canonical 就调它，
/// 不用猜这台机器开了什么编译开关。
pub fn parse_ordered(text: &str) -> Result<Value, SmlError> {
    Ok(parse_with(text, ParseOptions::new().ordered())?.value)
}

/// File entry with **explicit source order** (include expansion happens here, so this is the
/// only way to keep source order across `include`). ｜ 文件入口的显式保序版本：
/// `include` 只在文件入口展开，故跨 include 的源序必须走这里。
pub fn parse_file_ordered(path: impl AsRef<std::path::Path>) -> Result<Value, SmlError> {
    Ok(parse_file_with(path, ParseOptions::new().ordered())?.value)
}

/// Parse SML text and return the non-fatal diagnostics ｜ 解析 SML 文本，并把**非致命诊断**（键冲突告警）一并交出来。
///
/// 与 [`parse`] 的差别**只在返回值**：`parse` 的签名是 `Result<Value, _>`，承载不了
/// 诊断，只能丢掉；本入口把它们交给调用方。**解析结果与 `parse` 逐字一致** ——
/// 诊断只说明「数据被改写 / 覆盖过」，不改变 `Value`。
///
/// 目前产出两类告警（见 [`crate::ext::DiagnosticKind`]）：
///
/// | kind | 触发 | 后果 |
/// |---|---|---|
/// | `DuplicateKey` | 同一块内键名重复 | 已**提升为数组**（SML 既定语义，但结构被改写） |
/// | `KeyOverwritten` | 片段展开撞上父块已有的同名键 | 父块原值**已丢失**（片段合并不走数组提升） |
///
/// 两者此前都是**静默**的（雷 5 · A 组 4/5）。语义一律保留 —— 只是不再闷声改数据。
///
/// ```ignore
/// let (v, warns) = sml_parse::parse_with_warnings("k: 1\nk: 2\n")?;
/// assert_eq!(warns.len(), 1);          // DuplicateKey
/// assert!(matches!(v.get("k"), Some(Value::Array(_)))); // 值仍是数组（语义未变）
/// ```
pub fn parse_with_warnings(text: &str) -> Result<(Value, Vec<Diagnostic>), SmlError> {
    // 快路径（见 [`no_directives`]）：与 [`parse_with`] 共用同一条链
    if no_directives(text) {
        let out = parse_ext(text, FeatureSet::for_edition(Edition::V1), ParseOptions::new())?;
        return Ok((out.value, out.diagnostics));
    }
    let (rest, declared) = strip_version(text)?;
    let (rest, feats, touched, mode, base, had) = strip_features(&rest)?;
    let v = declared.or(base).unwrap_or(Edition::V1);
    let feats = features_for(v, feats, touched, mode, had);
    let (toks, spans) = tokenize_spans(&rest)?;
    let lines = token_lines(&rest, &spans);
    let out = parse_impl_tokens_ext(
        toks,
        spans,
        lines,
        feats,
        BTreeMap::new(),
        ParseOptions::new(),
    )
    .map_err(|e| with_position(&rest, e))?;
    Ok((out.value, out.diagnostics))
}

/// Parse SML text, restricting the declared edition to `allowed` ｜ 解析 SML 文本，并限制文档声明的**版本集**必须在 `allowed` 范围内。
///
/// 用于「库固定依赖某个 SML 版本集」的场景：若文档声明了 `allowed`
/// 之外的版本集（例如库只接受 `1.0`..`3.0`，却遇到 `@version v4`），立即报错，
/// 而不是用不兼容的语法静默解析。
///
/// 未声明版本的文档视为 `1.0`，只要 `allowed` 含 `1.0` 即放行。
pub fn parse_allowed(
    text: &str,
    allowed: &[Edition],
) -> Result<Value, SmlError> {
    // 快路径：无指令 ⇒ 版本必为 1.0 ⇒ 只可能因 `allowed` 不含 1.0 而失败。
    // 判据与报错文案必须与慢路径**逐字一致**，否则错误码虽同、文案会漂。
    if no_directives(text) {
        if !allowed.contains(&Edition::V1) {
            return Err(version_not_allowed(Edition::V1, allowed));
        }
        return parse_impl(text, FeatureSet::for_edition(Edition::V1), BTreeMap::new());
    }
    let (rest, declared) = strip_version(text)?;
    let (rest, feats, touched, mode, base, had) = strip_features(&rest)?;
    let v = declared.or(base).unwrap_or(Edition::V1);
    if !allowed.contains(&v) {
        return Err(version_not_allowed(v, allowed));
    }
    let feats = features_for(v, feats, touched, mode, had);
    parse_impl(&rest, feats, BTreeMap::new())
}

/// Parse SML text, restricting the feature subset to `allowed` ｜ 解析 SML 文本，同时限制文档使用的**特性子集**必须在 `allowed` 内。
///
/// 与 [`parse_allowed`]（版本范围）配套：`allowed` 是调用方（库作者）给出的
/// 白名单，文档内部的 `@feature enable/disable` 只能**收窄**这个集合，
/// 不能扩宽——否则文档就能自行绕过调用方的限制。交集为空则报错。
///
/// 未声明任何 `@feature` 的文档若仅靠版本基线（如 v3），则基线特性与
/// `allowed` 交集；只要交集非空即放行。
///
/// 返回 `(Value, FeatureSet)`，第二个是**实际生效**的集合（= 文档所求 ∩ `allowed`），
/// 不是调用方传进来的 `allowed` 本身。
///
/// 与 [`parse_with`] 的关系：本函数是它的**特性受限版**，代价是同样**不展开 include**，
/// 且同样**不返回诊断**（`DuplicateKey` / `KeyOverwritten` 会被丢掉）。
/// 要「限制特性 + 拿诊断」，今天没有单入口，需 `parse_with` 拿到 `ParseOutput` 后自行裁剪。
///
/// # 例子
///
/// ```
/// use sml_feature::{Edition, Feature, FeatureSet};
/// use sml_parse::parse_with_features;
///
/// // 调用方白名单：3.0 的默认功能集 + 显式允许 `when`
/// let allowed = FeatureSet::for_edition(Edition::V3).with(Feature::When);
/// let (v, effective) = parse_with_features("a: 1\n", allowed).unwrap();
/// assert_eq!(v.get("a").and_then(|x| x.as_i64()), Some(1));
/// // 第二个返回值是**交集**，不是 `allowed` 本身
/// assert_eq!(effective.intersection(allowed), effective);
/// ```
///
/// # 陷阱
///
/// ⚠️ **「文档开了但被悄悄削掉」不会报错**。实现只检查**整个交集**是否为空
/// （空 ⇒ `E-FEATURE-009`）；单个特性落在 `allowed` 之外时是**静默剔除**。
/// 于是文档作者写了 `@feature enable glob-include`、解析成功、那项能力却根本没开——
/// 与 [`parse`] 的行为正好相反（`parse` 用文档自己的特性，不设上限）。
/// 需要「越权即失败」时必须自己比对：`feats` 与 `effective` 的差集非空就报错。
///
/// ⚠️ **别把返回值当新的 `allowed` 往下传**：它是交集，再交集一次只会**越传越窄**，
/// 子文档（如被外层包含的片段）会看不到外层已启用的能力，且失败现场远离真正的肇事处。
///
/// ⚠️ **空交集与版本不符是两个码**：本函数报 `E-FEATURE-009`，
/// 版本不在范围内由 [`parse_allowed`] 报 `E-FEATURE-004`，别混着断言。
pub fn parse_with_features(
    text: &str,
    allowed: FeatureSet,
) -> Result<(Value, FeatureSet), SmlError> {
    // 快路径（见 [`no_directives`]）：无指令 ⇒ 文档所求即 v1 基线，交集照算
    if no_directives(text) {
        return with_effective(text, FeatureSet::for_edition(Edition::V1), allowed, BTreeMap::new());
    }
    let (rest, declared) = strip_version(text)?;
    let (rest, feats, touched, mode, base, had) = strip_features(&rest)?;
    let v = declared.or(base).unwrap_or(Edition::V1);
    let feats = features_for(v, feats, touched, mode, had);
    let effective = feats.intersection(allowed);
    if effective.is_empty() {
        return Err(feature_set_empty(&feats, &allowed));
    }
    let val = parse_impl(&rest, effective, BTreeMap::new())?;
    Ok((val, effective))
}

/// Like [`parse_with_features`], plus an env override table ｜ 与 [`parse_with_features`] 相同，但额外提供**环境变量覆盖表**。
///
/// `$env.X` 会先在此表中查找，未命中才回落到进程环境。表内的值**只作用于
/// 本次解析**，不会写入进程环境——因此该函数是线程安全的，也不会影响
/// `PATH` / `LD_PRELOAD` 等影响进程行为的变量。
///
/// 供 FFI（`sml_parse_ex` 的 `env` 选项）等需要注入变量、又不便改进程环境的
/// 场景使用。
///
/// # 与 [`parse_with_features`] 的差别
///
/// **只有 `$env.X` 的取值来源**不同：先查这张表，未命中才回落进程环境。
/// 特性侧的语义与陷阱**完全一致** —— 返回值同样是**交集**（不是 `allowed`）、
/// 单个特性越权同样是**静默剔除**、同样别把返回值当新的 `allowed` 往下传。
/// 那三条的展开说明见 [`parse_with_features`] 的「陷阱」一节，此处不重复。
pub fn parse_with_features_env(
    text: &str,
    allowed: FeatureSet,
    env: BTreeMap<String, String>,
) -> Result<(Value, FeatureSet), SmlError> {
    // 快路径（见 [`no_directives`]）
    if no_directives(text) {
        return with_effective(text, FeatureSet::for_edition(Edition::V1), allowed, env);
    }
    let (rest, declared) = strip_version(text)?;
    let (rest, feats, touched, mode, base, had) = strip_features(&rest)?;
    let v = declared.or(base).unwrap_or(Edition::V1);
    let feats = features_for(v, feats, touched, mode, had);
    let effective = feats.intersection(allowed);
    if effective.is_empty() {
        return Err(feature_set_empty(&feats, &allowed));
    }
    let val = parse_impl(&rest, effective, env)?;
    Ok((val, effective))
}

/// 把错误的字节偏移换算成「第 N 行第 M 列」缀到文案末尾。
///
/// 只在**出口**做这一步：位置属于呈现层，各端措辞可以不同；码与文案才是
/// 稳定契约（见 `SmlError` 的文档）。`text` 必须是 token 所来自的那份文本 ——
/// 剥离 `@version`/`@feature` 后的 `rest` 与原文**等长**（见 `scan.rs`），
/// 因此这里的行列对原文同样成立。
fn with_position(text: &str, e: SmlError) -> SmlError {
    let Some(pos) = e.pos() else {
        return e;
    };
    let pos = pos.min(text.len());
    let before = &text[..pos];
    let line = before.matches('\n').count() + 1;
    let col = pos - before.rfind('\n').map(|i| i + 1).unwrap_or(0) + 1;
    e.suffix(&format!("（第 {line} 行第 {col} 列）"))
}

/// 每个 token 所在的**行号**（0 起，与 `spans` 一一对应）。
///
/// 单趟扫描：游标只前进，逐 token 数到它起点为止的换行数。
/// 供 `@is` 多词的「遇换行即停」（D1）—— 没有它，`@is X` 的下一行键会被吞进契约名。
fn token_lines(text: &str, spans: &[Span]) -> Vec<usize> {
    let b = text.as_bytes();
    let mut out = Vec::with_capacity(spans.len());
    let mut pos = 0usize;
    let mut line = 0usize;
    for s in spans {
        let start = s.start.min(b.len());
        while pos < start {
            if b[pos] == b'\n' {
                line += 1;
            }
            pos += 1;
        }
        out.push(line);
    }
    out
}

/// 不含版本处理的底层解析（文本入口）
fn parse_impl(
    text: &str,
    features: FeatureSet,
    env: BTreeMap<String, String>,
) -> Result<Value, SmlError> {
    let (toks, spans) = tokenize_spans(text)?;
    let lines = token_lines(text, &spans);
    parse_impl_tokens(toks, spans, lines, features, env)
        .map_err(|e| with_position(text, e))
}

/// 不含版本处理的底层解析（token 流入口，供 include 展开后零拷贝复用）
///
/// 为什么这里**不收** `version`：版本的影响在调用这一层就已经全部折算进 `features` 了
/// （见 `features_for(v, …)`），`Parser` 只认特性集、不认版本号。
/// 原先保留这个参数是为"对称"，实际从没被读过 —— 与其挂着等一个 `unused` 警告，
/// 不如删掉：调用方少一个需要正确传递、却完全不产生效果的值。
///
/// `spans` 与 `toks` 一一对应；**拿不到就传空**（如文件入口的 include 展开
/// 是跨文件合并的 token 流，没有可映射的单一原文）—— 解析器会退回「不带位置」。
fn parse_impl_tokens(
    toks: Vec<Tok>,
    spans: Vec<Span>,
    lines: Vec<usize>,
    features: FeatureSet,
    env: BTreeMap<String, String>,
) -> Result<Value, SmlError> {
    // 无扩展路径：直接丢掉 diagnostics，与既有签名保持一致。
    Ok(parse_impl_tokens_ext(toks, spans, lines, features, env, ParseOptions::new())?.value)
}

/// 顶层首个 token 是否为「值型标量」：数字 / 引号串 / bool / null。
/// 裸词标识符（如 `server`、`a`，作为块类型名/块名）**不算**值型 —— 这是区分
/// T9 误报（`42 a { }`）与合法带类型块（`server web { }`）的关键。
fn is_scalar_value(tok: Option<&Tok>) -> bool {
    match tok {
        Some(Tok::Str(_)) => true,
        Some(Tok::Word(w)) => {
            matches!(w.as_str(), "true" | "false" | "null") || w.as_str().parse::<f64>().is_ok()
        }
        _ => false,
    }
}

/// 同 [`parse_impl_tokens`]，但携带外置扩展（见 [`crate::ext`]）。
fn parse_impl_tokens_ext(
    toks: Vec<Tok>,
    spans: Vec<Span>,
    lines: Vec<usize>,
    features: FeatureSet,
    env: BTreeMap<String, String>,
    opts: ParseOptions,
) -> Result<ParseOutput, SmlError> {
    let ordered = opts.ordered;
    let mut p = Parser {
        toks,
        spans,
        lines,
        i: 0,
        fragments: BTreeMap::new(),
        contracts: BTreeMap::new(),
        types: BTreeMap::new(),
        features,
        ordered,
        depth: 0,
        ns_stack: Vec::new(),
        path_stack: Vec::new(),
        env,
        loop_vars: BTreeMap::new(),
        directives: Arc::new(opts.directives),
        diags: Vec::new(),
        // 键冲突告警的开关：只影响 `diags`，不影响 `value`。
        warn_key_conflict: opts.warn_key_conflict,
        // 解析期是否应用文件内契约（`@is` / typed-block）；默认 `true`。
        apply_contracts: opts.apply_contracts,
        // 是否收集多条错误（TODO L16）；默认 `false`（遇错即停，行为不变）。
        collect_errors: opts.collect_errors,
        errors: Vec::new(),
        // 弱绑定旁路表：恒收集，但只有写了 `@weakbind` 才有内容（对主树零影响）。
        weak_binds: Vec::new(),
        // 内置契约表（宿主注入）与待做的块内容校验。
        builtin_contracts: opts.builtin_contracts,
        weak_pending: Vec::new(),
        // 弱数据旁路表：只有写了 `@weakdata` 才有内容。
        weak_data: Vec::new(),
        weak_docs: Vec::new(),
        contract_ext: Arc::new(opts.contract_ext),
        // 契约映射：opt-in。未开启时 None（零开销、结果逐字一致）。
        contract_map: if opts.trace_contracts {
            Some(ContractMap::new())
        } else {
            None
        },
    };
    // 顶层标量不可往返 ⇒ `E-PARSE-008`（W16 接线）。
    //
    // 改之前这里是**静默造键**：`42` 走下面的 `parse_block(None)`，被当成「键即值」的
    // 裸词键，解析成 `{"42": 42}` —— 重新序列化得到 `"42": 42` ≠ `42`，数据形状被悄悄
    // 改掉（与 W17 的 C 嵌套数组同族）。而这条码一直躺在 `sml-codes` 里、名字就叫
    // 「顶层标量不可往返」，却**从未被接线**：全仓只有常量定义与 doctest 引用，
    // 没有一处 `SmlError::new(E_PARSE_008, …)`。W16 起改为此处显式报错。
    //
    // 【判据（T9 收紧后，五端逐字同，Rust 是唯一事实来源）】
    //   ① 仅一个顶层 token 且是标量（裸词/引号串/数字都记 Word/Str）⇒ 报
    //      —— 保持原「恰好一个标量」语义（`42`/`hello`/`"hi"`/`true` 单标量都报）。
    //   ② T9 收紧：首个顶层 token 是**值型标量**（数字/引号串/bool/null；注意裸词标识符
    //      如 `server`/`a` 是块的类型名/块名，**不算**值型），且其后紧跟块/数组/键值
    //      （即被当成「块的类型名」吞掉）⇒ 报。
    //      - `42\na { b: 1 }` → `42` 是值、`a` 后跟 `{` ⇒ 报（T9：42 被重解释成块类型名）
    //      - `server web { }` → `server` 是标识符（块类型名）⇒ 不报（合法块）
    //      - `42\nb: 1` / `42\n[1,2]` → 后跟键值/数组 ⇒ 报
    //   排除：`hello world`（两 token 裸键对，值可往返）、`@version v1`+`42`（首结构是
    //    @ 指令，非标量）、`42 { a: 1 }`（名为 42 的块，紧跟 `{`）、`42: x`（键值）。
    let lone_scalar = matches!(p.toks.as_slice(), [Tok::Word(_)] | [Tok::Str(_)]);
    if lone_scalar {
        return Err(SmlError::from_code_msg(E_PARSE_008));
    }
    if is_scalar_value(p.toks.first()) {
        let followed_by_container = matches!(p.toks.get(1), Some(Tok::LBrack))
            || matches!(p.toks.get(1), Some(Tok::Word(_)) | Some(Tok::Str(_)))
                && matches!(p.toks.get(2), Some(Tok::LBrace) | Some(Tok::Colon));
        if followed_by_container {
            return Err(SmlError::from_code_msg(E_PARSE_008));
        }
    }
    // 顶层支持三种形态，与 `to_sml` 的输出对称：
    //   - `[ ... ]` 数组：to_sml 对非对象走 dump_inline，会输出顶层数组
    //     （如「历史记录」这类对象数组）。此前 parse 只认键值块，导致
    //     能序列化却读不回（"期望键, 得 LBrack"），是不对称缺陷。
    //   - `{ ... }` 顶层对象块
    //   - 键值块（传统形态）
    let value = match p.peek() {
        Some(Tok::LBrack) => {
            if !p.features.has(Feature::TopArray) {
                return Err(SmlError::from_code_msg(E_FEATURE_001)
                    .with_detail("top-level-array"));
            }
            p.next();
            p.parse_array()
        }
        Some(Tok::LBrace) => {
            p.next();
            p.parse_block(Some(Tok::RBrace))
        }
        _ => p.parse_block(None),
    };
    // 契约映射出口：排序后随输出返回（此后只读）。未开启时为 None。
    if let Some(m) = &mut p.contract_map {
        m.freeze();
    }
    Ok(ParseOutput {
        value: value?,
        diagnostics: p.diags,
        contract_map: p.contract_map,
        errors: p.errors,
        weak_binds: p.weak_binds,
        weak_data: p.weak_data,
        weak_docs: p.weak_docs,
        // 契约表回传：opt-in，不开时 None（与 contract_map 同一取舍）。
        contracts: if opts.capture_contracts {
            Some(p.contracts.clone())
        } else {
            None
        },
    })
}

// ---------------------------------------------------------------------------
// include 指令：把外部 .sml 文件内联进来
//
// 语法：`include "path.sml"` 或 `@include "path.sml"`（两种等价）
// 语义：**文本内联**（类似 C 的 #include），而非对象合并。
//   这样 include 可以出现在块内部引入一组字段，例如：
//       server web { &base include "common/port.sml" }
//   若做成对象合并就无法表达「注入若干字段到当前块」。
//
// 相对路径按**被包含文件自身所在目录**解析（与 C 预处理器一致），
// 而非进程工作目录，因此嵌套 include 时路径行为可预期。
// ---------------------------------------------------------------------------

/// Parse an SML file (expanding `include`) into a `Value`. ｜ 解析 SML 文件（展开 `include`）到 `Value`。
///
/// 相对路径按**该文件自身所在目录**解析；版本/特性声明从主文件剥离，
/// 最终允许集 = `FeatureSet::all() ∩ 文档声明`。本入口不接受 [`ParseOptions`]
/// —— 要传选项（如关掉文档内契约）请用 `parse_file_with`。
///
/// （原先挂在这里的「值嵌套深度上限」说明属于别的条目：`api.rs` 早已没有那个常量，
/// 它带来的 `MAX_INCLUDE_DEPTH` intra-doc 链接也是悬空的 —— 2026-11-02 删除。）
///

pub fn parse_file(path: impl AsRef<Path>) -> Result<Value, SmlError> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)
        .map_err(|e| SmlError::from_code_msg(E_IO_001).with_detail(format!("{}: {e}", path.display())))?;
    let base = path
        .parent()
        .map(|p| p.to_path_buf())
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("."));
    // 主文件先剥离版本/特性指令。
    // 便捷入口 `parse_file` 的「调用方允许集」为全开（文档自身声明决定启用哪些特性，
    // 真正的调用方限制由 `parse_with_features` / `parse_allowed` 负责）。
    let (rest, declared) = strip_version(&text)?;
    let (rest, feats, touched, mode, base_ver, had) = strip_features(&rest)?;
    let v = declared.or(base_ver).unwrap_or(Edition::V1);
    let feats = features_for(v, feats, touched, mode, had);
    let allowed = FeatureSet::all().intersection(feats);
    let (toks, spans) = resolve_includes_spans(&rest, &base, allowed)?;
    let spans = spans.unwrap_or_default();
    // 无 include ⇒ 可定位行列；有 include ⇒ 跨文件无单一原文，不造位置。
    let lines = if spans.is_empty() { Vec::new() } else { token_lines(&rest, &spans) };
    parse_impl_tokens(toks, spans, lines, allowed, BTreeMap::new())
        .map_err(|e| with_position(&rest, e))
}

/// Same as [`parse_file`], plus a caller-supplied allowed feature set `caller_allowed` intersected with the document's `@feature` declarations; lets FFI (`sml_load_file(flags)`) restrict file-entry capabilities via `flags` (e.g. disable `include` / `env`). ｜ 同 [`parse_file`]，但额外接受一个「调用方允许特性集」`caller_allowed`，与文档 `@feature` 声明取交集。用于 FFI（`sml_load_file(flags)`）让调用方通过 `flags` 限制文件入口能力（如禁用 `include`/`env`）。
///
/// 行为：
/// - 最终允许集 = `FeatureSet::all() ∩ 文档声明特性 ∩ caller_allowed`。
/// - 若交集为空（调用方禁用了一切可用特性），视为「不允许任何能力」并报错，
///   避免静默以全开集回退读取文件（那会令 `flags` 形同虚设）。
pub fn parse_file_features(
    path: impl AsRef<Path>,
    caller_allowed: FeatureSet,
) -> Result<Value, SmlError> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)
        .map_err(|e| SmlError::from_code_msg(E_IO_001).with_detail(format!("{}: {e}", path.display())))?;
    let base = path
        .parent()
        .map(|p| p.to_path_buf())
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from("."));
    let (rest, declared) = strip_version(&text)?;
    let (rest, feats, touched, mode, base_ver, had) = strip_features(&rest)?;
    let v = declared.or(base_ver).unwrap_or(Edition::V1);
    let feats = features_for(v, feats, touched, mode, had);
    let allowed = FeatureSet::all().intersection(feats).intersection(caller_allowed);
    if allowed.is_empty() {
        return Err(SmlError::from_code_msg(E_FEATURE_009));
    }
    let (toks, spans) = resolve_includes_spans(&rest, &base, allowed)?;
    let spans = spans.unwrap_or_default();
    // 无 include ⇒ 可定位行列；有 include ⇒ 跨文件无单一原文，不造位置。
    let lines = if spans.is_empty() { Vec::new() } else { token_lines(&rest, &spans) };
    parse_impl_tokens(toks, spans, lines, allowed, BTreeMap::new())
        .map_err(|e| with_position(&rest, e))
}

/// Parse into an object (throws `ParseError`, which carries an error code) ｜ 解析到对象 (失败抛 `ParseError`，它带**错误码**，见 [`sml_codes::SmlError`])
pub fn loads(text: &str) -> Result<Value, ParseError> {
    parse(text)
}