//! 表结构骨架（`docs/amberice/spec/smldb.md` §5.3 的 draft 设计）。
//!
//! 这份 DDL 是**骨架**，不是已定稿的验收指标：该文件仍是 `draft`，
//! `docs/amberice/plan/multi-writer.md:33` 明确「正文的【已定】以文件级 draft 为准，
//! 阈值与数字不可引用」。第一刀的 `MemoryBackend` **不执行**这些 DDL；
//! 保留在这里是为了给 SQLite 后端一个明确的落点，避免每写一次都翻文档。
//!
//! FTS5 外部内容表与三个触发器暂不落：D13 要求 `.ambr` 只提供 `extract_text()`，
//! 而该能力当前还没有（见 `HANDOFF-2026-11-05.md:237`）；先把 `docs` / `doc_tags` 两表钉住。

/// 主表：原文 BLOB 整存、不拆行；`path` 唯一。
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

/// 标签关联表：筛选走这里，`docs.tags` 只用于展示。
pub const DOC_TAGS_DDL: &str = "\
CREATE TABLE IF NOT EXISTS doc_tags (
    doc_id INTEGER NOT NULL REFERENCES docs(id) ON DELETE CASCADE,
    tag    TEXT NOT NULL,
    PRIMARY KEY (doc_id, tag)
)";

/// 索引骨架（时间倒序与按标签反查）。
pub const INDEXES_DDL: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS ix_created ON docs(created)",
    "CREATE INDEX IF NOT EXISTS ix_modified ON docs(modified)",
    "CREATE INDEX IF NOT EXISTS ix_tag ON doc_tags(tag)",
    "CREATE INDEX IF NOT EXISTS ix_tag_doc ON doc_tags(tag, doc_id)",
];
