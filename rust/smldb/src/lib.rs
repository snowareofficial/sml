// SPDX-License-Identifier: MulanPSL-2.0
//! smldb：`.ambr` 之上的文档数据库层（T8，随 v5 开放；当前 `publish = false`）。
//!
//! 第一刀只做窄闭环：**收一份合法的 `.ambr` BLOB、按 path 精确取回、按标签筛选**。
//! 不做 CSS 选择器、不做 FTS5、不做 CLI、不做 Sidecar —— 这些在
//! `docs/amberice/spec/smldb.md` 里仍是未定或依赖缺失能力（`extract_text()`）。
//! 后端用 [`Backend`] trait 隔开：`MemoryBackend` 供测试与嵌入场景，SQLite 后端另起一片。

pub mod schema;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// 文档 id（表的主键；第一刀用递增 u64，不与 SQLite rowid 绑死）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocId(pub u64);

/// 一条文档的元数据（对应 `docs` 表除 `body` / `content` 外的列）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    /// 文档路径（唯一键）。
    pub path: String,
    pub title: String,
    /// RFC3339 风格时间戳；第一刀只做字符串比较，不解析时间语义。
    pub created: String,
    pub modified: String,
    /// 标签列表（写入时同步进标签索引；重复标签去重）。
    pub tags: Vec<String>,
}

/// 取回的文档。**拥有** BLOB（`docs/amberice/spec/smldb.md` §5.6）：
/// SQLite 交出来的是 `Vec<u8>`，不能把借用的 `amberice::Store<'a>` 塞进返回值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Doc {
    bytes: Vec<u8>,
}

impl Doc {
    /// 从 `.ambr` 字节构造并**当场校验容器**（验段摘要）。
    /// 非法字节一律拒绝，不把坏 BLOB 写进库里。
    pub fn from_ambr(bytes: impl Into<Vec<u8>>) -> Result<Self> {
        let bytes = bytes.into();
        validate_ambr(&bytes)?;
        Ok(Self { bytes })
    }

    /// 原始 `.ambr` 字节（可直接喂给 `amberice::Store::open`）。
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// 校验一段字节是可打开的 `.ambr` 容器（含段摘要校验）。
pub fn validate_ambr(bytes: &[u8]) -> Result<()> {
    amberice::Store::open(bytes)
        .map(|_| ())
        .map_err(|e| Error::InvalidAmbr(e.to_string()))
}

/// 本 crate 的错误 ｜ errors are typed; the code table stays in `errors/codes.sml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    InvalidAmbr(String),
    DuplicatePath(String),
    NotFound(String),
    BadInput(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAmbr(s) => write!(f, "非法 .ambr 文档：{s}"),
            Self::DuplicatePath(s) => write!(f, "文档路径已存在：{s}"),
            Self::NotFound(s) => write!(f, "文档不存在：{s}"),
            Self::BadInput(s) => write!(f, "输入不合法：{s}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// 后端契约：第一刀只有**精确路径取回 / 标签反查 / 插入 / 删除**。
/// CSS 选择器与 FTS 等能力等规范定稿后再加独立方法，不塞进这个 trait 的默认实现。
pub trait Backend {
    fn insert(&mut self, meta: Meta, doc: Doc) -> Result<DocId>;
    fn get(&self, id: DocId) -> Result<Option<Doc>>;
    fn get_by_path(&self, path: &str) -> Result<Option<Doc>>;
    fn meta(&self, id: DocId) -> Result<Option<Meta>>;
    fn list(&self) -> Result<Vec<Meta>>;
    fn find_by_tag(&self, tag: &str) -> Result<Vec<DocId>>;
    fn delete(&mut self, id: DocId) -> Result<bool>;
}

/// 纯内存后端：零依赖、可确定性地测试 API 语义；不落盘。
#[derive(Debug, Default)]
pub struct MemoryBackend {
    next: u64,
    rows: BTreeMap<DocId, Row>,
    by_path: BTreeMap<String, DocId>,
}

#[derive(Debug, Clone)]
struct Row {
    meta: Meta,
    doc: Doc,
}

impl MemoryBackend {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Backend for MemoryBackend {
    fn insert(&mut self, meta: Meta, doc: Doc) -> Result<DocId> {
        if meta.path.is_empty() {
            return Err(Error::BadInput("文档路径不可为空".into()));
        }
        if self.by_path.contains_key(&meta.path) {
            return Err(Error::DuplicatePath(meta.path.clone()));
        }
        let id = DocId(self.next);
        self.next += 1;
        // 标签索引按集合语义：去重 + 排序，展示列与索引列同源。
        let meta = Meta { tags: normalize_tags(&meta.tags), ..meta };
        self.by_path.insert(meta.path.clone(), id);
        self.rows.insert(id, Row { meta, doc });
        Ok(id)
    }

    fn get(&self, id: DocId) -> Result<Option<Doc>> {
        Ok(self.rows.get(&id).map(|r| r.doc.clone()))
    }

    fn get_by_path(&self, path: &str) -> Result<Option<Doc>> {
        match self.by_path.get(path) {
            Some(id) => self.get(*id),
            None => Ok(None),
        }
    }

    fn meta(&self, id: DocId) -> Result<Option<Meta>> {
        Ok(self.rows.get(&id).map(|r| r.meta.clone()))
    }

    fn list(&self) -> Result<Vec<Meta>> {
        Ok(self.rows.values().map(|r| r.meta.clone()).collect())
    }

    fn find_by_tag(&self, tag: &str) -> Result<Vec<DocId>> {
        Ok(self
            .rows
            .iter()
            .filter(|(_, r)| r.meta.tags.iter().any(|t| t == tag))
            .map(|(id, _)| *id)
            .collect())
    }

    fn delete(&mut self, id: DocId) -> Result<bool> {
        let Some(row) = self.rows.remove(&id) else {
            return Ok(false);
        };
        self.by_path.remove(&row.meta.path);
        Ok(true)
    }
}

/// 把标签列表规整成可索引的集合（去重 + 排序，第一刀不做标签名校验）。
pub fn normalize_tags(tags: &[String]) -> Vec<String> {
    let set: BTreeSet<&str> = tags.iter().map(String::as_str).collect();
    set.into_iter().map(str::to_string).collect()
}
