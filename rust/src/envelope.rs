// SPDX-License-Identifier: MulanPSL-2.0
//! `.ambr` as a **general-purpose envelope**: wrap arbitrary bytes, unwrap them, and convert
//! SML text to/from the `VALUE` payload. Behind the `amberice` feature.
//! 把 `.ambr` 当**通用信封**用：装任意字节、取出、以及 SML 文本 ⇄ `VALUE` 载荷。
//! 名字挂在 `sml::envelope` 下，**只有 `amberice` feature 才存在**。
//!
//! 定位（`rust/amberice/README.md`，2026-11-05 起）：`.ambr` 是**不可变、可寻址、可验证的通用
//! 二进制信封**，SML 只是内置载荷之一。所以本模块的第一等公民是 `bytes ⇄ 信封`，
//! `sml_to_amberice` 只是「载荷之一」的便捷入口 —— 别把它当成主路径。
//! ｜ Envelope first, SML payload second — that order is the whole point of the repositioning.
//!
//! 为什么放在 `swsml` 而不是 `smltools`（用户 2026-11-05）：`smltools` **只是 bin**；
//! 库能力归门面 crate 的附属模块，并且用 feature 门控（关掉 `amberice` 就完全看不到它，
//! 依赖树里也不会出现 `amberice`）。
//! ｜ `smltools` stays bin-only; library surface belongs here, behind a feature.

use std::fmt;

use amberice::{ContainerError, DumpOptions, ManifestPayload, Value};

/// Why an envelope operation failed. ｜ 信封操作失败的原因。
///
/// 四类分开报，因为处置方式不同：解析错要去改文本、容器错说明文件有问题、
/// 载荷名不存在要问清楚名字、根本没有载荷要看是不是拿错了文件。
#[derive(Debug)]
#[non_exhaustive]
pub enum EnvelopeError {
    /// SML text did not parse ｜ SML 文本解析失败。
    Parse(sml_codes::SmlError),
    /// The `.ambr` container refused (bad layout, digest, capability, …)
    /// 容器层拒绝（布局、摘要、能力位……）。
    Container(ContainerError),
    /// The named payload is not inside the envelope ｜ 信封里没有这个载荷。
    PayloadMissing {
        /// The name asked for ｜ 要的名字。
        name: String,
        /// Names that are actually there ｜ 实际有的名字。
        available: Vec<String>,
    },
    /// The envelope has no payload at all ｜ 信封里一个载荷都没有。
    NoPayload,
}

impl fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnvelopeError::Parse(e) => write!(f, "SML 解析失败：{e}"),
            EnvelopeError::Container(e) => write!(f, "{e}"),
            EnvelopeError::PayloadMissing { name, available } => {
                write!(f, "信封里没有载荷 `{name}`")?;
                if available.is_empty() {
                    write!(f, "（一个载荷都没有）")
                } else {
                    write!(f, "；现有：{}", available.join(", "))
                }
            }
            EnvelopeError::NoPayload => write!(f, "信封里没有载荷"),
        }
    }
}

impl std::error::Error for EnvelopeError {}

impl From<ContainerError> for EnvelopeError {
    fn from(e: ContainerError) -> Self {
        EnvelopeError::Container(e)
    }
}

/// One payload inside an envelope. ｜ 信封里的一个载荷。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Payload {
    /// Name as stored ｜ 存下来的名字。
    pub name: String,
    /// Length in bytes ｜ 字节数。
    pub len: u32,
}

/// Wrap arbitrary bytes into a `.ambr` envelope.
/// 把任意字节装进 `.ambr` 信封。
///
/// `name` is what [`unwrap`] will ask for; for a single file the plain file name is the
/// obvious choice. Compression is opt-in (`compress` = zstd level).
/// `name` 就是将来 [`unwrap`] 要问的名字；单个文件通常直接用文件名。
pub fn wrap(name: &str, bytes: &[u8], compress: Option<i32>) -> Result<Vec<u8>, EnvelopeError> {
    let opts = DumpOptions {
        compress,
        ..Default::default()
    };
    Ok(amberice::pack_with(&[(name, bytes)], &opts)?)
}

/// Extract one payload from an envelope. ｜ 从信封里取出一个载荷。
///
/// `name = None` means “the only one” — convenient for single-file envelopes, and an error
/// (not “picks the first”) when there are several.
/// `name = None` 表示「就那一个」：单载荷时省事，多载荷时**报错**而不是随便挑一个。
pub fn unwrap(bytes: &[u8], name: Option<&str>) -> Result<Vec<u8>, EnvelopeError> {
    let all = list(bytes)?;
    let name = match name {
        Some(n) => n.to_string(),
        None if all.len() == 1 => all[0].name.clone(),
        None if all.is_empty() => return Err(EnvelopeError::NoPayload),
        None => {
            return Err(EnvelopeError::PayloadMissing {
                name: "<未指定>".to_string(),
                available: all.iter().map(|p| p.name.clone()).collect(),
            })
        }
    };
    if !all.iter().any(|p| p.name == name) {
        return Err(EnvelopeError::PayloadMissing {
            name,
            available: all.iter().map(|p| p.name.clone()).collect(),
        });
    }
    Ok(amberice::payload(bytes, &name)?.into_owned())
}

/// List the payloads of an envelope (cheap: reads the manifest only).
/// 列出信封里的载荷（很便宜：只读 manifest）。
pub fn list(bytes: &[u8]) -> Result<Vec<Payload>, EnvelopeError> {
    let declared: Vec<ManifestPayload> = amberice::payloads(bytes)?;
    Ok(declared
        .into_iter()
        .map(|p| Payload {
            name: p.name,
            len: p.len,
        })
        .collect())
}

/// SML text → envelope (stored as the `VALUE` payload). ｜ SML 文本 → 信封（存成 `VALUE` 载荷）。
///
/// 这是「SML 是内置载荷之一」的具体落点：与 [`wrap`] 产出的是同一种文件，只是载荷类型不同，
/// 故 `amberice cat` / `amberice check` 对两者一样有效。
/// Same file format as [`wrap`], different payload kind — so `amberice cat` / `check` work on both.
pub fn sml_to_amberice(text: &str, compress: Option<i32>) -> Result<Vec<u8>, EnvelopeError> {
    // 显式保序：文本里的键序就是数据的一部分，排序容器会把它丢掉。
    // `parse_ordered` 产出 `Map::Ordered`，即可签 canonical；`sml::envelope` 由
    // `feature = "amberice"` 门控，该 feature 会连带打开 `preserve_order`（空 feature）。
    let v: Value = crate::parse_ordered(text).map_err(EnvelopeError::Parse)?;
    value_to_amberice(&v, compress)
}

/// Already-parsed value → envelope, without a text round-trip.
/// 已经解析好的值 → 信封，**不经过文本往返**。
///
/// 工具侧（`smltools --to-amberice`）需要这一条：它手上已经是 `Value` 了，再 `to_sml` 一遍
/// 又 `parse` 一遍纯属白花 CPU，而且会在「文本表达不出某个值」时莫名失败。
/// ｜ The CLI already holds a `Value`; re-serialising it to text only to parse it back is waste.
pub fn value_to_amberice(value: &Value, compress: Option<i32>) -> Result<Vec<u8>, EnvelopeError> {
    value_to_amberice_with(value, compress, None)
}

/// Same as [`value_to_amberice`], optionally writing the `DIR` path index (D43).
/// 与 [`value_to_amberice`] 相同，另可选写 `DIR` 路径索引（D43）。
///
/// `dir_depth = Some(n)` 写索引；`None` 保持与旧路径逐字节相同。
pub fn value_to_amberice_with(
    value: &Value,
    compress: Option<i32>,
    dir_depth: Option<u32>,
) -> Result<Vec<u8>, EnvelopeError> {
    let opts = DumpOptions {
        compress,
        dir: dir_depth.is_some(),
        dir_depth: dir_depth.unwrap_or(amberice::manifest::DEFAULT_DIR_DEPTH),
        ..Default::default()
    };
    // 键序是**值层事实**：这里显式转换，无序值在转换点就被拒，
    // 而不是等到写盘才报 `key-order-unavailable`。
    let cv = amberice::CanonicalValue::try_from(value)
        .map_err(|e| EnvelopeError::Container(ContainerError::Canonical(e)))?;
    Ok(amberice::dump_with(&cv, &opts)?)
}

/// Same as [`value_to_amberice_with`], plus the verbatim original text as `SOURCE` (D46).
/// 与 [`value_to_amberice_with`] 相同，另把原文写进 `SOURCE`（D46）。
///
/// Without the `amberice-source` feature this refuses explicitly instead of silently dropping
/// the text. ｜ 未启用 `amberice-source` 时明确拒绝，绝不静默丢弃原文。
pub fn value_to_amberice_source_with(
    value: &Value,
    compress: Option<i32>,
    dir_depth: Option<u32>,
    source: &str,
) -> Result<Vec<u8>, EnvelopeError> {
    #[cfg(feature = "amberice-source")]
    {
        let opts = DumpOptions {
            compress,
            dir: dir_depth.is_some(),
            dir_depth: dir_depth.unwrap_or(amberice::manifest::DEFAULT_DIR_DEPTH),
            ..Default::default()
        };
        let cv = amberice::CanonicalValue::try_from(value)
            .map_err(|e| EnvelopeError::Container(ContainerError::Canonical(e)))?;
        Ok(amberice::dump_with_source(&cv, &opts, source)?)
    }
    #[cfg(not(feature = "amberice-source"))]
    {
        let _ = (value, compress, dir_depth, source);
        Err(EnvelopeError::Container(
            ContainerError::SourceFeatureDisabled,
        ))
    }
}

/// Envelope → SML text. ｜ 信封 → SML 文本。
///
/// Only works when the primary payload is a `Value`; a bytes envelope says so explicitly
/// instead of pretending to be an empty document.
/// 只有主载荷是 `Value` 时才行；字节信封会**明确说明**，不会假装是空文档。
pub fn amberice_to_sml(bytes: &[u8]) -> Result<String, EnvelopeError> {
    let v = amberice::load(bytes)?;
    Ok(crate::to_sml(&v))
}
