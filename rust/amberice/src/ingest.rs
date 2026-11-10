// SPDX-License-Identifier: MulanPSL-2.0
//! Batch ingest (P11 / D53): buffer frames, commit **one layer per batch**, single writer.
//! 批量归档（P11 / D53）：缓冲帧、每批只追加**一层**、单写者。
//!
//! # 边界
//!
//! - 单写者，不做 OS 锁；并发写请用 `--expect-head` 或给每个文件一个写者。
//! - `push` 不做 I/O；到阈值由 `push` 顺手提交，或调用方显式 `commit()`。
//! - 提交是「tmp + fsync + rename」，不会把半个层留在正式文件名下。
//! - `History::Keep` 追加未签名层；`History::Drop` 整份重写（旧值不再留在新文件里）。
//! - 读者要 reopen 才能看到新层（mmap 旧视图不会自动更新）。

use std::path::{Path, PathBuf};
use std::time::Instant;

use sml_value::{CanonicalValue, Value};

use crate::hash::HashAlgo;
use crate::layer::{self, LAYER_HEADER_LEN, TRAILER_LEN, TRAILER_MAGIC};

/// Whether the file keeps history. ｜ 文件是否保留历史。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum History {
    /// Append one unsigned layer per batch. ｜ 每批追加一层（未签名）。
    Keep,
    /// Rewrite the whole file; old values do not survive. ｜ 整份重写，旧值不留在新文件里。
    Drop,
}

/// Batch policy. ｜ 批量策略。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestOptions {
    /// Commit after this many frames (`0` = disable the frame trigger).
    /// 攒够多少帧提交（`0` = 关掉帧数触发）。
    pub frames_per_layer: u32,
    /// Commit when this many ms elapsed since the last commit (`0` = disable).
    /// 距上次提交多少毫秒后提交（`0` = 关掉时间触发）。
    pub max_delay_ms: u64,
    /// `fsync` the temp file before rename. ｜ rename 前是否 fsync。
    pub fsync: bool,
    /// Keep or drop history. ｜ 保留还是丢弃历史。
    pub history: History,
}

impl Default for IngestOptions {
    fn default() -> Self {
        Self {
            frames_per_layer: 64,
            max_delay_ms: 1000,
            fsync: true,
            history: History::Keep,
        }
    }
}

/// One commit result. ｜ 一次提交的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Commit {
    /// Layers in the file after this commit. ｜ 提交后文件里的层数。
    pub layers: u32,
    /// File size after this commit. ｜ 提交后的文件大小。
    pub bytes: usize,
}

/// Outcome of `push`. ｜ `push` 的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PushOutcome {
    /// Whether this push triggered a commit. ｜ 这次 push 是否触发了提交。
    pub committed: bool,
    /// Frames still buffered. ｜ 仍在缓冲的帧数。
    pub pending: u32,
}

/// Recovery result. ｜ 恢复结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recovery {
    /// Bytes cut off the tail. ｜ 截掉的尾部字节数。
    pub truncated_bytes: usize,
    /// Whether a partial batch was dropped. ｜ 是否丢掉了半批。
    pub dropped_tail: bool,
}

/// Why ingest failed. ｜ 归档失败的原因。
#[derive(Debug)]
#[non_exhaustive]
pub enum IngestError {
    /// File I/O failed. ｜ 文件读写失败。
    Io(String),
    /// The container refused. ｜ 容器层拒绝。
    Container(String),
    /// The layer chain refused. ｜ 层链拒绝。
    Layer(String),
    /// The value could not be canonicalised. ｜ 值无法 canonical 化。
    Canonical(String),
    /// No valid trailer could be found. ｜ 找不到合法 Trailer。
    RecoverFailed,
}

impl std::fmt::Display for IngestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IngestError::Io(why) => write!(f, "文件读写失败：{why}"),
            IngestError::Container(why) => write!(f, "容器层：{why}"),
            IngestError::Layer(why) => write!(f, "层链：{why}"),
            IngestError::Canonical(why) => write!(f, "值无法 canonical 化：{why}"),
            IngestError::RecoverFailed => write!(f, "找不到合法 Trailer，拒绝截断"),
        }
    }
}

impl std::error::Error for IngestError {}

/// Single-writer batch ingest. ｜ 单写者批量归档。
pub struct Ingestor {
    path: PathBuf,
    bytes: Vec<u8>,
    pending: Option<Value>,
    frames: u32,
    last_commit: Instant,
    opts: IngestOptions,
    algo: HashAlgo,
}

impl Ingestor {
    /// Open an existing `.ambr` file for batch append. ｜ 打开已有 `.ambr` 做批量追加。
    pub fn open(path: impl Into<PathBuf>, opts: IngestOptions) -> Result<Self, IngestError> {
        let path = path.into();
        let bytes = std::fs::read(&path).map_err(|e| IngestError::Io(e.to_string()))?;
        let algo = crate::container::hash_algo(&bytes)
            .map_err(|e| IngestError::Container(e.to_string()))?;
        crate::info(&bytes).map_err(|e| IngestError::Container(e.to_string()))?;
        Ok(Self {
            path,
            bytes,
            pending: None,
            frames: 0,
            last_commit: Instant::now(),
            opts,
            algo,
        })
    }

    /// Buffer one frame; commits automatically when a threshold is reached.
    /// 缓冲一帧；到阈值时自动提交。
    pub fn push(&mut self, frame: Value) -> Result<PushOutcome, IngestError> {
        self.pending = Some(match self.pending.take() {
            None => frame,
            Some(mut cur) => {
                merge_into(&mut cur, frame);
                cur
            }
        });
        self.frames += 1;

        let due_frames =
            self.opts.frames_per_layer > 0 && self.frames >= self.opts.frames_per_layer;
        let due_time = self.opts.max_delay_ms > 0
            && self.last_commit.elapsed().as_millis() as u64 >= self.opts.max_delay_ms;
        if due_frames || due_time {
            self.commit()?;
            Ok(PushOutcome {
                committed: true,
                pending: 0,
            })
        } else {
            Ok(PushOutcome {
                committed: false,
                pending: self.frames,
            })
        }
    }

    /// Force a commit of everything buffered. ｜ 强制提交当前缓冲。
    pub fn commit(&mut self) -> Result<Commit, IngestError> {
        let Some(patch) = self.pending.take() else {
            return Ok(Commit {
                layers: self.layer_count(),
                bytes: self.bytes.len(),
            });
        };

        let next = match self.opts.history {
            History::Keep => {
                layer::append_unsigned(&self.bytes, Some(patch), self.algo)
                    .map_err(|e| IngestError::Layer(e.to_string()))?
            }
            History::Drop => {
                let current =
                    crate::load(&self.bytes).map_err(|e| IngestError::Container(e.to_string()))?;
                let mut merged = current;
                merge_into(&mut merged, patch);
                let cv = CanonicalValue::try_from(&merged)
                    .map_err(|e| IngestError::Canonical(e.to_string()))?;
                crate::dump(&cv).map_err(|e| IngestError::Container(e.to_string()))?
            }
        };

        atomic_write(&self.path, &next, self.opts.fsync)?;
        self.bytes = next;
        self.frames = 0;
        self.last_commit = Instant::now();
        Ok(Commit {
            layers: self.layer_count(),
            bytes: self.bytes.len(),
        })
    }

    /// Number of layers currently on disk. ｜ 当前文件里的层数。
    pub fn layer_count(&self) -> u32 {
        layer::layers(&self.bytes)
            .map(|l| l.len() as u32)
            .unwrap_or(0)
    }

    /// Cut a partial trailing record/trailer (crash recovery). ｜ 截掉崩溃留下的半截记录。
    ///
    /// Scans back up to 1 MiB for the last trailer whose layer record is intact and whose
    /// `head_hash` matches; truncates there. **Never guesses**: no candidate ⇒ error.
    /// 找回最后一个完整 Trailer 并截断；找不到就报错，绝不截到不明位置。
    pub fn recover(&mut self) -> Result<Recovery, IngestError> {
        let bytes = std::fs::read(&self.path).map_err(|e| IngestError::Io(e.to_string()))?;
        if bytes.len() >= TRAILER_LEN {
            if let Some(t) = layer::Trailer::parse(&bytes) {
                if let Ok(ls) = layer::layers(&bytes) {
                    if ls.len() as u32 == t.layer_count {
                        self.bytes = bytes;
                        return Ok(Recovery {
                            truncated_bytes: 0,
                            dropped_tail: false,
                        });
                    }
                }
            }
        }

        let start = bytes.len().saturating_sub(1 << 20);
        for pos in (start..=bytes.len().saturating_sub(TRAILER_LEN)).rev() {
            if bytes[pos..pos + 8] != TRAILER_MAGIC {
                continue;
            }
            let Some(t) = layer::Trailer::parse(&bytes[..pos + TRAILER_LEN]) else {
                continue;
            };
            let off = t.last_layer_off as usize;
            if off + LAYER_HEADER_LEN > pos {
                continue;
            }
            let body_len =
                u32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
                    as usize;
            if off + LAYER_HEADER_LEN + body_len != pos {
                continue;
            }
            let body = &bytes[off + LAYER_HEADER_LEN..pos];
            let Ok(got) = crate::hash::digest(self.algo, body) else {
                continue;
            };
            if got != t.head_hash {
                continue;
            }
            let keep = pos + TRAILER_LEN;
            let truncated = bytes.len() - keep;
            self.bytes = bytes[..keep].to_vec();
            atomic_write(&self.path, &self.bytes, self.opts.fsync)?;
            return Ok(Recovery {
                truncated_bytes: truncated,
                dropped_tail: truncated > 0,
            });
        }
        Err(IngestError::RecoverFailed)
    }
}

fn merge_into(cur: &mut Value, frame: Value) {
    // `Value` 实现了 `Drop`，不能按值拆开；先借 `frame` 合并，再决定是否整值替换。
    if let (Value::Object(base), Value::Object(patch)) = (&mut *cur, &frame) {
        for (k, v) in patch.iter() {
            sml_value::insert_or_merge(base, k, v.clone(), |old, new| *old = new);
        }
        return;
    }
    *cur = frame;
}

fn atomic_write(path: &Path, bytes: &[u8], fsync: bool) -> Result<(), IngestError> {
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&tmp, bytes).map_err(|e| IngestError::Io(e.to_string()))?;
    if fsync {
        // Windows 上只读句柄的 `sync_all` 会报 access denied：显式要写权限。
        let f = std::fs::OpenOptions::new()
            .write(true)
            .open(&tmp)
            .map_err(|e| IngestError::Io(e.to_string()))?;
        f.sync_all()
            .map_err(|e| IngestError::Io(e.to_string()))?;
    }
    std::fs::rename(&tmp, path).map_err(|e| IngestError::Io(e.to_string()))?;
    Ok(())
}
