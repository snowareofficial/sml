//! 真实执行的 Turso 表结构（`docs/amberice/spec/smldb.md` §5.3 的骨架）。
//!
//! BLOB 整存原文、不拆行；`path` 唯一；筛选标签走 `doc_tags`，`docs.tags` 只展示。
//! FTS5 暂不建表：D13 要求 `.ambr` 先提供 `extract_text()`，当前还没有。

/// `docs` 主表。
pub const DOCS_DDL: &str = "\
CREATE TABLE IF NOT EXISTS docs (
    id         INTEGER PRIMARY KEY,
    path       TEXT NOT NULL UNIQUE,
    title      TEXT NOT NULL,
    created    TEXT NOT NULL,
    modified   TEXT NOT NULL,
    tags       TEXT NOT NULL,
    content    TEXT NOT NULL,
    body       BLOB NOT NULL
)";

/// `doc_tags` 关联表。
pub const DOC_TAGS_DDL: &str = "\
CREATE TABLE IF NOT EXISTS doc_tags (
    doc_id INTEGER NOT NULL REFERENCES docs(id) ON DELETE CASCADE,
    tag    TEXT NOT NULL,
    PRIMARY KEY (doc_id, tag)
)";

/// 索引（时间倒序、标签反查）。
pub const INDEXES_DDL: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS ix_created ON docs(created)",
    "CREATE INDEX IF NOT EXISTS ix_modified ON docs(modified)",
    "CREATE INDEX IF NOT EXISTS ix_tag ON doc_tags(tag)",
    "CREATE INDEX IF NOT EXISTS ix_tag_doc ON doc_tags(tag, doc_id)",
];
