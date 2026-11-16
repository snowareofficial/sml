// SPDX-License-Identifier: MulanPSL-2.0
//! smldb：`.ambr` 之上的文档数据库层（T8，随 v5 开放；`publish = false`）。
//!
//! 后端按用户裁决直接用 **Turso**（`libsql` 嵌入式 core），不另造后端抽象：
//! 原文 `.ambr` 整块进 `docs.body` BLOB，元数据与标签进索引表（表结构见 [`schema`]）。
//! 第一刀只做 path 精确取回、标签反查、删除；FTS / CSS 选择器 / Sidecar / CLI 未做。

pub mod schema;

use std::fmt;
use std::path::Path;

use libsql::{params, Connection};

use schema::{DOC_TAGS_DDL, DOCS_DDL, INDEXES_DDL};

/// 文档 id（Turso 的 rowid）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocId(pub i64);

/// 文档元数据（对应 `docs` 表除 `body` / `content` 外的列）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meta {
    /// 文档路径（唯一键）。
    pub path: String,
    pub title: String,
    /// RFC3339 风格时间戳；第一刀只存字符串，不解析时间语义。
    pub created: String,
    pub modified: String,
    /// 标签（入库前去重排序；`docs.tags` 只展示，筛选走 `doc_tags`）。
    pub tags: Vec<String>,
}

/// 取回的文档：**拥有** `.ambr` 字节（§5.6），不借 Turso 的缓冲。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Doc {
    bytes: Vec<u8>,
}

impl Doc {
    /// 从 `.ambr` 字节构造并当场校验容器（验段摘要）；坏字节不进库。
    pub fn from_ambr(bytes: impl Into<Vec<u8>>) -> Result<Self> {
        let bytes = bytes.into();
        validate_ambr(&bytes)?;
        Ok(Self { bytes })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// 校验一段字节是可打开的 `.ambr` 容器。
pub fn validate_ambr(bytes: &[u8]) -> Result<()> {
    amberice::Store::open(bytes)
        .map(|_| ())
        .map_err(|e| Error::InvalidAmbr(e.to_string()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    InvalidAmbr(String),
    DuplicatePath(String),
    NotFound(String),
    BadInput(String),
    Db(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAmbr(s) => write!(f, "非法 .ambr 文档：{s}"),
            Self::DuplicatePath(s) => write!(f, "文档路径已存在：{s}"),
            Self::NotFound(s) => write!(f, "文档不存在：{s}"),
            Self::BadInput(s) => write!(f, "输入不合法：{s}"),
            Self::Db(s) => write!(f, "Turso 错误：{s}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<libsql::Error> for Error {
    fn from(e: libsql::Error) -> Self {
        Self::Db(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// 唯一数据库入口：持有 Turso 连接，建表在 [`Store::open`] 时完成。
pub struct Store {
    conn: Connection,
}

impl Store {
    /// 打开（或新建）本地 Turso 数据库并建表。
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let db = libsql::Builder::new_local(path).build().await?;
        let conn = db.connect()?;
        let mut ddl = String::from("PRAGMA foreign_keys = ON;");
        for stmt in [DOCS_DDL, DOC_TAGS_DDL].iter().copied().chain(INDEXES_DDL.iter().copied()) {
            ddl.push_str(stmt);
            ddl.push(';');
        }
        conn.execute_batch(&ddl).await?;
        Ok(Self { conn })
    }

    /// 插入一份文档；重复 path 响亮拒绝。
    pub async fn insert(&self, meta: &Meta, doc: &Doc) -> Result<DocId> {
        if meta.path.is_empty() {
            return Err(Error::BadInput("文档路径不可为空".into()));
        }
        if self.get_by_path(&meta.path).await?.is_some() {
            return Err(Error::DuplicatePath(meta.path.clone()));
        }
        let tags = normalize_tags(&meta.tags).join(",");
        self.conn.execute(
            "INSERT INTO docs (path, title, created, modified, tags, content, body) \
             VALUES (?, ?, ?, ?, ?, '', ?)",
            params![
                meta.path.clone(),
                meta.title.clone(),
                meta.created.clone(),
                meta.modified.clone(),
                tags,
                doc.bytes.clone()
            ],
        ).await?;
        let id = self.conn.last_insert_rowid();
        for tag in normalize_tags(&meta.tags) {
            self.conn.execute(
                "INSERT INTO doc_tags (doc_id, tag) VALUES (?, ?)",
                params![id, tag],
            ).await?;
        }
        Ok(DocId(id))
    }

    /// 按路径精确取回（§7 选择器未定，本刀只有精确匹配）。
    pub async fn get_by_path(&self, path: &str) -> Result<Option<Doc>> {
        let mut rows = self.conn
            .query("SELECT body FROM docs WHERE path = ?", params![path.to_string()])
            .await?;
        match rows.next().await? {
            Some(row) => {
                let bytes: Vec<u8> = row.get(0)?;
                Ok(Some(Doc::from_ambr(bytes)?))
            }
            None => Ok(None),
        }
    }

    /// 按 id 取回。
    pub async fn get(&self, id: DocId) -> Result<Option<Doc>> {
        let mut rows = self.conn
            .query("SELECT body FROM docs WHERE id = ?", params![id.0])
            .await?;
        match rows.next().await? {
            Some(row) => {
                let bytes: Vec<u8> = row.get(0)?;
                Ok(Some(Doc::from_ambr(bytes)?))
            }
            None => Ok(None),
        }
    }

    /// 取元数据。
    pub async fn meta(&self, id: DocId) -> Result<Option<Meta>> {
        let mut rows = self.conn
            .query("SELECT path, title, created, modified, tags FROM docs WHERE id = ?", params![id.0])
            .await?;
        match rows.next().await? {
            Some(row) => Ok(Some(Meta {
                path: row.get(0)?,
                title: row.get(1)?,
                created: row.get(2)?,
                modified: row.get(3)?,
                tags: split_tags(&row.get::<String>(4)?),
            })),
            None => Ok(None),
        }
    }

    /// 列出全部文档元数据（按路径排序）。
    pub async fn list(&self) -> Result<Vec<Meta>> {
        let mut rows = self.conn
            .query("SELECT path, title, created, modified, tags FROM docs ORDER BY path", ())
            .await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push(Meta {
                path: row.get(0)?,
                title: row.get(1)?,
                created: row.get(2)?,
                modified: row.get(3)?,
                tags: split_tags(&row.get::<String>(4)?),
            });
        }
        Ok(out)
    }

    /// 按标签反查 id（走 `doc_tags` 索引）。
    pub async fn find_by_tag(&self, tag: &str) -> Result<Vec<DocId>> {
        let mut rows = self.conn
            .query(
                "SELECT d.id FROM docs d JOIN doc_tags t ON t.doc_id = d.id \
                 WHERE t.tag = ? ORDER BY d.id",
                params![tag.to_string()],
            ).await?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await? {
            out.push(DocId(row.get::<i64>(0)?));
        }
        Ok(out)
    }

    /// 删除（级联清理标签索引）；不存在返回 `false`。
    pub async fn delete(&self, id: DocId) -> Result<bool> {
        let n = self.conn
            .execute("DELETE FROM docs WHERE id = ?", params![id.0])
            .await?;
        Ok(n > 0)
    }
}

/// 标签入库规整：去重 + 排序，空标签丢弃。
pub fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut out: Vec<String> = tags.iter().filter(|t| !t.is_empty()).cloned().collect();
    out.sort();
    out.dedup();
    out
}

fn split_tags(s: &str) -> Vec<String> {
    s.split(',').filter(|t| !t.is_empty()).map(str::to_string).collect()
}
