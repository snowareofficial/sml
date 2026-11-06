# `smldb` 数据库层（草案 · 未开工）

> **产品线** amberice ｜ **状态** draft ｜ **最后核对** 2026-11-06
>
> ⚠️ **本文全部内容尚未开工**：`smldb` crate 不存在，选择器语法未定，FTS 方案未落地。
> 已想清楚的部分固定下来供开工直接施工；未定的标「待拍板」并指向 `../decisions.md`。
> `.ambr` 格式本体见 `format.md`；元数据在 `docs/meta.sml`。
>
> **章节编号是 `amberice` 产品线的全局编号**，不是本文件内的连续编号：第 0–4, 8–9, 11, 18–21, 23–25 节在 `format.md`；第 10, 12, 15–16 节在 `../decisions.md`；第 22 节在 `tools.md`。
> 这样跨文件的交叉引用（`§19.9`、`§21.1b` 之类）永远指同一件事。

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
    body     BLOB    NOT NULL    -- .ambr 原始字节, 永不拆行
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
- `content` 列由 `sml` 遍历树提取纯文本（FTS5 无法索引二进制 `.ambr`），写入时算一次。

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
> 但 D2 已定 `.ambr` **编 `Value`**（对象/数组/标量 + `__type`），节点模型不再成立。
> 因此：**`.ambr` 文件本身只提供路径访问**（`get("a.b[0]")` + `DIR` 段），
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

## 13. 数据库地位与产品取向（sml × DB）【草案 · 待拍板 · 未定 · ⚠️需二次蒸馏】

> 愿景方向来自产品讨论，**尚未拍板、暂不实现**，记下供决策。
> ⚠️ **二次蒸馏**：以下是愿景层浮想，需在拍板前精炼成「产品明确到什么程度 / 技术落地路径」的定稿。

### 13.1 产品取向

- **默认不暴露数据库特点**：sml 当配置 / 文档用时，新手与绝大多数场景**无需知道底层是 DB**。
  入口极简，把「内存解析」与「持久化」统一进同一套 sml 心智，底层 DB 只是可选加速/落盘层。
- **半独立于门面（2026-11-06 用户确认）**：`smldb` 只依赖 `amberice`（→ `sml-value`），
  **不依赖 `swsml` / `sml-parse`** —— 与 D3 同向：DB 层不该把解析器和六个发射后端拖进依赖图。
  要读 `.sml` 文本时由调用方自己带 `swsml`；只读 `.ambr` 的场景完全不碰语言层。
- **UX 铁律（贯穿始终）：操作好上手，同时功能强大。** 常用路径像配置文件一样直觉（面向新手），
  进阶路径不设天花板（面向老手）；默认行为简单，能力用显式入口暴露，不牺牲表达力换省事。
- **提供 sml-style（或 amberice）的 NoSQL**：`.swl`（sml 文本）文档当 NoSQL 文档用 —— 键值 / 文档语义，
  无 join、无 SQL；查询走 §7 选择器而不是 SQL。`.ambr`(二进制衍生) 可作**传输载荷的候选二进制格式之一**（还有文本 `.swl` 等）。
- **sml 本身可作 DB 的“地图”**：schema / 索引 / 关系 / 约束的描述**本身也是一份 `.sml` 文档**
  （元数据即数据），DB 的视图由它生成 —— 避免「身份在 DB 里、数据在 DB 里、却要用第二套语言描述」。
- **目标**：在提供 DB 能力（持久化、FTS 检索、事务、索引）的同时，**性能更好、安全性更好、心智负担更低**。

### 13.2 落地含义（待拍板）

- 与 §5 / §6 **已定项不冲突**：底层仍是 SQLite BLOB + 提取列 + FTS5；只是把 DB 包装层藏到
  sml 心智之后。默认后端 = §6.5 `DefaultBackend`，Turso 等仅命中 §6.6 触发条件才切。
- 要补的是一个「不暴露 DB 细节」的薄层：**路径即键、文档即值、`.swl` 文件=文档、搜索用选择器而非 SQL**。
- 「sml 作为 DB 地图」的具体形态（如何描述 schema / 扩展点）属后续设计，此处暂不展开。
- 与前面后缀的关系：`.swl`=sml 文本（地图/文档），`.ambr`=sml 二进制衍生（传输/BLOB），`.sml`=旧后缀兼读取别名；
  三者分工已确认 `.swl`(文本)/`.ambr`(二进制)，`.sml` 的「默认写出」位在 v5 让给 `.swl`（读端长期兼容）。


---

## 14. DB 的 API 友好度：四层下潜 / 用多少学多少【想法 · 未定 · ⚠️需二次蒸馏】

> 核心一句话：**「让用户永远看不到数据库」**。
> ⚠️ 本节是 API 形态的浮想，**未拍板、未实现**，需二次蒸馏成「API 签名 + 命名取舍 + 覆盖矩阵」的定稿。
> 与 §13 数据库产品取向同主题，是它的「对外 API 表达层」延伸；§13 只有愿景方向，这里给出具体交互形态。

### 14.1 四层下潜（原话 AI 留给后人）

> 「核心是让用户永远看不到数据库。四层下潜，用多少学多少。」

```rust
// L0 一行流 —— 完全不知道有数据库这回事
let doc = sml::open("notes/hello.swl")?;

// L1 文件夹隐喻 —— Store 当 HashMap<path, Doc> 用
let mut store = Store::open("notes")?;      // 就是个目录
store.insert("2026/hello.swl", &doc)?;      // 路径即 key
let doc = store.get("2026/hello.swl")?;
for e in store.list("2026/")? { /* 前缀列出 */ }

// L2 声明式查询 —— 依然没有 SQL
store.find().tag("post").created_after(ts).limit(20).load()?

// L3 逃逸口 —— 只有需要时才出现，且名字要劝退
store.raw(|conn| conn.prepare("..."))?      // 文档标注「需懂 SQL」
```

### 14.2 下潜思路点评（AI 技术意见，非拍板）

「下潜 + 用多少学多少」确实是好思路，和 §13 UX 铁律（上手易 + 功能强）同构，一脉相承。**核心洞察是：把「学习成本」从「横切（一开始就得全懂）」改成「纵切（按需逐步暴露）」**：

- **L0（一行流）**：对新手与绝大多数用途，`open` 一个 `.swl` 当文档读 —— 目标用户**永远碰不到 DB 概念**，无学习成本。
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

## 17. 施工顺序（草稿 · 未开工 · 与 v5 同步开放）

> 开放节奏见 D1：**设计与实现可以现在做，对外开放等 v5**。阶段 1–3 与 v5 定稿并行不冲突。

| 阶段 | 做什么 | 验收判据 |
|---|---|---|
| **T0 前置** ✅ 已落地（2026-11-04，见 `format.md §18.6`） | `Value::canonical_bytes()` 的规则定稿（保序、数字格式、`__*` 位置、转义）+ **跨端测试向量**（文本 ↔ 二进制各 N 组，期望字节写死） | 无 `preserve_order` 时**显式报错**；向量在 Rust/JS 上产出同一字节（**JS 侧待 T7**，见 `format.md §18.6` 待确认 ②） |
| **T1 编码（L0）** ✅ 已落地（2026-11-04，见 `format.md §19.6`） | 新 crate `amberice` 骨架 + 文件头 + 段表 + `dump` / `load`（`info` / `ls` 提前到 T1） | `dump→load→dump` 字节相同；`Int(1)` 与 `Float(1.0)` 可区分；`__type`/`__featags` 原样往返；未知段可跳过（用额外段实测） |
| **T2 容器（L1）** ✅ **全落**（2026-11-04/05，见 `format.md §19.8` / `format.md §19.9`） | 段表 ✅ · 摘要（SHA-256 / SM3，按 `hash_id`）✅ · manifest + `primary` + 能力位 ✅ · 外来载荷 `EMBED` + `payloads[]` + `pack`/`unpack` ✅ · `check` ✅ | 只读 manifest 能列全部段 ✅；未知 `optional` 跳过、缺 `required` 明确拒 ✅；改一字节被检出 ✅ |
| **T3 读（L2）** ✅ **全落**（2026-11-05，见 `format.md §19.10`） | `Store`：`open` / `subtree_bytes` / `get` / `text` / `text_of` / `history_at` / `payload` / `payloads` ✅；`MappedFile`（`mmap` feature）✅；`DIR` / `provenance` 未做（`DIR` 故意不写） | 「取单个子树不遍历全树」✅（改坏兄弟 tag 仍能取别的）；**RSS 判据达成**：100 MB 文件堆增量 **256 B**（< 1 MB） |
| **T4 历史与签名（L3）** ✅ **全落**（2026-11-05，见 `format.md §21.1b`） | 层链 + `sign` / `append` / `verify` / `delegate` / `tree` / `compact` + 历史回放 + `--trust` + 写者锁（D20） + **基文档绑定（`base_hash` 进签名，甲）** ✅ | 「值来自哪层」✅；未授权换钥被拒 ✅；陈旧链头 ⇒ `AppendConflict` ✅；整条剥离与**基文档被换**都能抓到 ✅ |
| **T5 压缩与加密（L4）** ✅ **全落**（2026-11-05，见 `format.md §19.11` / `format.md §19.12` / `format.md §19.13`） | 段级 zstd ✅ · 帧级 `FRAMES` + 索引 ✅ · 导航器按需解帧 ✅ · 解压预算 ✅ · **AES-256-GCM 按块 AEAD + 口令派生 ✅** | 「只解压/解密被访问的部分」✅；「未知算法 ID 明确拒绝」✅；改段头也能被 AAD 抓到 ✅ |
| **T6 工具（T6）** ⏳ 库接口已落（2026-11-05） | **库在门面 crate**：`sml::envelope` —— `wrap` / `unwrap` / `list` / `sml_to_amberice` / `amberice_to_sml`，门控 feature **`amberice`**（连带 `preserve_order`）；要段级压缩再开 **`amberice-compress`**（多出来的那层是故意的：不该让「只想用信封」的人被动背上 `zstd`）。`smltools` **仍是纯 bin**，库能力不在它里面。`amberice` 的 `pack/unpack/sign/verify/append` CLI ✅；**`smltools --to-amberice` / `--from-amberice` ✅**（二进制不走 `emit`，改在写盘处另开一条 `Vec<u8>` 出口；`--amberice-compress N` 要 `amberice-compress` feature） | 两轮往返稳定 ✅（`rust/tests/envelope.rs` 7 例 + CLI 实跑）；转换不丢信息 ✅；签名跨端待 T7 |
| **T7 五端** | JS / Lua 解码器（只读 `VALUE` 段 + 透传载荷）+ 一致性用例 | 同一 `.ambr` 五端读出同树；载荷字节原样 |

**依赖关系**：T0 是 T1 的前置（没有 canonical 就没有幂等与签名）；T2 依赖 T1；T3–T5 可并行；T6/T7 依赖 T2。
**明确不在阶段内**：`smldb`（T8，见 §5）、SIMD（`format.md §8`，等 profile 证明）、FTS 索引（D13）。

