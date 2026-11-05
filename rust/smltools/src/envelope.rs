// SPDX-License-Identifier: MulanPSL-2.0
//! `.smlb` as a **general-purpose envelope**: wrap arbitrary bytes, unwrap them, and convert
//! SML text to/from the `VALUE` payload.
//! 把 `.smlb` 当**通用信封**用：装任意字节、取出、以及 SML 文本 ⇄ `VALUE` 载荷。
//!
//! 定位（`rust/smlb/README.md`，2026-11-05 起）：`.smlb` 是**不可变、可寻址、可验证的通用二进制
//! 信封**，SML 只是内置载荷之一。所以本模块的第一等公民是 `bytes ⇄ 信封`，
//! SML 文本只是「载荷之一」的便捷入口 —— 不要把 `sml_to_smlb` 当成主路径。
//! ｜ Envelope first, SML payload second — that order is the whole point of the repositioning.

use std::fmt;

use smlb::{ContainerError, DumpOptions, ManifestPayload, Value};

/// Why an envelope operation failed. ｜ 信封操作失败的原因。
///
/// 四类分开报，因为它们**处置方式不同**：解析错要去改文本、容器错说明文件有问题、
/// 载荷名不存在要问清楚名字、根本不是信封要看扩展名是不是给错了。
#[derive(Debug)]
#[non_exhaustive]
pub enum EnvelopeError {
    /// SML text did not parse ｜ SML 文本解析失败。
    Parse(sml_codes::SmlError),
    /// The `.smlb` container refused (bad layout, digest, capability, …)
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

/// Wrap arbitrary bytes into a `.smlb` envelope.
/// 把任意字节装进 `.smlb` 信封。
///
/// `name` is what [`unwrap`] will ask for; for a single file the plain file name is the
/// obvious choice. Compression is opt-in (`compress` = zstd level).
/// `name` 就是将来 [`unwrap`] 要问的名字；单个文件通常直接用文件名。
pub fn wrap(name: &str, bytes: &[u8], compress: Option<i32>) -> Result<Vec<u8>, EnvelopeError> {
    let opts = DumpOptions {
        compress,
        ..Default::default()
    };
    Ok(smlb::pack_with(&[(name, bytes)], &opts)?)
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
    Ok(smlb::payload(bytes, &name)?.into_owned())
}

/// List the payloads of an envelope (cheap: reads the manifest only).
/// 列出信封里的载荷（很便宜：只读 manifest）。
pub fn list(bytes: &[u8]) -> Result<Vec<Payload>, EnvelopeError> {
    let declared: Vec<ManifestPayload> = smlb::payloads(bytes)?;
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
/// 这是「SML 是内置载荷之一」的具体落点：和 [`wrap`] 产出的是同一种文件，只是载荷类型不同，
/// 故 `smlb cat` / `smlb check` 对两者一样有效。
/// Same file format as [`wrap`], different payload kind — so `smlb cat` / `check` work on both.
pub fn sml_to_smlb(text: &str, compress: Option<i32>) -> Result<Vec<u8>, EnvelopeError> {
    let v: Value = sml::parse(text).map_err(EnvelopeError::Parse)?;
    let opts = DumpOptions {
        compress,
        ..Default::default()
    };
    Ok(smlb::dump_with(&v, &opts)?)
}

/// Envelope → SML text. ｜ 信封 → SML 文本。
///
/// Only works when the primary payload is a `Value`; a bytes envelope says so explicitly
/// instead of pretending to be empty.
/// 只有主载荷是 `Value` 时才行；字节信封会**明确说明**，不会假装是空文档。
pub fn smlb_to_sml(bytes: &[u8]) -> Result<String, EnvelopeError> {
    let v = smlb::load(bytes)?;
    Ok(sml::to_sml(&v))
}
