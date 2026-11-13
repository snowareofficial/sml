// SPDX-License-Identifier: MulanPSL-2.0
//! Hash dispatch by §20 algorithm id — **SHA-256 default, SM3 and BLAKE3 implemented**.
//! 按 §20 的算法编号分派摘要 —— **默认 SHA-256，SM3 与 BLAKE3 均已实现**。
//!
//! Why a dispatcher at all / 为什么要这一层：段头里那 32 B 摘要**不带算法标记**（§19.1 固定
//! 32 B，§20.1 第 2 条：三个算法的摘要都是 32 B）⇒ 算法只能由**文件自己声明**（manifest 的
//! `hash_id`），读者照它算。所以「算出摘要」这一步必须能按 id 走，而不是硬写 SHA-256 ——
//! 否则「可选 SM3」就只是一句空话。
//!
//! 三条不许越界 / three lines not to cross：
//!
//! 1. **未知 id 明确拒绝**（§20「编号一经发布永不复用」+ 未知算法跳过等于拿错算法验签）；
//! 2. **登记 ≠ 实现**：表里有号但本构建算不出 ⇒ [`HashError::NotImplemented`]，不是一个
//!    「大概也行」的默认（`0x32` 曾长期处于这个状态，实现见 [`crate::blake3`]）；
//! 3. **默认只影响写、不影响读**：写文件时默认 SHA-256；读文件一律以 manifest 的 `hash_id` 为准
//!    （`hash_id` 缺失时也退回默认，那是 T1 时代没有 manifest 的文件）。
//!
//! # 分组命名空间 / Group namespaces（§20.4）
//!
//! 一个字节只有 256 个号，若「一个算法占一个号」很快就会耗尽、逼着改段布局 —— 那是格式寿命
//! 的尽头。故 §20 把编号按**区间分组**：`0x30..=0x3F` 是哈希组，别组不得复用组内号码；
//! 组内末两号固定语义：`0x3E` = **扩展命名空间转义**（后面再跟子编号，形成二级命名空间）、
//! `0x3F` = **私有/实验**（不进互通文件）。中间未分配的号统一登记为「预留」，
//! 一律 `NotImplemented`，永不回收、永不挪作它用。
//! ｜ One byte cannot name every future algorithm. §20 therefore allocates **ranges as
//! namespaces**: `0x30..=0x3F` belongs to hashes, `0x3E` escapes into a sub-namespace and
//! `0x3F` is private/experimental. Unassigned slots stay reserved forever.

/// 哈希组的编号区间（`§20.4` 的分组命名空间）｜ The hash group's id range.
/// ｜ 组外编号一律 `Unknown`，组内未分配编号一律 `Reserved`。
pub const HASH_GROUP_FIRST: u8 = 0x30;
pub const HASH_GROUP_LAST: u8 = 0x3F;
/// 组内转义号：进入二级命名空间（本构建未实现，读到即 `NotImplemented`）。
/// ｜ Escape into a second-level namespace (not implemented in this build).
pub const HASH_ESCAPE: u8 = 0x3E;
/// 组内私有/实验号：不得出现在互通文件里。｜ Private/experimental; never in interoperable files.
pub const HASH_PRIVATE: u8 = 0x3F;

/// Algorithms from §20's hash range (all produce 32-byte digests).
/// §20 哈希区间的算法（都产出 32 B 摘要）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum HashAlgo {
    /// `0x30` — SHA-256 (FIPS 180-4). ｜ `0x30` ✓ 已实现。
    Sha256,
    /// `0x31` — SM3 (GB/T 32905-2016). ｜ `0x31` ✓ 已实现。
    Sm3,
    /// `0x32` — BLAKE3 (unkeyed 32 B digest; see [`crate::blake3`]).
    /// `0x32` ✓ 已实现（无密钥 32 B 摘要，见 [`crate::blake3`]）。
    Blake3,
    /// `0x33` — **预留**：国密 SM 系列抗量子 / 量子安全哈希（算法待国标发布）。
    /// ｜ Reserved for a future Chinese-standard (SM family) post-quantum digest.
    SmPqcDigest,
    /// `0x34` — **预留**：通用后量子摘要（非国密路线，算法待定）。
    /// ｜ Reserved for a non-SM post-quantum digest.
    PqcDigest,
    /// `0x3E` — **预留**：扩展命名空间转义（二级编号，§20.4）。
    /// ｜ Reserved: escape into a second-level namespace.
    ExtendedNamespace,
    /// `0x3F` — **预留**：私有 / 实验，不进互通文件。
    /// ｜ Reserved: private/experimental, never interoperable.
    PrivateUse,
    /// `0x35..=0x3D` — **预留**：组内未分配槽（统一表达，逐个号仍可反查）。
    /// ｜ Unassigned slots inside the hash group, kept reserved forever.
    Reserved(u8),
}

/// What this build writes when the caller does not choose. ｜ 调用方不指定时的写入口径。
pub const DEFAULT: HashAlgo = HashAlgo::Sha256;

impl HashAlgo {
    /// §20 algorithm id. ｜ §20 的算法编号。
    pub const fn id(self) -> u8 {
        match self {
            HashAlgo::Sha256 => 0x30,
            HashAlgo::Sm3 => 0x31,
            HashAlgo::Blake3 => 0x32,
            HashAlgo::SmPqcDigest => 0x33,
            HashAlgo::PqcDigest => 0x34,
            HashAlgo::ExtendedNamespace => HASH_ESCAPE,
            HashAlgo::PrivateUse => HASH_PRIVATE,
            HashAlgo::Reserved(n) => n,
        }
    }

    /// Map a §20 id back; `None` = unknown id (must be rejected, never guessed).
    /// 反查编号：`None` = 组外 / 未知编号（必须拒绝，绝不猜）。
    /// 组内**全部** 16 个号都有归属 —— 未分配的也登记成 `Reserved`，不是「不知道」。
    pub const fn from_id(id: u8) -> Option<HashAlgo> {
        match id {
            0x30 => Some(HashAlgo::Sha256),
            0x31 => Some(HashAlgo::Sm3),
            0x32 => Some(HashAlgo::Blake3),
            0x33 => Some(HashAlgo::SmPqcDigest),
            0x34 => Some(HashAlgo::PqcDigest),
            0x35..=0x3D => Some(HashAlgo::Reserved(id)),
            HASH_ESCAPE => Some(HashAlgo::ExtendedNamespace),
            HASH_PRIVATE => Some(HashAlgo::PrivateUse),
            _ => None,
        }
    }

    /// Human-readable name for tool output. ｜ 给工具看的名字。
    pub const fn name(self) -> &'static str {
        match self {
            HashAlgo::Sha256 => "SHA-256",
            HashAlgo::Sm3 => "SM3",
            HashAlgo::Blake3 => "BLAKE3",
            HashAlgo::SmPqcDigest => "SM-PQC-DIGEST (reserved)",
            HashAlgo::PqcDigest => "PQC-DIGEST (reserved)",
            HashAlgo::ExtendedNamespace => "extended namespace (reserved)",
            HashAlgo::PrivateUse => "private/experimental (reserved)",
            HashAlgo::Reserved(_) => "reserved (unassigned)",
        }
    }

    /// Does this build actually compute it? ｜ 本构建真能算吗？
    pub const fn is_implemented(self) -> bool {
        matches!(self, HashAlgo::Sha256 | HashAlgo::Sm3 | HashAlgo::Blake3)
    }

    /// 已登记但故意留空的号（含转义 / 私有 / 未分配槽）。
    /// ｜ Registered but intentionally unimplemented; readers must refuse explicitly.
    pub const fn is_reserved(self) -> bool {
        !self.is_implemented()
    }
}

/// 多级转义路径：manifest 里写成 `[0x3E, ns_version, sub1, sub2, …]`（§20.3.3）。
///
/// 用户 2026-11-13 裁决：`hash` 可以是**二元组**，且将来可能三级；**第一个子号是命名空间
/// 版本号**（1 起、只增），其后每级 2 字节子号。这样一级号用尽后不必改段头 / 段布局，
/// 靠「版本 + 多级子号」延长格式寿命。
/// ｜ Multi-level escape path: `[ESCAPE, ns_version, sub1, sub2, ...]`. The first sub-value
/// is the **namespace version** (starts at 1, only increases); every further level is a
/// 2-byte sub-id. Registering the escape keeps a one-byte `hash_id` from capping the format.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct HashPath {
    /// 二级命名空间的版本（1 起，只增）｜ Namespace version, starts at 1.
    pub ns_version: u8,
    /// 逐级子号（每级 2 字节，至少一级）｜ Per-level sub-ids, 2 bytes each, at least one.
    pub subs: Vec<u16>,
}

/// 路径最多 4 级：够用且能挡住畸形 manifest 塞超长数组。
/// ｜ At most four levels: enough, and it rejects absurd manifests.
pub const MAX_HASH_SUBS: usize = 4;

/// `hash` 元组写不出来 / 读不懂的原因。
/// ｜ Why a `hash` tuple could not be encoded or decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum HashPathError {
    /// Not an array ｜ 不是数组。
    NotAnArray,
    /// First element is not the escape id ｜ 首元素不是转义号。
    BadEscape(i64),
    /// Namespace version out of `0..=255` ｜ 版本号越界。
    BadVersion(i64),
    /// No sub-id after the version ｜ 版本号之后没有子号。
    EmptyPath,
    /// More than [`MAX_HASH_SUBS`] levels ｜ 子号级数过多。
    TooManySubs(usize),
    /// A sub-id is outside `0..=65535` ｜ 子号越界（每级 2 字节）。
    SubOutOfRange(i64),
}

impl std::fmt::Display for HashPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HashPathError::NotAnArray => write!(f, "hash 元组必须是数组"),
            HashPathError::BadEscape(v) => {
                write!(f, "hash 元组首元素必须是转义号 0x{HASH_ESCAPE:02x}，读到 {v}")
            }
            HashPathError::BadVersion(v) => write!(f, "命名空间版本号越界：{v}（应在 0..=255）"),
            HashPathError::EmptyPath => write!(f, "hash 元组缺少子号"),
            HashPathError::TooManySubs(n) => {
                write!(f, "hash 元组子号级数 {n} 超过上限 {MAX_HASH_SUBS}")
            }
            HashPathError::SubOutOfRange(v) => {
                write!(f, "子号越界：{v}（每级 2 字节，应在 0..=65535）")
            }
        }
    }
}

impl std::error::Error for HashPathError {}

impl HashPath {
    /// 编成 manifest 里的值：`[0x3E, version, sub…]`。
    /// ｜ Encode as the manifest value `[ESCAPE, version, sub...]`.
    pub fn to_value(&self) -> crate::Value {
        let mut items = Vec::with_capacity(2 + self.subs.len());
        items.push(crate::Value::Int(HASH_ESCAPE as i64));
        items.push(crate::Value::Int(self.ns_version as i64));
        for s in &self.subs {
            items.push(crate::Value::Int(*s as i64));
        }
        crate::Value::Array(items)
    }

    /// 从 manifest 的值读回来，并做全部形状 / 范围校验。
    /// ｜ Decode from the manifest value, validating shape and ranges.
    pub fn from_value(v: &crate::Value) -> Result<Self, HashPathError> {
        let items = match v {
            crate::Value::Array(a) => a,
            _ => return Err(HashPathError::NotAnArray),
        };
        let int_at = |i: usize| match items.get(i) {
            Some(crate::Value::Int(n)) => Some(*n),
            _ => None,
        };
        let escape = int_at(0).ok_or(HashPathError::NotAnArray)?;
        if escape != HASH_ESCAPE as i64 {
            return Err(HashPathError::BadEscape(escape));
        }
        let version = int_at(1).ok_or(HashPathError::BadVersion(-1))?;
        if !(0..=255).contains(&version) {
            return Err(HashPathError::BadVersion(version));
        }
        let raw = &items[2..];
        if raw.is_empty() {
            return Err(HashPathError::EmptyPath);
        }
        if raw.len() > MAX_HASH_SUBS {
            return Err(HashPathError::TooManySubs(raw.len()));
        }
        let mut subs = Vec::with_capacity(raw.len());
        for item in raw {
            let n = match item {
                crate::Value::Int(n) => *n,
                _ => return Err(HashPathError::SubOutOfRange(-1)),
            };
            if !(0..=65535).contains(&n) {
                return Err(HashPathError::SubOutOfRange(n));
            }
            subs.push(n as u16);
        }
        Ok(HashPath { ns_version: version as u8, subs })
    }

    /// 人读形式（工具输出用）｜ Human-readable form for tool output.
    pub fn name(&self) -> String {
        let subs: Vec<String> = self.subs.iter().map(|s| format!("0x{s:04x}")).collect();
        format!("escape v{} / {}", self.ns_version, subs.join("."))
    }
}

/// Why a digest could not be produced. ｜ 算不出摘要的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum HashError {
    /// The id is not in §20 at all ｜ 编号不在 §20 表里。
    Unknown(u8),
    /// The id is in §20 but this build does not implement it ｜ 表里有号但本构建没实现。
    NotImplemented(u8),
}

impl std::fmt::Display for HashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HashError::Unknown(id) => write!(f, "未知哈希算法 id 0x{id:02x}（§20 表里没有）"),
            HashError::NotImplemented(id) => write!(
                f,
                "哈希算法 id 0x{id:02x}（{}）已登记但本构建未实现，拒绝猜一个算法去验",
                HashAlgo::from_id(*id).map(|a| a.name()).unwrap_or("?")
            ),
        }
    }
}

impl std::error::Error for HashError {}

/// Compute a 32-byte digest with the requested algorithm.
/// 用指定算法算 32 B 摘要。
pub fn digest(algo: HashAlgo, data: &[u8]) -> Result<[u8; 32], HashError> {
    match algo {
        HashAlgo::Sha256 => Ok(crate::sha256::digest(data)),
        HashAlgo::Sm3 => Ok(crate::sm3::digest(data)),
        HashAlgo::Blake3 => Ok(crate::blake3::digest(data)),
        // 所有预留位（含转义号、私有号、未分配槽）统一明确拒绝，
        // 绝不退回 SHA-256 假装验过。`Reserved(n)` 的号原样带进错误里便于排查。
        other => Err(HashError::NotImplemented(other.id())),
    }
}

/// Same, but from a raw §20 id — the form a file gives us.
/// 同上，但输入是文件里那个 §20 编号。
pub fn digest_by_id(id: u8, data: &[u8]) -> Result<[u8; 32], HashError> {
    let algo = HashAlgo::from_id(id).ok_or(HashError::Unknown(id))?;
    digest(algo, data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for algo in [HashAlgo::Sha256, HashAlgo::Sm3, HashAlgo::Blake3] {
            assert_eq!(HashAlgo::from_id(algo.id()), Some(algo));
        }
    }

    #[test]
    fn every_slot_in_the_hash_group_is_registered() {
        // 组内 16 个号全部有归属：3 个实现 + 13 个预留（含转义 / 私有）。
        for id in HASH_GROUP_FIRST..=HASH_GROUP_LAST {
            let algo = HashAlgo::from_id(id).unwrap_or_else(|| panic!("0x{id:02x} 未登记"));
            assert_eq!(algo.id(), id, "0x{id:02x} 反查后编号漂移");
            if id <= 0x32 {
                assert!(algo.is_implemented() && !algo.is_reserved());
            } else {
                assert!(algo.is_reserved() && !algo.is_implemented(), "0x{id:02x}");
                assert_eq!(
                    digest_by_id(id, b"abc"),
                    Err(HashError::NotImplemented(id)),
                    "0x{id:02x} 必须明确拒绝"
                );
            }
        }
    }

    #[test]
    fn named_reservations_keep_their_ids() {
        assert_eq!(HashAlgo::SmPqcDigest.id(), 0x33);
        assert_eq!(HashAlgo::PqcDigest.id(), 0x34);
        assert_eq!(HashAlgo::ExtendedNamespace.id(), HASH_ESCAPE);
        assert_eq!(HashAlgo::PrivateUse.id(), HASH_PRIVATE);
        assert_eq!(HashAlgo::from_id(0x35), Some(HashAlgo::Reserved(0x35)));
        assert_eq!(HashAlgo::from_id(0x3D), Some(HashAlgo::Reserved(0x3D)));
    }

    #[test]
    fn group_boundaries_are_unknown_not_reserved() {
        // 组外编号不能因为「多预留」而被吞掉：分组命名空间的意义就在于边界清楚。
        assert_eq!(digest_by_id(0x2F, b"x"), Err(HashError::Unknown(0x2F)));
        assert_eq!(digest_by_id(0x40, b"x"), Err(HashError::Unknown(0x40)));
        assert_eq!(digest_by_id(0x99, b"x"), Err(HashError::Unknown(0x99)));
    }

    #[test]
    fn hash_path_two_and_three_levels_round_trip() {
        // 用户裁决：`[0x3E, ns_version, sub…]`，版本号在前，最多 4 级。
        let two = HashPath { ns_version: 1, subs: vec![0x0001] };
        let three = HashPath { ns_version: 1, subs: vec![0x0001, 0x0002] };
        for p in [two, three] {
            let v = p.to_value();
            assert_eq!(HashPath::from_value(&v), Ok(p));
        }
    }

    #[test]
    fn hash_path_rejects_malformed_values() {
        use crate::Value;
        let cases = [
            (Value::Int(0x3E), HashPathError::NotAnArray),
            (Value::Array(vec![Value::Int(0x30), Value::Int(1), Value::Int(1)]),
             HashPathError::BadEscape(0x30)),
            (Value::Array(vec![Value::Int(0x3E), Value::Int(256), Value::Int(1)]),
             HashPathError::BadVersion(256)),
            (Value::Array(vec![Value::Int(0x3E), Value::Int(1)]),
             HashPathError::EmptyPath),
            (Value::Array(vec![Value::Int(0x3E), Value::Int(1), Value::Int(70000)]),
             HashPathError::SubOutOfRange(70000)),
            (Value::Array(vec![
                Value::Int(0x3E), Value::Int(1), Value::Int(1), Value::Int(2),
                Value::Int(3), Value::Int(4), Value::Int(5),
            ]), HashPathError::TooManySubs(5)),
        ];
        for (v, want) in cases {
            assert_eq!(HashPath::from_value(&v), Err(want));
        }
    }

    #[test]
    fn three_implemented_algorithms_differ() {
        let d = [
            digest(HashAlgo::Sha256, b"abc").unwrap(),
            digest(HashAlgo::Sm3, b"abc").unwrap(),
            digest(HashAlgo::Blake3, b"abc").unwrap(),
        ];
        assert_ne!(d[0], d[1]);
        assert_ne!(d[0], d[2]);
        assert_ne!(d[1], d[2]);
    }
}
