// SPDX-License-Identifier: MulanPSL-2.0
//! `DIR` segment (0x03): a path → offset index for random access (N05 / spec §19.3).
//! `DIR` 段（0x03）：路径 → 偏移索引，供随机访问使用（N05 / §19.3）。
//!
//! # Why it exists ｜ 为什么要有它
//!
//! `Store::get` walks the node tree by skipping siblings with `subtree_len`: O(siblings) per
//! level, because each sibling's key must be compared. `DIR` turns the indexed levels into a
//! binary search: O(log n) per level. It is **derived data** — missing, stale, or corrupt ⇒
//! fall back to the linear walk, never a wrong answer.
//! ｜ `get` 每层要线性比键；`DIR` 把「已索引的层」变成二分。它是派生数据：缺失/过期/损坏一律回退线性走，
//! 不可能给出错误答案。
//!
//! # Body layout ｜ 段体布局（本实现补全 §19.3 未写死的字节细节）
//!
//! ```text
//! DIR     = head_layer:u32le ‖ count:varint ‖ entries[count]
//! entries = (parent_off:u32le ‖ key_id:varint ‖ child_off:u32le)*   sorted by (parent, key)
//! ```
//!
//! - offsets are **byte offsets into the plain `VALUE` body** (0 = the root node);
//! - `head_layer` = how many layers the file had when the index was built. A file that has
//!   been appended to since then has a different layer count ⇒ the whole segment is ignored
//!   (§19.3: appends invalidate the index without rewriting it);
//! - only object keys are indexed, and only for objects at depth `< max_depth`
//!   (manifest `defaults.dir_depth`, default 4).

use crate::container::ContainerError;

/// One index entry: `(parent object offset, key string id, child node offset)`.
/// 一条索引项：`(父对象偏移, 键的串 id, 子节点偏移)`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// Byte offset of the parent object's node header in `VALUE` ｜ 父对象节点头在 `VALUE` 里的偏移。
    pub parent: u32,
    /// `STRTAB` id of the key ｜ 键在 `STRTAB` 的 id。
    pub key: u32,
    /// Byte offset of the child node header ｜ 子节点节点头的偏移。
    pub child: u32,
}

/// A parsed `DIR` body. ｜ 已解析的 `DIR` 段体。
#[derive(Debug, Clone)]
pub struct Dir {
    /// Layer count at build time; mismatch ⇒ ignore ｜ 建索引时的层数；不一致就整段忽略。
    pub head_layer: u32,
    /// Entries sorted by `(parent, key)` ｜ 按 `(parent, key)` 排好序的索引项。
    pub entries: Vec<Entry>,
}

impl Dir {
    /// Binary search for `(parent, key)`. ｜ 二分查找 `(parent, key)`。
    pub fn find(&self, parent: u32, key: u32) -> Option<u32> {
        let want = (parent, key);
        self.entries
            .binary_search_by_key(&want, |e| (e.parent, e.key))
            .ok()
            .map(|i| self.entries[i].child)
    }
}

/// Build a `DIR` body from plain canonical `VALUE` bytes. ｜ 从明文 canonical `VALUE` 字节造 `DIR` 段体。
///
/// `head_layer` is the file's layer count at write time (0 for a freshly dumped file).
/// ｜ `head_layer` 是写入时的层数（新 dump 的文件为 0）。
pub fn build(value: &[u8], head_layer: u32, max_depth: u32) -> Result<Vec<u8>, DirError> {
    if value.is_empty() {
        return Err(DirError::Empty);
    }
    let mut entries = Vec::new();
    let end = walk(value, 0, 0, max_depth, &mut entries)?;
    if end != value.len() {
        return Err(DirError::Trailing { at: end, len: value.len() });
    }
    entries.sort_unstable_by_key(|e| (e.parent, e.key));
    entries.dedup_by_key(|e| (e.parent, e.key));

    let mut out = Vec::with_capacity(8 + entries.len() * 12);
    out.extend_from_slice(&head_layer.to_le_bytes());
    put_varint(&mut out, entries.len() as u64);
    for e in &entries {
        out.extend_from_slice(&e.parent.to_le_bytes());
        put_varint(&mut out, e.key as u64);
        out.extend_from_slice(&e.child.to_le_bytes());
    }
    Ok(out)
}

/// Parse a `DIR` body. ｜ 解析 `DIR` 段体。
pub fn parse(body: &[u8]) -> Result<Dir, DirError> {
    if body.len() < 5 {
        return Err(DirError::Truncated { at: body.len() });
    }
    let head_layer = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
    let mut pos = 4usize;
    let count = get_varint(body, &mut pos)?;
    if count > (body.len() as u64) {
        return Err(DirError::BadCount { count });
    }
    let mut entries = Vec::with_capacity(count as usize);
    for _ in 0..count {
        if pos + 4 > body.len() {
            return Err(DirError::Truncated { at: pos });
        }
        let parent = u32::from_le_bytes([body[pos], body[pos + 1], body[pos + 2], body[pos + 3]]);
        pos += 4;
        let key = get_varint(body, &mut pos)?;
        if pos + 4 > body.len() {
            return Err(DirError::Truncated { at: pos });
        }
        let child = u32::from_le_bytes([body[pos], body[pos + 1], body[pos + 2], body[pos + 3]]);
        pos += 4;
        entries.push(Entry {
            parent,
            key: u32::try_from(key).map_err(|_| DirError::KeyIdTooLarge { key })?,
            child,
        });
    }
    Ok(Dir { head_layer, entries })
}

/// Walk one node; returns the offset just past it, and records object children.
/// 走一个节点；返回它之后的偏移，并记录对象子节点的索引项。
fn walk(
    b: &[u8],
    pos: usize,
    depth: u32,
    max_depth: u32,
    out: &mut Vec<Entry>,
) -> Result<usize, DirError> {
    if pos + 6 > b.len() {
        return Err(DirError::Truncated { at: pos });
    }
    let tag = b[pos];
    let sub = u32::from_le_bytes([b[pos + 2], b[pos + 3], b[pos + 4], b[pos + 5]]) as usize;
    let payload = pos + 6;
    let end = payload
        .checked_add(sub)
        .ok_or(DirError::Truncated { at: pos })?;
    if end > b.len() {
        return Err(DirError::Truncated { at: pos });
    }

    match tag {
        // Arr: count:varint + nodes
        6 => {
            let mut p = payload;
            let count = get_varint(b, &mut p)?;
            for _ in 0..count {
                p = walk(b, p, depth + 1, max_depth, out)?;
            }
            if p != end {
                return Err(DirError::SubtreeLen { at: pos });
            }
        }
        // Obj: count:varint + (key_id:varint + node)*
        7 => {
            let mut p = payload;
            let count = get_varint(b, &mut p)?;
            for _ in 0..count {
                let key = get_varint(b, &mut p)?;
                let child = p;
                p = walk(b, p, depth + 1, max_depth, out)?;
                if depth < max_depth {
                    let p32 = u32::try_from(pos).map_err(|_| DirError::OffsetTooLarge { at: pos })?;
                    let c32 =
                        u32::try_from(child).map_err(|_| DirError::OffsetTooLarge { at: child })?;
                    let k32 = u32::try_from(key).map_err(|_| DirError::KeyIdTooLarge { key })?;
                    out.push(Entry {
                        parent: p32,
                        key: k32,
                        child: c32,
                    });
                }
            }
            if p != end {
                return Err(DirError::SubtreeLen { at: pos });
            }
        }
        _ => {
            // Scalars / strings: nothing to index, but the declared length must still fit.
        }
    }
    Ok(end)
}

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let byte = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn get_varint(b: &[u8], pos: &mut usize) -> Result<u64, DirError> {
    let start = *pos;
    let mut shift = 0u32;
    let mut out = 0u64;
    loop {
        if shift > 63 {
            return Err(DirError::VarintTooLong { at: start });
        }
        let byte = *b.get(*pos).ok_or(DirError::Truncated { at: start })?;
        *pos += 1;
        out |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok(out);
        }
        shift += 7;
    }
}

/// Why a `DIR` body could not be built or parsed. ｜ `DIR` 段体无法构建/解析的原因。
#[derive(Debug)]
pub enum DirError {
    /// Empty `VALUE` body ｜ `VALUE` 段体为空。
    Empty,
    /// Node header or payload runs past the end ｜ 节点头或载荷越界。
    Truncated { at: usize },
    /// `subtree_len` disagrees with the bytes actually consumed ｜ `subtree_len` 与实际消费不符。
    SubtreeLen { at: usize },
    /// Bytes left after the root node ｜ 根节点之后还有剩余字节。
    Trailing { at: usize, len: usize },
    /// Offset does not fit `u32` ｜ 偏移超出 `u32`。
    OffsetTooLarge { at: usize },
    /// A `STRTAB` id does not fit `u32` ｜ 串 id 超出 `u32`。
    KeyIdTooLarge { key: u64 },
    /// Declared entry count is larger than the body ｜ 声明的条目数比段体还大。
    BadCount { count: u64 },
    /// LEB128 longer than 10 bytes ｜ LEB128 超过 10 字节。
    VarintTooLong { at: usize },
}

impl std::fmt::Display for DirError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DirError::Empty => write!(f, "VALUE 段体为空"),
            DirError::Truncated { at } => write!(f, "DIR/VALUE 在偏移 {at} 处越界"),
            DirError::SubtreeLen { at } => write!(f, "偏移 {at} 处 subtree_len 与实际消费不符"),
            DirError::Trailing { at, len } => write!(f, "根节点结束于 {at}，但段体长度是 {len}"),
            DirError::OffsetTooLarge { at } => write!(f, "偏移 {at} 超出 u32"),
            DirError::KeyIdTooLarge { key } => write!(f, "串 id {key} 超出 u32"),
            DirError::BadCount { count } => write!(f, "声明的条目数 {count} 超过段体长度"),
            DirError::VarintTooLong { at } => write!(f, "偏移 {at} 处的 LEB128 超过 10 字节"),
        }
    }
}

impl std::error::Error for DirError {}

impl From<DirError> for ContainerError {
    fn from(e: DirError) -> Self {
        ContainerError::Dir(e.to_string())
    }
}
