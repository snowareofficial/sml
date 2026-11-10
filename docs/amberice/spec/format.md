# `.ambr` 格式规范（`amberice`）

> **产品线** amberice ｜ **状态** current ｜ **最后核对** 2026-11-09
>
> 本文是 `.ambr` 二进制格式的**唯一权威**：信封、canonical 规则、`VALUE` 段布局、算法表、
> 默认值表、侧信道防护、设计终态。决策记录（D1–D41 与待拍板项）在 `../decisions.md`；
> 工具面在 `tools.md`；数据库层 `smldb`（**未开工**）在 `smldb.md`；元数据在 `docs/meta.sml`。
>
> **章节编号是 `amberice` 产品线的全局编号**，不是本文件内的连续编号：第 5–7, 13–14, 17 节在 `smldb.md`；第 10, 12, 15–16 节在 `../decisions.md`；第 22 节在 `tools.md`。
> 这样跨文件的交叉引用（`§19.9`、`§21.1b` 之类）永远指同一件事。

---

## 0. 定位与命名

- **SML** = 语言本身。核心 crate 是 `swsml`（crate 名，库名 `sml`）；门面 crate 是 `swml`。
- **`amberice`** = SML 的**二进制化**（`.ambr` 容器 / crate / CLI），独立发布线。
- **`smldb`** = 计划中的可选持久化层，**未开工**（见 `smldb.md`）。
- 性质：标记语言（HTML/XML/Markdown 类）解析库，自有实现。
- 目标：极低内存占用 + 零依赖核心 + 可选持久化层。
- 📌 **支持文件名**（2026-11-06 用户裁决：文本后缀定 `.swl`）：
  - `.swl` — **sml 文本文档**（人类可读的源/文档）✅ **v5 起作为默认写出后缀**
  - `.sml` — **v1–v4 既有后缀 + 长期读取别名**（读端继续接受；写出默认值改到 v5）
  - `.ambr` — **sml 的二进制衍生**（随机访问 / DB BLOB / NoSQL 传输载荷候选之一）✅
  > `.swl` / `.smld` 二选一已定：**`.swl`**。`.smld` 与 `.mlx` / `.mlb` 一并作废。
  > 存量 `.sml` 引用的规模**不手写**，用 `git grep -lE '\.sml([^a-zA-Z0-9]|$)'` 现测；
  > 迁移排期与未迁移项见 `TODO.md` §11.2。

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
│   .ambr 序列化 / extract_text()                   │
└────────────────────┬────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────┐
│ .ambr 二进制格式                                  │
│   前序排列 + subtree_len + 全局字符串表             │
└─────────────────────────────────────────────────┘
```

> 数据库层（`smldb`）是图中的可选上层，**未开工**，独立成文：`smldb.md`。

### 铁律

1. **`sml` 主 crate 零依赖**。任何数据库、async runtime、压缩库都不得进入主 crate。
2. **绝不把树结构拆成数据库行**（见 `smldb.md §5` 实测）。
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

### 2.3 现行值模型

值层不是自造的节点 AST，而是 `sml-value` 的 `Value`（D2：`.ambr` 编的是 `Value`）。
对象内部结构与索引策略见 `docs/swsml/spec/value-layout.md`；`.ambr` 的字节编码见 §19。

上面两张表推出的三条纪律仍然有效：

1. **文本不复制** —— 解析产物用 `Span` 指向源文本，不按节点复制字符串；
2. **键名去重** —— 字符串键的空气税只能靠「同一份字符串只存一次」消掉：
   `.ambr` 侧是全局字符串表（§19），内存侧是值层的键索引；
3. **容器不内嵌 `Vec`** —— 每节点多 24 B 三段 + 一次堆分配。

### 2.4 实体解码【已定】

```rust
fn decode<'a>(raw: &'a str) -> Cow<'a, str> {
    if !raw.contains('&') {
        Cow::Borrowed(raw)   // 99% 文本走这里，零分配
    } else {
        Cow::Owned(raw.replace("&amp;", "&").replace("&lt;", "<"))
    }
}
```

---

## 3. `.ambr` 二进制信封【已定】

> **段布局与字段编码以 §19（`VALUE` 段字节布局）为准** —— 二进制里编的是 `Value`（D2），
> 不是 HTML 式节点树。本节只留**信封层面**的取舍与实测体积；随机访问靠前序排列 +
> 固定长度 `subtree_len` 字段（§19.2），不遍历即可整棵跳过。

### 3.1 为什么要有二进制层

二进制**不是必须**。它本质是：把「体积 / 惰性 / 随机访问」这三件在 protobuf 场景才需要的事，
做成 sml 文档原生的、无需 schema、无需 codegen 的一层；必要时再叠 zstd 帧。

纯「解析 → 用 → 丢」的调用方完全用不到它。

### 3.2 体积实测【5,032 KB 文本样本 / 52,000 节点】

| 编码 | 体积 | /原始 |
|---|---|---|
| sml 文本（HTML 式） | 5032 K | 1.000 |
| protobuf wire | 3317 K | 0.659 |
| 文本 + gzip -9 | 734 K | 0.146 |
| 文本 + zstd -3 | 869 K | 0.173 |
| protobuf + zstd -3 | 690 K | 0.137 |
| **.ambr（裸）** | **867 K** | 0.172 |
| **.ambr + zstd -3** | **540 K** | **0.107** |
| .ambr + zstd -19 | 512 K | 0.102 |
| .ambr + zstd -3 + 训练字典 | 534 K | 0.106 |

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
| 容器 | 裸 `.ambr` + **mmap**，不压缩 | `.ambr` + zstd -3 帧 |
| RSS | 2.6 MB（源由 OS 分页管理） | — |
| 随机访问 | O(1) | 需先解压 |

**mmap 的干净页在内存压力下被内核直接丢弃，下次缺页重读** —— 等于 OS 免费做了压缩，比任何压缩都彻底（真正 0 占用），代价仅一次缺页中断。

**冷数据绝不能压缩**：为了省磁盘（最便宜的资源）而付出「每次访问要解压 + 无法随机跳转 + 常驻内存反而更高」的代价，纯亏。别跟内核抢活干。


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

1. **`sml`：`.ambr` 格式 + 零拷贝解析 + 选择器 + `extract_text()`**（零依赖，所有场景的地基）
2. **`serialize()` / `deserialize()`**（CRUD 前置）
3. **`smldb` crate + rusqlite 后端**（BLOB + 提取列 + FTS5）
4. （搁置）Turso 后端 —— 等触发条件
5. （搁置）SIMD stage1 —— 等 profile 证明是瓶颈

第 3 步之前不碰数据库，`sml` 保持零依赖。


---

## 11. 常见误区清单

| 误区 | 正解 |
|---|---|
| 用 Huffman/zstd 压 String 内存 | 压不掉 48 B 空气税，空气只能不交，不能压缩 |
| 用 protobuf 省内存 | prost 解析出仍是 `String`/`Vec`，内存零收益 |
| 直接 gzip 文本就够了 | 传输场景比 protobuf 小 4.5 倍，但 .ambr+zstd 更小 38% |
| 冷数据也压缩省空间 | 毁掉 O(1) 随机访问，fsync/解压成本反噬 |
| 把树拆成数据库行方便查询 | 实测大 85% + 69.7 ms 重建开销 |
| 用逗号字符串存 tags | `LIKE` 比索引慢 1500 倍 |
| 先写朴素解析器再优化 | SIMD 与零拷贝是同一重构，返工等于推倒重来 |
| 用 `Option<Span>` 表示无文本 | 对齐膨胀，Node 从 36 B 变 40 B |
| Node 里嵌 `Vec` 存子节点/属性 | 每 Node 多 24 B + 一次堆分配 |


---

## 18. canonical 规则（草案 v0.1 · P5 的施工图）

> 目的：让「同一份数据」无论来自 `.sml` 文本、程序构造还是另一个 `.ambr`，都能产出**同一串字节**。
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
| **C2** | **键序 = 源序**（D6 保序）。对象为 `Map::Unordered` 时**直接报错**（`CanonicalError::KeyOrderUnavailable`），绝不退化成排序；`parse_ordered()` / `Map::ordered()` 产出的 `Map::Ordered` 可签。判定看值本身，与 feature 档位无关 |
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
| **C13** | **非有限浮点（NaN / ±Inf）没有字面量**：语言里只能「由算式得到」（如 `1/0`），而 canonical 是静态文本 ⇒ **遇到非有限浮点直接报错**（`E-AMBR-非有限浮点`）。确实要存时，由调用者显式转成字符串（`"nan"` / `"inf"`），库不代劳 |

### 18.2b 实现澄清（T0 落地时补，2026-11-04）

> 下面各条是**把 C1–C13 写实**时必须回答、而规则表本身没写死的问题。写进这里，是为了让
> JS / Lua / C / C++ 的移植不用去猜 Rust 的实现（判据以 `tests/vectors/amberice-canonical.v1.json`
> 为准，见 §18.6）。

| # | 澄清 | 理由 |
|---|---|---|
| **C5-a** | **canonical 一律忽略 `Float` 的 raw** | raw 是解析期的书写痕迹、不参与相等比较；若让它进 canonical，同一份数据「从文本读入」与「从 `.ambr` 读入」会得到两串不同文本 ⇒ C12 当场失效，签名随之分裂。`raw` 仍由 `to_sml` 保留（那是给人看的输出） |
| **C5-b** | 浮点写法 = **十进制展开 + 整数值补 `.0`**（`Display` 最短往返数字，不用科学计数法：`1e10` → `10000000000.0`；`-0.0` → `-0.0`） | 与 `to_sml` 的浮点写法共用同一份实现（`text_rules::format_float`），避免两套发射器漂移。⚠️ 跨端需照此展开 JS 的 `1e+21` 形式（见 §18.6 待确认 ②） |
| **C7-a** | 转义集 = `\n` `\t` `\r` `\0` `\\` `\"` + 其余 C0/0x7F 走 **`\uXXXX`**（4 位十六进制，**不是** Rust 的 `\u{...}`） | 跟随现词法层的转义集；canonical 必须是**能解析回去**的文本（P5-d） |
| **C8-a** | 「单行 / 分行」判据 = **直接子项是否全为标量**（不含容器）。全标量 ⇒ `[ a b ]` / `{ k: v, k2: v2 }` 单行；否则逐项分行 | 判据只看一层、只看值的形状，与行宽无关 ⇒ 确定性（C12）。**数组元素间用空格**（C8 原文），扁平对象字段间用 `, `（与 `to_sml` 的行内形式一致，且已能往返） |
| **C8-b** | 多行容器的收尾括号**回到键的缩进**；`键: {` 与 `键: [` 的括号留在**同一行** | 「键与它的值永远相邻」，减少一条排版自由度 |
| **C9-a** | 顶层对象写**字段列表**（不套 `{ }`）；空对象仍写 `{ }` | 顶层是「文档」而不是「值」；空文档没有字段可写，只能退到 C9 的空对象形式 |
| **P5-d-a** | 「输出是合法 SML」的边界 = **顶层为容器**。顶层标量照常产出文本（签名字节唯一），但**不可作为文档回读** | parser 明确要求顶层为容器（`E-PARSE-008`「顶层须为容器，单独的标量无法往返」）。这是 parser 的既有约束，不是 canonical 的取舍 —— 若要改，改 parser 的顶层规则，而 canonical 跟着走 |
| **C2-a** | **只有含 `Map::Unordered` 对象**的值报错；标量等不含键的值可签 | 键序是对象自身的状态，不是构建档位；把「有没有键」当成档位问题会让错误原因不可判定 |


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
signed_bytes = "amberice/sig/v1\n"
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

**落点**（D3 的边界：`.ambr` 只依赖值模型）

| 东西 | 在哪 | 说明 |
|---|---|---|
| `Value::canonical_text()` / `canonical_bytes()` / `from_canonical_bytes()` | `rust/sml-value/src/canonical.rs` | 规格把 API 写作 `Value::canonical_*`（§18.1），孤儿规则决定它必须住在定义 `Value` 的 crate；**容器层（段表/manifest/层链/签名/加密）一概不进 `sml-value`** |
| `CanonicalError`（13 个变体 + `kind()`） | 同上 | 容器错误不占 `errors/codes.sml` 五端码位，当前给稳定 `kind()`；深度沿用 `E-LIMIT-004`。落 `E-AMBR-*` 须改 `errors/codes.sml` 并重生成五端产物 |
| 「文本原子」规则（裸词/引号/转义、浮点写法） | `rust/sml-value/src/text_rules.rs` | 从 `dump.rs` 搬出来，**三个发射器共用一份**（`to_sml` / `ser_text` / `canonical`） |
| crate `amberice` + `amberice vectors` CLI | `rust/amberice/` | T0 只有门面 + 向量；编码器骨架归 T1。`publish = false`（D1） |
| 跨端向量 | `tests/vectors/amberice-canonical.v1.json` | 21 例，`canonical_text` + `bytes_hex` 双期望值 |

**四条硬判据都已写进测试**（不是口头）

| 判据 | 钉在哪 |
|---|---|
| `Map::Unordered` ⇒ **显式报错**（C2，绝不排序） | `rust/tests/canonical.rs` + `rust/amberice/tests/vectors.rs`（两档构建同一判据） |
| 非有限浮点 ⇒ **报错**（C13，不伪造字面量） | `canonical.rs` 单测 + `tests/canonical.rs` |
| 同一逻辑数据 ⇒ 同一串字节（C12） | `tests/canonical.rs::c12_*`、`amberice vectors` 自检 |
| `dump→load→dump` 逐字节相同 / `Int(1)`≠`Float(1.0)` / `__type`·`__featags` 原样往返 | `tests/canonical.rs` |

**怎么跑**

```
cargo test --test canonical                                  # C2 与内容判据
cargo test --features preserve_order --test canonical        # 兼容名档：同一组判据
cargo test -p amberice --features preserve_order                 # 向量文件与代码一致
cargo run -p amberice --features preserve_order -- vectors --check
```

⚠️ `amberice` 的 `preserve_order` 是**兼容名**（空 feature，opt-in，不进 `default`）：
上列命令保留它只为不打断既有调用。canonical 的键序判据看值本身，`parse_ordered()` /
`Map::ordered()` 产出的 `Map::Ordered` 在两档构建下都能签，标量不受键序约束。

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
- `seg_id`：`0x01=VALUE 0x02=STRTAB 0x03=DIR 0x10=FRAMES 0x20=EMBED 0x30=SIG 0x40=PROFILE 0x50=SOURCE 0x60=PROVENANCE 0x70=TYPED 0x71=SCHEMA`（后两段见 §25）
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
- 写入口：库用 `DumpOptions.dir` / `dir_depth`；CLI 用 `smltools --to-amberice --dir [N]`。默认不写（D43）。

**段体字节布局（N05 落地时写死，2026-11-06）**：

```text
DIR     = head_layer:u32le ‖ count:varint ‖ entries[count]
entries = (parent_off:u32le ‖ key_id:varint ‖ child_off:u32le)*   按 (parent, key) 升序
```

- 偏移是**明文 `VALUE` 段体内的字节偏移**（`0` = 根节点）；
- **默认不写**：`DumpOptions.dir = false`。索引会改字节，故与「不写这两段的文件与收录前逐字节相同」（§25.7）保持一致；
- 写出时归到 `caps_optional` 的 `dir`（不认就忽略）；
- 实现：`rust/amberice/src/dir.rs`（构建 + 解析）、`Store::lookup_dir`（逐层二分，答不了就回退线性走）。
- ⚠️ **代价实测**：`flat@16000` 单层 16k 键 —— 文件 447.2 → 611.2 KiB（+36.7%），中键 get 54,097 → **412 ns**（−99%）。
  索引项与兄弟数成正比，故「一个巨大扁平对象」是它最费空间的形状；`dir_depth` 帮不上（深度只有 1）。
  用不用按场景定：读多写少、键多而深 ⇒ 开；写多或只需整树读 ⇒ 不开。

> ✅ **v1 收录（用户 2026-11-06 裁决）**：写者**可选**写；不写不影响任何正确性。
> 五端解码器对它的要求只有「认段头 + 按 `len` 跳过」，**不要求解析条目**。
> 实现状态：`SEG_DIR` 已登记（`rust/amberice/src/container.rs`）、`dir_depth` 默认值已落 manifest；
> **写入端未做** → N05。§25.1 的收录前提对它同样有效。

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
 0  4  magic          "AMBR"（沿用 `../decisions.md §12.3` 的既有约定）
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

落地：`amberice::container::{ALIGN_MIN, ALIGN_BLOCK, FLAG_ALIGN_BLOCK, align_of(flags)}`，
`DumpOptions::block_align` 是写者旋钮（默认 `false`）。T5 加密时强制置位。

### 19.8 段摘要 + `check` / `cat`（T2 前半，2026-11-04）

**摘要已真写**：段头的 `sha256:32`（字段名沿用 §19.1，实际算法由 `hash_id` 定）由零依赖自实现填。
用户 2026-11-04 定「**默认 SHA-256，可选 SM3 / BLAKE3**」，故：

| 算法 | 编号 | 状态 | 依据 |
|---|---|---|---|
| SHA-256 | `0x30` | ✅ 已实现（默认） | FIPS 180-4；NIST 向量 + 与 CPython `hashlib` 交叉核对 |
| SM3 | `0x31` | ✅ 已实现（可选） | GB/T 32905-2016；标准向量 + 5 条填充边界与 `hashlib` 核对 |
| BLAKE3 | `0x32` | ✅ 已实现（可选，D44） | 零依赖标量实现；官方向量覆盖单块 / 单 chunk / 跨 chunk / 3072 B |

分派在 `rust/amberice/src/hash.rs`（`HashAlgo` / `digest_by_id`）：**写**默认 SHA-256，**读**一律以
manifest 的 `hash_id` 为准。语义三条：

| 规则 | 说明 | 理由 |
|---|---|---|
| **全 0 摘要 = 未校验**（不是「无效」） | 段头摘要全 0 时 `load` / `check` 都不比对，只是校验更少 | 向前兼容 T1 时代写的文件；「没带摘要」与「摘要不符」是两件事，混成一个错误就会让旧文件打不开 |
| **`load` 只验它用到的段** | `VALUE` / `STRTAB` 必验；未知段按 `len` 跳过、不验 | 不变量 1：不认识的段影响不到值，验它只是白花 CPU |
| **`check` 全验且不早退** | 逐段打印「摘要 OK / 不符 / 未带摘要」，最后给总判；结构性错误（头/段表/截断）仍是硬错 | `tools.md §22` 要「打印每段结论」；一遇错就抛会让人只看到第一条 |

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

**工具**：`amberice check <f>`（结构 + 逐段摘要 + 能否解码，退出码 1 = 未通过）；
`amberice cat <f> [<path>]`（`a.b[0]` 语法，走 `sml_value::Path`，输出 **canonical 文本**）。
给 `<path>` 取该处当前值；**不给则输出整份**（Unix `cat` 的直觉）。历史回放已拆到
`amberice history <f> <path> --at N` —— 同一个命令既当「整份输出」又当「历史查询」，
读者无法从名字预期它的行为（评审 2026-11-05）。
`cat` 当前是「整树解出再找路」，`DIR` 段（T3）才把它降到 O(1) —— 先让语义站稳，再上索引。

**「不可信字节」这条线**：解码器吃的是别人的文件，一次 panic 就是一次 DoS。故有一条用例
**逐字节**改（每字节两种翻转）喂 `info` / `load` / `check`，只允许 `Ok` / `Err`，不许 panic。
文件 ~8 KB，全扫一遍约 1 秒，代价可以接受。｜ Any single-byte mutation must return, never panic.

### 19.9 `MANIFEST` 段（✅ 已落定并落地 · 2026-11-04）

> 本节先以草案形式给用户过目，**用户 2026-11-04 裁决后已落代码**（`rust/amberice/src/manifest.rs`）：
> 编码形态 A ✅、字段表 ✅、`caps` 用**名字**（甲）✅、`hash_id` 默认 SHA-256 且 SM3 可选 ✅。
> BLAKE3 已由 D44 收口为「实现」（零依赖标量版）。

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
  tool: "amberice 0.6.9-dev.0"                    // 只展示（D12 溯源）
}
```

**四条与既有决定的对应**（不是新发明）：

| 字段 | 出处 | 落点 |
|---|---|---|
| `primary` | P8「manifest 永远有 `primary` 指针」 | `load` 从「必须有 `VALUE`」改成「按 `primary` 找主内容」，`.ambr` 才真能兼作通用归档容器（D23） |
| `segments[]` | §18.3 `canonical_manifest = 按段 id 升序 + 每段 (id, algo, len, sha256)` | **补齐段头摘要覆盖不到的那半**（§19.8 的缺口）：算法与长度进签名 |
| `crypto` / `hash_id` | §20 / §20.2 | 编号一律用 §20 的表；`none = 0x00` |
| `caps` | D11 两位集 + P10「同一判定函数」 | 见 19.9.3 |
| `defaults` | §21「默认值写进 manifest」 | 读时以文件为准，改默认值不影响旧文件 |
| `payloads[]` | D23「外来载荷是一等段」+ `tools.md §22` `pack/unpack` | 归档类文件的文件清单（名字/长度/摘要），偏移靠 `len` 走出来 |
| `created` / `tool` | D17 / D12 | **只展示**，绝不进验证的必要条件 |

**§19.9.2b 归档：为什么是「一个 `EMBED` 段 + 一张 `payloads[]`」**

段是按 `seg_id` 认的（`segments[]` 也按 id 立条目），同种段来两个就说不清谁是谁 ——
§18.3 的「按 id 升序」直接失效。故约定：

- **一个 `.ambr` 至多一个 `EMBED` 段**，段体是各载荷**按 `payloads[]` 顺序拼接**；
- `payloads[]` 只记 (name, len, sha256)，**不记偏移** —— 偏移由 `len` 累加走出来
  （与段表同一套思路：表里没有位置，位置就不可能和内容不一致）；
- `primary` 指向 `EMBED`（P8 允许主内容不是 `VALUE`）⇒ 这类文件 `load` 会明确报
  `PrimaryNotValue`，取数据走 `payloads()` / `payload(name)`；
- 归档默认 `caps.required: [embed]`：读不懂 `embed` 的读者应当明说读不懂，而不是当空文件。

⚠️ **解包必须挡目录穿越**：载荷名来自**文件**，可以写成 `../../etc/passwd`。
`amberice unpack` 对「绝对路径 / 含 `..` / 含盘符」一律拒绝（tar-slip 是最老的解包漏洞之一，
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
| 容器（`rust/amberice/src/store.rs`） | `Store::open(&[u8])` → `container::open_raw` | 只解析头 + 段表 + manifest + 层链；`VALUE` / `STRTAB` 段体**保持 `&[u8]`**，不构造任何 `Value` |
| 值（`rust/sml-value/src/canonical.rs`） | `subtree_at(strtab, value, &Path)` | 在节点树上按路径下行，**兄弟节点用 `subtree_len` 跳过**，只对命中的那棵调 `decode_node` |

导航放在 `sml-value` 而不是 `amberice`：线格式只有一份，两份导航器必然漂移（与 `text_rules`
同一条理由）。`amberice::Store` 只是把两段拼起来，并补上「层补丁覆盖顶层键」的读取口径。

**判据怎么钉的**（`rust/amberice/tests/store.rs`，7 例）

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

**判据已达成（2026-11-05，user 裁决引入 `memmap2`）**

`mmap` feature 提供 `MappedFile::open(path)` → `store()` / `store_unverified()` / `info()`：

```rust
let f = MappedFile::open("big.ambr")?;   // 只 mmap，不往堆里搬
let store = f.store()?;                  // 借用 f（不是自引用结构，无需 unsafe 骗借用检查）
store.get("port")?;
```

**为什么是 `MappedFile` 而不是 `Store::open_path`**：`Store<'a>` 借用 `&'a [u8]`，若让它自己持有
映射再借用自己，就是自引用结构 —— 要么 `unsafe` 骗过借用检查，要么引 `ouroboros` 之类。
让调用方持有映射、再向它借 `Store`，类型系统自然说清「映射活得比读者久」，一行 `unsafe` 都不用。

那条 `unsafe` 只在 `Mmap::map` 一处，理由写进代码：**`.ambr` 按定位不可变，且每段带摘要，
被改会在 `Store::open` 时以 `digest-mismatch` 检出** —— 风险由设计接住，不靠运气。

**实测（不是估）**：`rust/amberice/tests/mmap.rs` 用**计数分配器**量堆增量（比读 RSS 可移植）：

```
文件 104,858,327 B（100 MB），打开 + 取一个小字段 ⇒ 堆增量 256 B
```

判据要求 < 1 MB，实得 **256 B**（差 4000 倍）。另两条用例钉正确性与错误面：mmap 读与整解读
逐值一致；文件不存在时给 `io::ErrorKind::NotFound` 而不是 panic。

`memmap2` 台账：一条 **INFO**（RUSTSEC-2026-0186），只涉 `advise_range` / `flush_range`，
**修于 >= 0.9.11**；本项目只用 `Mmap::map` + 切片，钉 0.9.11 以上。

`DIR` 仍**故意不写**：它只能省每层 O(兄弟数) 的键比较，等真出现「万级兄弟 + 高频取键」再上。

### 19.11 段级压缩（T5 前半，2026-11-05）

**用户裁决**：压缩走 zstd 的 C 绑定链（`zstd` → `zstd-safe` → `zstd-sys`），**不**换 deflate。
台账（实测）：`zstd 0.14.3` / `zstd-safe 8.2.0` / `zstd-sys 2.1.2+zstd.1.5.7`；
`osv_check.py` 扫 **111 个包**（含 T5 加密引入的 `aes-gcm` / `argon2` / `hkdf` / `sha2`）「未发现任何已知漏洞」。`zstd-sys` 2026-10-26 那条（allocator shim
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
`load` / `cat`；`amberice check` 会打出「压缩 0x01：36 B → 200002 B」证明解压链路真的通。

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
| `FRAMES` | `FrameCursor`（`amberice` 侧，一帧缓存） | **只解覆盖目标区间的帧**；跳过只做算术 |

`Store` 现在对分帧文件也能 `open` / `get` / `text_of`：用例「把**最后一帧**弄坏 ⇒ `rows[0]`
照常取到、`rows[299]` 报错、整解必失败」把这条性质钉住了。
`Store::subtree_bytes` 统一返回 `SubtreeBytes`：明文 `Borrowed`（零拷贝），分帧 `Owned`
（只解覆盖目标区间的帧）。不再有「分帧单独做不到」的错误（D42）。

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
（`kms://…`、`/etc/amberice.key`）会带来两个真问题：

1. **文件被绑死在某个基础设施上** —— 换 KMS 就得改文件，而文件是**已签名/已归档**的东西；
2. **泄露内部拓扑** —— 文件会被到处传（归档、备份、邮件），里面写着内网路径就是泄题。

所以：`master` 的**获取方式永远在文件之外**。文件只声明「这段密文用什么算法、用什么 KDF
参数（若有）」，不声明「钥匙在谁的抽屉里」。这也与 D18「信任根选择交给调用者」同口径。

#### 与既有决定怎么接（不是新发明）

```text
master(32 B)  ──HKDF(master, "amberice:dek")──▶  per-file DEK        （D29）
DEK           ──HKDF(DEK, "amberice:"+algo)──▶  AEAD key            （D29）
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

读者入口 `load_encrypted(bytes, &KeySource)` 与 `get_encrypted(bytes, path, &KeySource)`；
写者 `dump_encrypted(…, &KeySource)` / `dump_with(…, keys)`。
**`KeySource::Bytes` 就是外部秘密的接缝** —— 也允许调用方传一个闭包/回调去取（由上层决定
何时问 KMS、缓存多久、失败怎么退），库只认「最后拿到 32 B」这一件事。

#### §19.13.1 用户级 API 树（已落地）

> 根下一层只放用户级动作；段布局与低层符号留在各自子模块。
>
> 用户 2026-11-05 定了三条：**一个 enum**（不拆写/读两个类型）、**命名要一眼就懂**、
> **API 树要扁平**（一个树、每层节点少一些）。

**API 树（库根下一层：用户级动作 + 类型）**

```text
amberice
├─ dump / dump_with / dump_encrypted              写
├─ load / load_encrypted / base_value            读
├─ get  / get_encrypted                          按路径取一个节点
├─ check / info                                  只看结构
├─ pack* / payload / payloads / embed_body       EMBED 归档
├─ layers / verify / append_layer / compact      层链（sign feature）
└─ KeySource / KdfParams / DumpOptions / Store / MappedFile / CanonicalValue / ContainerError
```

段布局常量、帧工具、段头结构，以及 dir / hash / typed / manifest 的低层符号都在各自子模块
（`container` / `layer` / `typed` / `dir` / `hash` / `manifest`），不占根名。
CLI 的 `ls` / `cat` / `unpack` / `text` 是命令，不是库 API。

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
use amberice::{dump_encrypted, DumpOptions, KeySource};

let master: [u8; 32] = load_master_from_kms("prod/amberice/key")?;   // 库不管钥匙从哪来

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
$ amberice check secret.ambr
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

#### §19.13.2 块布局与实现（**已落地** 2026-11-05）

```text
加密段体 = plain_len:u32le ‖ block_count:u32le ‖ block…
block    = ciphertext(≤4096 B) ‖ tag(16 B)              ← 最后一块可短
nonce    = layer_id:u32le ‖ block_index:u32le ‖ file_salt[0..4]
AAD      = seg_id:u8 ‖ comp:u8 ‖ block_index:u32le ‖ block_count:u32le ‖ plain_len:u32le
密钥      master ─HKDF(salt=file_salt, "amberice:0x10")→ AEAD key
```

| 规则 | 为什么 |
|---|---|
| **AAD 绑段头**（`seg_id`/`comp`/块序/块数/明文长） | 改段头 ⇒ 解密当场失败。这**补上了 §19.8 那个缺口**（「改段头 `flags` 无人察觉」）在加密文件上的那一半 |
| **`MANIFEST` 永不加密** | `kdf` 参数与 `salt` 就在它里面；加密它 = 「要先解密才知道怎么解密」 |
| **加密 ⇒ 强制 4 KiB 对齐** | 块加密与「追加不改旧字节」都要它；`dump_encrypted` **替调用方强制**，不写「看起来加密了、追加会毁数据」的文件 |
| **先压后加 / 先解后解压** | D28 的顺序固定，读侧反着来 |
| **`check` 不需要钥匙** | 结构、摘要、`enc` 算法、块数都能读；只报「加密段未解密」，把「是不是合法文件」与「能不能读明文」分成两件事 |
| **加密件不是字节确定的** | `file_salt` 每文件随机（D41 允许它进一次性标识）；所以只有明文 `dump` 承诺 C12 的「同值同字节」 |

**用例钉住的六件事**（`rust/amberice/tests/crypto.rs`，9 条）：往返 / 错钥 `auth-failed` /
改密文 `auth-failed` / **改段头 `auth-failed`** / 口令往返（读侧参数取自 manifest）/
写口令缺参数 `kdf-required`、读时参数不符 `kdf-mismatch` / 零拷贝对加密件 `encrypted-not-zero-copy` /
两种钥混用明确报错 / `MANIFEST` 保持明文可读。

**还没做**：追加语义（加密文件的**新增段必须落在新块边界**）—— 与 T4 的 `append` 一起做。


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
| **D** | `delegation` 的签名对象是**独立文本** `amberice/delegate/v1\nfrom:…\nto:…\nmax_index:…\n` | 授权是「从 A 到 B 直到第 N 层」这件事本身，与它被放进哪一层无关 |

**E · 基文档绑定（用户 2026-11-05 裁决「甲」，当日落地）**

原先 `Layer` 的可签字段只有 `index / prev_hash / author / patch / delegation`，**不含基文档**，
于是链头只由层记录决定：换掉基文档并重算段摘要，签名依然通过（段摘要是完整性校验，
不是来源认证）。实测 `v1`（port=8080）与 `compact(v2)`（port=9090）链头**完全相同**。

**甲方案**：`Layer` 增 `base_hash: [u8; 32]`，值 = `H(第一条层记录之前的全部字节)`，
写进 `to_value` ⇒ 进入 `signed_text`，也进入 `canonical_bytes`（`prev_hash` / 链头 / Trailer
`head_hash` 因而一起变化）。

| 谁在检查 | 怎么挡 |
|---|---|
| `sign::verify` | 重算当前文件的 `base_hash`，与**每层**记的比对；不符 ⇒ `ok=false`，报「基文档被换过」 |
| `layer::verify_chain` | 同样一步 —— **没有 `sign` feature 的构建也能发现**（链头只用到摘要） |
| 签名本身 | `base_hash` 在签名文本里，攻击者若重算它，签名立即失效 —— 两头都堵死 |

配套的两个原语：`layer::base_prefix_len`（沿反向链 `prev_off` 走到 0，即最旧层记录的偏移）
与 `layer::base_hash`。

**用例**（`tests/layers.rs`）：
- `swapping_the_base_document_is_detected`：只改基文档区 1 字节（层区一字不动），
  `verify` 与 `verify_chain` 都必须失败且**点名 `base_hash`**；
- `compact_changes_the_head`：合并换了基文档 ⇒ 链头必须变，旧 `--trust` 锚点正确失效。

⚠️ **格式变更的代价**：读层记录时 `base_hash` 缺失一律按 `Missing("base_hash")` 拒收，
不静默降级成旧格式。本项目 `publish = false`、没有需要兼容的历史签名文件，故选择
fail-closed；已有签名文件需要重签。**旧 `--trust` 锚点也随之作废，必须重建。**

⚠️ **一个必须说清的边界**：**整条层链可被剥离**（把 EOF 的 Trailer 连同层记录一起删掉，
文件就退回「未签名」）。没有外部锚点（记住旧 `head_hash` / 对方渠道核对）就无法察觉 ——
这正是 D18「信任根选择交给调用者」的含义。**不要把「验签通过」读成「这份文件没被换过」。**

**`--trust` 是这条边界的补法（2026-11-05 落地）**：

```bash
$ amberice verify v1.ambr
amberice: hint: 本次链头 fd4f90d0…（下次可用 `--trust` 钉住它，防整条链被剥离）
结论     全部通过（1 层）

$ amberice verify v1.ambr --trust fd4f90d0…          # 对得上
锚点     head 与期望一致（外部锚点，D18）        → 通过，exit 0

$ amberice verify base.ambr --trust fd4f90d0…         # 有人拿未签名件冒充
层链     没有层（未签名文件）
锚点     文件里一层都没有，而 --trust 给了期望值 —— 层链可能被整个剥离
                                                  → 未通过，exit 1
```

三个实现要点：

| 要点 | 为什么 |
|---|---|
| **链头 = 最后一层规范字节的摘要**（`layer::chain_head`） | 它与签名无关，只用摘要 ⇒ 没有 `sign` feature 的构建**也能钉锚点**（层体是 canonical 字节，需 `parse_ordered()` / `Map::ordered()` 产出的显式保序值；两档构建同一判据） |
| **没有层 + 给了锚点 ⇒ 未通过** | 「本来没签」与「签了又被剥掉」在字节上完全一样，只有锚点能把它们分开 |
| 未给锚点时打印一行 `hint` **把链头打出来** | 让人**记得住**下次要钉什么；不打印就等于永远没人用得上这条 |

`verify` 判断的是「链条自身一致且签名有效」，`--trust` 判断的是「这就是我上次看到的那个链条」。
前者防伪造层，后者防**整条替换**，两件事不能互相代替。

**写者锁（D20，2026-11-05 落地）**：`append_layer_checked(..., expected_head)` 与 CLI 的
`--expect-head HEX64`。这是**乐观并发控制**（不是 OS 文件锁）：调用方先记住
`layer::chain_head`，做完自己的事再带着它回来追加；期间有人追加过 ⇒ 链头必变 ⇒
后者拿到明确的 `AppendConflict`，而不是把自己的层悄悄挂到别人后面。未签名文件的「当前头」
按约定是全 0（与首层 `prev_hash` 一致）。D21 说「多写交 `smldb`」，故单文件层面不做 OS 锁。

**`compact`（D8 的唯一例外）**：读当前值（base + 各层补丁）→ `dump` 成全新基文件 →
签第 0 层（无补丁）。历史被**有意丢弃**，不变量 3 在本次整体重写时被打破，因此**必须重签**；
旧签名与旧 `--trust` 锚点随之作废，调用方要重建锚点。实跑：`2 层 → 1 层`，验签通过，
当前值保持 `9090`。

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

### 21.2 能力（`caps`）登记与准入（P10 的治理面，2026-11-05）

`caps.required` / `caps.optional` 必须是**开放集合**，否则格式无法扩展；但它同时是
最典型的「碎片化引擎」入口 —— 两年后一堆互相读不了的文件，通常就是从「谁都能随手加
一个名字」开始的。故把**登记**与**理解**拆成两件不同的事：

| 概念 | 函数 | 回答什么 | 后果 |
|---|---|---|---|
| 本构建**理解** | `manifest::known_capabilities()` | 「这个文件我能完整处理吗」 | `required` 缺 ⇒ 拒；`optional` 缺 ⇒ 跳过 |
| 格式**已登记** | `manifest::registered_capabilities()` | 「这个名字属于本规范吗」 | 未登记 ⇒ `check` 标「未登记能力」 |

**准入规则（加一个名字必须同时做三件事）**：

1. 在 `registered_capabilities()` 里登记；
2. 加**跨端向量**（`tests/vectors`）—— 没有向量就没有判据，T7 五端无从对齐；
3. 在本文档加一行，写清「读不懂时的后果」。

缺任一条视为未登记。**未登记的名字写侧仍然允许**（否则「声明未来能力」这条唯一扩展
通道就被掐死），但 `amberice check` 会把它标出来，评审时看得见。

**写侧只校验格式**：名字必须是小写标识符（`[a-z0-9.-]`、不以数字开头）、不得重复
（`required` 与 `optional` 之间也算重复）。错误码 `bad-capability-name` /
`duplicate-capability`。

> 为什么不按 `known_capabilities()` 闭集硬拒：`sig` / `dir` / `frames` 这些名字
> 正是「写者先声明、读者逐步跟上」的结果。写侧硬拒会让格式永远停在 v1 的四个段种类上。


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
| `smldb` 侧 | 选择器语法（`smldb.md §7` 挪过去后重新定义）、FTS 具体方案、表结构细节 | `smldb` 开工时 |
| `compact` | 合并多层后如何保留「旧签名证据」 | T5+ |
| 五端 | JS / Lua 解码器的错误码映射 | T7 |
| canonical 三条 | 顶层标量能否不可回读（P5-d-a）· 浮点跨端口径（C5-b）· 元数据在字节侧是否前置（L5） | ①③ 可随时定；② 按用户 2026-11-04 的裁决**与四端统一同步时定**（详见 §18.6） |

### 23.4 开工顺序（详见 `smldb.md §17`）

```
T0 canonical + vectors  →  T1 编码  →  T2 容器  →  T3 读 / T4 层链 / T5 加密（可并行）
                                                  →  T6 工具  →  T7 五端
```


---

## 24. 侧信道与冗余（防护清单 · 用户 2026-11-04 点名「反侧信道、大小冗余」）

> 威胁模型先说清：**我们防「观察者」与「离线攻击者」**（共享存储、备份、CDN、被审计的磁盘），
> **不防**「已在同机拿到 root 的攻击者」（那要进程级隔离，不是文件格式能解决的）。

### 24.0 威胁模型：工具保证什么、不保证什么（2026-11-05）

> 读这一节前先接受一件事：**`verify` 通过 ≠ 文件可信**。可信与否取决于「你是否知道
> 该信任谁」—— 那是 D18 交给调用者的决定，格式只负责把事实摆出来。

| 你想防的 | 工具给什么 | 你必须做什么 |
|---|---|---|
| 内容被改 | 逐段 SHA-256 + `check` | 无（自动） |
| 层被伪造 | `verify`：**全部**签名必须通过（D36） | 无（自动） |
| **基文档被换** | `verify` 报 `base_hash` 不符（§21.1b E，甲方案） | 无（自动） |
| **整条层链被删**、冒充「未签名文件」 | `verify` **看不出来**（0 层也「通过」） | **必须 `--trust <链头>`** |
| 内容被读 | 按块 AEAD（D28 / D35） | 提供钥匙；**加密件不承诺字节确定**（salt 随机，D41） |
| 别人猜出能力要求 | `caps.required` 只是**读侧拒绝** | 别把它当权限系统：它不阻止任何人读字节 |
| 从长度推断 | 4 KiB 块 + 64 B 量化 | 敏感场景自己调粗（§24.1 S1） |
| 同机 root | **不防** | 要进程级隔离，不是文件格式能解决的 |

**三条「用户必须做对才安全」的事**（工具无法代劳，也只能提示、不能强制）：

1. **`--trust` 锚点** —— 不记住链头，剥离检测就形同虚设。`verify` 未给锚点时会打印
   一行 `hint` 把链头吐出来，这是唯一的提醒机制，不是保证。
2. **钥匙保管** —— 格式不备份、不恢复、不轮换密钥；D16 只定义授权链，不定义运营。
3. **`caps` 准入** —— 只在「不随手加名字」的前提下才是安全的（见 §21.2）。

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
purpose_key = HKDF(master, "amberice:" + purpose)
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


---

## 25. `TYPED` / `SCHEMA` 段（v1 收录 · 仅 Rust 实现 · experimental）

> 用户 2026-11-06 裁决：**`.ambr` v1 带 DIR + TYPED**。本节把 `docs/amberice/plan/plan.md `smldb.md §7`` 的
> 「候选」升为 v1 正文；DIR 的字节布局原本就在 §19.3，那里只补了收录状态。

### 25.1 收录前提（破一条就不许收录）

1. **`VALUE` 必写** —— 任何 `.ambr` 都必须有 `0x01 VALUE`（+ `0x02 STRTAB`）。`TYPED` **不得**成为唯一载荷。
2. **canonical 相等与签名语义只看 `VALUE`** —— `TYPED` 是旁路，不参与 `eq_ordered` / `canonical_text` / `canonical_bytes`。
3. **未知段可跳过**（§23.1 ①）不因这两段改变：不认 `TYPED` 的读取方只按段头 `len` 跳过。
4. **不动版本位**：段 id 已登记，加段属向后兼容扩展，`fmt` 补丁位不变。

⇒ T7 判据（D45 起）：五端解码器对这两段要「**认段头 + 解 `SCHEMA`（canonical `Value`）+ 按 schema 渲染 `TYPED`**」；`VALUE` 仍是唯一的相等 / 签名对象，不因这两段改变。

### 25.2 `0x70 TYPED` 段体

```text
TYPED   = count:varint ‖ entries[count] ‖ payload
entries = (var_id:varint, type_id:u64le, off:u32le, len:u32le)
```

> 📌 `type_id` 是 **2026-11-06（N04 落地时）补进目录的**：§25.5 要求读出时比对类型，
> 而原来那版三字段项**没地方放** `type_id`。加这一笔时还没有任何文件写出过 `TYPED`，
> 故直接扩布局，而不是发一份无法校验的契约。

- `entries` **按 `var_id` 升序** ⇒ `Store::var(id)` 二分 O(log n)；
- `off` 相对 `payload` 起点计数，`payload` 紧跟目录；
- `var_id = 0` 保留（表示「未指定」）；重复 `var_id` 写入时**直接报错**，不自动分配、不猜测；
- 目录在段内 ⇒ 取变量**不依赖** `SCHEMA`，`var(id)` 无需外部类型信息。

### 25.3 `0x71 SCHEMA` 段（可选）

只放「给人 / 给别的语言看」的东西：`type_id → 字段 id / 变体 id → 名字与类型`。不写也能读，只是读端必须在编译期知道 `T`。**默认不落盘。**

| 模式 | 文件里有什么 | 谁需要类型信息 | 适用 |
|---|---|---|---|
| **外置 schema（默认）** | 只有 `TYPED` | 调用方编译期知道 `T` | 热路径、内部存储、体积最小 |
| **内嵌 schema（可选）** | `TYPED` + `SCHEMA` | 读端动态读 | `cat` 工具、跨语言、长期归档 |

**段体形状（D45）**：自包含 canonical `Value`（L0 字节，可单独解，不依赖 `VALUE`）：

```sml
{
  schema_version: 1
  types: [
    {
      id: "1a2b3c4d5e6f7788"      # type_id 的 16 位小写十六进制（u64，绕开 Int 上限）
      name: "User"
      kind: "struct"              # struct | enum
      fields: [                   # struct 用
        { id: 1, name: "id",   ty: "u64" }
        { id: 2, name: "name", ty: "str" }
      ]
      variants: [ { id: 0, name: "Allow" } ]   # enum 用
    }
  ]
}
```

`ty` 的 v1 词表：`bool` / `u8`–`u64` / `i8`–`i64` / `f32` / `f64` / `str` / `bytes`；
容器写 `{ vec: <ty> }` / `{ opt: <ty> }`；引用另一个类型写 `{ type: "<id>" }`。
v1 **不自动生成** schema：调用方手写或由工具生成，`Writer::put_schema` 只负责落段。

### 25.4 线格式（零依赖、可手写）

| 项 | 写法 | 理由 |
|---|---|---|
| 无符号整数 | LEB128 varint（复用 canonical 的 varint） | 小数字占 1 字节 |
| 有符号整数 | zigzag → varint | 负数不膨胀 |
| `f32` / `f64` | 固定 4 / 8 字节小端 | 二进制里不需要「十进制展开」，那套只服务 canonical 文本 |
| 字符串 / 字节 | varint 长度 + 原始字节 | 不做 UTF-8 二次校验（`&str` 已保证） |
| 数组 | varint 元素数 + 元素 | 同样带预算 |
| `Option` | 1 字节 tag（0/1） | 可空字段仍要区分「无」与「零值」 |
| 枚举 | varint 变体 id + 载荷 | 「变量 + 其 id」的核心 |
| 结构体 | 按字段 id 升序**全量写** | 类型不变 ⇒ 不需要 presence bitset / 缺省字段跳过，解码路径最短 |

**硬约束**：深度上限、总字节上限、元素数上限（与 `MAX_DECOMPRESSED` 同族），防「声明 4G 元素」的炸弹；解码全程 `safe`，不写 `unsafe`。

### 25.5 「变量类型不变」是契约，不是提醒

1. **写入时记 `type_id`**：类型路径 + 字段 id 表 + 变体 id 表的稳定指纹（不是类型名字符串本身）；
2. **读出时比对**：不等 ⇒ `TypedError::TypeMismatch`，**不猜、不升级、不强转**，错误里带「文件里是什么 / 代码里要什么」；
3. **v1 不做 schema 演进**：加字段、改字段类型、改变体顺序都算**新类型**（新 `type_id` 或新 `var_id`）；
4. 因此 `Option<T>` 的「有没有」是**值语义**，不是「字段缺不缺」。

### 25.6 caps：DIR 沿用 `dir`，TYPED 不新增 cap

- **DIR**：沿用已注册的 `dir`（`caps_optional`），语义是「索引存在，不认就忽略」；
- **TYPED**：**不新增 cap 名**。理由：`VALUE` 必写 ⇒ 不认 TYPED 的读取方本来就能读出全部值；用 `info` 列出段名（`TYPED` / `SCHEMA`）即可告知「本文件还带 typed 载荷」。
- **加密一致性（D5 裁决丙，2026-11-06）**：加密文件里除 `MANIFEST` 外**每段都必须加密**；
  出现明文段 ⇒ 拒读（`PlaintextSegmentInEncryptedFile`）；明文文件里出现加密段 ⇒ 同样拒读。
  代价：加密文件里的 `DIR`/`TYPED` 是密文，`Store::get` 用不上索引 —— 安全优先于速度。
  **白名单（已实现，裁决甲）**：段头 `flags` 位 0 = 「本段声明明文」。加密文件里每段
  `enc == 0` 当且仅当该位为 1；两者不一致 ⇒ 拒读。写侧 `DumpOptions.crypto_plain: Vec<u8>`。
  典型用法：把 `DIR` 声明为明文以保住索引（用保密性换速度），或让 `TYPED` 保持可读。
  ⚠️ **当前没有消费者**：`Store::open` 在加密文件上连 `VALUE` 都拒绝（`EncryptedNotZeroCopy`），
  且没有接收密钥的入口。所以「明文 `DIR`」目前只是**格式上的许可**，不产生任何性能收益；
  要等带密钥的零拷贝读取路径落地才真正有用。
  在此之前，声明明文只对「实现方自己解密的路径」有意义（如 `load_encrypted` 之后的再读）。
- 不新增 cap 也意味着**不需要改 `registered_capabilities()`**，不产生「声明了却没人认」的假能力。

### 25.7 状态

| 项 | 状态 |
|---|---|
| 段 id 登记（`0x70` / `0x71`） | ✅ 本规范 |
| DIR 写入（可选段） | ✅ **N05（2026-11-06）**：`DumpOptions.dir`（默认关）+ `dir_depth`；`Store::get` 逐层二分 |
| Rust `put_var` / `get_var` | ✅ **N04（2026-11-06）**：`Writer::put_var` + `Store::var`；整数/浮点/bool/String/`Bytes`/`Vec<T>`/`Option<T>` |
| 内嵌 schema（Rust） | ✅ **D45（2026-11-10）**：`Writer::put_schema` + `Store::schema`；段体是自包含 canonical `Value` |
| 五端解 schema / 渲染 TYPED | ❌ 未做（D45 已裁，排在 T7） |
| serde 桥 | 待定（`amberice-plan.md `smldb.md §7.12``） |
| 字节布局影响 | **零**：不写这两段的文件与收录前逐字节相同 |

