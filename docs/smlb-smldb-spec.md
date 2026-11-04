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

## 7. 选择器语法 v1【草案，待确认部分项】

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

| # | 事项 | 选项 | 建议 |
|---|---|---|---|
| 1 | **是否需要全文检索（FTS）** | 要 / 不要 | 要 → 锁死 SQLite（Turso 给不了 FTS5）。文档库不配搜索，用户会去 grep BLOB，体验崩坏 |
| 2 | **`:has()` 进 v1 吗** | 进 / 暂缓 | 暂缓：唯一可能退化到 O(n·depth)，v1 无明确需求，将来可无痛加 |
| 3 | **`.content` 是否映射 `[class~=content]`** | 是 / 否 | 是，CSS 语义，空格分隔多类名 |
| 4 | **`id` 是否强制唯一** | 校验报错 / 当普通属性 | 标记语言里用户可能重复用，建议当普通属性不校验 |
| 5 | **规模信封**（单文档典型/最大、文档数量级、写入频率、持久性要求） | 未知则按 §5.9 自适应 | 未知就用 Policy 默认值（8 MB），事后一行配置可调 |

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