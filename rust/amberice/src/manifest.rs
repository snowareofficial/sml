// SPDX-License-Identifier: MulanPSL-2.0
//! The `MANIFEST` segment (`0x04`) — the file's self-description (D10 / P8 / P9).
//! `MANIFEST` 段 —— 文件的自描述（D10 / P8 / P9）。
//!
//! # 为什么它值得单独一层
//!
//! 头 + 段表能回答「有哪些段、各多长」，答不了这五件事：
//!
//! 1. **主内容是哪个段**（P8：`primary` 永远有值，`.ambr` 才能兼作通用容器）；
//! 2. **摘要用哪个算法**（§20：段头那 32 B 没有算法标记，只能由文件自己说）；
//! 3. **段头的算法/长度能进签名**（§18.3；段表本身不进签名，因为偏移随追加而变）；
//! 4. **能力声明**（D11：读不懂必须拒 / 可跳过）；
//! 5. **默认值以文件为准**（§21 / D37：改默认值不影响旧文件）。
//!
//! # 编码
//!
//! manifest 就是**一个 `Value`（Object）**，用 [`sml_value::Value::canonical_bytes`] 落成
//! **自包含的 L0 形态**（`count ‖ STRTAB ‖ 节点树`）。为什么这里用 L0 而 `VALUE` 段用裸节点：
//! manifest 必须在**还不知道 `STRTAB` 在哪**的时候就能读 —— 引导顺序决定了它不能依赖别的段。
//! ｜ The manifest is encoded as a self-contained L0 blob so it can be read before anything else.
//!
//! 收益：零新增编解码器（复用 T0 的 canonical），且 `--with-mirror` 想导出可读版本时，
//! 直接把同一个 `Value` 用 `to_sml` 写出来即可（P9 的 `MANIFEST_SML` 镜像段）。
//!
//! # 三处容易踩的点
//!
//! 1. **`segments[]` 不含 manifest 自己**：含自己就是「我的摘要要算进我自己的字节」，循环。
//!    与 §18.4「排除 `SIG` 段自身」同一个道理。
//! 2. **`segments[]` 与段表是故意重复的**：段表负责定位（含偏移、不进签名），
//!    `segments[]` 负责可签（算法/长度/摘要、按 id 升序）。两者不一致 ⇒ `check` 报错，
//!    重复本身就是一次交叉验证的机会。
//! 3. **`created` / `tool` 只展示**（D17：系统时间不可信）；它们进不了验证的必要条件。

use sml_value::{CanonicalError, Map, Value};

use crate::hash::HashAlgo;

/// Segment id of the manifest itself ｜ manifest 自己的段 id。
pub const SEG_MANIFEST: u8 = 0x04;

/// One entry of `segments[]` — the **signable** view of a segment (no offset, §18.3).
/// `segments[]` 的一条 —— 段的**可签**视图（不含偏移，§18.3）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestSegment {
    /// Segment kind ｜ 段种类。
    pub id: u8,
    /// Compression algorithm id ｜ 压缩算法 id。
    pub comp: u8,
    /// Encryption algorithm id ｜ 加密算法 id。
    pub enc: u8,
    /// Stored length ｜ 落盘长度。
    pub len: u32,
    /// Digest (algorithm from [`Manifest::hash`]) ｜ 摘要（算法由 `hash` 决定）。
    pub digest: [u8; 32],
}

/// One entry of `payloads[]` — a foreign blob inside the `EMBED` segment (D23).
/// `payloads[]` 的一条 —— `EMBED` 段里的一段外来载荷（D23）。
///
/// 为什么载荷要单独一张表、而不是「一个文件一个 `EMBED` 段」：段是按 `seg_id` 认的
/// （`segments[]` 也按 id 立条目），同种段来两个就说不清谁是谁。故约定
/// **一个文件至多一个 `EMBED` 段**，段体是各载荷按序拼接，`payloads[]` 按序描述它们
/// —— 偏移不存，靠 `len` 走出来（与段表同一套思路）。
/// ｜ One `EMBED` segment per file; the body is the payloads concatenated in order, and
/// `payloads[]` describes them in the same order (offsets are walked, never stored).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestPayload {
    /// File/part name, e.g. `notes/2026.md` (informational + lookup key) ｜ 名字（查找键）。
    pub name: String,
    /// Payload length ｜ 载荷长度。
    pub len: u32,
    /// Digest of the payload bytes ｜ 载荷字节摘要。
    pub digest: [u8; 32],
}

/// Default `dir_depth` (§21: `DIR` indexes depth ≤ 4). ｜ 默认 `dir_depth`。
pub const DEFAULT_DIR_DEPTH: u32 = 4;
/// Default crypto block size (§21 / D28: 4 KiB). ｜ 默认加密块。
pub const DEFAULT_CRYPTO_BLOCK: u32 = 4096;
/// Default padding quantum (§21 / D28: 64 B). ｜ 默认量化档。
pub const DEFAULT_PAD_QUANTUM: u32 = 64;
/// Default patch strategy (§21 / D22). ｜ 默认补丁策略。
pub const DEFAULT_PATCH_STRATEGY: &str = "override";

/// The manifest (a `Value` in memory, T0 bytes on disk). ｜ manifest 的内存形态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// `primary`: which segment holds the main content (P8) ｜ 主内容所在段 id。
    pub primary: u8,
    /// `hash_id`: digest algorithm for every segment header (§20) ｜ 摘要算法。
    /// 一级形态时就是它；多级转义时 `hash == HashAlgo::ExtendedNamespace` 且
    /// [`Manifest::hash_path`] 带 `[版本, 子号…]`。
    pub hash: HashAlgo,
    /// 多级转义路径（`[0x3E, ns_version, sub…]`）；一级形态为 `None`。
    /// ｜ Multi-level escape path; `None` for the one-level form.
    pub hash_path: Option<crate::hash::HashPath>,
    /// Signable segment list, ascending id, **excluding this manifest** ｜ 可签段清单（升序、不含自己）。
    pub segments: Vec<ManifestSegment>,
    /// Foreign payloads inside the `EMBED` segment, in body order (D23)
    /// `EMBED` 段里的外来载荷，按段体内顺序（D23）。
    pub payloads: Vec<ManifestPayload>,
    /// `caps.required`: unknown ⇒ refuse (D11) ｜ 读不懂必须拒。
    pub caps_required: Vec<String>,
    /// `caps.optional`: unknown ⇒ skip (D11) ｜ 读不懂可跳过。
    pub caps_optional: Vec<String>,
    /// `patch.strategy` ｜ 补丁策略。
    pub patch_strategy: String,
    /// `defaults.dir_depth` ｜ `DIR` 索引深度。
    pub dir_depth: u32,
    /// `defaults.crypto_block` ｜ 加密块大小。
    pub crypto_block: u32,
    /// `defaults.pad_quantum` ｜ 长度量化档。
    pub pad_quantum: u32,
    /// `crypto.enc` (§20) ｜ 加密算法。
    pub crypto_enc: u8,
    /// `crypto.sig` (§20; array — D36: 全部必须通过) ｜ 签名算法数组。
    pub crypto_sig: Vec<u8>,
    /// `crypto.kem` (§20) ｜ KEM 算法。
    pub crypto_kem: u8,
    /// `crypto.kdf` (§20: `0x00` none / `0x51` Argon2id) ｜ 口令派生算法。
    pub kdf_kind: u8,
    /// Argon2id parameters (only when `kdf_kind != 0`) ｜ 口令派生参数。
    pub kdf: Option<crate::crypto::KdfParams>,
    /// Per-file salt (hex in the value form); seeds nonce + KDF ｜ 每文件一个盐。
    pub file_salt: Option<[u8; 16]>,
    /// `created` — display only (D17) ｜ 时间戳，只展示。
    pub created: Option<String>,
    /// `tool` — display only (D12) ｜ 写入者，只展示。
    pub tool: Option<String>,
}

impl Manifest {
    /// A minimal manifest for `segments`, defaulting everything §21 gives a default.
    /// 给 `segments` 造一个最小 manifest，其余按 §21 的默认值。
    pub fn minimal(primary: u8, hash: HashAlgo, segments: Vec<ManifestSegment>) -> Manifest {
        Manifest {
            primary,
            hash,
            hash_path: None,
            segments,
            payloads: Vec::new(),
            caps_required: Vec::new(),
            caps_optional: Vec::new(),
            patch_strategy: DEFAULT_PATCH_STRATEGY.to_string(),
            dir_depth: DEFAULT_DIR_DEPTH,
            crypto_block: DEFAULT_CRYPTO_BLOCK,
            pad_quantum: DEFAULT_PAD_QUANTUM,
            crypto_enc: 0,
            crypto_sig: Vec::new(),
            crypto_kem: 0,
            kdf_kind: 0,
            kdf: None,
            file_salt: None,
            created: None,
            tool: None,
        }
    }

    /// Serialize to the manifest segment body (self-contained L0 bytes).
    /// 落成 manifest 段体（自包含的 L0 字节）。
    pub fn to_bytes(&self) -> Result<Vec<u8>, CanonicalError> {
        self.to_value().canonical_bytes()
    }

    /// Parse from the manifest segment body. ｜ 从 manifest 段体解析。
    ///
    /// **Loose on purpose** (D10): unknown keys are ignored, missing optional keys take §21's
    /// defaults. A missing `primary` / `hash_id` / `segments` is an error — those three are the
    /// reason the manifest exists at all.
    /// **故意宽松**（D10）：未知键忽略、缺省键取 §21 默认值；但 `primary` / `hash_id` /
    /// `segments` 三者缺一即错 —— 它们正是 manifest 存在的理由。
    pub fn from_bytes(bytes: &[u8]) -> Result<Manifest, ManifestError> {
        let v = Value::from_canonical_bytes(bytes).map_err(ManifestError::Canonical)?;
        Manifest::from_value(&v)
    }

    /// Encode as a `Value` (the SML-mirror-friendly form). ｜ 编成 `Value`（可导出成 SML 的形态）。
    ///
    /// Field order is **fixed** by construction ⇒ the bytes are deterministic (C12 applies to the
    /// manifest too, because it is what signatures cover).
    /// 字段顺序**固定** ⇒ 字节确定（manifest 也是签名对象，同样受 C12 约束）。
    pub fn to_value(&self) -> Value {
        let mut m = Map::ordered();
        m.insert("primary".to_string(), Value::Int(self.primary as i64));
        // 一级写整数（保持既有字节形态）；多级写 `[0x3E, ns_version, sub…]`。
        // ｜ One level stays an integer; multi-level is the escape tuple.
        let hash_value = match &self.hash_path {
            Some(p) => p.to_value(),
            None => Value::Int(self.hash.id() as i64),
        };
        m.insert("hash_id".to_string(), hash_value);

        let segs: Vec<Value> = self
            .segments
            .iter()
            .map(|s| {
                let mut e = Map::ordered();
                e.insert("id".to_string(), Value::Int(s.id as i64));
                e.insert("comp".to_string(), Value::Int(s.comp as i64));
                e.insert("enc".to_string(), Value::Int(s.enc as i64));
                e.insert("len".to_string(), Value::Int(s.len as i64));
                e.insert("sha256".to_string(), Value::Str(hex(&s.digest)));
                Value::Object(e)
            })
            .collect();
        m.insert("segments".to_string(), Value::Array(segs));

        if !self.payloads.is_empty() {
            let list: Vec<Value> = self
                .payloads
                .iter()
                .map(|p| {
                    let mut e = Map::ordered();
                    e.insert("name".to_string(), Value::Str(p.name.clone()));
                    e.insert("len".to_string(), Value::Int(p.len as i64));
                    e.insert("sha256".to_string(), Value::Str(hex(&p.digest)));
                    Value::Object(e)
                })
                .collect();
            m.insert("payloads".to_string(), Value::Array(list));
        }

        let mut crypto = Map::ordered();
        crypto.insert("enc".to_string(), Value::Int(self.crypto_enc as i64));
        crypto.insert(
            "sig".to_string(),
            Value::Array(
                self.crypto_sig
                    .iter()
                    .map(|id| Value::Int(*id as i64))
                    .collect(),
            ),
        );
        crypto.insert("kem".to_string(), Value::Int(self.crypto_kem as i64));
        if self.kdf_kind != 0 {
            crypto.insert("kdf".to_string(), Value::Int(self.kdf_kind as i64));
            if let Some(k) = &self.kdf {
                crypto.insert("kdf_m".to_string(), Value::Int(k.m_cost as i64));
                crypto.insert("kdf_t".to_string(), Value::Int(k.t_cost as i64));
                crypto.insert("kdf_p".to_string(), Value::Int(k.p_cost as i64));
            }
        }
        if let Some(s) = &self.file_salt {
            crypto.insert("salt".to_string(), Value::Str(hex(s)));
        }
        m.insert("crypto".to_string(), Value::Object(crypto));

        let mut caps = Map::ordered();
        caps.insert("required".to_string(), str_array(&self.caps_required));
        caps.insert("optional".to_string(), str_array(&self.caps_optional));
        m.insert("caps".to_string(), Value::Object(caps));

        let mut patch = Map::ordered();
        patch.insert(
            "strategy".to_string(),
            Value::Str(self.patch_strategy.clone()),
        );
        m.insert("patch".to_string(), Value::Object(patch));

        let mut defaults = Map::ordered();
        defaults.insert("dir_depth".to_string(), Value::Int(self.dir_depth as i64));
        defaults.insert(
            "crypto_block".to_string(),
            Value::Int(self.crypto_block as i64),
        );
        defaults.insert(
            "pad_quantum".to_string(),
            Value::Int(self.pad_quantum as i64),
        );
        m.insert("defaults".to_string(), Value::Object(defaults));

        // 只展示的两个字段：有才写（写空串会让 canonical 输出多两行噪音）
        if let Some(c) = &self.created {
            m.insert("created".to_string(), Value::Str(c.clone()));
        }
        if let Some(t) = &self.tool {
            m.insert("tool".to_string(), Value::Str(t.clone()));
        }

        Value::Object(m)
    }

    /// Decode from a `Value`, applying §21 defaults for anything missing.
    /// 从 `Value` 解码，缺的按 §21 默认值补。
    pub fn from_value(v: &Value) -> Result<Manifest, ManifestError> {
        let obj = match v {
            Value::Object(m) => m,
            _ => return Err(ManifestError::NotAnObject),
        };
        let primary = int_field(obj, "primary")
            .ok_or(ManifestError::Missing("primary"))?;
        if !(0..=u8::MAX as i64).contains(&primary) {
            return Err(ManifestError::BadInt("primary"));
        }
        // `hash_id` 允许两种形态：整数（一级）或 `[0x3E, ns_version, sub…]`（多级）。
        // ｜ `hash_id` is either an integer or the escape tuple.
        let (hash, hash_path) = match obj.get("hash_id") {
            None => return Err(ManifestError::Missing("hash_id")),
            Some(Value::Int(n)) => {
                if !(0..=255).contains(n) {
                    return Err(ManifestError::BadShape("hash_id"));
                }
                let algo = HashAlgo::from_id(*n as u8)
                    .ok_or(ManifestError::UnknownHash(*n as u8))?;
                (algo, None)
            }
            Some(v @ Value::Array(_)) => {
                let path = crate::hash::HashPath::from_value(v)
                    .map_err(ManifestError::BadHashPath)?;
                // 多级一律解析成转义号：`digest` 会明确报 NotImplemented(0x3E)。
                (HashAlgo::ExtendedNamespace, Some(path))
            }
            Some(_) => return Err(ManifestError::BadShape("hash_id")),
        };

        let segs = match obj.get("segments") {
            Some(Value::Array(a)) => a,
            Some(_) => return Err(ManifestError::BadShape("segments")),
            None => return Err(ManifestError::Missing("segments")),
        };
        let mut segments = Vec::with_capacity(segs.len());
        for e in segs {
            let m = match e {
                Value::Object(m) => m,
                _ => return Err(ManifestError::BadShape("segments[]")),
            };
            let id = int_field(m, "id").ok_or(ManifestError::Missing("segments[].id"))?;
            let comp = int_field(m, "comp").unwrap_or(0);
            let enc = int_field(m, "enc").unwrap_or(0);
            let len = int_field(m, "len").ok_or(ManifestError::Missing("segments[].len"))?;
            let digest_hex = match m.get("sha256") {
                Some(Value::Str(s)) => s.clone(),
                Some(_) => return Err(ManifestError::BadShape("segments[].sha256")),
                None => return Err(ManifestError::Missing("segments[].sha256")),
            };
            let digest = from_hex(&digest_hex)
                .ok_or(ManifestError::BadShape("segments[].sha256（需 64 位十六进制）"))?;
            segments.push(ManifestSegment {
                id: id as u8,
                comp: comp as u8,
                enc: enc as u8,
                len: len as u32,
                digest,
            });
        }

        let mut payloads = Vec::new();
        if let Some(Value::Array(a)) = obj.get("payloads") {
            for e in a {
                let m = match e {
                    Value::Object(m) => m,
                    _ => return Err(ManifestError::BadShape("payloads[]")),
                };
                let name = match m.get("name") {
                    Some(Value::Str(s)) => s.clone(),
                    Some(_) => return Err(ManifestError::BadShape("payloads[].name")),
                    None => return Err(ManifestError::Missing("payloads[].name")),
                };
                let len = int_field(m, "len").ok_or(ManifestError::Missing("payloads[].len"))?;
                let digest_hex = match m.get("sha256") {
                    Some(Value::Str(s)) => s.clone(),
                    Some(_) => return Err(ManifestError::BadShape("payloads[].sha256")),
                    None => return Err(ManifestError::Missing("payloads[].sha256")),
                };
                let digest = from_hex(&digest_hex)
                    .ok_or(ManifestError::BadShape("payloads[].sha256（需 64 位十六进制）"))?;
                payloads.push(ManifestPayload {
                    name,
                    len: len as u32,
                    digest,
                });
            }
        }

        let crypto = obj.get("crypto").and_then(as_object);
        let caps = obj.get("caps").and_then(as_object);
        let patch = obj.get("patch").and_then(as_object);
        let defaults = obj.get("defaults").and_then(as_object);

        Ok(Manifest {
            primary: primary as u8,
            hash,
            hash_path,
            segments,
            caps_required: caps
                .and_then(|c| c.get("required"))
                .map(str_list)
                .unwrap_or_default(),
            caps_optional: caps
                .and_then(|c| c.get("optional"))
                .map(str_list)
                .unwrap_or_default(),
            payloads,
            patch_strategy: patch
                .and_then(|p| match p.get("strategy") {
                    Some(Value::Str(s)) => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| DEFAULT_PATCH_STRATEGY.to_string()),
            dir_depth: defaults
                .and_then(|d| int_field(d, "dir_depth"))
                .map(|n| n as u32)
                .unwrap_or(DEFAULT_DIR_DEPTH),
            crypto_block: defaults
                .and_then(|d| int_field(d, "crypto_block"))
                .map(|n| n as u32)
                .unwrap_or(DEFAULT_CRYPTO_BLOCK),
            pad_quantum: defaults
                .and_then(|d| int_field(d, "pad_quantum"))
                .map(|n| n as u32)
                .unwrap_or(DEFAULT_PAD_QUANTUM),
            crypto_enc: crypto.and_then(|c| int_field(c, "enc")).unwrap_or(0) as u8,
            crypto_sig: crypto
                .and_then(|c| match c.get("sig") {
                    Some(Value::Array(a)) => Some(
                        a.iter()
                            .filter_map(|v| v.as_i64())
                            .map(|n| n as u8)
                            .collect::<Vec<u8>>(),
                    ),
                    _ => None,
                })
                .unwrap_or_default(),
            crypto_kem: crypto.and_then(|c| int_field(c, "kem")).unwrap_or(0) as u8,
            kdf_kind: crypto.and_then(|c| int_field(c, "kdf")).unwrap_or(0) as u8,
            kdf: crypto.and_then(|c| {
                Some(crate::crypto::KdfParams {
                    m_cost: int_field(c, "kdf_m")? as u32,
                    t_cost: int_field(c, "kdf_t")? as u32,
                    p_cost: int_field(c, "kdf_p")? as u32,
                })
            }),
            file_salt: crypto.and_then(|c| match c.get("salt") {
                Some(Value::Str(s)) => from_hex16(s),
                _ => None,
            }),
            created: obj.get("created").and_then(|v| match v {
                Value::Str(s) => Some(s.clone()),
                _ => None,
            }),
            tool: obj.get("tool").and_then(|v| match v {
                Value::Str(s) => Some(s.clone()),
                _ => None,
            }),
        })
    }
}

/// How this build classifies one declared capability name (D52).
/// 本构建把一个能力名归到哪一类（D52）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapStatus {
    /// Understood by this build. ｜ 认得。
    Known,
    /// Registered in the spec, but this build was compiled without it (feature pruned).
    /// 规范已登记，但本构建被裁剪掉了。
    Pruned,
    /// Not in the spec's registry at all — a writer's extension name.
    /// 规范里根本没登记 —— 写者的扩展名。
    Unregistered,
}

/// The capability registry: `known` (this build) vs `registered` (the spec), D52.
/// 能力注册表：`known`（本构建）vs `registered`（规范），D52。
///
/// `verdict()` is the **only** predicate; pruning is "pass a smaller `known`", not a second
/// code path. ｜ `verdict()` 是唯一判据；裁剪 = 传更小的 `known`，不写第二条路径。
#[derive(Debug, Clone)]
pub struct CapabilityRegistry<'a> {
    /// Names this build actually understands; shrinks with features.
    /// 本构建真认得的能力；随 feature 收缩。
    pub known: Vec<&'a str>,
    /// Names the spec has registered (superset of `known`, plus future names).
    /// 规范已登记的名字（`known` 的超集，含未来能力）。
    pub registered: &'static [&'static str],
}

impl CapabilityRegistry<'static> {
    /// This build's registry. ｜ 本构建的注册表。
    pub fn current() -> CapabilityRegistry<'static> {
        // 基础读取能力常开：读段/值、`EMBED`、`DIR`、`SOURCE`、`TYPED` 都不需要额外 feature。
        #[allow(unused_mut)]
        let mut known: Vec<&'static str> =
            vec!["value", "strtab", "manifest", "embed", "dir", "source", "typed"];
        #[cfg(feature = "sign")]
        known.push("sig");
        #[cfg(feature = "compress")]
        known.push("frames");
        #[cfg(feature = "crypto")]
        known.push("crypto");
        CapabilityRegistry {
            known,
            registered: registered_capabilities(),
        }
    }
}

impl<'a> CapabilityRegistry<'a> {
    /// Classify one name. ｜ 给一个名字分类。
    pub fn status(&self, name: &str) -> CapStatus {
        if self.known.contains(&name) {
            CapStatus::Known
        } else if self.registered.contains(&name) {
            CapStatus::Pruned
        } else {
            CapStatus::Unregistered
        }
    }

    /// Apply a manifest's `caps` — the one predicate both `load` and `check` use.
    /// 用本注册表判定 manifest 的 `caps` —— `load` 与 `check` 共用的唯一判据。
    pub fn verdict(&self, manifest: &Manifest) -> CapsVerdict {
        let mut entries = Vec::new();
        let mut missing_required = Vec::new();
        let mut ignored_optional = Vec::new();
        for name in &manifest.caps_required {
            let st = self.status(name);
            if st != CapStatus::Known {
                missing_required.push(name.clone());
            }
            entries.push((name.clone(), st));
        }
        for name in &manifest.caps_optional {
            let st = self.status(name);
            if st != CapStatus::Known {
                ignored_optional.push(name.clone());
            }
            entries.push((name.clone(), st));
        }
        CapsVerdict {
            missing_required,
            ignored_optional,
            entries,
        }
    }
}

/// Convenience: this build's `known` set. ｜ 便捷入口：本构建的 `known` 集合。
pub fn known_capabilities() -> Vec<&'static str> {
    CapabilityRegistry::current().known
}

/// 格式**已登记**、但本构建不处理的能力名。
/// Registered in the format, but this build does not act on them.
///
/// 与 [`known_capabilities`] 的分工：「认得」决定**能不能打开**，「已登记」决定
/// **这个名字是否属于本规范**。写者可以声明后者里的名字（例如未来实现 `sig` 层链的
/// 读者），读者不认识就按 `required` / `optional` 拒绝或跳过 —— 这条通道不能掐死，
/// 否则格式永远无法扩展。
///
/// **加名字要过评审**（`docs/amberice/spec/format.md` §21.4）：
/// ① 在这里登记；② 同时加跨端向量；③ §21.4 表里加一行说明语义。
/// 缺任一条就会出现「两年后一堆互相读不了的文件」。
pub fn registered_capabilities() -> &'static [&'static str] {
    &[
        "sig",
        "dir",
        "frames",
        "source",
        "crypto",
        "embed",
        "typed",
        "provenance",
    ]
}

/// A capability name must be a lowercase identifier (no spaces, no uppercase,
/// no leading digit). This is the **format** check; membership is a separate question.
/// 能力名的**格式**校验：小写标识符。是否属于规范另由登记表回答。
pub fn is_well_formed_capability(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with(|c: char| c.is_ascii_digit())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
}

/// Outcome of applying `caps` to a build's known set (D11 + P10: one predicate for both paths).
/// 把 `caps` 套到「本构建认得的集合」上的结论（D11 + P10）。
///
/// `P10` 要求「编译期裁剪」与「异构读者」走同一个判定函数 —— 本函数就是那个函数：
/// 裁剪掉的段种类从 [`CapabilityRegistry::known`] 里消失即可，判定逻辑一行都不用改。
/// P10 wants feature-pruning and foreign readers to share one predicate; this is it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapsVerdict {
    /// Required names this build does not understand ⇒ must refuse. ｜ 必拒清单。
    pub missing_required: Vec<String>,
    /// Optional names this build does not understand ⇒ skipped (not an error). ｜ 跳过清单。
    pub ignored_optional: Vec<String>,
    /// Every declared name with its classification, required first then optional.
    /// 每个声明名的分类（先 required 后 optional），供 `check` 打印三种文案。
    pub entries: Vec<(String, CapStatus)>,
}

impl CapsVerdict {
    /// OK to open? ｜ 能打开吗？
    pub fn is_ok(&self) -> bool {
        self.missing_required.is_empty()
    }
}

/// Apply a manifest's `caps` to this build's registry.
/// 用本构建的注册表判定 manifest 的 `caps`。
pub fn check_caps(manifest: &Manifest) -> CapsVerdict {
    CapabilityRegistry::current().verdict(manifest)
}

/// Same, against an explicit set — the seam that keeps P10 honest (pruning = pass a smaller set).
/// 同上，但显式给集合 —— P10 的接缝就在这里（裁剪 = 传一个更小的集合）。
pub fn check_caps_against(manifest: &Manifest, known: &[&str]) -> CapsVerdict {
    CapabilityRegistry {
        known: known.to_vec(),
        registered: registered_capabilities(),
    }
    .verdict(manifest)
}

/// Why a manifest could not be decoded. ｜ manifest 解不开的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ManifestError {
    /// The manifest bytes are not a valid canonical encoding ｜ 字节不是合规的 canonical。
    Canonical(CanonicalError),
    /// The decoded value is not an object ｜ 解出来的不是对象。
    NotAnObject,
    /// A required field is absent ｜ 缺必填字段。
    Missing(&'static str),
    /// A field has the wrong shape ｜ 字段形状不对。
    BadShape(&'static str),
    /// An integer field is out of range ｜ 整数字段越界。
    BadInt(&'static str),
    /// `hash_id` is not in §20 ｜ `hash_id` 不在 §20 表里。
    UnknownHash(u8),
    /// `hash_id` 元组（多级转义）形状不合法 ｜ Malformed escape tuple.
    BadHashPath(crate::hash::HashPathError),
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManifestError::Canonical(e) => write!(f, "manifest 字节不合规：{e}"),
            ManifestError::NotAnObject => write!(f, "manifest 顶层必须是对象"),
            ManifestError::Missing(k) => write!(f, "manifest 缺必填字段 `{k}`"),
            ManifestError::BadShape(k) => write!(f, "manifest 字段 `{k}` 形状不对"),
            ManifestError::BadInt(k) => write!(f, "manifest 字段 `{k}` 越界"),
            ManifestError::BadHashPath(e) => write!(f, "manifest 的 hash_id 元组不合法：{e}"),
            ManifestError::UnknownHash(id) => {
                write!(f, "manifest 的 hash_id 0x{id:02x} 不在 §20 表里")
            }
        }
    }
}

impl std::error::Error for ManifestError {}

fn as_object(v: &Value) -> Option<&Map> {
    match v {
        Value::Object(m) => Some(m),
        _ => None,
    }
}

fn int_field(m: &Map, key: &str) -> Option<i64> {
    match m.get(key) {
        Some(Value::Int(n)) => Some(*n),
        Some(Value::Float(f, _)) if f.fract() == 0.0 => Some(*f as i64),
        _ => None,
    }
}

fn str_array(items: &[String]) -> Value {
    Value::Array(items.iter().cloned().map(Value::Str).collect())
}

fn str_list(v: &Value) -> Vec<String> {
    match v {
        Value::Array(a) => a
            .iter()
            .filter_map(|x| match x {
                Value::Str(s) => Some(s.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// Parse a 32-hex-digit string into 16 bytes (per-file salt).
/// 把 32 位十六进制解析成 16 字节（每文件盐）。
fn from_hex16(s: &str) -> Option<[u8; 16]> {
    if s.len() != 32 || !s.is_ascii() {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

fn from_hex(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 || !s.is_ascii() {
        return None;
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        out[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Manifest {
        let mut m = Manifest::minimal(
            crate::container::SEG_VALUE,
            HashAlgo::Sm3,
            vec![ManifestSegment {
                id: crate::container::SEG_VALUE,
                comp: 0,
                enc: 0,
                len: 42,
                digest: [7u8; 32],
            }],
        );
        m.caps_required = vec!["value".into()];
        m.caps_optional = vec!["provenance".into()];
        m.created = Some("2026-11-04T00:00:00Z".into());
        m.tool = Some("amberice test".into());
        m
    }

    #[test]
    fn round_trips_through_bytes() {
        let m = sample();
        let bytes = m.to_bytes().unwrap();
        assert_eq!(Manifest::from_bytes(&bytes).unwrap(), m);
    }

    #[test]
    fn encoding_is_deterministic() {
        assert_eq!(sample().to_bytes().unwrap(), sample().to_bytes().unwrap());
    }

    #[test]
    fn multi_level_hash_id_round_trips() {
        // `[0x3E, ns_version, sub…]` 经 canonical bytes 往返后语义不变。
        let mut m = sample();
        m.hash = HashAlgo::ExtendedNamespace;
        m.hash_path = Some(crate::hash::HashPath { ns_version: 1, subs: vec![0x0001, 0x0002] });
        let bytes = m.to_bytes().unwrap();
        let back = Manifest::from_bytes(&bytes).unwrap();
        assert_eq!(back.hash, HashAlgo::ExtendedNamespace);
        assert_eq!(back.hash_path, m.hash_path);
        // 写回的值必须是数组，而不是整数（字节形态要稳定）。
        let v = back.to_value();
        let obj = match &v {
            Value::Object(o) => o,
            _ => panic!("manifest 不是对象"),
        };
        assert!(matches!(obj.get("hash_id"), Some(Value::Array(a)) if a.len() == 4));
    }

    #[test]
    fn one_level_hash_id_stays_an_integer() {
        let v = sample().to_value();
        let obj = match &v {
            Value::Object(o) => o,
            _ => panic!("manifest 不是对象"),
        };
        assert!(matches!(obj.get("hash_id"), Some(Value::Int(_))));
    }

    #[test]
    fn malformed_multi_level_hash_id_is_rejected() {
        let bad = |v: Value| {
            let mut m = sml_value::Map::ordered();
            m.insert("primary".into(), Value::Int(1));
            m.insert("hash_id".into(), v);
            m.insert("segments".into(), Value::Array(vec![]));
            Manifest::from_value(&Value::Object(m))
        };
        assert!(bad(Value::Array(vec![Value::Int(0x30), Value::Int(1), Value::Int(1)])).is_err());
        assert!(bad(Value::Array(vec![Value::Int(0x3E), Value::Int(1)])).is_err());
        assert!(bad(Value::Str("0x3e".into())).is_err());
    }

    #[test]
    fn unknown_keys_are_ignored_and_defaults_apply() {
        // 只给三个必填字段，其余靠 §21 默认值
        let v = sml_value::Value::from_canonical_bytes(
            &{
                let mut m = Map::ordered();
                m.insert("primary".into(), Value::Int(1));
                m.insert("hash_id".into(), Value::Int(0x30));
                m.insert("segments".into(), sml_value::Value::Array(vec![]));
                m.insert("future_field".into(), Value::Str("ignore me".into()));
                sml_value::Value::Object(m).canonical_bytes().unwrap()
            },
        )
        .unwrap();
        let m = Manifest::from_value(&v).unwrap();
        assert_eq!(m.hash, HashAlgo::Sha256);
        assert_eq!(m.dir_depth, DEFAULT_DIR_DEPTH);
        assert_eq!(m.crypto_block, DEFAULT_CRYPTO_BLOCK);
        assert_eq!(m.patch_strategy, DEFAULT_PATCH_STRATEGY);
        assert!(m.created.is_none());
    }

    #[test]
    fn missing_required_fields_are_errors() {
        let empty = Value::Object(Map::new());
        assert!(matches!(
            Manifest::from_value(&empty),
            Err(ManifestError::Missing("primary"))
        ));
    }

    #[test]
    fn caps_split_into_refuse_and_skip() {
        let m = sample();
        let v = check_caps(&m);
        assert!(v.is_ok(), "value 是认得的");
        assert_eq!(v.ignored_optional, vec!["provenance".to_string()]);
        assert_eq!(v.entries[1], ("provenance".to_string(), CapStatus::Pruned));

        let mut m2 = sample();
        m2.caps_required = vec!["future-x".into()];
        let v2 = check_caps(&m2);
        assert!(!v2.is_ok());
        assert_eq!(v2.missing_required, vec!["future-x".to_string()]);
        assert_eq!(v2.entries[0].1, CapStatus::Unregistered);
    }

    /// P10: pruning is "pass a smaller set", not "write another predicate".
    /// P10：裁剪就是「传一个更小的集合」，不是另写一个判据。
    #[test]
    fn pruning_reuses_the_same_predicate() {
        let mut m = sample();
        m.caps_required = vec!["provenance".into()];
        assert!(!check_caps(&m).is_ok()); // 已登记但本构建不处理
        let pruned = &["value", "manifest"]; // 想象把 strtab 也裁掉
        assert!(!check_caps_against(&m, pruned).is_ok());
        // 把同一个名字加进 known，同一判据立刻通过 —— 没有第二条路径。
        assert!(check_caps_against(&m, &["value", "manifest", "provenance"]).is_ok());
    }

    #[test]
    fn registry_classifies_and_registers_extension_names() {
        let reg = CapabilityRegistry::current();
        assert_eq!(reg.status("value"), CapStatus::Known);
        assert_eq!(reg.status("provenance"), CapStatus::Pruned);
        assert_eq!(reg.status("not-in-spec"), CapStatus::Unregistered);

        // D52：`embed` / `typed` / `provenance` 必须真正进登记表。
        for name in ["embed", "typed", "provenance"] {
            assert!(reg.registered.contains(&name), "{name} 未登记");
        }
        assert!(reg.known.contains(&"embed"));

        #[cfg(feature = "sign")]
        assert_eq!(reg.status("sig"), CapStatus::Known);
        #[cfg(not(feature = "sign"))]
        assert_eq!(reg.status("sig"), CapStatus::Pruned);

        #[cfg(feature = "compress")]
        assert_eq!(reg.status("frames"), CapStatus::Known);
        #[cfg(not(feature = "compress"))]
        assert_eq!(reg.status("frames"), CapStatus::Pruned);

        #[cfg(feature = "crypto")]
        assert_eq!(reg.status("crypto"), CapStatus::Known);
        #[cfg(not(feature = "crypto"))]
        assert_eq!(reg.status("crypto"), CapStatus::Pruned);
    }

    #[test]
    fn verdict_entries_keep_required_then_optional_order() {
        let mut m = sample();
        m.caps_required = vec!["value".into(), "provenance".into()];
        m.caps_optional = vec!["future-x".into()];
        let v = check_caps(&m);
        let names: Vec<&str> = v.entries.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["value", "provenance", "future-x"]);
        assert_eq!(v.entries[1].1, CapStatus::Pruned);
        assert_eq!(v.entries[2].1, CapStatus::Unregistered);
    }
}
