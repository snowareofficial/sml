// SPDX-License-Identifier: MulanPSL-2.0
//! SML 解析的**公开 API**（`parse` / `parse_file` / `loads` 等）。
//!
//! 与 [`crate::parser`] 分开：后者是内部实现，这里只做入口组装
//! （特性求取 → 指令剥离 → tokenize → 解析 → include 展开）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sml_feature::{Feature, FeatureSet, Version};
use sml_include::{expand_includes, resolve_includes};
use sml_lex::{Tok, tokenize};
use sml_value::{MAX_VALUE_DEPTH, Value};

use crate::error::ParseError;
use crate::parser::Parser;
use crate::scan::{features_for, strip_features, strip_version};

/// 解析 SML 文本，并返回其声明的语法版本。
///
/// 未声明版本时按 `V1` 处理（裸词即字符串），**既有文档不受影响**；
/// 显式 `@version v3` 则返回 `V3`（此时字符串需引号）。
pub fn parse_versioned(text: &str) -> Result<(Value, Version), String> {
    let (rest, declared) = strip_version(text)?;
    let (rest, feats, base, had) = strip_features(&rest)?;
    // 版本优先级：@version 显式声明 > @feature base > 默认 V1
    let v = declared.or(base).unwrap_or(Version::V1);
    let feats = features_for(v, feats, had);
    Ok((parse_impl(&rest, v, feats, BTreeMap::new())?, v))
}

/// 解析 SML 文件：展开 include，并返回其声明的语法版本
pub fn parse_file_versioned(path: impl AsRef<Path>) -> Result<(Value, Version), String> {
    let path = path.as_ref();
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("读取失败 {}: {e}", path.display()))?;
    let base = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let (rest, declared) = strip_version(&text)?;
    let (rest, feats, base_ver, had) = strip_features(&rest)?;
    let allowed = FeatureSet::all().intersection(feats);
    let v = declared.or(base_ver).unwrap_or(Version::V1);
    let feats = features_for(v, allowed, had);
    let toks = resolve_includes(&rest, &base, allowed)?;
    let val = parse_impl_tokens(toks, v, feats, BTreeMap::new())?;
    Ok((val, v))
}

/// 解析 SML 文本
///
/// 会自动识别并剥离 `@version` / `@feature` 声明（需要版本信息时用
/// [`parse_versioned`]，需要特性裁剪信息时用 [`parse_with_features`]）。
///
/// **向后兼容**：未声明 `@version` 的文档按 `V1` 解析（裸词即字符串），
/// 既有大量 v1 文档不受影响；仅显式 `@version v2|v3` 才启用严格字符串。
pub fn parse(text: &str) -> Result<Value, String> {
    let (rest, declared) = strip_version(text)?;
    let (rest, feats, base, had) = strip_features(&rest)?;
    let v = declared.or(base).unwrap_or(Version::V1);
    let feats = features_for(v, feats, had);
    parse_impl(&rest, v, feats, BTreeMap::new())
}

/// 解析 SML 文本，并限制文档声明的版本必须在 `allowed` 范围内。
///
/// 用于「库固定依赖某个 SML 语法版本」的场景：若文档声明了 `allowed`
/// 之外的版本（例如库只接受 v1..v3，却遇到 `@version v4`），立即报错，
/// 而不是用不兼容的语法静默解析。
///
/// 未声明版本的文档视为 `V1`，只要 `allowed` 含 `V1` 即放行。
pub fn parse_allowed(
    text: &str,
    allowed: &[Version],
) -> Result<Value, String> {
    let (rest, declared) = strip_version(text)?;
    let (rest, feats, base, had) = strip_features(&rest)?;
    let v = declared.or(base).unwrap_or(Version::V1);
    if !allowed.contains(&v) {
        return Err(format!(
            "sml: 文档声明版本 {} 不在本库接受的版本范围 {{{}}} 内",
            v.name(),
            allowed
                .iter()
                .map(|x| x.name())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let feats = features_for(v, feats, had);
    parse_impl(&rest, v, feats, BTreeMap::new())
}

/// 解析 SML 文本，同时限制文档使用的**特性子集**必须在 `allowed` 内。
///
/// 与 [`parse_allowed`]（版本范围）配套：`allowed` 是调用方（库作者）给出的
/// 白名单，文档内部的 `@feature enable/disable` 只能**收窄**这个集合，
/// 不能扩宽——否则文档就能自行绕过调用方的限制。交集为空则报错。
///
/// 未声明任何 `@feature` 的文档若仅靠版本基线（如 v3），则基线特性与
/// `allowed` 交集；只要交集非空即放行。
pub fn parse_with_features(
    text: &str,
    allowed: FeatureSet,
) -> Result<(Value, FeatureSet), String> {
    let (rest, declared) = strip_version(text)?;
    let (rest, feats, base, had) = strip_features(&rest)?;
    let v = declared.or(base).unwrap_or(Version::V1);
    let feats = features_for(v, feats, had);
    let effective = feats.intersection(allowed);
    if effective.is_empty() {
        return Err(format!(
            "sml: 文档请求的特性 {feats} 与调用方允许的特性 {allowed} 无交集"
        ));
    }
    let val = parse_impl(&rest, v, effective, BTreeMap::new())?;
    Ok((val, effective))
}

/// 与 [`parse_with_features`] 相同，但额外提供**环境变量覆盖表**。
///
/// `$env.X` 会先在此表中查找，未命中才回落到进程环境。表内的值**只作用于
/// 本次解析**，不会写入进程环境——因此该函数是线程安全的，也不会影响
/// `PATH` / `LD_PRELOAD` 等影响进程行为的变量。
///
/// 供 FFI（`sml_parse_ex` 的 `env` 选项）等需要注入变量、又不便改进程环境的
/// 场景使用。
pub fn parse_with_features_env(
    text: &str,
    allowed: FeatureSet,
    env: BTreeMap<String, String>,
) -> Result<(Value, FeatureSet), String> {
    let (rest, declared) = strip_version(text)?;
    let (rest, feats, base, had) = strip_features(&rest)?;
    let v = declared.or(base).unwrap_or(Version::V1);
    let feats = features_for(v, feats, had);
    let effective = feats.intersection(allowed);
    if effective.is_empty() {
        return Err(format!(
            "sml: 文档请求的特性 {feats} 与调用方允许的特性 {allowed} 无交集"
        ));
    }
    let val = parse_impl(&rest, v, effective, env)?;
    Ok((val, effective))
}

/// 不含版本处理的底层解析（文本入口）
fn parse_impl(
    text: &str,
    version: Version,
    features: FeatureSet,
    env: BTreeMap<String, String>,
) -> Result<Value, String> {
    let toks = tokenize(text)?;
    parse_impl_tokens(toks, version, features, env)
}

/// 不含版本处理的底层解析（token 流入口，供 include 展开后零拷贝复用）
fn parse_impl_tokens(
    toks: Vec<Tok>,
    version: Version,
    features: FeatureSet,
    env: BTreeMap<String, String>,
) -> Result<Value, String> {
    let mut p = Parser {
        toks,
        i: 0,
        fragments: BTreeMap::new(),
        contracts: BTreeMap::new(),
        types: BTreeMap::new(),
        features,
        depth: 0,
        ns_stack: Vec::new(),
        env,
        loop_vars: BTreeMap::new(),
    };
    // 顶层支持三种形态，与 `to_sml` 的输出对称：
    //   - `[ ... ]` 数组：to_sml 对非对象走 dump_inline，会输出顶层数组
    //     （如「历史记录」这类对象数组）。此前 parse 只认键值块，导致
    //     能序列化却读不回（"期望键, 得 LBrack"），是不对称缺陷。
    //   - `{ ... }` 顶层对象块
    //   - 键值块（传统形态）
    // 注：顶层**标量**仍不可往返（SML 顶层需为容器），这是格式固有限制。
    match p.peek() {
        Some(Tok::LBrack) => {
            if !p.features.has(Feature::TopArray) {
                return Err("sml: 顶层数组需要特性 `top-level-array`，但当前特性集已禁用".into());
            }
            p.next();
            p.parse_array()
        }
        Some(Tok::LBrace) => {
            p.next();
            p.parse_block(Some(Tok::RBrace))
        }
        _ => p.parse_block(None),
    }
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

/// 值嵌套深度上限：防止 `a{a{a{ ... }}}` 这类深度嵌套触发递归下降的栈溢出。
///
/// 与 [`MAX_INCLUDE_DEPTH`] 互补 —— 后者只保护 include 的文件嵌套，不保护
/// 单个文档内部块/数组的嵌套。栈溢出在 Rust 中是 abort，
/// **无法被 catch_unwind 捕获**，因此必须在递归入口主动限深，
/// 而不是依赖上层错误处理。
///

pub fn parse_file(path: impl AsRef<Path>) -> Result<Value, String> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("读取失败 {}: {e}", path.display()))?;
    let base = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    // 主文件先剥离版本/特性指令。
    // 便捷入口 `parse_file` 的「调用方允许集」为全开（文档自身声明决定启用哪些特性，
    // 真正的调用方限制由 `parse_with_features` / `parse_allowed` 负责）。
    let (rest, declared) = strip_version(&text)?;
    let (rest, feats, base_ver, had) = strip_features(&rest)?;
    let v = declared.or(base_ver).unwrap_or(Version::V1);
    let feats = features_for(v, feats, had);
    let allowed = FeatureSet::all().intersection(feats);
    let toks = resolve_includes(&rest, &base, allowed)?;
    parse_impl_tokens(toks, v, allowed, BTreeMap::new())
}

/// 同 [`parse_file`]，但额外接受一个「调用方允许特性集」`caller_allowed`，
/// 与文档 `@feature` 声明取交集。用于 FFI（`sml_load_file(flags)`）让调用方
/// 通过 `flags` 限制文件入口能力（如禁用 `include`/`env`）。
///
/// 行为：
/// - 最终允许集 = `FeatureSet::all() ∩ 文档声明特性 ∩ caller_allowed`。
/// - 若交集为空（调用方禁用了一切可用特性），视为「不允许任何能力」并报错，
///   避免静默以全开集回退读取文件（那会令 `flags` 形同虚设）。
pub fn parse_file_features(
    path: impl AsRef<Path>,
    caller_allowed: FeatureSet,
) -> Result<Value, String> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("读取失败 {}: {e}", path.display()))?;
    let base = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let (rest, declared) = strip_version(&text)?;
    let (rest, feats, base_ver, had) = strip_features(&rest)?;
    let v = declared.or(base_ver).unwrap_or(Version::V1);
    let feats = features_for(v, feats, had);
    let allowed = FeatureSet::all().intersection(feats).intersection(caller_allowed);
    if allowed.is_empty() {
        return Err("sml: 调用方 flags 与文档特性交集为空，不允许任何解析能力".into());
    }
    let toks = resolve_includes(&rest, &base, allowed)?;
    parse_impl_tokens(toks, v, allowed, BTreeMap::new())
}

/// 解析到对象 (失败抛 `ParseError`)
pub fn loads(text: &str) -> Result<Value, ParseError> {
    parse(text).map_err(ParseError)
}

