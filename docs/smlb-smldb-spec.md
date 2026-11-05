> ⚠️ **状态（2026-11-02 入库）**：本文件此前为未跟踪的临时草稿，现纳入版本控制以便保存与检索。
> **内容可能再议**：其中标注【已定】的结论是**写作当时**的判断，不代表已获批准；凡与 TODO.md /
> IDEAS.md / HANDOFF-2026-11-02.md 冲突，以那三处为准。待议清单见 HANDOFF-2026-11-02.md §4.5。
>
> 🔒 **开放节奏（用户 2026-11-04 定）：`.smlb` / `smldb` 与 v5 一起开放。**
> 含义：① v5 定稿是 I-18 的**前置**，v5 未定稿前不动实现；
> ② 二者共用同一批新语法（`type.featag` 等）与版本位 `5.0`，所以「一起开放」也是对外的**一次**口径；
> ③ 若 v5 的 A 表条目有变动，本文 §10 的「编 `Value` 还是另建 AST」等前提要跟着复核。

# sml 项目架构规范

> 本文档为执行方 AI 准备的落地规范。所有结论均基于实测数据，标注【已定】的条目可直接实施，标注【待拍板】的需先确认。

---

## 0. 项目定位与命名

- 项目名称统一为 **sml**（早期讨论中用过 swsml，已废弃）
- 性质：标记语言（HTML/XML/Markdown 类）解析库，自有实现
- 目标：极低内存占用 + 零依赖核心 + 可选持久化层
- 📌 **支持文件名（sml / smld / smlb 三种，未来规范生效, 尚未实现）**：
  - `.smld` — **sml 文本**（人类可读的源/文档）✅
  - `.smlb` — **sml 的二进制衍生**（随机访问 / DB BLOB / NoSQL 传输载荷候选之一）✅
  - `.sml` — 语言/项目名 / 通用文本兜底后缀（与 `.smld` 的确切分工待确认）
  > 上一轮记的 `.mlx` / `.mlb` 已作废。`.smld`/`.smlb` 分工已确认。

---

## 1. 分层架构【已定】

```
┌─────────────────────────────────────────────────┐
│ smldb  (独立 crate, 可失败/可重建)                │
│   文档级 CRUD / 元数据索引 / FTS5 / 事务          │
│   依赖: rusqlite + sml                            │
└────────────────────┬────────────────────────────┘
                     │ 单向依赖
┌────────────────────▼────────────────────────────┐
│ sml  (主 crate, 零依赖)                           │
│   解析 / Span 零拷贝 / 标签 interning / 选择器     │
│   .smlb 序列化 / extract_text()                   │
└────────────────────┬────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────┐
│ .smlb 二进制格式                                  │
│   前序排列 + subtree_sz + 全局字符串表             │
└─────────────────────────────────────────────────┘
```

### 铁律

1. **`sml` 主 crate 零依赖**。任何数据库、async runtime、压缩库都不得进入主 crate。
2. **绝不把树结构拆成数据库行**（见 §5 实测）。
3. **数据库是可丢弃的索引**，真数据在 BLOB 里，崩溃后可重建。

---

## 2. 内存布局：为什么这么做

### 2.1 问题根源【实测】

每个 `String` 的真实成本：

| 费用 | 大小 | 说明 |
|---|---|---|
| 栈上三段 (ptr/cap/len) | 24 B | |
| malloc header | 16 B | |
| size class 向上取整 | ~8 B | 平均浪费 |
| **空气合计** | **≈ 48 B** | **与内容长度无关** |

一个内容 3 字节的 `"div"`，真实成本 51 字节，97% 是空气。

### 2.2 改造收益【实测，2MB 源文本 / 52,000 节点】

| 方案 | 构成 | 总计 | 相对源文本 |
|---|---|---|---|
| A 朴素 `Vec<String>` | 14.4 空气 + 1.6 元数据 + 2.6 副本 | **18.6 MB** | 9.3× |
| B Span + interning | 1.8 节点 + 2.0 源 + 0.8 属性 | **4.6 MB** | 2.3× |
| C B + mmap | 1.8 节点 + 0.8 属性（源不计 RSS） | **2.6 MB** | 1.3× |

**A → B 降 75%，零损失、零 CPU、访问更快（缓存局部性）。**

### 2.3 核心数据结构【已定】

```rust
pub struct Doc {
    src: String,        // 全进程唯一一份
    nodes: Vec<Node>,
}

#[derive(Clone, Copy)]
pub struct Span(u32, u32);   // 8 字节，可 Copy

impl Span {
    #[inline]
    pub fn get<'a>(&self, src: &'a str) -> &'a str {
        &src[self.0 as usize..self.1 as usize]
    }
}

pub struct Node {
    tag: Tag,              //  2 B  #[repr(u16)] 枚举，非 String
    name: Span,            //  8 B  指向 src
    text: Span,            //  8 B  u32::MAX 作「无文本」哨兵
    attrs: Range<u32>,     //  8 B  指向全局 attrs 侧表
    children: Range<u32>,  //  8 B  扁平子节点区间
}                          // 34 B → align 4 → 36 B
```

三个必须遵守的细节：

- **`text` 用 `u32::MAX` 哨兵，不用 `Option<Span>`**。后者因对齐膨胀到 12 B，`Node` 会变成 40 B 而非 36 B。
- **子节点用 `Range<u32>`，不用 `Vec<Box<Node>>`**。后者每子节点多 8 B 指针 + 一次堆分配。
- **属性抽到全局侧表**，不在 `Node` 内嵌 `Vec`（嵌 Vec 意味着每 Node 自带 24 B 三段 + 一次堆分配）。

### 2.4 标签名 interning【已定】

标记语言的标签名是**封闭词表**，这不是通用 JSON，必须利用：

```rust
#[repr(u16)]
enum Tag { Div, Span, A, P, Img, /* ... */ Custom(u32) }
```

`String`(51 B 真实成本) → `u16`(2 B)。

### 2.5 实体解码【已定】

```rust
fn decode<'a>(raw: &'a str) -> Cow<'a, str> {
    if !raw.contains('&') {
        Cow::Borrowed(raw)   // 99% 文本走这里，零分配
    } else {
        Cow::Owned(raw.replace("&amp;", "&").replace("&lt;", "<"))
    }
}
```

### 2.6 补充手段（按需）

| 手段 | 适用 |
|---|---|
| `Box<str>` 替代 `String` | 砍掉 cap，24 B → 16 B，改动成本近零 |
| `compact_str` / `smartstring` | 24 B 以内短串不碰堆（SSO） |
| `Arc<str>` | 跨结构共享同一内容 |
| 列式布局 `{ offsets: Vec<u32>, data: String }` | 替代 `Vec<String>`，O(1) 随机访问 |
| `bumpalo` arena | 只追加、统一释放 |
| 换 allocator（mimalloc/jemalloc） | 碎片与 overhead 差 20~30% |

---

## 3. `.smlb` 二进制格式【已定】

```
[Header] magic "SWSB" | version | flags | root_offset
[StrTab] count + (len:varint, bytes)*        全局字符串表，去重
[Nodes ] 前序排列，每节点:
          tag_id      varint
          nattrs      varint
          (name_id, val_id)*
          text_id     varint (0 = 无)
          nchildren   varint
          subtree_sz  varint     ← 关键
```

**`subtree_sz` 是随机访问的核心**：前序排列下，节点 i 的整棵子树为 `[i+1, i+subtree_sz]`，不遍历即可整棵跳过。

### 体积实测【5,032 KB 文本样本 / 52,000 节点】

| 编码 | 体积 | /原始 |
|---|---|---|
| sml 文本 `.mlx`（HTML 式） | 5032 K | 1.000 |
| protobuf wire | 3317 K | 0.659 |
| 文本 + gzip -9 | 734 K | 0.146 |
| 文本 + zstd -3 | 869 K | 0.173 |
| protobuf + zstd -3 | 690 K | 0.137 |
| **.smlb（裸）** | **867 K** | 0.172 |
| **.smlb + zstd -3** | **540 K** | **0.107** |
| .smlb + zstd -19 | 512 K | 0.102 |
| .smlb + zstd -3 + 训练字典 | 534 K | 0.106 |

结论：
- **zstd -3 是甜点**。-19 只再省 28 K，压缩耗时涨数倍。仅「一次压缩、万次分发」才用 -19。
- **训练字典无收益**（534 vs 540 K）。字符串表已消除重复，字典无事可做。省掉这个复杂度。
- **gzip 不用**：解压 14.09 ms，是 zstd 的 13 倍，体积还无优势。
- **protobuf 在两个场景都不是最优**。

---

## 4. 场景分野【已定】—— 重要

冷数据与传输的目标互相冲突（压缩会毁掉随机访问），必须分开处理：

| | 冷数据（存着偶尔访问） | 传输（网络/IO） |
|---|---|---|
| 容器 | 裸 `.smlb` + **mmap**，不压缩 | `.smlb` + zstd -3 帧 |
| RSS | 2.6 MB（源由 OS 分页管理） | — |
| 随机访问 | O(1) | 需先解压 |

**mmap 的干净页在内存压力下被内核直接丢弃，下次缺页重读** —— 等于 OS 免费做了压缩，比任何压缩都彻底（真正 0 占用），代价仅一次缺页中断。

**冷数据绝不能压缩**：为了省磁盘（最便宜的资源）而付出「每次访问要解压 + 无法随机跳转 + 常驻内存反而更高」的代价，纯亏。别跟内核抢活干。

---

## 5. 数据库层（smldb）【已定】

### 5.1 核心决策：BLOB 存原文，绝不拆行

实测（21,000 节点 / 39,306 属性，原文 2036 KB，拆成 20 个文档）：

| 存储方案 | DB 体积 | /原文 |
|---|---|---|
| A 行式拆解（节点表 + 属性表） | 2520 K | 1.24 |
| **B BLOB 存原文 + 提取元数据列** | **372 K** | **0.18** |

读取开销差距更悬殊：

| 操作 | 耗时 |
|---|---|
| A 重建整棵树（全表 JOIN） | **69.722 ms** ← 每次全量读都要付 |
| B 取一个文档 BLOB（18 KB） | 0.0030 ms |
| B 元数据查询（不读 body） | 0.0154 ms |
| mmap 打开整个文件 | 0.0035 ms（O(1)，之后随机读 ~0.0001 ms） |

**损失是结构性的**：行式存储每节点要付 row header + 字段开销，相对 packed bytes 是恒定倍数。规模越大绝对损失越大。此结论与数据规模无关。

### 5.2 职责边界

```
SQLite 管：哪个文档、什么元数据、怎么找到它、全文检索
sml   管：文档内部怎么读、怎么改
```

### 5.3 SQLite 主场实测（10,000 文档）

| 操作 | 耗时 |
|---|---|
| 按主键精确查 | 0.0023 ms |
| 按时间排序取 20 条 | 0.0119 ms |
| 索引总大小 | 712 KB（每文档 73 B） |

### 5.4 BLOB 尺寸阈值【实测】

| BLOB 大小 | 写入 DB | 读取 DB | 页内开销 |
|---|---|---|---|
| 1 MB | 0.8 ms | 0.1 ms | 0.5% |
| 6 MB | 6.0 ms | 0.5 ms | 0.2% |
| 22 MB | 26.3 ms | 4.3 ms | 0.1% |
| 89 MB | **267 ms** | **98 ms** | 0.1% ⚠️ |

- ≤ 22 MB/文档：直接塞 BLOB
- 64 MB+：开始劣化，且与 DB 争夺页缓存
- > 1 GB：走 sidecar 文件 + mmap

### 5.5 fsync 是真正的悬崖【实测】

| 尺寸 | 写（无 fsync） | 写 + fsync |
|---|---|---|
| 1 MB | 0.13 ms | 0.64 ms |
| 4 MB | 0.54 ms | 1.84 ms |
| 16 MB | 3.09 ms | **121.79 ms** |
| 64 MB | 13.95 ms | **495.71 ms** |

无 fsync 写入线性增长；带 fsync 在 4→16 MB 之间坍塌 15 倍（超出回写缓存、撞上真实设备）。

**应对策略**：默认 WAL + `synchronous=NORMAL`（那 15 倍悬崖主要是 `FULL` 造成的）。

### 5.6 表结构

```sql
CREATE TABLE docs (
    id       INTEGER PRIMARY KEY,
    path     TEXT    NOT NULL UNIQUE,
    title    TEXT,
    created  INTEGER NOT NULL,
    modified INTEGER NOT NULL,
    tags     TEXT,               -- 仅展示用, 不用于筛选!
    content  TEXT,               -- 提取的纯文本, 仅供 FTS
    body     BLOB    NOT NULL    -- .smlb 原始字节, 永不拆行
);
CREATE INDEX ix_created  ON docs(created  DESC);
CREATE INDEX ix_modified ON docs(modified DESC);

-- 多值标签用关联表
CREATE TABLE doc_tags (
    doc_id INTEGER NOT NULL REFERENCES docs(id) ON DELETE CASCADE,
    tag    TEXT    NOT NULL
);
CREATE INDEX ix_tag     ON doc_tags(tag);
CREATE UNIQUE INDEX ix_tag_doc ON doc_tags(tag, doc_id);

-- FTS5 外部内容表
CREATE VIRTUAL TABLE docs_fts USING fts5(
    title, content,
    content='docs', content_rowid='id',
    tokenize='unicode61'
);
CREATE TRIGGER docs_ai AFTER INSERT ON docs BEGIN
    INSERT INTO docs_fts(rowid, title, content)
    VALUES (new.id, new.title, new.content);
END;
CREATE TRIGGER docs_ad AFTER DELETE ON docs BEGIN
    INSERT INTO docs_fts(docs_fts, rowid, title, content)
    VALUES ('delete', old.id, old.title, old.content);
END;
CREATE TRIGGER docs_au AFTER UPDATE ON docs BEGIN
    INSERT INTO docs_fts(docs_fts, rowid, title, content)
    VALUES ('delete', old.id, old.title, old.content);
    INSERT INTO docs_fts(rowid, title, content)
    VALUES (new.id, new.title, new.content);
END;
```

**必须遵守的两个坑**：
- `tags` 逗号字符串仅供展示，**筛选一律走 `doc_tags` 关联表**。实测 `LIKE 'post%'` 要 3.45 ms，走索引 0.0023 ms，差 1500 倍。
- `content` 列由 `sml` 遍历树提取纯文本（FTS5 无法索引二进制 `.smlb`），写入时算一次。

### 5.7 API 规范

```rust
use smldb::Store;
let mut store = Store::open("app.smldb")?;

// Create
let doc = sml::parse(r#"
    div.content
      h1 "Hello"
      p[lang=en] "some text"
"#)?;
let id = store.insert("posts/hello.mlx", &doc)?;
let id = store.insert_with("posts/a.mlx", &doc,
    Meta { title: "Hello", tags: &["post"], ..Default::default() })?;

// Read：两级惰性
let refs = store.find()                 // 只查元数据索引, 0.015 ms
    .tag("post")
    .created_after(ts)
    .order(Order::CreatedDesc)
    .limit(20)
    .load()?;                           // -> Vec<DocRef>, 未读 body

let doc = refs[0].open()?;              // 此时才取 BLOB
let text = doc.select("p[lang]")?.next()?.text();

// Update：闭包内编辑, 原子写回
store.update(id, |doc| {
    for mut p in doc.select_mut("p")? {
        p.set_text(p.text().to_uppercase());
    }
    Ok(())
})?;

// Delete / 全文检索
store.delete(id)?;
for hit in store.search("lorem ipsum")? { /* FTS5 */ }
```

实现要点：

- **`find()` 不读 body** —— 列表页只付 0.015 ms，这是性能关键
- **`hits[0].open()` 才取 BLOB** —— 两级惰性
- **`update()` 内部**：读 → 改 → 重新序列化 → 事务写回，整个过程原子
- **选择器语法与内存版完全一致** —— 用户不学第二套 DSL

### 5.8 `Doc` 返回 owned 而非借用

SQLite 取 BLOB 给的是 `Vec<u8>`，无法返回 `&'a [u8]`：

```rust
pub struct Doc {
    bytes: Vec<u8>,      // 拥有 BLOB
    nodes: Vec<Node>,    // Span 索引进 bytes
}
```

字符串仍不单独分配（Span 指向同一块 Vec），仅多一次 BLOB 拷贝（1 MB 文档 0.1 ms，可接受）。

### 5.9 自适应策略【已定】—— 应对未知规模

不要现在猜测文档大小，做成配置：

```rust
pub enum Storage {
    Inline,    // BLOB 存 DB：单文件分发 + 真事务
    Sidecar,   // 落盘 + mmap：DB 只存路径
}
pub enum WritePolicy {
    Rewrite,   // 全量重写：永远紧凑，无碎片，无日志回放
    Append,    // append-only 段 + 定期 compact
}
pub struct Policy {
    pub inline_max_bytes: usize,   // 默认 8 MB，超过自动转 Sidecar
    pub rewrite_max_bytes: usize,  // 默认 8 MB，超过自动转 Append
}
```

默认阈值挂在实测 fsync 悬崖（4–16 MB）之前。`insert()` 时按大小自动选，用户零感知。

**小规模下「全量重写就是最好的 compaction」**，不要引入 append log 回放复杂度。

---

## 6. 后端选型：SQLite，不用 Turso【已定】

### 6.1 决定性理由：Turso 没有 FTS5

实测证据：

```
CREATE VIRTUAL TABLE t USING fts5
  → Err("Parse error: no such module: fts5")     ← 不支持

CREATE INDEX idx ON t USING fts (title, body)
  → Err("Parse error: index method is an experimental feature.
        Enable with --experimental-index-method flag")
```

| | SQLite | Turso |
|---|---|---|
| 全文检索 | **FTS5，成熟** | Tantivy，`fts_match()`/`fts_score()` |
| 状态 | 生产可用 | 实验特性，需 flag |
| 查询语法 | `MATCH` | `fts_match('idx', ...)` |
| 依赖 | 无 | 拉入 tantivy（有版本冲突前科） |

**两套语法完全不兼容，FTS 代码不可移植。**

### 6.2 async 传染（架构上最贵的一条）

Turso Rust API 是 **async 独有**，`tokio` 是硬依赖：

```rust
#[tokio::main]                        // 必须有
async fn main() -> turso::Result<()> {
    conn.execute("...", ()).await?;   // 全 await
}
```

加了 Turso，`smldb` 的 API 就变 async，所有用户被迫上 tokio。三个选择都有代价：

| 方案 | 代价 |
|---|---|
| sync trait + `spawn_blocking` 包 Turso | Turso 的 async 优势归零 |
| async trait + rusqlite 也包进去 | 强制所有用户上 tokio |
| 拆两个 crate 两套 API | 维护翻倍 |

### 6.3 workload 不匹配

| Turso 卖点 | sml 用得上吗 |
|---|---|
| MVCC 并发写 | 单用户文档库，不需要 |
| async + io_uring | 瓶颈在 fsync，不在引擎 |
| CDC / embedded replica | 需云同步才有意义 |
| 向量检索 | 除非做 RAG |
| WASM | 除非浏览器跑 sml |

第三方实测：单节点下两者基本打平（写 648 vs 666 RPS）。Turso 的 2.5x 优势在**集群复制**——嵌入式单文件库无此场景。

### 6.4 成熟度

- Turso 未到 1.0，140 release 中 8 次 breaking，当前 `0.9.0-pre.2`
- 2026-10-02 被 Supabase 收购，方向未定
- 官方 FAQ 仍建议保留独立备份

### 6.5 零成本留门

```rust
pub trait Backend {
    fn get(&self, id: DocId) -> Result<Vec<u8>>;
    fn put(&self, id: DocId, meta: &Meta, body: &[u8], content: &str) -> Result<()>;
    fn query(&self, q: &Query) -> Result<Vec<DocRef>>;
    fn search(&self, q: &str, limit: usize) -> Result<Vec<Hit>>;  // 必须单独抽!
}
type DefaultBackend = RusqliteBackend;
```

**`search` 必须独立** —— `MATCH` 与 `fts_match()` 不兼容，混入 `Backend` 会在切换时全废。

两者文件格式兼容，将来切换是换一行类型别名，零迁移。

### 6.6 切换触发条件（命中任一再动手）

- 同一 store 多写者并发 → MVCC / `BEGIN CONCURRENT`
- async 服务里 `spawn_blocking` 成为 profile 热点 → 原生 async I/O
- 文档向量检索（RAG）→ 内置 vector 类型
- 浏览器 / WASM 目标 → 原生 wasm
- 需要 embedded replica / 云同步 → CDC

---

## 7. 选择器语法 v1【**已挪出本文件** —— 属 `smldb` / 目录层，`2026-11-04` 用户定】

> ⚠️ **归属变更**：本节当初是照 **HTML 节点模型**写的（`.class` / `[attr=]` / `:has()` / `.content`）。
> 但 D2 已定 `.smlb` **编 `Value`**（对象/数组/标量 + `__type`），节点模型不再成立。
> 因此：**`.smlb` 文件本身只提供路径访问**（`get("a.b[0]")` + `DIR` 段），
> 选择器（含 `:has()`、`.content`、class 语义）**整体归 `smldb`**，在此仅作历史留存。

设计原则：**复用用户已会的 CSS，不发明新 DSL**。

### 7.1 语法

```
div                  标签名
.content             含 class="content"   （简写, 等价 [class~=content]）
#header              id="header"
*                    任意节点

div p         后代（任意深度）
div > p       直接子节点
h1 + p        紧邻的下一个兄弟
h1 ~ p        之后的所有兄弟

p[lang]              有 lang 属性
p[lang=en]           等于
p[lang!=en]          不等于
p[lang^=en]          前缀
p[lang$=us]          后缀
p[lang*=en]          包含
a[href][title]       同时满足（隐式 AND）

p:first-child        第一个子节点
p:last-child
p:nth-child(2)       第 n 个（1-based）
p:nth-child(2n+1)    奇数
li:empty             无子节点
p:has-text("lorem")  文本包含（sml 特有，CSS 没有）

h1, h2, h3           并集
```

### 7.2 用法

```rust
let doc = sml::parse(src)?;
doc.select("p[lang]")?
doc.select("div.content > p")?
doc.select("div#main p.text[lang=en]:first-child")?
doc.select("h1, h2, h3")?
doc.select("p:has-text(\"lorem ipsum\")")?

for node in doc.select("a[href^=https]")? {
    println!("{}", node.attr("href").unwrap_or(""));
    if node.text().is_empty() { break; }   // 可提前退出
}
```

Builder 风格（同一能力的备选，用于动态拼条件）：

```rust
doc.find()
   .tag("div").class("content")
   .child(|q| q.tag("p").attr("lang", "en"))
   .iter()
```

两种写法编译到同一个 `Vec<Step>`，性能一致。

### 7.3 执行模型

选择器编译为步骤数组，**从最右端锚点驱动**：

```
div.content > p[lang]
     ↓
 [锚点: p[lang]]  →  [验证: 父节点是 div.content]
```

节点是**前序扁平数组 + parent 指针**，验证祖先链是 O(深度) 向上跳，无需回溯整棵树。21,000 节点的选择器查询在内存中为**几十微秒**，比走数据库快两个数量级。

`:has-text()` 是唯一需读文本的，但只检查已匹配的候选节点，不扫全表。

### 7.4 v1 建议子集

| 类别 | v1 支持 |
|---|---|
| 基础 | `tag` `.class` `#id` `*` |
| 组合 | `空格` `>` `+` `~` |
| 属性 | `[a]` `[a=v]` `[a^=]` `[a$=]` `[a*=]` `[a!=]` |
| 伪类 | `:first-child` `:last-child` `:nth-child()` `:empty` `:has-text()` |
| 分组 | `,` |
| **暂缓** | `:has()` —— 扁平数组上需子树扫描，唯一可能退化到 O(n·depth) |

---

## 8. SIMD 优化【暂缓，但注意架构耦合】

### 8.1 实测

2 MB 标记文本，扫结构字符（`<` `>` `&` `/` `=` `"`）：

```
标量 (1B/步)   :  0.84 ms    2.500 GB/s
AVX2 (32B/步)  :  0.18 ms   11.725 GB/s
加速比 4.69x                 每次仅省 0.66 ms
```

### 8.2 定位

- **SIMD 优化 CPU 周期，不动内存布局**。压不掉 String 的空气税。
- **2 MB 单次解析，瓶颈不在这里**。仅当处理成千上万个文档或跑在延迟敏感服务里才值得。

### 8.3 意外收获：SIMD 架构天然就是零拷贝架构

simdjson 两阶段设计：

```
Stage 1（SIMD）：一次吃 32/64 字节 → cmpeq → movemask 压成位图
                → 产出 Vec<u32> 结构字符位置 → 顺带 UTF-8 验证
Stage 2（标量）：遍历位置数组 → 构建 tape / 惰性迭代器
```

**Stage 1 的产出就是一串 `u32` 位置 —— 正是 `Span(u32,u32)`。**

即「让解析变快」和「让内存变小」指向同一个重构。做零拷贝改造时顺手把 stage1 SIMD 化，两个收益一次拿到。反之若先写了朴素 `Vec<String>` 解析器，再上 SIMD 等于推倒重来。

### 8.4 落地路径（按 ROI）

1. **白嫖现成**：`memchr`（已 SIMD 化）、`simdutf8`（drop-in 替换 `from_utf8`）
   - 注意 `simdutf8` 对 < 64 B 输入反而比标量慢，别无脑替换
2. `std::arch` intrinsics + 运行时分派（`is_x86_feature_detected!`）
3. `core::simd`（portable_simd）—— **仍 nightly-only，勿用于生产**
4. 手写 `asm!` —— 几乎不需要，LLVM 对 `cmpeq+movemask+tzcnt` 模式已生成良好

**SIMD 会 over-read**：需在处理 buffer 末尾预留 32 字节 padding，否则 Rust 下是 UB。

**短标签名的穷人 SIMD**：标签名基本 ≤ 4 B，直接 load 成整数比较即可：

```rust
let word = u32::from_le_bytes([src[i], src[i+1], src[i+2], src[i+3]]);
if word == u32::from_le_bytes(*b"div>") { /* ... */ }
```

---

## 9. 落地顺序

1. **`sml`：`.smlb` 格式 + 零拷贝解析 + 选择器 + `extract_text()`**（零依赖，所有场景的地基）
2. **`serialize()` / `deserialize()`**（CRUD 前置）
3. **`smldb` crate + rusqlite 后端**（BLOB + 提取列 + FTS5）
4. （搁置）Turso 后端 —— 等触发条件
5. （搁置）SIMD stage1 —— 等 profile 证明是瓶颈

第 3 步之前不碰数据库，`sml` 保持零依赖。

---

## 10. 待拍板项【执行前必须确认】

| # | 事项 | **结案（2026-11-04）** |
|---|---|---|
| 1 | 是否要全文检索（FTS） | ✅ **折中（D13）**：`.smlb` 只给 `extract_text()`，索引归 `smldb` / 外部 |
| 2 | `:has()` 进 v1 吗 | ✅ **随选择器整体挪到 `smldb`（§7 归属变更）**，`.smlb` 不做选择器 |
| 3 | `.content` 等价 `[class~=content]` 吗 | ✅ **不再有 class 语义**：`Value` 模型里用 `__type` 数组或多值键表达；class 属 HTML 遗留 |
| 4 | `id` 是否强制唯一 | ✅ **不强制**（当普通键）；唯一性交给 profile / 契约 |
| 5 | 规模信封 | ✅ 默认：单文档 **≤ 8 MB**、文档数 **≤ 10⁵**、写入 **≤ 1/s**、**每批 fsync**；§5.9 自适应可调 |

---

## 11. 常见误区清单

| 误区 | 正解 |
|---|---|
| 用 Huffman/zstd 压 String 内存 | 压不掉 48 B 空气税，空气只能不交，不能压缩 |
| 用 protobuf 省内存 | prost 解析出仍是 `String`/`Vec`，内存零收益 |
| 直接 gzip 文本就够了 | 传输场景比 protobuf 小 4.5 倍，但 .smlb+zstd 更小 38% |
| 冷数据也压缩省空间 | 毁掉 O(1) 随机访问，fsync/解压成本反噬 |
| 把树拆成数据库行方便查询 | 实测大 85% + 69.7 ms 重建开销 |
| 用逗号字符串存 tags | `LIKE` 比索引慢 1500 倍 |
| 先写朴素解析器再优化 | SIMD 与零拷贝是同一重构，返工等于推倒重来 |
| 用 `Option<Span>` 表示无文本 | 对齐膨胀，Node 从 36 B 变 40 B |
| Node 里嵌 `Vec` 存子节点/属性 | 每 Node 多 24 B + 一次堆分配 |

---

## 12. `.smlb` 用途定位与生态对标【草案 · 待拍板 · 未定 · ⚠️需二次蒸馏】

> 本节只是技术分析 + 取舍，**尚未拍板**，新功能仍暂缓、暂不实现。
> ⚠️ **二次蒸馏**：以下是未经整理的原始信息，需在拍板前把它精炼成「取舍明确 + 结论唯一」的定稿，避免携带冗余分析。
> 记下供决策与后人参照：📌 `.smlb` 不是通用互换格式，是 sml 专属的「持久化 / 传输 / 惰性读取」信封。

### 12.1 用途定位 —— 先构思"为什么需要它"

二进制**不是必须**。它本质是：**把 protobuf 场景的优化（体积 / lazy / 随机访问）做成 sml 文档原生的、无需 schema、无需 codegen 的一层**，可叠加 zstd 帧（必要时）。

只有命中下列场景才有价值；纯「解析→用→丢」的调用方完全用不到它：

| 场景 | 用到的机制 | 数据支撑（§3/§5） |
|---|---|---|
| a. 文档反复存/读（DB / 文件） | BLOB 存原始字节 + mmap | DB 0.18×、单文档读 0.003 ms |
| b. 大文档随机访问 / 惰性（选择器、按需读子树） | 前序 + `subtree_sz` + 绝对偏移 | O(1) 跳整棵，不整树解码 |
| c. 内存受限（嵌入式 / 服务端常驻） | Node 36 B 平坦布局 + Span 共享 | A→B 降 75%，2.6 MB RSS |
| d. 批量传输 / 网络 | zstd -3 帧 | 0.107×（低于 protobuf+zstd） |
| e. 跨平台分发 | 明码 LE + u32 偏移 + 零依赖 | 可 mmap / WASM，无 arch 依赖 |

**它不是答案的场景**（职责划分）：
- 要跨语言 schema 契约、字段号演进约束 → **protobuf**
- 人类可读写的互操作 / 手改 / diff → **文本 sml / toml / json**
- 纯 parse-and-drop → 全部不需要，.smlb 可整体搁置

### 12.2 与 toml / json / protobuf 库的全面对比（Rust 生态）

| 维度 | sml 文本 | `.smlb` | toml (`toml_edit`) | json (`serde_json`) | protobuf (`prost`) |
|---|---|---|---|---|---|
| 人类可读写 | ✅ 易 | ❌ 字节 | ✅ 易 | ✅ 易 | ❌ |
| Schema 要求 | 无 | 无（固有多叉树节点模型） | 无 | 无 | **必须 `.proto` + 代码生成** |
| 类型保真 | Int/Float(+raw)/Bool/Str/Array/Object/null | 同左 | 弱（日期无、数字弱） | 弱（f64 混用） | 强但受 schema 约束 |
| 体积（相对原文 1.0） | 1.000 | 裸 0.172 / zstd 0.107 | ~≈ json | 文本级大 | 0.659 |
| 随机访问 | ❌ | ✅ O(1) 跳子树 | ❌ | ❌ | ❌ 须整树解码 |
| 惰性/按需 | ❌ | ✅ 绝对偏移 | ❌ | ❌ | ❌ |
| mmap | ❌ | ✅（源由 OS 分页） | ❌ | ❌ | ❌ |
| 解析后内存 | 高（String 空气税） | 低（Node 36 B） | 高 | 高 | **仍 String/Vec，零收益**（§10 误区） |
| 选择器/子树查询 | ✅ sml 原生 | ✅ | ❌ | ❌ | ❌ |
| 全文检索 | 经 extract_text→FTS5 | 同左 | SQL LIKE（慢） | 不确定索引 | ❌ |
| 核心零依赖 | ✅ | ✅ | ✅ 纯 rust | ✅ 纯 rust | ❌ prost+codegen |
| 格式演进/向前兼容 | 文本 diff 易 | version + 容忍未知 tag | 文本 break | 文本 break | 靠字段号，最优 |
| 跨语言互换性 | ✅ 文本皆可 | 需专用库读 | ✅ | ✅ | ✅ 最优 |

**结论倾向**：三者职责不打架 —— schema 契约互交换 protobuf；人类互操作走文本 sml/toml/json；**sml 文档的持久化 + 随机访问 + 零依赖内存走 `.smlb`**。

### 12.3 `.smlb` 格式的语法取舍（设计决策面，各带取舍理由）

| 决策 | 取舍 | 理由 / 代价 |
|---|---|---|
| magic `"SWSB"` + version + flags(明码 LE) | 固定 LE 牺牲 BE 直接读 | 换跨平台确定性（同 protobuf）；version 留演进闸门 |
| 计数用 varint | 小文件省字节，解码有分支 | count/ids 小值占多数；体积 vs 解码速度 |
| 全局 strtab 去重 | 淘汰训练字典（§3 534 vs 540 无收益） | 代价：写入需 hash 去重，`u32` 偏移 ≤4 GB；超大文档要 u64 |
| 前序 + `subtree_sz` | O(1) 整棵跳过 ↔ 插入/修改要重算 | 与 §5.9「Rewrite 全量重写」互补，非 append 友好 |
| 不内带自定界（无长度帧头） | 格式职责单纯 | 文档边界靠容器告知（单文件 / DB BLOB 长度 / zstd 帧） |
| 键序决定性（按键排序写） | 换来确定性字节 + 缓存 / diff / 去重 | 代价≈0（BTreeMap 本就按键序） |
| Float 保留 raw（`has_raw` flag） | 保 P3 原文逐字还原 | 占 1 flag 位 |
| 未知 tag 容忍 | 靠 `subtree_sz` 跳过未知子树 | 向前兼容；约束：新 tag 必须在结构上可整体跳过 |
| 标签 interning 分两层 | 内存=封闭词表 `Tag` enum；`.smlb`=strtab 偏移 | 职责不混，内存与序列化各做各的 |

### 12.4 建议（技术意见，**未拍板**）

- 📌 认同你的判断：**`.smlb` 不是必须**，定位 = sml 文档的持久化/传输/惰性信封，恰好覆盖「protobuf 场景优化 + 必要时 zstd」。
- 若目标是纯解析库 → `.smlb` 整块可搁置，proto 优化对纯解析用户零收益。
- 若目标是文档库（存 / 查 / FTS，§1 架构里的 `smldb` BLOB 层）→ `.smlb` 是前提，值得做。
- **触发条件**（仿 §6.6，命中再动手）：出现持久化/重复读需求、内存受限目标、或批量传输；否则暂停。
- 明确不做：不把它做成通用 schema 互换格式（那是 protobuf 的地盘），不自造通用 value 二进制（serde_json/bincode 可代）。

---

## 13. 数据库地位与产品取向（sml × DB）【草案 · 待拍板 · 未定 · ⚠️需二次蒸馏】

> 愿景方向来自产品讨论，**尚未拍板、暂不实现**，记下供决策。
> ⚠️ **二次蒸馏**：以下是愿景层浮想，需在拍板前精炼成「产品明确到什么程度 / 技术落地路径」的定稿。

### 13.1 产品取向

- **默认不暴露数据库特点**：sml 当配置 / 文档用时，新手与绝大多数场景**无需知道底层是 DB**。
  入口极简，把「内存解析」与「持久化」统一进同一套 sml 心智，底层 DB 只是可选加速/落盘层。
- **UX 铁律（贯穿始终）：操作好上手，同时功能强大。** 常用路径像配置文件一样直觉（面向新手），
  进阶路径不设天花板（面向老手）；默认行为简单，能力用显式入口暴露，不牺牲表达力换省事。
- **提供 sml-style（或 smlb）的 NoSQL**：`.smld`（sml 文本）文档当 NoSQL 文档用 —— 键值 / 文档语义，
  无 join、无 SQL；查询走 §7 选择器而不是 SQL。`.smlb`(二进制衍生) 可作**传输载荷的候选二进制格式之一**（还有文本 `.smld` 等）。
- **sml 本身可作 DB 的“地图”**：schema / 索引 / 关系 / 约束的描述**本身也是一份 `.sml` 文档**
  （元数据即数据），DB 的视图由它生成 —— 避免「身份在 DB 里、数据在 DB 里、却要用第二套语言描述」。
- **目标**：在提供 DB 能力（持久化、FTS 检索、事务、索引）的同时，**性能更好、安全性更好、心智负担更低**。

### 13.2 落地含义（待拍板）

- 与 §5 / §6 **已定项不冲突**：底层仍是 SQLite BLOB + 提取列 + FTS5；只是把 DB 包装层藏到
  sml 心智之后。默认后端 = §6.5 `DefaultBackend`，Turso 等仅命中 §6.6 触发条件才切。
- 要补的是一个「不暴露 DB 细节」的薄层：**路径即键、文档即值、`.smld` 文件=文档、搜索用选择器而非 SQL**。
- 「sml 作为 DB 地图」的具体形态（如何描述 schema / 扩展点）属后续设计，此处暂不展开。
- 与前面后缀的关系：`.smld`=sml 文本（地图/文档），`.smlb`=sml 二进制衍生（传输/BLOB），`.sml`=通用兜底；
  三者分工已确认 `.smld`(文本)/`.smlb`(二进制)，仅 `.sml` 兜底位待定稿。

---

## 14. DB 的 API 友好度：四层下潜 / 用多少学多少【想法 · 未定 · ⚠️需二次蒸馏】

> 核心一句话：**「让用户永远看不到数据库」**。
> ⚠️ 本节是 API 形态的浮想，**未拍板、未实现**，需二次蒸馏成「API 签名 + 命名取舍 + 覆盖矩阵」的定稿。
> 与 §13 数据库产品取向同主题，是它的「对外 API 表达层」延伸；§13 只有愿景方向，这里给出具体交互形态。

### 14.1 四层下潜（原话 AI 留给后人）

> 「核心是让用户永远看不到数据库。四层下潜，用多少学多少。」

```rust
// L0 一行流 —— 完全不知道有数据库这回事
let doc = sml::open("notes/hello.smld")?;

// L1 文件夹隐喻 —— Store 当 HashMap<path, Doc> 用
let mut store = Store::open("notes")?;      // 就是个目录
store.insert("2026/hello.smld", &doc)?;     // 路径即 key
let doc = store.get("2026/hello.smld")?;
for e in store.list("2026/")? { /* 前缀列出 */ }

// L2 声明式查询 —— 依然没有 SQL
store.find().tag("post").created_after(ts).limit(20).load()?

// L3 逃逸口 —— 只有需要时才出现，且名字要劝退
store.raw(|conn| conn.prepare("..."))?      // 文档标注「需懂 SQL」
```

### 14.2 下潜思路点评（AI 技术意见，非拍板）

「下潜 + 用多少学多少」确实是好思路，和 §13 UX 铁律（上手易 + 功能强）同构，一脉相承。**核心洞察是：把「学习成本」从「横切（一开始就得全懂）」改成「纵切（按需逐步暴露）」**：

- **L0（一行流）**：对新手与绝大多数用途，`open` 一个 `.smld` 当文档读 —— 目标用户**永远碰不到 DB 概念**，无学习成本。
- **L1（文件夹隐喻）**：把 `Store` 当成「一个目录」，路径即键、文档即值 —— 心智是文件系统，不是数据库。适合「我有一点数据要多次读写」。
- **L2（声明式查询）**：仍然用链式选择器（不是 SQL），延续 §7 选择器心智 —— 适合「我要筛一批」。
- **L3（逃逸口）**：`raw(|conn| ...)` 拿到真连接写 SQL/docs —— **只在需要完整 DB 能力的极少数场景才出现**，且刻意重申「需懂 SQL」，命名上就劝退大多数人，防止它变成默认路径。

**为什么成立**：四层是**递进门槛**而非**割裂接口**。L0 用户永远不知道 L3 存在；L3 用户从 L0 一路下来心智是连续的（前面几层的「文档/目录/选择器」概念在 L3 依旧成立，只是多拿到一个原始连接口）。学习成本随需渐增，而不是一开始全额预付。

**待蒸馏的落点（拍板前要回答的）**：
- 每层的真正**触发条件**和**目标用户**如何划分（什么时候该用 L1 而非 L0、何时该上 L2）？
- L1 的 `Store` 与 §13.2「路径即键、文档即值」是否就是同一层（疑似一致，需确认是否合并）？
- L2 的 `find()` 链与 §7 既有的「选择器」语法如何统一表达、还是两套并存？
- `raw` 的劝退命名具体形式（`raw`/`escape_hatch`/`unsafe` 风格），以及若被滥用如何兜底？
- API 覆盖矩阵：四层各需要哪些方法、返回类型、错误处理，写成可验收的签名清单。

---

## 15. 对象键序（key order）问题【记录 · ⚠️须先确认为哪个点】

> 用户点名「记一下键序的问题」。落档现状约定 + 候选分歧点，**未拍板**。
> ⚠️ 先对齐：下面区分「已是约定」与「可能要讨论的点」，需用户确认到底记哪一面。

### 15.1 现状约定（已写进文档，是**既定事实**，非待定）

- Rust 侧 `Value::Object` 用 `BTreeMap` ⇒ 序列化**按键字典序**输出（`rust/README.md` / `sml-value/README.md`）。
- C / C++ / JS / Lua **保书写顺序**。
- 语言约定：**对象键序不保证**；需要有序序列时用**数组** `[ … ]`（`site/…/book/ch12-smltools.md`、主 `README.md`）。
- 五端一致性中，键序（C 保源序 vs Rust 排序）被归为「纯渲染差异」，**不误报**（`tests/conformance/known_divergences.md:27`）。
- `smltools` JSON 互转侧：JSON 键按字典序输出，原始书写序不保留（`ch12-smltools.md:93-95`）。

### 15.2 候选分歧点 / 可能要想的问题（逐个待确认）

1. **跨五端字典序 vs 源序不一致**：同输入五种实现产出字节不同 —— 是否要统一？
   （现有裁决：视作语言约定、接受分歧，`known_divergences` 已登记）
2. **确定性问题**：`smltools` 推断（infer）走 `BTreeMap` 保证同输入逐字节相同（`TASK-agent-w24.md:391`）——
   这是「效果好」的一方；会不会有人要求 Rust 也保源序来做 diff / 人类可读往返？
3. **与 §12.3 `.smlb`「键序决定性（按键排序写）」的关联**：二进制格式已拟按排序写，换取确定性字节 ——
   与当前「对象键序不保证」是否要统一成一个总说法？
4. **`to_string_direct`（I-1 直出）特意保留字段声明顺序**（为保 `BTreeMap` 重排的替代）——与「对象键序不保证」
   是否存在文档/心智上的冲突需要澄清？

### 15.3 「少量空间换保序」的方案分析（用户询问，未实现）

用户提：「能不能拿少量空间换（单独做一个树状表）解决键序保序？或更好的方案？」

**现状关键**：`Value::Object` = `BTreeMap`（dict序），`as_object` 暴露 `&BTreeMap`，
dump 直接遍历它输出；但 `ser_text.rs` 已另走一条 `Vec<(String, Node)>` **保序**路子（直出用，不进 `Value`）。

**对「单独树状表」的判断 —— 方向对了一半，形态不对**：
- 需要额外信息保住序 ✓（直觉正确）
- 但旁路顺序表（无论 `Vec<String>` 还是树）**要再存一份 key** ⇒ String key 空气税 ×2（§2：一份≈48B）。
  对象越大这个「换」越贵，不是真正「少量空间」。
- 还要同步两份结构（插/删/改 key），不一致就静默乱序，心智负担比 dict序 更糟。

**更好的方案（三档）**：
| 方案 | 结构 | 序 | 查找 | 空间 | 复制度 |
|---|---|---|---|---|---|
| B1 | `Vec<(String, Value)>` | 保序天然 | O(n) 线性 | key 一份，比 BTreeMap 省（无二叉树指针） | 低；`ser_text` 已趟过 |
| B2 | Vec 保序 + `HashMap<String,usize>` 索引（本地 IndexMap） | 保序 | O(1) | +~8B/键 索引（key 仍一份） | 高；零依赖铁律禁引 indexmap，须自实现≈200 行 |
| B3 | 不动结构，dump 按源序排 | 需另记源序 | — | 等同一份序信息 | 等于没解法 |

**技术意见（非拍板）**：放弃「树状表」。偏爱 **B1** —— 贴 sml 实际形态（小对象、配置文件式）、
key 更少一份、零依赖，`ser_text` 已趟过路；O(n) 对真实负载够用。需要大对象随机查时再给 B1 补 B2 索引，
往后升级不推倒。B2 仅在「大对象 + 频繁随机查」才值得。

**⚠️ 大副作用（须先裁决）**：改保序会**推翻已写死多年、且五端一致性登记为「不误报差异」的语言约定**
（README 明文「对象键序不保证、要保序用数组」）。动手前须重新对齐 `known_divergences` + conformance，
并裁决「是否为保序而付出这份破坏性代价」。

> 📌 现状：未实现、未拍板。默认仍是「对象键序不保证」。方案需用户裁决后再谈落地。

### 15.4 「树的局部保序」可行性（用户认可方向，未实现）

用户问「树的局部保序可以支持吗」，方向获得认可（`ok`），落档分析如下。

**三种含义的界定**：
- **A. 个别对象 opt-in 保序**（新变体 `OrderedObject(Vec<...>)`）—— 可行、破坏面最小。
- **B. 按语义分层保序**（配置子对象保序 / 多实例聚合块 dict 序）—— 语义最贴需求，
  但 parser 在归并时刻往往还不知道块的性质（类型到解析末尾才定型），实现难度高。
- **C. 只对根到某深度路径保序** —— 动机弱、实现最拧巴，不推荐。

**红线圈定（A 能成立的前提）**：保序只对**未参与合并**的干净子块成立。sml 的归并路径
（`&base` `extend`、emit 片段 `extend`、子容器 `insert_field_val`）语义本质是「按名并集」，
一进 `extend`，源的书写序在数学上就丢了 —— 合并过的对象给「保序」是**假保序**（给出拼装后顺序，更误导）。

**落地方向（非拍板、未实现）**：
- 走 **＋A 型**：`Value` 加新变体，不动 `Object(BTreeMap)` 的 log n 查找与既有破坏面；
  边界绑定「该对象未参与合并」。
- 与五端一致性天然兼容：只有 Rust 侧显式新变体保序，老路径行为不变，`known_divergences` 不动。
- 对比 §15.3：全树保序是大手术，局部 opt-in 是小补丁，代价与收益都小一个量级。

---

## 16. 设计决策记录（2026-11-04 设计会话 · 进行中）

> 本节逐条落档会话结论：**「已定」= 用户明确裁决**，「待定」= 仍空着。
> 与 §1–§15 的旧文冲突时，**以本节为准**（旧文是 2026-11-02 之前的写作判断）。

### 16.1 已定

| # | 决定 | 影响 |
|---|---|---|
| **D1** | **发布节奏**：`.smlb` / `smldb` 与 v5 一起开放，不单独发 | 设计现在做，不进排期 |
| **D2** | **编 `Value`** —— `.smlb` 是 **SML 的二进制化**，不是 HTML 节点 AST | §3 的节点式布局（`tag_id`/`nattrs`/`text_id`/`nchildren`）**要重写** |
| **D3** | **独立 crate**：`smlb` 在 monorepo，只依赖值模型，**不依赖 `sml-parse`** | 保证「读一个文件不必拖进解析器」；zstd / 加密走 feature |
| **D4** | **两档不混**：`.smlb` = 不可变档（mmap + 零拷贝）；热数据走 `smldb` 的 BLOB + 缓存 | 「实时读写」不塞进 `.smlb` 文件格式 |
| **D5** | **版本 = `edition`（跟语言同号）+ `fmt` 补丁位**（常态 `0`，仅字节布局变动才 +1） | 语言与格式可独立演进，用户平时只看见一个号 |
| **D6** | **键序：保序**（源序进源序出） | 键序成为**数据的一部分** ⇒ canonical 必须保留源序；签名/去重/缓存键随之变化 |
| **D7** | **签名对象**：默认覆盖**全部数据**（**除签名段自身**），范围可由元数据声明 | 「元数据看范围」这是唯一例外；不做 raw/语义两档并存 |
| **D8** | **签名后常常只读，但非绝对** ⇒ 必须提供**追加 / 合并**通道 | 见 D9 |
| **D9** | **做合并机制（追加支持）** | 语义细节见 §16.2 P1 |
| **D10** | **manifest 用 SML 自描述**（段表 / 算法 / profile / feature），可挂契约校验；未知字段按 `loose` 降级 | 继承 `.smlex` 已验证的路子；代价是一次数十 µs 的解析 |
| **D11** | **能力位两档**：`required`（读不懂必须拒）/ `optional`（可跳过） | 否则「能力扩展」就是空话 |
| **D12** | **补齐六项常用能力**：跨语言可读（五端一致）· 字节序与对齐明确 · 段 checksum + 损坏隔离 · 溯源元数据（**纳入签名范围**）· 压缩与加密顺序固定 · canonical 往返幂等（`dump→load→dump` 字节相同） | 缺任一项就会出现「别的格式轮子」 |
| **D13** | **FTS：折中** —— `.smlb` 不做索引，只提供 `extract_text()`；索引归 `smldb` 或外部（FTS5 / Tantivy） | `.smlb` 保持零依赖；搜索引擎与文件格式解耦 |
| **D14** | **不在 `.smlb` 内承担**：可变 DB 文件（MVCC）· 追加式日志 · 媒体编解码 · 通用搜索引擎 | 明确边界=避免万能胶退化 |
| **D15** | **键冲突必须显式声明**，同一文件内**签名要求一致** | 不做隐式 last-wins |
| **D16** | **新签名须由旧签名在失效前授权**（转授权链），否则不认 | 轮换密钥的唯一合法路径 |
| **D17** | **系统时间不可信**（可被改 / 可能离线 / 连不上权威服务器） | 有效期只是**提示**，不进验证的必要条件 |
| **D18** | **基座定位**：只做「SML 数据的二进制化」；信任根选择、撤销、锚点验证交给调用者 | 与 D14 同向：策略不进格式 |
| **D19** | **追加默认链式**；多来源须先合并成一个文件对象再按序追加（线性，无分支） | 文件内永远一条历史 |
| **D20** | **并发写 = 多读一写**（写者加锁；读者 mmap 旧版本不受影响） | 追加必须基于「当前头」，`prev_hash` 不匹配即冲突 |
| **D21** | **多读多写交给 `smldb`**（SQLite WAL），`.smlb` 不自建 OCC | 与 D4「两档不混」一致 |
| **D22** | **数组补丁 = 整体替换**（不拼接、不按 index 合并） | 按位改数组是 featag `[n]` 的职责，避免两套机制打架 |
| **D23** | **`.smlb` 可以装「不经过 SML 的东西」**：外来载荷（其它格式的原始字节）是**一等段**，原样存取，库不解码 | ⇒ `.smlb` = **容器（manifest + 段）+ Value 编码**，不是「只能装 Value」的格式；与 D12「其它格式嵌入」、D14「不做媒体编解码（只当信封）」一致 |
| **D24** | **自定义衍生格式只能定义「语义」**（段 id 含义 / 必填段 / 扩展键语义），**不能定义「字节编码」** | 「未知段可跳过」是唯一不变量；允许字节层自定义 ⇒ 读前要先跑解释器，不变量即失效 |
| **D25** | **profile 只影响工具行为**（校验 / 补全 / 签名策略），**不得成为读 `VALUE` 段的前置** | 否则 bootstrap 循环 |
| **D26** | **API 面 = 5 组 20 个函数**（值↔字节 / 文件 / 读 / 写 / 流·能力·安全），可作为其它 crate 的**样板** | 组内一名一职；`append` 是唯一写路径；不给 `save` / `update` / `set` / `sync` |
| **D27** | **逆向导出走 A + C 兜底**：默认 `to_value()` + 由 `smltools`/`swsml` 出文本；`smlb` 另提供可选 feature `text` 转发到 `sml::emit::to_sml` | 不内置第二个文本发射器（B 方案）—— 那是长期债 |
| **D28** | **加密粒度 = 4 KiB 物理块**（小于一块的段**合并进同一块**）+ 长度量化 **64 B 档**（manifest 可配 `crypto_block` / `pad_quantum`） | 段级加密会被 AEAD 头开销吃掉（20 B 段 → 48 B 密文） |
| **D29** | **nonce 永不复用**：`per-file DEK` + `HKDF(DEK, "smlb:"+algo)` 派生 key，`nonce = layer_id ‖ block_index ‖ salt` | 换密钥即换 DEK，旧层 nonce 空间随之作废 |
| **D30** | **先压缩后加密**；提供 `--no-compress` 关闭压缩以断开 CRIME/BREACH 类推断链 | 长度量化只防「精确长度」，不防「压缩后长度」 |
| **D31** | **签明文再加密**（sign-then-encrypt）；`SIG` 段**明文存放**，不参与加密 | 否则「验签」要以「解密」为前置 |
| **D32** | **`VALUE` 段布局 = 前序 + 固定 4 B `subtree_len`**（跳子树 O(1)） | 见 §19；写入留占位、段尾回填 |
| **D33** | **`DIR` 段做**（可选、派生）：`(parent_off, key_id, child_off)` 排序 + `head_layer`；默认只索引深度 ≤ 4 | 缺失/过期只影响速度；追加后旧索引自动失效 |
| **D34** | **段头用正交双字段 `comp` + `enc`**（段头 41 B），不枚举「压缩+加密」组合 | 见 §19.1 / §20 |
| **D35** | **只允许 AEAD**：禁 CBC / CTR / 任何「加密 + 独立 MAC」组合 | 加密与完整性一次做对 |
| **D36** | **多签名 = 数组，全部必须通过**（不是任一） | 抗量子那一路失效必须暴露出来 |
| **D37** | **默认值表**（§21）：默认写进 manifest；「启用才有代价」的默认关、「关掉只影响性能」的默认开 | 改默认值不影响旧文件 |
| **D38** | **配套工具面**（§22）：`info`/`ls`/`cat`/`check`/`verify`/`sign`/`append`/`pack`/`unpack`/`vectors` + `smltools` 互转 + 五端解码器 | 「格式的价值有一半在工具」；`vectors` 是 T0 前置 |
| **D39** | **填充必须确定性**（只按长度量化，禁随机填充） | 随机填充会破坏幂等（C12）、测试向量与字节级去重 |
| **D40** | **侧信道基线**（§24）：解密失败统一报错 · 多签名全验不短路 · 常数时间比较 · 访问/写入模式泄露**记录在案** | 前三条必须，后两条明确接受 |
| **D41** | **熵只进一次性标识，伪随机只进可复现路径**（§24.4）：`PRNG(HKDF(master,purpose), file_id‖layer‖block)`；禁 OS RNG 影响布局、禁弱 PRNG 顶替 nonce、禁跨用途复用 `purpose` | 同时拿到「幂等」与「外部不可预测」 |

### 16.2 待定（按依赖排序）

| # | 问题 | 为什么重要 |
|---|---|---|
| **P1** | ~~合并语义~~ **已定（D15 / D20 / D21 / D22）** | 键冲突声明式、数组整体替换、并发多读一写、多写交 `smldb` |
| **P2** | **签名的层链形态**：每层独立签名 + hash 链 / 只签最新层 | 决定「验签能不能回答『这个值来自哪一层』」 |
| **P7** | **`.smlb` 是否带可选 `SOURCE` 段**（原文 + 行号映射） | 决定「注释与源码位置能不能保住」，也决定体积与 round-trip 保真度 |
| **P8** | ~~是否允许无 `VALUE` 段~~ **已定：允许（①）** + **manifest 永远有 `primary` 指针**（可指任意段） | `.smlb` 兼作通用归档容器；「主内容是什么」永远有答案 |
| **P9** | ~~manifest 形态~~ **已定：③** —— **二进制 manifest 作权威** + 可选 `MANIFEST_SML` 镜像段（人读 / 工具校验，库不解析） | 零依赖与「自描述可挂契约」两者都保住：契约校验在 `smltools` 侧 |
| **P10** | **能力裁剪与格式扩展的判定**：编译期 feature 关掉 与 异构读者，必须**走同一个判定函数** | 否则两种失败模式会长得不一样，排障时无法判断「是裁剪还是版本旧」 |
| **P3** | **加密与国密 / 量子**：算法 ID 表、协商、与压缩的顺序 | 迁移那天整个格式作废与否在此 |
| **P4** | `§10` 剩余四条：`:has()` 进不进 v1 · `.content` 是否等价 `[class~=content]` · `id` 是否强制唯一 · 规模信封 | FTS 已定（D13） |
| **P5** | 键序落地细节：canonical 规则、compact 之后签名怎么办 | 与 D6 绑定 |
| **P6** | `§3` 格式重写：编 `Value` 的具体字节布局（对象/数组/标量/元数据键） | D2 的施工图 |

---

## 17. 施工顺序（草稿 · 未开工 · 与 v5 同步开放）

> 开放节奏见 D1：**设计与实现可以现在做，对外开放等 v5**。阶段 1–3 与 v5 定稿并行不冲突。

| 阶段 | 做什么 | 验收判据 |
|---|---|---|
| **T0 前置** ✅ 已落地（2026-11-04，见 §18.6） | `Value::canonical_bytes()` 的规则定稿（保序、数字格式、`__*` 位置、转义）+ **跨端测试向量**（文本 ↔ 二进制各 N 组，期望字节写死） | 无 `preserve_order` 时**显式报错**；向量在 Rust/JS 上产出同一字节（**JS 侧待 T7**，见 §18.6 待确认 ②） |
| **T1 编码（L0）** ✅ 已落地（2026-11-04，见 §19.6） | 新 crate `smlb` 骨架 + 文件头 + 段表 + `dump` / `load`（`info` / `ls` 提前到 T1） | `dump→load→dump` 字节相同；`Int(1)` 与 `Float(1.0)` 可区分；`__type`/`__featags` 原样往返；未知段可跳过（用额外段实测） |
| **T2 容器（L1）** ✅ **全落**（2026-11-04/05，见 §19.8 / §19.9） | 段表 ✅ · 摘要（SHA-256 / SM3，按 `hash_id`）✅ · manifest + `primary` + 能力位 ✅ · 外来载荷 `EMBED` + `payloads[]` + `pack`/`unpack` ✅ · `check` ✅ | 只读 manifest 能列全部段 ✅；未知 `optional` 跳过、缺 `required` 明确拒 ✅；改一字节被检出 ✅ |
| **T3 读（L2）** ✅ 除 mmap 外已落（2026-11-05，见 §19.10） | `Store`：`open` / `subtree_bytes` / `get` / `text` / `text_of` / `history_at` / `payload` / `payloads` ✅；`DIR` / `Mmap::open` / `provenance` 未做 | 「取单个子树不遍历全树」已证（改坏兄弟 tag 仍能取别的）✅；**RSS 判据未达成**（等 mmap 依赖裁决） |
| **T4 历史与签名（L3）** ✅ 核心已落（2026-11-05，见 §21.1b） | 层链（`prev_hash`/`index`/Trailer）+ `sign` / `append` / `verify` / `delegate` / `tree` + `patch` 合并（D22）；**写者锁（D20）未做** | 验签能答「值来自哪层」（`cat --at N`）✅；未授权的换钥被拒 ✅；并发冲突检测等锁落地 |
| **T5 压缩与加密（L4）** ⏳ 压缩全落、加密未做（2026-11-05，见 §19.11 / §19.12） | 段级 zstd ✅ · 帧级 `FRAMES` + 索引 ✅ · 导航器按需解帧 ✅ · 解压预算 ✅；**加密 ⬜** | 「只解压被访问的部分」✅（段级 + 帧级都达成）；「未知算法 ID 明确拒绝」✅ |
| **T6 工具（T6）** | `smltools --to smlb` / `--from smlb` / `--keep-source`；`smlb sign/verify/append` CLI | 文本 ↔ 二进制双向可跑；签名可被另一端验证 |
| **T7 五端** | JS / Lua 解码器（只读 `VALUE` 段 + 透传载荷）+ 一致性用例 | 同一 `.smlb` 五端读出同树；载荷字节原样 |

**依赖关系**：T0 是 T1 的前置（没有 canonical 就没有幂等与签名）；T2 依赖 T1；T3–T5 可并行；T6/T7 依赖 T2。
**明确不在阶段内**：`smldb`（T8，见 §5）、SIMD（§8，等 profile 证明）、FTS 索引（D13）。

---

## 18. canonical 规则（草案 v0.1 · P5 的施工图）

> 目的：让「同一份数据」无论来自 `.sml` 文本、程序构造还是另一个 `.smlb`，都能产出**同一串字节**。
> 只有它稳定，签名（D7）、幂等（D12）、`dump→load→dump` 判据才成立。

### 18.1 两种形态

| 形态 | 用途 | 是否受 `fmt` 影响 |
|---|---|---|
| **`canonical_text(&Value) -> String`** | 签名对象、跨端测试向量、人类可读比对 | **不受**（与字节布局解耦） |
| **`canonical_bytes(&Value) -> Vec<u8>`** | `dump()` 的输出，供 `load()` 往返 | 受（`fmt` 变化即变） |

⚠️ 关键取舍：**签名签 `canonical_text`，不签 `canonical_bytes`** —— 否则 `fmt` 一升级，所有历史签名全部失效。**⬜ 待你确认（P5-a）**

### 18.2 文本 canonical 规则

| # | 规则 |
|---|---|
| **C1** | 编码 UTF-8，**无 BOM**，行尾一律 `\n`（不受平台影响） |
| **C2** | **键序 = 源序**（D6 保序）。无 `preserve_order` 的构建**直接报错**，绝不排序 |
| **C3** | 缩进固定 2 空格；不写行尾空格；文件以单个 `\n` 结尾 |
| **C4** | `Int`：十进制 ASCII，无前导零、无 `+`、`-0` 归一为 `0` |
| **C5** | `Float`：最短往返表示，**必须带小数点或指数**（`1.0` 不能写成 `1`，否则 load 回 Int）；`-0.0` 保留符号 |
| **C6** | `Bool` 写 `true` / `false`；`Null` 写 `null` |
| **C7** | 字符串：能裸就裸（裸词规则同 v1），否则双引号；**必须转义** `\` `"` 与 C0 控制字符（`\n` `\t` `\r` `\u{XX}`） |
| **C8** | 数组：单行 `[ a b ]` 或分行；元素间用空格分隔（不用逗号），保持元素顺序 |
| **C9** | 空对象写 `{ }`，空数组写 `[ ]`（括号内一个空格，避免与 `{}` 混淆） |
| **C10** | 元数据键 `__type` / `__name` / `__args` / `__featags` **固定在块内最前**，顺序即此序（见 P5-c） |
| **C11** | 不允许输出注释（`Value` 里没有注释；原文在 `SOURCE` 段时才保留） |
| **C12** | 同一逻辑数据必须产出同一串字节 —— **任何实现不得「顺手美化」**（如把 `[a b]` 拆行） |
| **C13** | **非有限浮点（NaN / ±Inf）没有字面量**：语言里只能「由算式得到」（如 `1/0`），而 canonical 是静态文本 ⇒ **遇到非有限浮点直接报错**（`E-SMLB-非有限浮点`）。确实要存时，由调用者显式转成字符串（`"nan"` / `"inf"`），库不代劳 |

### 18.2b 实现澄清（T0 落地时补，2026-11-04）

> 下面各条是**把 C1–C13 写实**时必须回答、而规则表本身没写死的问题。写进这里，是为了让
> JS / Lua / C / C++ 的移植不用去猜 Rust 的实现（判据以 `tests/vectors/smlb-canonical.v1.json`
> 为准，见 §18.6）。

| # | 澄清 | 理由 |
|---|---|---|
| **C5-a** | **canonical 一律忽略 `Float` 的 raw** | raw 是解析期的书写痕迹、不参与相等比较；若让它进 canonical，同一份数据「从文本读入」与「从 `.smlb` 读入」会得到两串不同文本 ⇒ C12 当场失效，签名随之分裂。`raw` 仍由 `to_sml` 保留（那是给人看的输出） |
| **C5-b** | 浮点写法 = **十进制展开 + 整数值补 `.0`**（`Display` 最短往返数字，不用科学计数法：`1e10` → `10000000000.0`；`-0.0` → `-0.0`） | 与 `to_sml` 的浮点写法共用同一份实现（`text_rules::format_float`），避免两套发射器漂移。⚠️ 跨端需照此展开 JS 的 `1e+21` 形式（见 §18.6 待确认 ②） |
| **C7-a** | 转义集 = `\n` `\t` `\r` `\0` `\\` `\"` + 其余 C0/0x7F 走 **`\uXXXX`**（4 位十六进制，**不是** Rust 的 `\u{...}`） | 跟随现词法层的转义集；canonical 必须是**能解析回去**的文本（P5-d） |
| **C8-a** | 「单行 / 分行」判据 = **直接子项是否全为标量**（不含容器）。全标量 ⇒ `[ a b ]` / `{ k: v, k2: v2 }` 单行；否则逐项分行 | 判据只看一层、只看值的形状，与行宽无关 ⇒ 确定性（C12）。**数组元素间用空格**（C8 原文），扁平对象字段间用 `, `（与 `to_sml` 的行内形式一致，且已能往返） |
| **C8-b** | 多行容器的收尾括号**回到键的缩进**；`键: {` 与 `键: [` 的括号留在**同一行** | 「键与它的值永远相邻」，减少一条排版自由度 |
| **C9-a** | 顶层对象写**字段列表**（不套 `{ }`）；空对象仍写 `{ }` | 顶层是「文档」而不是「值」；空文档没有字段可写，只能退到 C9 的空对象形式 |
| **P5-d-a** | 「输出是合法 SML」的边界 = **顶层为容器**。顶层标量照常产出文本（签名字节唯一），但**不可作为文档回读** | parser 明确要求顶层为容器（`E-PARSE-008`「顶层须为容器，单独的标量无法往返」）。这是 parser 的既有约束，不是 canonical 的取舍 —— 若要改，改 parser 的顶层规则，而 canonical 跟着走 |
| **C2-a** | 未开 `preserve_order` 时**整份文档一律报错**（连标量也报） | 放行「恰好没有键的值」会让入口变成「有时能用、有时不能用」，比一律报错更难排查 |


### 18.3 元数据（manifest）canonical

manifest 本身也要被签，所以它也要 canonical 形式：

```
canonical_manifest = 按段 id 升序 + 每段 (id, algo, len, sha256(段字节))
```

- **不含** `offset`（布局会因追加而变，位置不是语义）；
- **不含** 时间/工具版本之外的易变字段（否则每次打开都不同）；
- 段字节本身参与哈希 ⇒ 改任一字节签名即失效。

### 18.4 签名覆盖范围（把 D7 写实）

```
signed_bytes = "smlb/sig/v1\n"
             ‖ canonical_manifest
             ‖ canonical_text(primary_value)      // 无 VALUE 段时为空
             ‖ 各外来段 (id, sha256) 按 id 升序
排除：SIG 段自身、以及 manifest 里标了 exclude_sign 的段
```

### 18.5 本节的四个子问题（**已定**）

| # | 结论 |
|---|---|
| **P5-a** | **签 `canonical_text`**（不签 `canonical_bytes`）⇒ `fmt` 升级不作废历史签名 |
| **P5-b** | **非有限浮点无字面量** ⇒ canonical 遇 NaN/±Inf **报错**（C13）；要存就由调用者转字符串 |
| **P5-c** | **元数据键固定最前**（`__type` → `__name` → `__args` → `__featags`），不跟源序 |
| **P5-d** | canonical **由软件映射生成**（`Value ⇄ 文本` 的双向映射就是实现方式）；但仍要求**输出是合法 SML 文本**——理由见下 |

**为什么 P5-d 仍要求「合法 SML」**（即使映射由软件做）：

1. 现有 parser 能直接验证 canonical 的正确性（不用另写校验器）；
2. 用户能用 `.sml` 文本读到同一份数据，`diff` / 代码审查有落脚点；
3. 否则 canonical 会变成**第三种语法**（`.sml` 一种、canonical 一种），与「不造轮子」相悖。

⇒ 判据写法：**不把「解析回等价 `Value`」写成规范要求**（避免绑定 parser 的实现细节），而是把它当 **T0 的测试向量判据**。

### 18.6 T0 落地状态（2026-11-04）

> 本节是**实现即文档**：写的是代码现在长什么样、怎么验、还剩什么没定。

**落点**（D3 的边界：`.smlb` 只依赖值模型）

| 东西 | 在哪 | 说明 |
|---|---|---|
| `Value::canonical_text()` / `canonical_bytes()` / `from_canonical_bytes()` | `rust/sml-value/src/canonical.rs` | 规格把 API 写作 `Value::canonical_*`（§18.1），孤儿规则决定它必须住在定义 `Value` 的 crate；**容器层（段表/manifest/层链/签名/加密）一概不进 `sml-value`** |
| `CanonicalError`（13 个变体 + `kind()`） | 同上 | 码表还没有 `E-SMLB-*`（面在 T2 定），故先给稳定 `kind()`；深度沿用 `E-LIMIT-004` |
| 「文本原子」规则（裸词/引号/转义、浮点写法） | `rust/sml-value/src/text_rules.rs` | 从 `dump.rs` 搬出来，**三个发射器共用一份**（`to_sml` / `ser_text` / `canonical`） |
| crate `smlb` + `smlb vectors` CLI | `rust/smlb/` | T0 只有门面 + 向量；编码器骨架归 T1。`publish = false`（D1） |
| 跨端向量 | `tests/vectors/smlb-canonical.v1.json` | 21 例，`canonical_text` + `bytes_hex` 双期望值 |

**四条硬判据都已写进测试**（不是口头）

| 判据 | 钉在哪 |
|---|---|
| 无 `preserve_order` ⇒ **显式报错**（C2，绝不排序） | `rust/tests/canonical.rs`（默认档）+ `rust/smlb/tests/vectors.rs`（默认档） |
| 非有限浮点 ⇒ **报错**（C13，不伪造字面量） | `canonical.rs` 单测 + `tests/canonical.rs` |
| 同一逻辑数据 ⇒ 同一串字节（C12） | `tests/canonical.rs::c12_*`、`smlb vectors` 自检 |
| `dump→load→dump` 逐字节相同 / `Int(1)`≠`Float(1.0)` / `__type`·`__featags` 原样往返 | `tests/canonical.rs` |

**怎么跑**

```
cargo test --test canonical                                  # 默认档：验 C2
cargo test --features preserve_order --test canonical        # 保序档：验内容
cargo test -p smlb --features preserve_order                 # 向量文件与代码一致
cargo run -p smlb --features preserve_order -- vectors --check
```

⚠️ `smlb` 的 `preserve_order` 是**opt-in**，不让它硬开：cargo 的特性按构建图取并集，
硬开会让 `cargo test --workspace`（CI 的命令）把整个 workspace 的 `sml-value` 一起切到
保序档 —— 等于顺手改掉 `swsml` / `smltools` 上百条「默认档 = 字节序」的 golden。

**待确认（不阻塞 T1，但会影响跨端）**

| # | 问题 | 现状 / 建议 |
|---|---|---|
| ① | **顶层标量**要不要允许 canonical 文本「不可回读」（P5-d-a） | 现状：允许，文本照出、签名字节唯一。若要求严格合法，则只能改 parser 的顶层规则（`E-PARSE-008`），不是 canonical 能单方面决定的 |
| ② | 浮点跨端口径（C5-b）：Rust 走十进制展开，JS `toString()` 在 `1e21` / `1e-7` 处切指数 | **用户 2026-11-04：跨端的之后再说，与四端统一同步**（与 S9「四端之后统一同步」同口径）⇒ 此处只留证据与两个候选（现状：Rust 十进制展开，向量里 `1e10` → `10000000000.0`；另一候选是把 canonical 改成 ECMAScript 形式），**此刻不裁决**，等四端一起定 |
| ③ | 元数据键在字节侧是否也前置（L5 的副作用） | 现状：文本前置（C10）、字节保源序（§19.2 原文）⇒ 两份仅在元数据位置不同的数据，文本相同而字节不同。若要「字节 = f(文本)」这一更强性质，需改 §19.2 |

---

## 19. `VALUE` 段字节布局（v1 草案 · P6 落定）

> 选型：**A 布局（前序 + 固定 4 B `subtree_len`）** + **可选 `DIR` 段**（路径 → 偏移索引）。
> 取代 §3 那份建立在「HTML 节点」上的旧布局（D2 已改判为编 `Value`）。

### 19.1 段头（41 B）

```
seg_id:u8 | comp:u8 | enc:u8 | flags:u16 | len:u32 | sha256:32
```
- `seg_id`：`0x01=VALUE 0x02=STRTAB 0x03=DIR 0x10=FRAMES 0x20=EMBED 0x30=SIG 0x40=PROFILE 0x50=SOURCE 0x60=PROVENANCE`
- **`comp` 与 `enc` 正交两字段**（D34）：压缩与加密组合不枚举，避免组合爆炸
- `sha256:32` 固定 32 B，**具体哈希由 manifest 的 `hash_id` 决定**（SHA-256 / SM3 / BLAKE3 都是 32 B，见 §20）
- 段起始 **4 KiB 对齐**（配合 D28 的块加密与 mmap）

### 19.2 编码规则

| 项 | 规则 |
|---|---|
| 字节序 | 定长 little-endian；变长 **LEB128** |
| 类型标签 | `0=Null 1=False 2=True 3=Int 4=Float 5=Str 6=Arr 7=Obj` |
| `Int` | **zigzag + LEB128**（小值 1 B） |
| `Float` | IEEE-754 **f64 小端 8 B**；与 `Int` 靠 tag 区分（不靠格式） |
| `Str` | `STRTAB` 的 id（varint）；表内 `(len:varint, utf8)*`，全局去重 |
| `Arr` | `count:varint` + 逐元素 |
| `Obj` | 按**源序**写 `count:varint` + 逐条 `(key_id, value)` |
| 元数据键 | **不特化**：`__type` / `__name` / `__args` / `__featags` 就是普通键；只在 `flags` 放 1 bit「本对象含 `__type`」以省一次查表 |
| 子树长度 | 每节点固定 **4 B** `subtree_len`（子树上界 4 GiB），写入时留占位、段尾统一回填 |
| 跳子树 | `pos += 4 + subtree_len` ⇒ **O(1)** |

节点线格式：

```
tag:u8 | flags:u8 | subtree_len:u32 | payload…
  Null/Bool/Int/Float: payload 即值本身（Bool 用 tag 区分，无 payload）
  Str: payload = str_id:varint
  Arr: payload = count:varint + 元素*（递归）
  Obj: payload = count:varint + (key_id:varint + 值)*（递归，源序）
```

### 19.3 `DIR` 段（可选，为随机访问加速）

- 条目：`(parent_off:u32, key_id:varint, child_off:u32)`，按 `(parent_off, key_id)` 排序，二分查找；
- **带 `head_layer` 字段**：与当前 head 不一致 ⇒ 整段忽略（追加后旧索引自然失效，不需要重写）；
- 默认只索引 **深度 ≤ 4** 的对象键（`dir_depth` 可配）：52k 节点下约 10k 条目 ≈ 120 KB；
- 索引是**派生数据**：缺失/过期都只影响速度，不影响正确性（D11 的 `optional` 位）。

### 19.4 与加密/压缩的关系

- 加密单位 4 KiB（D28）⇒ 段内按块切分，`subtree_len` 让「跨块子树」也能整块跳过；
- 压缩（`FRAMES`）在段之上，帧边界不要求与节点边界对齐（解帧后再走节点）。

### 19.5 实现澄清（T0 落地时补，2026-11-04）

| # | 澄清 | 理由 |
|---|---|---|
| **L1** | `subtree_len` = **载荷字节数**（不含 `tag:u8 + flags:u8 + len:u32` 这 6 B 节点头）⇒ 跳过子树是 `pos + 4 + subtree_len`（`pos` 指向 `subtree_len` 字段本身） | §19.2 的「跳子树」公式只在这个口径下成立；写成「含头长度」会让公式差 6 B，且两端口径一旦不一致就静默读错 |
| **L2** | `flags` 位 0 已用：**本对象含 `__type`**（§19.2 原文）。其余位 v1 一率为 0，读到未知位 ⇒ **忽略该位继续解码**（位是加速提示，不是语义） | 位是「省一次查表」的优化，不是数据；把它当语义会让 `flags` 变成第二个版本号 |
| **L3** | **T0 的 `canonical_bytes` 是自包含的「L0 编码」**：`STRTAB 段内容 ‖ VALUE 段内容`，其中 STRTAB 前补一个 `count:varint` | T0 没有容器，`-> Vec<u8>` 必须能被单独 `load()` 回来；而 §19.2 的表内格式 `(len,utf8)*` 本身不带个数，只有段头的 `len` 能界定边界。容器化后（T1）由段头 `len` 界定，`count` 可省 —— 届时两段各自成型，节点线格式**一个字节都不变** |
| **L4** | 键与字符串值**共用一张 `STRTAB`**，id = 前序遍历 + 源序首次出现序；全局去重 | §19.2「表内 `(len:varint, utf8)*`，全局去重」；共用一张表意味着 `"rule"` 作为键与作为值只占一格（向量 `strtab-dedup` 钉着） |
| **L5** | ⚠️ **元数据键在字节侧不前置**：文本按 C10 提到最前，字节按源序写（§19.2 原文「元数据键不特化」） | 两条规则各自有据：C10 是为了「人读 canonical 时元数据总在同一处」，§19.2 是为了「字节里键序就是源序，一条不改」。**副作用**：两份仅在元数据位置上不同的数据，`canonical_text` 相同而 `canonical_bytes` 不同 ⇒ 见 §18.6 待确认 ③ |
| **L6** | 解码侧硬校验（都会拒绝，不静默降级）：`subtree_len` 与实际消费字节数必须相等；树读完后**不许有剩余字节**；未知 tag 拒绝；对象内重复键拒绝（D15）；非有限浮点拒绝（C13）；`count` 须与剩余字节数自洽（防伪造巨大 count 触发巨量分配） | 「未知段可跳过」的前提是**长度可信**；`subtree_len` 不自洽就等于长度不可信。剩余字节不拒，则「追加不改旧字节」会被静默破坏 |

### 19.6 文件头（T1 落定，2026-11-04）

> 这一节此前**是空白**：§19.1 只定了段头（41 B），整个文件的头没写死；
> 唯一出现过的是 §3 时代的 `magic | version | flags | root_offset`（`Value` 编成字节后
> `root_offset` 已无意义）。T1 落地时按用户裁决补齐。

**40 B，定长，写一次永不变**（一律小端）：

```
off sz field          含义
 0  4  magic          "SWSB"（沿用 §12.3 的既有约定）
 4  2  edition        低字节 = major，高字节 = minor（v4 ⇒ 04 00；v5 ⇒ 05 00）
 6  1  fmt            字节布局补丁位（D5），常态 0
 7  1  hdr_len        头部总长（本版 40）；旧读者据此跳过将来新增字段
 8  8  seg_table_off  u64 —— 用户 2026-11-04：u64 现在就付（v1 恒 = hdr_len）
16  4  seg_count
20  2  flags          **位 0 = 段体对齐粒度**（0 = 8 B / 1 = 4 KiB，§19.7 丙）；其余位忽略
22  2  reserved       0
24  8  tail_off       T4 的尾部段表预留，v1 恒 0
32  8  reserved       0
```

**布局**：头 `[0,40)` → 段表 `seg_count × 41 B`（**紧跟头**，不再钉在 4096）→ 按 `flags` 位 0
声明的粒度对齐 → 各段数据 → （T4 起）**层记录区**（只追加）→ 末尾 64 B 的 Trailer。
Trailer 从 EOF 反向读固定长度，细节见 §21.1b。

为什么头里**不放** `Trailer` 偏移、也不放 manifest：两者都会随追加而变，而头是「写一次永不
变」的东西（否则不变量 3 立刻失守）。manifest 仍是**段**（`0x04`，T1 只登记不写内容）。

⚠️ **四件必须知道的实现事实**

1. **段数据没有偏移字段**：位置由「表尾对齐 → `align_up(prev + prev.len)`」走出来。
   好处是段表项保持 §19.1 的 41 B，且与 §18.3「manifest 不含 offset」同口径；代价是取第 k 段
   要走过前 k 段（段数几十 ⇒ 纳秒级）。
2. ~~**`sha256:32` 在 T1 写全 0**~~ —— **T2 起已写真摘要**（零依赖 SHA-256，见 §19.8）；
   T1 时代的文件（摘要全 0）仍可读，只是「未校验」。`info` 对两种文件分别明说。
3. **`STRTAB` 只在非空时落段**：空表没有字节，写它只是白白多 4 KiB 对齐填充。
4. **小文档的固定开销（已按 §19.7 丙落地）**：T1 时代一份 65 B 载荷落成 **12,302 B**；
   丙落地后同一份是 **772 B**（头 40 + 段表 123 + 三个段体，8 B 对齐），其中 **516 B 是
   manifest**（`segments[]` 里每条 64 位十六进制摘要最占地方）。加密文件回到 4 KiB 对齐
   （≈12.8 KB），那是 D28 的块加密要求，不是格式税。

### 19.7 待裁：段对齐 vs 「小段合并进同一块」

| 说法 | 原文 | 冲突点 |
|---|---|---|
| 甲 | §19.1「段起始 **4 KiB 对齐**」 | 每个段各占 4 KiB 的整数倍 ⇒ 51 B 的 `VALUE` 也吃 4 KiB |
| 乙 | D28「加密粒度 = 4 KiB 物理块（**小于一块的段合并进同一块**）」 | 隐含「多个段可以**共享**一个 4 KiB 块」⇒ 段不各自对齐 |

**✅ 已裁决（用户 2026-11-04）：走丙** —— `flags` 位 0 声明对齐粒度，写者决定：

| 位 0 | 粒度 | 谁用 |
|---|---|---|
| `0` | **8 B** | 明文文件（默认）。小文档不再吃 4 KiB 固定税 |
| `1` | **4 KiB** | 加密文件（T5 强制）：块 AEAD 要求段体落在块边界上，否则追加会重写旧块 |

**为什么粒度必须自描述**：段数据没有偏移字段、位置靠走表算出来（§19.6 事实 1），
粒度猜错 ⇒ 整张表错位 ⇒ 读到垃圾。写一位、读者无条件照它走，是唯一不靠猜的做法；
而「不懂位 0 的旧读者」会把 4 KiB 文件按 8 B 走 —— 错位后**立刻**被长度/摘要检查拦下，
是**响亮的失败**而不是静默读错。

落地：`smlb::ALIGN_MIN` / `ALIGN_BLOCK` / `FLAG_ALIGN_BLOCK` / `align_of(flags)`，
`DumpOptions::block_align` 是写者旋钮（默认 `false`）。T5 加密时强制置位。

### 19.8 段摘要 + `check` / `cat`（T2 前半，2026-11-04）

**摘要已真写**：段头的 `sha256:32`（字段名沿用 §19.1，实际算法由 `hash_id` 定）由零依赖自实现填。
用户 2026-11-04 定「**默认 SHA-256，可选 SM3 / BLAKE3**」，故：

| 算法 | 编号 | 状态 | 依据 |
|---|---|---|---|
| SHA-256 | `0x30` | ✅ 已实现（默认） | FIPS 180-4；NIST 向量 + 与 CPython `hashlib` 交叉核对 |
| SM3 | `0x31` | ✅ 已实现（可选） | GB/T 32905-2016；标准向量 + 5 条填充边界与 `hashlib` 核对 |
| BLAKE3 | `0x32` | **只登记，未实现** | 树哈希代码量大且无小体量可自验向量；声明它 ⇒ `unsupported-hash` 明确拒绝，不猜 |

分派在 `rust/smlb/src/hash.rs`（`HashAlgo` / `digest_by_id`）：**写**默认 SHA-256，**读**一律以
manifest 的 `hash_id` 为准。语义三条：

| 规则 | 说明 | 理由 |
|---|---|---|
| **全 0 摘要 = 未校验**（不是「无效」） | 段头摘要全 0 时 `load` / `check` 都不比对，只是校验更少 | 向前兼容 T1 时代写的文件；「没带摘要」与「摘要不符」是两件事，混成一个错误就会让旧文件打不开 |
| **`load` 只验它用到的段** | `VALUE` / `STRTAB` 必验；未知段按 `len` 跳过、不验 | 不变量 1：不认识的段影响不到值，验它只是白花 CPU |
| **`check` 全验且不早退** | 逐段打印「摘要 OK / 不符 / 未带摘要」，最后给总判；结构性错误（头/段表/截断）仍是硬错 | §22 要「打印每段结论」；一遇错就抛会让人只看到第一条 |

⚠️ **已知覆盖缺口（必须写明白）**：摘要只覆盖**段体**，不含段头字段（`comp` / `enc` / `len` /
`flags`）。所以：

- 改 `comp`/`enc` ⇒ `load` 报 `unsupported-algorithm`（有感知，但不是「摘要不符」）；
- 改 `len` ⇒ 段体偏移错位，通常**先**被摘要或长度检查拦下；
- 改 `flags` ⇒ **无人察觉**（v1 段头 `flags` 恒 0，读到未知位置接忽略，见 §19.5 L2）。

T2 已经把每段的 `(id, comp, enc, len, sha256)` 写进 manifest 的 `segments[]`（§19.9），`check` 会
拿它与段表**交叉核对** —— 所以**现在**改段头字段也会被 `check` 报出来（一致性对不上）。
但这一层挡的是**意外损坏**：篡改者可以连 manifest 一起改（没有签名挡他）。
**防伪造**要等 T4 的 `canonical_manifest` 进签名范围。
在那之前：**`check` 能检出「文件被动过」，检不出「谁动的」**，文档与工具文案都不许越界承诺。

**工具**：`smlb check <f>`（结构 + 逐段摘要 + 能否解码，退出码 1 = 未通过）；
`smlb cat <f> <path>`（`a.b[0]` 语法，走 `sml_value::Path`，输出**canonical 文本**）。
`cat` 当前是「整树解出再找路」，`DIR` 段（T3）才把它降到 O(1) —— 先让语义站稳，再上索引。

**「不可信字节」这条线**：解码器吃的是别人的文件，一次 panic 就是一次 DoS。故有一条用例
**逐字节**改（每字节两种翻转）喂 `info` / `load` / `check`，只允许 `Ok` / `Err`，不许 panic。
文件 ~8 KB，全扫一遍约 1 秒，代价可以接受。｜ Any single-byte mutation must return, never panic.

### 19.9 `MANIFEST` 段（✅ 已落定并落地 · 2026-11-04）

> 本节先以草案形式给用户过目，**用户 2026-11-04 裁决后已落代码**（`rust/smlb/src/manifest.rs`）：
> 编码形态 A ✅、字段表 ✅、`caps` 用**名字**（甲）✅、`hash_id` 默认 SHA-256 且 SM3 可选 ✅。
> 仍待裁的只有「BLAKE3 要不要真实现」。

**§19.9.1 编码形态（建议 A，几乎无争议）**

manifest 就是**一个 `Value`（Object）**，用 §19.2 的 `VALUE` 节点编码，装进 `0x04` 段。
理由：D10「manifest 用 SML 自描述」+ P2-b「层记录复用 `VALUE` 编码」+ P9「二进制权威」
三条同向；实现上**零新增编码器**，且能一键导出成 `.sml` 给人看（`MANIFEST_SML` 镜像段，`--with-mirror`）。

**§19.9.2 字段表（v1 最小集；其余键按 D10 的 `loose` 降级）**

```sml
manifest {
  primary: 1                 // P8：主内容所在段 id。**永远有**，可指任意段
  hash_id: 0x30              // §20：摘要算法（SHA-256）；段头那 32 B 按它解释
  segments: [                // §18.3 canonical 形态：**按 id 升序**，不含 offset
    { id: 1  comp: 0x00  enc: 0x00  len: 42  sha256: "…64 hex…" }
    { id: 2  comp: 0x00  enc: 0x00  len: 14  sha256: "…" }
  ]
  payloads: [                // 仅归档类文件有：EMBED 段里的外来载荷，**按段体内顺序**
    { name: "notes/a.md"  len: 8   sha256: "…" }
    { name: "bin/blob.dat" len: 5  sha256: "…" }
  ]
  crypto { enc: 0x00  sig: [ ]  kem: 0x00 }   // §20.2：只允许 AEAD；sig 是数组、全部要过
  caps  { required: [ ]  optional: [ ] }      // D11：见 19.9.3
  patch { strategy: override }                // D22 / §21
  defaults { dir_depth: 4  crypto_block: 4096  pad_quantum: 64 }  // §21 / D37
  created: "2026-11-04T12:00:00Z"             // 只展示、不验签（D17）
  tool: "smlb 0.6.9-dev.0"                    // 只展示（D12 溯源）
}
```

**四条与既有决定的对应**（不是新发明）：

| 字段 | 出处 | 落点 |
|---|---|---|
| `primary` | P8「manifest 永远有 `primary` 指针」 | `load` 从「必须有 `VALUE`」改成「按 `primary` 找主内容」，`.smlb` 才真能兼作通用归档容器（D23） |
| `segments[]` | §18.3 `canonical_manifest = 按段 id 升序 + 每段 (id, algo, len, sha256)` | **补齐段头摘要覆盖不到的那半**（§19.8 的缺口）：算法与长度进签名 |
| `crypto` / `hash_id` | §20 / §20.2 | 编号一律用 §20 的表；`none = 0x00` |
| `caps` | D11 两位集 + P10「同一判定函数」 | 见 19.9.3 |
| `defaults` | §21「默认值写进 manifest」 | 读时以文件为准，改默认值不影响旧文件 |
| `payloads[]` | D23「外来载荷是一等段」+ §22 `pack/unpack` | 归档类文件的文件清单（名字/长度/摘要），偏移靠 `len` 走出来 |
| `created` / `tool` | D17 / D12 | **只展示**，绝不进验证的必要条件 |

**§19.9.2b 归档：为什么是「一个 `EMBED` 段 + 一张 `payloads[]`」**

段是按 `seg_id` 认的（`segments[]` 也按 id 立条目），同种段来两个就说不清谁是谁 ——
§18.3 的「按 id 升序」直接失效。故约定：

- **一个 `.smlb` 至多一个 `EMBED` 段**，段体是各载荷**按 `payloads[]` 顺序拼接**；
- `payloads[]` 只记 (name, len, sha256)，**不记偏移** —— 偏移由 `len` 累加走出来
  （与段表同一套思路：表里没有位置，位置就不可能和内容不一致）；
- `primary` 指向 `EMBED`（P8 允许主内容不是 `VALUE`）⇒ 这类文件 `load` 会明确报
  `PrimaryNotValue`，取数据走 `payloads()` / `payload(name)`；
- 归档默认 `caps.required: [embed]`：读不懂 `embed` 的读者应当明说读不懂，而不是当空文件。

⚠️ **解包必须挡目录穿越**：载荷名来自**文件**，可以写成 `../../etc/passwd`。
`smlb unpack` 对「绝对路径 / 含 `..` / 含盘符」一律拒绝（tar-slip 是最老的解包漏洞之一，
格式不做这件事，工具就必须做）。

**§19.9.3 `caps` 用名字还是位号 —— ✅ 已裁决：甲（名字）**

| 方案 | 长什么样 | 取舍 |
|---|---|---|
| **甲（建议）名字** | `caps { required: [ "value" "dir" ]  optional: [ "provenance" ] }` | 首发前不冻结位号；未知名字按 D11 分两档；将来映射到位号也只是查表。代价：多几十字节字符串 |
| 乙 位号 | `required: 0x0000_0003` | 紧凑；但**位号一经发布永不复用**（§20 的规矩），首发前定死等于把「哪个能力占第几位」一次性焊死 |

用户裁决 **甲**（并注明「实际上乙也是可以很灵活的」）⇒ 名字先走，位号当**可选加速**留到能力集合稳定
（v5 定稿）后再加 `caps_bits` 冗余字段，不必改结构。

落地：`known_capabilities()`（本构建认得的集合，v1 = `value` / `strtab` / `manifest`）+
`check_caps()` / `check_caps_against()`，后者就是 P10 要的那个接缝 —— **裁剪 = 传一个更小的集合**，
判定逻辑一行不改。`load` 遇到读不懂的 `required` ⇒ `MissingCapability` 直接拒，**绝不半读**。

**§19.9.4 落地状态**

| # | 项 | 状态 |
|---|---|---|
| 1 | `Manifest` struct + `to_value` / `from_value` / `to_bytes` / `from_bytes`（复用 `Value` 编码，L0 自包含） | ✅ |
| 2 | `dump` 写 `0x04` 段；`load` 走 `primary`，无 manifest 时退回「VALUE 就是主内容」（兼容 T1 文件） | ✅ |
| 3 | `caps` 判定函数 + 未知 `required` ⇒ `MissingCapability` | ✅ |
| 4 | 验收：manifest 列全内容段（不含自己）· 缺 `required` 明确拒 · 未知 `optional` 跳过 · 改一字节被检出 | ✅ |
| 5 | `pack` / `unpack`（`0x20 EMBED` + `payloads[]` + CLI） | ✅ |
| 6 | `primary` 指向非 Value 段时的裸字节读取路径（`embed_body` / `payloads` / `payload`） | ✅ |

⚠️ **引导顺序**（实现者必须知道）：manifest 的**摘要算法声明在它自己体内**（`hash_id`），
所以只能「先解码 → 拿 `hash_id` → 再用它验 manifest 自己的摘要」。这就是它必须用
**自包含的 L0 形态**（`count ‖ STRTAB ‖ 节点树`）而不是「裸节点 + 外部 STRTAB」的根本原因。
`VALUE` 段则可以「裸节点 + 外部 `STRTAB`」，因为它前面已经有个 manifest 在替它说话了。

### 19.10 随机访问（T3 落地，2026-11-05）

**问题**：`load()` 把整棵树解成 `Value` —— 读 100 MB 文档只为一个字段，就得在堆上造出
几百 MB 的节点。T3 的判据是「**取单个子树不遍历全树**」。

**做法（两段）**

| 层 | 入口 | 做了什么 |
|---|---|---|
| 容器（`rust/smlb/src/store.rs`） | `Store::open(&[u8])` → `container::open_raw` | 只解析头 + 段表 + manifest + 层链；`VALUE` / `STRTAB` 段体**保持 `&[u8]`**，不构造任何 `Value` |
| 值（`rust/sml-value/src/canonical.rs`） | `subtree_at(strtab, value, &Path)` | 在节点树上按路径下行，**兄弟节点用 `subtree_len` 跳过**，只对命中的那棵调 `decode_node` |

导航放在 `sml-value` 而不是 `smlb`：线格式只有一份，两份导航器必然漂移（与 `text_rules`
同一条理由）。`smlb::Store` 只是把两段拼起来，并补上「层补丁覆盖顶层键」的读取口径。

**判据怎么钉的**（`rust/smlb/tests/store.rs`，7 例）

| 判据 | 怎么证 |
|---|---|
| **解码量 == 目标子树**（不是整树） | 把某个**兄弟**的 tag 改成 `0xFF`：整树 `load` 必失败，而 `get` 其它兄弟仍成功 —— 说明坏掉的那棵**根本没被解码**（只是按 `subtree_len` 跳过） |
| 取值与整树一致 | 4 条路径与 `load()` + `get_path` 逐值比对 |
| 零拷贝 | `subtree_bytes(path)` 返回的切片 < 段体长度的 1/10；单独解码仍等于整树里的那个节点 |
| 兄弟扫描的开销是可见的 | `nodes_visited`：取 `arr[499]` ≈ 500，取 `arr[1]` < 10 —— 这个数**只**反映兄弟比较（`DIR` 要省的那部分） |
| 打开默认验摘要 | 改段体 1 字节 ⇒ `Store::open` 报 `digest-mismatch`；`open_unverified` 仍能开（留给 `check`） |

**`DIR` 为什么还没写**：判据要的「不遍历全树」（= 解码量、分配量）现在已经满足；`DIR`
能省的只是**每层 O(兄弟数) 的键比较**。把索引当成「语义的替代品」是危险的，故先让字节导航
站住，索引等真正出现「万级兄弟 + 高频取键」的场景再上。

**⚠️ 一条判据尚未达成**：`Mmap::open`（打开 100 MB 文件 RSS 增量 < 1 MB）需要真正的内存映射；
`Store::open(&[u8])` 本身是零拷贝的（调用方 mmap 后传进来即可满足），但我们没提供
`open_path`：零依赖下拿不到 mmap。补法是加 `mmap` feature + `memmap2`（公告 RUSTSEC-2026-0186
只涉及 `advise_range` / `flush_range` 系列，**修于 0.9.11**，而 `Store` 只用 `Mmap::map` + 切片，
不碰那些函数）。**这条要等一句批准**，在那之前 RSS 判据不算过。

### 19.11 段级压缩（T5 前半，2026-11-05）

**用户裁决**：压缩走 zstd 的 C 绑定链（`zstd` → `zstd-safe` → `zstd-sys`），**不**换 deflate。
台账（实测）：`zstd 0.14.3` / `zstd-safe 8.2.0` / `zstd-sys 2.1.2+zstd.1.5.7`；
`osv_check.py` 扫 85 个包「未发现任何已知漏洞」。`zstd-sys` 2026-10-26 那条（allocator shim
内存破坏）**只落 wasm32**，本 crate 不构建 wasm —— ⚠️ **将来若上 wasm，这条必须重新评审**。

**机制**：用 §19.1 **早就有的** `comp:u8` 字段做**段级**压缩（`0x01 = zstd`），不新造段。

**三条硬规则**（都是踩过才知道的）：

| 规则 | 为什么 |
|---|---|
| **只有真变小才压** | 40 B 的段经 zstd 可能**变大**（帧头 + 熵编码开销），读者还要为它付一次解压 —— 白亏两头。`STRTAB` 是短串集合，尤其容易压不动 |
| **`MANIFEST` 段永不压缩** | 引导顺序：先读 manifest 才知道用什么算法，它自己不能依赖解压 |
| **摘要覆盖「落盘字节」** | 段头摘要算的是**压缩后**的字节。若对解压结果验，则「压缩数据被改」验不出来，还会把篡改误报成「解压失败」（实现时第一版就踩了这个坑） |

**解压预算**：[`MAX_DECOMPRESSED`] = 256 MiB（§24.1 S5 的内存预算）。`len` 是**压缩后**长度，
对「能膨胀到多大」一无所知 —— 100 B 可以声称 100 GB。`zstd::bulk::decompress(data, cap)` 把
`cap` 当**分配上限**，超出即失败，不是先分配再发现。

**与 T3 的冲突（已明确报错，不静默降级）**：压缩段要先解压才能跳节点，而解压等于把「零拷贝」
变成「全部展开」。故 `Store::open` 对压缩文件返回 `zero-copy-needs-plain` 并指路
`load` / `cat`；`smlb check` 会打出「压缩 0x01：36 B → 200002 B」证明解压链路真的通。

**实测**：200 KB 的重复文本 ×2 打成归档 —— 未压缩 200,810 B，`--zstd` 后 **844 B**（238×）；
`unpack` 逐字节还原。

### 19.12 帧级压缩：`FRAMES`（T5 后半之一，2026-11-05）

**段体布局**（`seg_id = 0x10`）：

```text
FRAMES = frame_count:u32le
       ‖ entries[frame_count]（每项 16 B：uncomp_off, comp_off, comp_len, uncomp_len 均 u32le）
       ‖ frames（各帧是**独立** zstd 帧，顺序拼接）
```

语义：`uncomp_off` 是该帧在**逻辑值树**里的起点，`comp_off` 是它在**段体内**的起点。
索引校验五条（都由 `parse_frame_index` 强制）：帧数非 0；`uncomp_off` 从 0 起**连续**
（不重叠不断档）；`comp_off` 紧随索引区且逐帧相接（帧区不许压到索引上）；每帧不越出段体；
帧区合计**恰好**用完段体（多一字节都拒）。

**两个旋钮的分工**（`DumpOptions`）：`frame_size` 管**节点树**（走 `FRAMES`，`primary = 0x10`），
`compress` 管**其余段**（主要是 `STRTAB`）。`FRAMES` 段自己不再套段级压缩（内部已是压缩帧）。

**实测四档**（300 行文档：节点树 7,327 B / `STRTAB` 60,504 B）：

| 形态 | 体积 | 说明 |
|---|---|---|
| 不压 | 68,519 B | 基线 |
| 段级（`compress=5`） | **2,126 B** | `STRTAB` 里重复串压得极狠 |
| 分帧（1 KiB 帧） | 62,522 B | 只压了树（7.3 KB → ~0.9 KB），表原封不动 |
| 分帧 + 段级 | 2,594 B | 比纯段级**多 468 B**：索引 + 切分把 7 KB 的树切碎了，反而更差 |

⚠️ **结论（反直觉但必须记）**：对**文本重**的文档，段级压缩就够，分帧只会多付索引开销。
分帧真正划算的两种情形：① 节点树本身很大（大量数值/布尔/深嵌套、字符串少）；
② 需要「只解被访问的那几帧」。所以 `frame_size` 是**写者按数据形态选**的旋钮，不是默认更好。

**只解一帧的原语**：`frame_bytes_at(body, uncomp_off)`；用例证明「坏掉第 1 帧，第 0 帧与最后
一帧照样解得出来，而全解必然失败」。

**导航器已接上帧（2026-11-05，同轮完成）**：值层把导航抽成 `TreeCursor` + `subtree_range`
（返回 `[start, end)` 而不是字节），于是两条路各自最省：

| 来源 | 游标 | 代价 |
|---|---|---|
| 明文 `VALUE` | `SliceCursor` | 零拷贝切片，一个字节都不复制 |
| `FRAMES` | `FrameCursor`（`smlb` 侧，一帧缓存） | **只解覆盖目标区间的帧**；跳过只做算术 |

`Store` 现在对分帧文件也能 `open` / `get` / `text_of`：用例「把**最后一帧**弄坏 ⇒ `rows[0]`
照常取到、`rows[299]` 报错、整解必失败」把这条性质钉住了。
⚠️ 唯一仍然做不到的是 `Store::subtree_bytes`（返回 `&[u8]`）—— 跨帧没法借用，它明确报
`framed-not-zero-copy`；要拿字节请用 `Store::subtree_vec`（物化）。

**还没做**：加密本体（见 §19.13 的裁决与接口约定）。

### 19.13 密钥来源（用户 2026-11-05 裁决：**丙**）

**结论先说**：支持两条入口 —— **原始密钥**与**口令派生**；而**外部秘密管理（KMS / HSM /
secret manager / 环境变量）不需要任何新格式**，它就是「原始密钥」那条路。

#### 三条入口，归约到同一个 32 B `master`

| 入口 | 文件里写什么 | 谁提供 `master` |
|---|---|---|
| 甲 · 原始密钥 | `kdf: 0x00`（none） | 调用方给 32 B |
| 乙 · 口令 | `kdf: 0x51` + `salt` / `m` / `t` / `p` | 库用 Argon2id 从口令算出来 |
| **丙 · 外部秘密** | **同甲**（`kdf: 0x00`） | **调用方从 KMS/HSM/secret manager 取回 32 B 再交进来** |

#### 为什么「外部秘密」不该进格式（这是关键的一条）

格式能表达的只有「**怎么从口令派生**」，表达不了「**密钥存在哪**」。把密钥位置写进文件
（`kms://…`、`/etc/smlb.key`）会带来两个真问题：

1. **文件被绑死在某个基础设施上** —— 换 KMS 就得改文件，而文件是**已签名/已归档**的东西；
2. **泄露内部拓扑** —— 文件会被到处传（归档、备份、邮件），里面写着内网路径就是泄题。

所以：`master` 的**获取方式永远在文件之外**。文件只声明「这段密文用什么算法、用什么 KDF
参数（若有）」，不声明「钥匙在谁的抽屉里」。这也与 D18「信任根选择交给调用者」同口径。

#### 与既有决定怎么接（不是新发明）

```text
master(32 B)  ──HKDF(master, "smlb:dek")──▶  per-file DEK        （D29）
DEK           ──HKDF(DEK, "smlb:"+algo)──▶  AEAD key            （D29）
nonce = layer_id ‖ block_index ‖ salt                          （D29：永不复用）
```

熵只进一次性标识、伪随机只进可复现路径（D41）—— 因此**布局里不许出现 OS 随机数**，
否则同一份数据两次加密得到不同字节，`check` / 追加全乱。

#### 接口约定（落代码时照这个形状）

```rust
pub enum KeySource<'a> {
    /// 甲 / 外部秘密：KMS 拉回来的就是一个 32 B 密钥
    Bytes(&'a [u8; 32]),
    /// 乙：口令 + 文件里记的 KDF 参数
    Passphrase { secret: &'a [u8], kdf: KdfParams },
}
```

读者入口 `open_with_keys(bytes, &KeySource)`；写者 `dump_with(…, keys)`。
**`KeySource::Bytes` 就是外部秘密的接缝** —— 也允许调用方传一个闭包/回调去取（由上层决定
何时问 KMS、缓存多久、失败怎么退），库只认「最后拿到 32 B」这一件事。

#### §19.13.1 接口草案（示意，**尚未实现** · 2026-11-05）

> 先定「用起来长什么样」，再写实现 —— 用例不顺手就说明接口错了，改接口比改实现便宜。
>
> 用户 2026-11-05 定了三条：**一个 enum**（不拆写/读两个类型）、**命名要一眼就懂**、
> **API 树要扁平**（一个树、每层节点少一些）。

**API 树（根下一层，六个动作 + 四个类型）**

```text
smlb
├─ dump / dump_with / dump_encrypted        写
├─ load / load_encrypted                    读成 Value
├─ get  / get_encrypted                     按路径取一个节点
├─ check / info / ls                        只看结构（不需要钥匙）
├─ pack / unpack                            EMBED 归档
└─ KeySource / KdfParams / DumpOptions / Store
```

扁平的理由：多一层模块就多一个要记的路径。`KdfParams` 挂在根上而不是
`crypto::kdf::Argon2Params` —— 名字已经说明一切，位置不必再说明一次。

#### 密钥：一个 enum

```rust
/// 造钥材料。写时 `kdf` 必须给；读时留 `None`，**文件里写的说了算**。
pub enum KeySource<'a> {
    /// 32 字节原始密钥 —— 也就是**外部秘密**那条路（KMS/HSM/secret manager 取回来就是它）
    Bytes(&'a [u8; 32]),
    /// 口令：写要选参数（`Some`），读要听文件的（`None`）
    Passphrase {
        secret: &'a [u8],
        kdf: Option<KdfParams>,
    },
}
```

两个方向各错一次就会报错，**不会静默走偏**：

| 情形 | 结果 |
|---|---|
| 写时 `kdf: None` | `Err(kdf-required)` —— 写者必须选参数 |
| 读时 `kdf: Some(p)` 且与文件不符 | `Err(kdf-mismatch)` —— 不假装没看见 |
| 读时 `kdf: None` | 正常：参数取自 manifest |

**写：原始密钥（= 外部秘密）**

```rust
use smlb::{dump_encrypted, DumpOptions, KeySource};

let master: [u8; 32] = load_master_from_kms("prod/smlb/key")?;   // 库不管钥匙从哪来

let bytes = dump_encrypted(
    &value,
    &DumpOptions { block_align: true, ..Default::default() },   // 加密强制 4 KiB
    &KeySource::Bytes(&master),
)?;                                                            // enc=0x10，manifest 记 kdf: 0x00
```

**写：口令**

```rust
let bytes = dump_encrypted(
    &value,
    &DumpOptions { block_align: true, ..Default::default() },
    &KeySource::Passphrase {
        secret: b"correct horse battery staple",
        kdf: Some(KdfParams::argon2id_default()),   // 写者选；salt 由库现算并写进 manifest
    },
)?;
```

**读：对钥 / 错钥**

```rust
let v = load_encrypted(&bytes, &KeySource::Bytes(&master))?;   // Ok
let e = load_encrypted(&bytes, &KeySource::Bytes(&wrong_key))
        .expect_err("错钥必须失败");
assert_eq!(e.kind(), "auth-failed");     // 明确是「认证失败」，不是「文件坏了」
```

**读：口令（参数听文件的）**

```rust
let v = load_encrypted(&bytes, &KeySource::Passphrase { secret: pass, kdf: None })?;
```

**取一个字段：只解覆盖目标的块**

```rust
let e = Store::open(&bytes).expect_err("加密文件不能零拷贝");
assert_eq!(e.kind(), "encrypted-not-zero-copy");

let port = get_encrypted(&bytes, "config.port", &KeySource::Bytes(&master))?;
```

**`check` 不需要钥匙**（结构、摘要、块数、算法都能读）

```text
$ smlb check secret.smlb
算法      SHA-256（hash_id 0x30）
0x01 VALUE      摘要 OK (…)      enc=0x10 AES-256-GCM  12 块
0x04 MANIFEST   摘要 OK (…)
manifest  primary = 0x01 · kdf = 0x00（原始密钥）
crypto    加密：AES-256-GCM（本构建可解，需提供密钥）
结论      通过（3 段，… 字节，对齐 4096 B）
```

**错误面（新增，各有 `kind()`）**

| `kind()` | 何时 |
|---|---|
| `encrypted-not-zero-copy` | 对加密文件调用零拷贝 `Store` 读取 |
| `auth-failed` | 块认证失败：钥匙错、密文被改、**段头被改**（AAD 绑了段头） |
| `key-required` | 文件加密但没给 `KeySource` |
| `kdf-required` | 写口令时没给 `KdfParams` |
| `kdf-mismatch` | 读口令时给的参数与文件里的不符 |
| `kdf-unsupported` | `kdf` 编号本构建不实现（如将来的 scrypt） |
| `crypto-feature-disabled` | 本构建没开 `crypto` feature |

#### crate 台账（2026-11-05 实测 RustSec）

| crate | 用途 | 公告 | 处置 |
|---|---|---|---|
| `aes-gcm` | §20 `0x10` | RUSTSEC-2023-0096（MEDIUM：`decrypt_in_place_detached` 在 tag 失败时仍暴露明文），**修于 0.10.3** | ✅ 用 0.10.3+，**只走高层 `decrypt`**，不碰 `_detached` |
| `chacha20poly1305` | §20 `0x11` | 无 | ✅ 备选 |
| `argon2` | `0x51` 口令派生 | 无 | ✅ |
| `hkdf` | D29 派生 | 无 | ✅ |

#### 还没做的（下一步）

1. `crypto` feature + 上面四个 crate（默认构建仍零第三方依赖）；
2. 段头 `enc` 写入 + **按 4 KiB 块 AEAD**（每块一个 tag）+ `flags` 位 0 强制为 1（§19.7 丙）；
3. 「压缩→加密」顺序固定（先压后加，D28），并在 `check` 里报出块数/算法；
4. 追加语义：加密文件的**新增段必须落在新块边界**（否则重写旧块 ⇒ 破坏不变量 3）。

---

## 20. 算法 ID 表（v1 · P3 落定）

> 原则：**编号一经发布，永不复用**；未知 ID **明确拒绝**（算法属 `required` 类能力，跳过等于拿密文当明文）。
>
> **压缩实现选型（2026-11-05）**：用户裁决「用 zstd 的 C 绑定链（`zstd` → `zstd-safe` → `zstd-sys`）」，
> 即接受 `zstd-sys` 那条公告只落在 wasm32；deflate 留作备选。背景：`0x01 zstd` 的三个实现近期全被点名
> （`zstd` 2026-10-26 三条 / `zstd-sys` 同日 wasm 内存破坏 / `ruzstd` 2026-10-13 **HIGH 解压炸弹**），
> `0x02 lz4` 的纯 Rust 实现 `lz4_flex` 也有 2026-03-17 **HIGH**（畸形数据泄露未初始化内存）。
> 故 **T5 优先做 `0x03 deflate`（`flate2` + `miniz_oxide`，零公告）**，zstd 等生态冷却后再补；
> 解压侧一律加「输出上限」挡炸弹（§24.1 S5 的内存预算）。

| 区间 | 类别 | 已分配 |
|---|---|---|
| `0x00` | 无 | `0x00 = none`（明文、未压缩） |
| **`0x01–0x0F`** | 压缩 | `0x01 zstd` · `0x02 lz4`（预留）· `0x03 deflate`（预留） |
| **`0x10–0x1F`** | 对称加密（经典） | `0x10 AES-256-GCM` · `0x11 ChaCha20-Poly1305` |
| **`0x20–0x2F`** | 对称加密（国密） | `0x20 SM4-GCM` |
| **`0x50–0x5F`** | **口令派生（KDF）** | `0x00 none`（原始密钥）· `0x51 Argon2id` |
| **`0x30–0x3F`** | 哈希 | `0x30 SHA-256` · `0x31 SM3` · `0x32 BLAKE3` |
| **`0x40–0x5F`** | 签名（经典） | `0x40 Ed25519` · `0x41 ECDSA-P256-SHA256` · `0x50 SM2-with-SM3` |
| **`0x60–0x7F`** | 签名（PQC） | `0x60 ML-DSA-65` · `0x61 ML-DSA-87` |
| **`0x80–0x9F`** | KEM | `0x80 X25519` · `0x81 ML-KEM-768` · `0x82 ML-KEM-1024` |
| **`0xA0–0xAF`** | 混合 | `0xA0 X25519+ML-KEM-768` · `0xA1 SM2+ML-KEM-768` |

### 20.1 四条硬规矩

1. **只允许 AEAD**（D35）：`enc` 里不出现需要另配 MAC 的模式（CBC / CTR / 任何「加密+独立 MAC」）；
2. **哈希统一 32 B**：`SHA-256` / `SM3` / `BLAKE3` 摘要都是 32 B ⇒ 段头 `sha256:32` 不变长，选哪个由 manifest 的 `hash_id` 决定；
3. **混合必须显式编号**：不允许「两个算法字段隐式拼起来」，否则验签时说不清「哪些字节是谁签的」；
4. **多签名 = 数组，且全部必须通过**（D36）：若允许「任一通过」，抗量子那一路失效也无人发现。

### 20.2 配置形态（manifest 内）

```sml
crypto {
  enc: 0x10              // AES-256-GCM
  hash: 0x30             // SHA-256
  sig: [ 0x40 0x60 ]     // Ed25519 + ML-DSA-65（双签，两个都要过）
  kem: 0xA0              // X25519 + ML-KEM-768（首个成功即用）
}
```

- `sig` 是**数组**（可多个，**全部验过**才算通过）；
- `kem` 是**单个**（首个成功即用）；
- 量子迁移只是**换编号**，不动结构 ⇒ 这是「算法敏捷」的全部内容。

---

## 21. 默认值与开关（D37）

> 元规则两条：**① 默认值写进 manifest**，读时以文件为准（改默认值不影响旧文件）；
> **② 「启用才有代价」的默认关**（加密/签名/压缩），**「关掉只影响性能」的默认开**（`DIR`/时间戳）。

| 项 | 默认 | 开关 |
|---|---|---|
| 层记录编码 | **二进制**（复用 `VALUE` 编码） | `--with-sml-mirror` 附 SML 文本镜像 |
| `MANIFEST_SML` 镜像 | **不带** | `--with-mirror` |
| `SOURCE` 段（P7） | **不带** | `--keep-source` |
| `DIR` 段 | **带**（深度 ≤ 4） | `--no-dir` / `dir_depth=N` |
| 时间戳 `created` | **写**，只展示、**不验签** | `--no-timestamps` |
| 压缩 | 传输档 **zstd** / 冷数据档 **不压** | `--no-compress` / `--compress` |
| 加密 | **关** | `--encrypt` + 算法 |
| 签名 | **关** | `--sign` |
| 哈希 | **SHA-256** | `--hash sm3` / `blake3` |
| 加密块 / 量化 | **4 KiB / 64 B** | manifest 字段 |
| 补丁策略 | manifest 内显式（默认 `override`） | `patch.strategy` |
| `edition` / `fmt` | 当前 edition / `0` | 写入时决定 |

### 21.1 层链三条（P2 落定）

| # | 结论 |
|---|---|
| **P2-a** | `prev_hash = H(前一层记录的 canonical 字节)` —— 不覆盖整个文件前缀，追加保持 **O(1)** |
| **P2-b** | 层记录**复用 `VALUE` 编码**；签名对象 = `canonical_text(layer)` |
| **P2-c** | `delegation` 放在**层记录里**（而不是独立段）⇒ 天然被**前一层签名**覆盖，正是 D16 要的 |

验签判定链（六步）：`Trailer.head_hash` → 逐层 `prev_hash` → `index` 连续 → 各签名全过（D36）→
换钥须有前一层覆盖的 `delegation` 且 `j ≤ max_index` → 各段 `sha256` 一致。**时间只进告警**（D17）。

### 21.1b 层链的落地形态（T4，2026-11-05）

**文件尾**（追加区，写完不再动）：

```text
… 段数据 | L0 | T0 | L1 | T1 …
                       ↑ 读者只认 EOF 往前 64 B 的 Trailer
L_n = len:u32 ‖ prev_off:u64 ‖ reserved:u32 ‖ body（Value 的 L0 字节）
T   = magic "SMLTRLR1"(8) ‖ layer_count u32 ‖ last_layer_off u64 ‖ head_hash(32) ‖ reserved
```

**为什么是反向链**（每层记 `prev_off`，Trailer 只指最后一层）：追加只能往 EOF 写；正向链要改
上一层的 `next` 指针 —— 那就**改了旧字节**。反向链让不变量 3 在**字节层面**也成立，
不只是「旧签名仍然有效」。旧 Trailer 变成中间字节，读时只看最后一个。

**层体字段**（就是 `VALUE` 编码的对象，P2-b）：

```sml
layer {
  index: 3
  prev_hash: "…64 hex…"      // 首层全 0；其余 = H(上一层的 canonical 字节)
  author: "…64 hex…"         // Ed25519 公钥（§20 的 0x40）
  patch: { … }               // 可选：本层要合并进当前值的对象
  delegation { from sig max_index }   // 可选：换钥授权（D16）
  sig: [ { algo: 64  bytes: "…hex…" } ]   // 数组，全部必须通过（D36）
}
```

**四处实现澄清**（写死在这里，别再让实现者猜）：

| # | 澄清 | 理由 |
|---|---|---|
| **A** | **签名对象 = 去掉 `sig` 字段后的 `canonical_text(layer)`** | P2-b 写「签 `canonical_text(layer)`」，可 `sig` 就在 layer 里，直接签会循环 —— 与 §18.4「排除 `SIG` 段自身」同一个道理 |
| **B** | **偏移不进签名**：`prev_off` / Trailer 都在可签对象之外 | §18.3「位置不是语义」的同一口径；好处是「同一份数据被追加在不同位置」签出来一样 |
| **C** | **`patch` 的合并规则** = 键已存在则原位覆盖、不存在则追加到尾部、**数组整体替换**（D22）；`load` 返回的是**当前值**（base + 各层补丁），历史值走 `layer::value_at(bytes, base, n)`（`n` 是**层号**，0 起） | 「值来自哪一层」（P2-a）必须有读取面；而 `dump→load→dump` 在**有层**时本来就不该字节相同（D8：`compact` 是唯一例外且必须重签） |
| **D** | `delegation` 的签名对象是**独立文本** `smlb/delegate/v1\nfrom:…\nto:…\nmax_index:…\n` | 授权是「从 A 到 B 直到第 N 层」这件事本身，与它被放进哪一层无关 |

⚠️ **一个必须说清的边界**：**整条层链可被剥离**（把 EOF 的 Trailer 连同层记录一起删掉，
文件就退回「未签名」）。没有外部锚点（记住旧 `head_hash` / 对方渠道核对）就无法察觉 ——
这正是 D18「信任根选择交给调用者」的含义。**不要把「验签通过」读成「这份文件没被换过」。**

**第三方 crate 台账**（用户 2026-11-04 批准引入，要求避开 RustSec 热点；2026-11-05 实测）：

| crate | 用途 | RustSec 历史 | 处置 |
|---|---|---|---|
| `ed25519-dalek` 3.0 | §20 `0x40` 签名 | 1 条：RUSTSEC-2022-0093（双公钥签名预言机），1.0.1 起修复 | ✅ 用，`sign` feature 下 |
| `curve25519-dalek`（间接） | 底座 | 1 条：RUSTSEC-2024-0344（标量减法时序），**>=4.1.3** 修复 | ✅ 随 3.0 带 5.x，已越过 |
| `getrandom` | keygen 取熵（D41） | 无公告 | ✅ |
| **`ring`** | 备选 | **RUSTSEC-2025-0010 = UNMAINTAINED** | ❌ 排除 |
| **`ruzstd`** | 备选压缩 | 2026-10-13 **HIGH 解压炸弹** + 2024-11-18 越界 | ❌ 排除 |
| **`zstd`（C 绑定）** | 备选压缩 | 2026-10-26 三条（<0.8.0；修 >=0.8.0 + zstd-safe>=8.2） | ⏸ **T5 暂缓**，等冷却 |

仓库既有门禁 `rust/osv_check.py` 扫的是 `Cargo.lock`（含可选依赖），实测 75 个包
「未发现任何已知漏洞」—— 选型不是凭名片，是被门禁验过的。

---

## 22. 配套工具（D38 · 用户 2026-11-04 点名「还得有配套工具」）

> 原则：**格式的价值有一半在工具**。没有 `info` / `verify` / `cat`，`.smlb` 就是个打不开的黑盒。

| 工具 / 子命令 | 用途 | 阶段 | v1 必须 |
|---|---|---|---|
| `smlb info <f>` | 一眼看清：edition/fmt/段表/能力位/是否签名、是否加密 | T2 | ✅ **已实现**（T1 提前；能力位/签名状态待 T2/T4 补） |
| `smlb ls <f> [--segments]` | 列段与外来载荷（id/长度/算法） | T2 | ✅ **已实现**（并列出 `payloads[]` 的名字与长度） |
| `smlb cat <f> <path>` | 按路径取值（`a.b[0]`），走 `DIR` | T3 | ✅ **已实现**（带 `--at N` 历史回放） |
| `smlb get <f> <path> [--bytes]` | 零拷贝取一棵子树（不解全树），`--bytes` 出原始字节 | T3 | ✅ **已实现**（不依赖 `DIR`：用 `subtree_len` 跳兄弟） |
| `smlb text <f> [--path P]` | 整份或某一子树的 canonical 文本 | T3 | ✅ **已实现** |
| `smlb check <f>` | 结构校验：段哈希、层链、能力位、截断位置 | T2 | ✅ **已实现**（段哈希/截断；层链待 T4、能力位待 §19.9） |
| `smlb verify <f> [--trust k.pub]` | 验链 + 验签，打印每层结论 | T4 | ✅ **已实现**（`--trust` 外部锚点未做） |
| `smlb sign / append / delegate` | 签名、追加补丁、换钥授权 | T4 | ✅ **已实现**（`keygen` 也在，`sign` 默认原地写） |
| `smlb pack / unpack` | 把多文件打成 `.smlb`（`EMBED` + `payloads[]`），或反向取出 | T2 | ✅ **已实现**（`unpack` 拒绝目录穿越；`pack --zstd` 段级压缩） |
| `smlb tree <f>` | 打印层链（index/作者/补丁/签名数/授权） | T4 | ✅ **已实现** |
| `smlb vectors --gen/--check` | 生成/校验跨端 canonical 测试向量（T0 判据） | T0 | ✅ **已实现**（2026-11-04，另带「自检」：往返 + 文本一致） |
| `smlb bench <f>` | 读路径基准（mmap 打开、单子树取、解帧、解密） | T5 | 建议 |
| `smlb migrate --to fmt=N` | `fmt` 升级（重写 + 重签，需保留旧签名证据） | T5+ | 建议 |
| `smlb compact` | 合并多层（会**重签**，D8 的例外情况） | T5+ | 建议 |
| `smltools --to smlb` / `--from smlb` | 与 SML 文本互转（D27 的 A 路） | T6 | ✅ |
| `smltools --to smlb --keep-source` | 带 `SOURCE` 段 | T6 | 建议 |
| JS / Lua 解码器 | 只读 `VALUE` + 透传载荷（D12 五端可读） | T7 | ✅ |
| VSCode 扩展（只读） | 直接打开 `.smlb` 浏览树 / 悬停看段信息 | T7+ | 建议 |

**顺序**：`info`/`ls`/`check` 跟着 T2 长出来；`cat`/`verify`/`sign`/`append` 跟 T3/T4；`vectors` 是 T0 的**前置**（没有向量就不知道 canonical 对不对）。

---

## 23. 设计终态（2026-11-04 · 收尾）

### 23.1 三条不变量（任何改动都不得破坏）

1. **未知段可跳过** —— 长度自描述；不允许「读前先跑解释器」（D24/D25 的根）。
2. **签名签 `canonical_text`** —— 与 `fmt` 解耦，历史签名不因字节布局升级而失效（P5-a）。
3. **追加不改旧字节** —— 旧签名永久有效；`compact` 是唯一例外，且必须重签（D8/D9）。

### 23.2 决策索引（D1–D38）

| 组 | 内容 |
|---|---|
| **定位** D1–D4 | 与 v5 同开 · **编 `Value`** · 独立 crate（不依赖 parser）· 冷热两档不混 |
| **版本与能力** D5, D11, D34 | `edition` + `fmt` 补丁位 · `required`/`optional` 两位集 · 段头双字段 `comp`/`enc` |
| **数据** D6, D32, D33 | **保序** · 前序 + 固定 4 B `subtree_len` · `DIR` 段（深度 ≤4、带 `head_layer`） |
| **签名** D7, D16, D31, D35, D36 | 默认覆盖全部（除 SIG 自身）· 转授权链 · 签明文再加密 · 只允许 AEAD · 多签名全过 |
| **时间** D17 | 时间不可信：只进告警，不进必要条件 |
| **并发与追加** D19, D20, D21, D22 | 线性链（鱼骨）· **多读一写** · 多写交 `smldb` · 数组补丁整体替换 |
| **容器** D23, D24, D25, D26, D37 | 可装外来载荷 · profile 只定义语义 · profile 不作读前置 · API 5 组 20 个 · 默认值表 |
| **加密细节** D28, D29, D30 | 4 KiB 块 + 64 B 量化 · DEK/HKDF + 全局唯一 nonce · 先压后加（可 `--no-compress`） |
| **其它** D10, D12, D13, D14, D18, D27, D38 | manifest 二进制权威 + SML 镜像 · 补六项常用 · FTS 折中 · 四项不做 · 基座定位 · 导出 A+C · 工具面 |

### 23.3 仍未定（**不阻塞开工**）

| # | 内容 | 谁需要 |
|---|---|---|
| `smldb` 侧 | 选择器语法（§7 挪过去后重新定义）、FTS 具体方案、表结构细节 | `smldb` 开工时 |
| `compact` | 合并多层后如何保留「旧签名证据」 | T5+ |
| 五端 | JS / Lua 解码器的错误码映射 | T7 |
| canonical 三条 | 顶层标量能否不可回读（P5-d-a）· 浮点跨端口径（C5-b）· 元数据在字节侧是否前置（L5） | ①③ 可随时定；② 按用户 2026-11-04 的裁决**与四端统一同步时定**（详见 §18.6） |

### 23.4 开工顺序（详见 §17）

```
T0 canonical + vectors  →  T1 编码  →  T2 容器  →  T3 读 / T4 层链 / T5 加密（可并行）
                                                  →  T6 工具  →  T7 五端
```

---

## 24. 侧信道与冗余（防护清单 · 用户 2026-11-04 点名「反侧信道、大小冗余」）

> 威胁模型先说清：**我们防「观察者」与「离线攻击者」**（共享存储、备份、CDN、被审计的磁盘），
> **不防**「已在同机拿到 root 的攻击者」（那要进程级隔离，不是文件格式能解决的）。

### 24.1 侧信道：八条

| # | 信道 | 现状 / 对策 | 状态 |
|---|---|---|---|
| **S1** | **长度**（值多长、文档多大） | 加密块内**长度量化到 64 B**（D28）；敏感场景可配更粗档；**文件总量不量化**（代价太大） | 已定 |
| **S2** | **压缩 + 长度**（CRIME/BREACH 类） | `--no-compress` 断开；或**每块独立压缩**（不跨块，体积变大） | 已定（D30） |
| **S3** | **访问模式**（读了哪个块） | 4 KiB 块粒度可被观察 ⇒ **记录为已知泄露**；可选的"致盲读"（一次解 k 块）默认关 | ⚠️ 记录在案 |
| **S4** | **写入模式**（追加层暴露"改了哪些键、多久改一次、改多大"） | 建议**批量提交 + 固定大小的层**；不强制 | ⚠️ 建议 |
| **S5** | **校验/解密错误的 oracle**（padding oracle 类） | 解密失败**统一报错**，不区分"密钥错 / 密文被改"；细节只在 `--explain` 下给出 | **必须** |
| **S6** | **签名验证短路**（第几个签名错会泄露） | 多签名**全部验完再汇总**（与 D36「全部必须通过」一致），不做 early-return | **必须** |
| **S7** | **时间侧信道**（tag/签名的比较） | 一律**常数时间比较**（AEAD 库自带；多签名汇总也用常数时间写法） | **必须** |
| **S8** | **内存残留**（解密后的明文、mmap 页被换出） | 可选 feature `zeroize`（用完即清）；可选 `mlock` / `MADV_DONTDUMP`（默认关，小内存机器会被打死） | 可选 |

### 24.2 冗余：分清「检测」与「恢复」

| 层 | 手段 | 结论 |
|---|---|---|
| **检测**（篡改/损坏能不能发现） | 每段 `sha256` + 每加密块 AEAD tag + 层链 `prev_hash` | **v1 就做**（D12） |
| **恢复**（坏了能不能修） | 纠删码 / 多副本（`PARITY` 段） | **v1 不做**，留 optional 段位；理由：文档库的备份由上层负责，格式内做纠删码会把复杂度翻倍 |
| **截断容忍** | 读到坏段即停，前面仍可用（D12） | **v1 就做** |

### 24.3 一条必须写死的约束：**填充必须确定性**

随机填充（每块随机长度）会让**同一份数据两次 dump 产出不同字节**，直接破坏：

- canonical 幂等（C12）、`dump→load→dump` 判据；
- 测试向量（T0 的判据会随机失败）；
- 字节级去重与 diff。

⇒ **只允许「按长度量化」的确定性填充**，禁止随机填充。若将来要随机化，必须做成**显式选项**并接受"该文件的字节不可重现"。

### 24.4 巧用伪随机数（D41 · 用户 2026-11-04 点名）

> **核心原则：两种随机必须分开** —— 熵只进「一次性标识」，伪随机只进「可复现路径」。

| 类型 | 用途 | 能否影响字节布局 |
|---|---|---|
| **一次性熵**（OS RNG） | `file_id` / DEK / salt，**只做一次** | ❌ 绝不 |
| **可复现伪随机**（keyed PRNG） | 填充内容 / 长度抖动 / 诱饵块 / fuzz 种子 | ✅ 但必须**可复现** |

**种子构成**（同一份数据 + 同一 key ⇒ 永远同一串字节）：

```
purpose_key = HKDF(master, "smlb:" + purpose)
stream      = PRNG(purpose_key, info = file_id ‖ layer_id ‖ block_index ‖ purpose)
```

**五个用法**

| # | 用法 | 解决什么 |
|---|---|---|
| ① | **确定性填充内容**（填充字节取 PRNG 流，而非全 0） | 全 0 填充在密文里可识别、压缩率也异常 |
| ② | **长度抖动**（量化后加 `[0, jitter)`，jitter 由 key 定） | 防「64 B 档内仍被猜中精确长度」 |
| ③ | **诱饵块 / 致盲读**（读第 k 块时顺带解 PRNG 选的 m 个无关块，默认关） | 混淆 S3 的访问模式 |
| ④ | **可复现 fuzz 与测试向量**（固定 seed） | 测试不再是「一次性随机」 |
| ⑤ | **溯源指纹**（可选，必须放**独立段**，不得混进 `VALUE`） | 泄露追踪 |

**三条禁忌**

1. **OS RNG 不得进入「每次 dump 都会走到」的路径** —— 否则幂等（C12）直接失效；
2. **不得用弱 PRNG 顶替 nonce 的唯一性** —— nonce 仍按 D29 由 `(layer_id, block_index, salt)` 派生；
3. **同一 `purpose` 不得跨用途复用** —— 否则可从填充推出抖动规律。

**实现**：`ChaCha20` keystream 或 `HKDF-Expand` 即可，**不需要 OS 熵源、不需要 `rand` 依赖**（`crypto` feature 下才有；明文档完全不涉及）。

**未采纳的两招**（记录理由，避免以后重提）：

| 招法 | 为什么不 |
|---|---|
| **块重排 / 段序混淆** | 破坏「段按 id 排序」与随机访问的确定性；且 `VALUE` 前序遍历本身依赖顺序 |
| **随机化层大小** | 与「追加 O(1) + 线性链」冲突；层大小固定是 S4 的建议方向 |

### 24.5 与既有决定的交叉检查

| 冲突点 | 结论 |
|---|---|
| 随机填充 vs 幂等 | 幂等优先（§24.3） |
| 致盲读 vs 随机访问性能 | 默认关，可选开 |
| 统一错误 vs 排障体验 | 默认统一，`--explain` 显式打开 |
| 层大小固定 vs 追加灵活 | 建议而非强制（S4） |