> 来源：`c:\Users\sakeen\Documents\deepseek-harness\default-workspace\swsml-tasting\docs\value-layout.md`
> 状态：【想法 · 未定 · ⚠️需二次蒸馏】——外部参考草案（草案 v0.1，2026-10-31），已按用户指示归档进当前项目 docs/，供日后拍板参考，非既定决定。
>
> **外部候选 crate 核实记录（2026-10-31，均用 `cargo search` / `cargo info` 直接查 crates.io registry 实证）：**
> - `WorkTablesIndex` v0.0.15 —— **真实存在**（Pathscale/`indexset` fork 生态，与 `worktable`/`wt-slice` 同族），两层 B 树 + 迭代/索引。→ **结论：存在成立，但对 `Value::Object` 层错**（嵌入表引擎的排序原语，非通用保序映射）；零依赖铁律下不引入。注：早前曾误判"查无此名"，系搜索引擎未索引该冷门 crate，已更正。
> - `worktable` v1.11.0 —— **真实存在**（Pathscale，MIT）。嵌入表/行/列/ORM 式数据库。→ 属 §13/§14 DB/Store 层候选，非 §15 值层。
> - `btree-slab` v0.6.1 —— **真实存在**（timothee-haudebourg，MIT/Apache-2.0，依赖 `slab`）。slab 后 B 树。近亲 `btree-plus-store` v0.2.1（arena + copyable/immutable）。→ 同样为值层外的排序结构，是否值得自实现 `Vec<Entry>` 替代，需 bench 说话，非默认引入。
> - **值层结论不变**：`Value::Object` 维持 `Vec<Entry>` + `OnceLock` 三档索引的零依赖方案（本文 §3/§5），不引上述任何第三方。
>
> **「保序能买到什么」简注（补记）：**
> - ✅ 直接 1：读写往返稳定，`parse→改→to_sml` 逐字节稳定（键序==源序），diff 无"假删假增"噪音（G1）。
> - ✅ 直接 2：人工可读——输出序 = 作者组织序，不被打散。
> - ✅ 直接 3：合并/拼接确定——配合 §6.3 规则（已存在原位替换、不存在追加尾部），`&base`/include 结果可测可押。
> - ✅ 深层 4：顺序成为可定义维度——`eq_ordered`、与 `sort_keys` 解耦的 canonical、C-ABI 迭代序可写进头文件。
> - ✅ 深层 5：为 §13/§14 DB/.smlb 层铺路——值层能保序，`.smlb`/Store 才能稳定按序序列化检索。
> - ⚠️ 保序**不买**：确定性字节（另走 canonical/sort_keys）、查找性能（另走三档索引）、顺序参与相等（默认仍集合相等，见 §6.1）。四者是独立开关/特性。
>
> **toml-rs 保序做法参考（2026-10-31）：** 高层 `toml` crate 默认 `Value::Table = BTreeMap`（保序），靠 **`preserve_order` feature 才切 `IndexMap` 保序**；真正 round-trip（往返编辑+保留注释格式）走底层 `toml_edit`，其 Table 用 `IndexMap<String, Item>` 保插入序。→ 印证两点：(a) 业界把「保序」做成**默认关、opt-in**，与本文「全局保序 feature 默认关」一致；(b) 它靠第三方 `IndexMap` 实现，我们零依赖需自实现 §5 三档，等价效果、不破铁律。
>
> **保序的「效果五层 + 不买三项」速记（2026-10-31，供拍板判断值不值）：**
> - 买① 往返稳定：`parse→改→to_sml` 键序==源序，diff 干净（G1）。② 合并确定：`&base`/include 合并结果可预测、可测（§6.3）。③ 顺序可定义：`eq_ordered`、C-ABI 迭代序写进头文件（§8）。④ 与 `sort_keys` 解耦：保序 vs 确定性(canonical) 两个独立开关（§6.2）。⑤ DB/.smlb 铺路：值层保序是 §13/§14「大量保序持久化」的前置。
> - 不买① 确定性字节（同值同字节靠 canonical/sort_keys，非保序）。② 查找快（保序via Vec 是 O(n)，快靠懒索引三档，另一特性）。③ 相等含序（PartialEq 仍按集合，§6.1）。
>
> **toml-rs 对标·一行**：`toml_edit`(indexmap 铁保序+保注释) + `toml`(默认 BTreeMap，靠 `preserve_order` feature 切 IndexMap)。→ 启示：①「保序=默认关+opt-in」是业界惯例，印证我们全局保序 feature 默认关的定位；② round-trip(连格式保序) 与 纯数据保序是两个不同 feature，勿混进值层；③ toml-rs 靠第三方 indexmap，我们零依赖自实现 §5 三档，效果等价——是我们的差异化卖点。
>
> **preserve_order 验证记录（2026-10-31，已实现并测）：**
> - 实现：`sml-value/src/object.rs` 新增 `Object`(Vec\<Entry{key:Box<str>,value:Value}\> + OnceLock\<Box\<Index\>\> 三档惰性索引：≤8 线性/9–4096 排序下标/>4096 FNV 哈希)，索引增量维护；feature 门控 `Value::Object(Map)`，默认档 BTreeMap、preserve_order 档 Object；PartialEq 两档均集合相等，顺序敏感走 `eq_ordered`。
> - 输出解耦：`to_sml` 按 entries 序 / `to_sml_canonical` 走 sort_keys 拷贝排序，互不影响。合并走 `sml_value::insert_or_merge`，冲突原位、`&frag` 原位覆盖/尾部追加。
> - 全量测试（workspace, 无 feature）：**唯一失败为既有 `_stack_probe::probe`（需 STACK_KB 环境变量，与本次无关），其余全绿**；preserve_order 套件 14/14（`-p swsml --features preserve_order --test preserve_order`）。
> - 已知局限：`--workspace --features preserve_order` 会裂 crate（smltools/sml-pattern dev-dep 编出第二份 sml-value/sml-feature），须逐包 `-p` 验证；已除残留 `_parse_bench` 进程(fix LNK1104)。
> - **逐包 feature 验证（2026-10-31）**：`sml-value/sml-feature/sml-lex/sml-pattern/sml-include/sml-contract/sml-parse/smltools/swsml` 全部 `-p --features preserve_order` exit 0，`smltools` 实测 80+45 全过、`swsml` 14/14——转发完整、无裂 crate。
> - 性能(_parse_bench release best-of-5)：25k R=0.96 / 100k R=1.04 / 400k R=1.05，**均落在噪声带内（本机±15%）——解析"不付费"是设计预期（惰性索引+增量维护），收益在保序/迭代/emit 侧**。勿仅凭 0.96~1.05 判可信；三条回退(每改重建55s/定长Box<u32>重分配/get_mut双查找)证明当前持平是"优化到位"而非侥幸，不可回退。
> - **一句话：保序买到「人写的顺序是语义的一部分」；它与确定性/查找/相等是四个独立开关。**

# 值布局设计：有序对象 + 三档查找 + 契约定长槽位

> 状态：**草案 v0.1**（2026-10-31）
> 目标：替换 `Value::Object(BTreeMap<String, Value>)`，同时拿到**保源序**、**解析不降速**、**内存下降**、**C-ABI 可文档化**四件事。
> 前置阅读：`docs/iter-parse-plan.md`（解析器的递归/栈改造）。本文件只谈**值层**，不改语法。
> 落点：仓库根 `docs/`；实现落点 `rust/sml-value/`。

---

## 目标 / 非目标

**目标**

| # | 目标 | 判据 |
|---|---|---|
| G1 | 保源序 | `parse → to_sml → parse` 逐字节稳定，且键序 == 源序 |
| G2 | 解析不付排序代价 | 25k 键解析 ≤ 现状；100k 键 ≤ 现状 × 0.85 |
| G3 | 查找不退步 | 小对象线性、大对象 O(1)/O(log n)，与 BTreeMap 同量级或更好 |
| G4 | 内存下降 | 同等语料峰值 RSS 下降 ≥ 25% |
| G5 | ABI 可文档化 | 指针有效期、迭代顺序写进 `include/sml.h` |
| G6 | 零新增第三方依赖 | 默认特性下依赖数不变（`IndexMap` 方案见 §11） |

**非目标**

- 不做并发写（值树一经构造即不可变）。
- 不做持久化结构共享（`im` / `rpds` 系）。
- 不做流式解析（另案）。
- 不改语法、不改契约语义。

---

## 现状代价（先量化，再动手）

`BTreeMap<String, Value>` 每个键的代价：

1. **每次插入 O(log n) 次字符串比较** + 一次节点分配（`BTreeMap` 节点是数组块，插入可能分裂）。
2. **迭代是指针追逐**：节点不连续，缓存命中率差。
3. **输出被排序**：`to_sml` 吐出来的键序 != 源序 ⇒ 文档类输出（emit / 人工 diff）全部错位。
4. **每条目额外开销**：`String` 24B + BTreeMap 节点摊销 ~24B + `Value` 的 tag/padding。

粗估每条目 **80–110 B**（以 25k 键语料实测校准），其中真正有用的键值数据约 50 B。

---

## 数据结构

```rust
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64, Option<String>),
    Str(String),
    Array(Vec<Value>),
    Object(Box<Object>),          // ← 唯一改动点：由 BTreeMap 换成 Object
}

pub struct Object {
    entries: Vec<Entry>,          // 插入序 == 源序；查找靠 index
    index: OnceLock<Index>,       // 惰性，首次 get 才建
}

struct Entry {
    key: Box<str>,                // 16B，比 String 省 8B（不可变键，无需 capacity）
    value: Value,
}

enum Index {
    Sorted(Box<[u32]>),           // 按 key 字节序排好的槽位
    Hash(HashIndex),              // 大对象：hash -> 槽位
}
```

**`Value` 大小约束**（加进 CI）：

```rust
const _: () = assert!(std::mem::size_of::<Value>() <= 48);
```

按各变体估算，`Float(f64, Option<String>)` 是最大变体（8 + 24 + tag ≈ 40），
加对齐后 `Value` 应为 40–48 B。**若超 48 B，先查 `Option<String>` 能否换成 `Option<Box<str>>`。**

---

## 不变式（实现与测试都要守）

1. **键唯一**：`entries` 内无重复 key。重复键在**解析期**已提升为数组（既有行为，不改）。
2. **索引一致**：`index` 一旦建立，必须与 `entries` 完全对应；任何 `&mut` 操作（`insert` / `remove` / 排序）必须先 `index = OnceLock::new()`。
3. **顺序稳定**：`insert` 追加到尾部；**覆盖已有键时原位替换值，不移动位置**（见 §6.3）。
4. **键序定义**：排序按 **UTF-8 字节序**（对合法 UTF-8 等价于码点序），避免 locale 差异。
5. **不可变后共享**：构造完成后 `Object` 不再变动 ⇒ `Value: Send + Sync` 成立（`OnceLock` 是 `Sync`）。

---

## 三档查找

| 条目数 | 策略 | 索引构建时机 | 单次查找代价 |
|---|---|---|---|
| `0..=8` | **线性扫描**比较 key | 不建 | ≤ 8 次短比较（通常 1 个 cacheline 内） |
| `9..=4096` | **排序下标表** `Box<[u32]>` + 二分 | 首次 `get` | ~log₂n 次比较（≤12），4 B/键 |
| `>4096` | **哈希** `hash(key) -> u32` 槽位，命中后校验 key | 首次 `get` | 1 次哈希 + 1 次比较，8–16 B/键 |

**为什么中间档用「排序下标」而不是哈希**

- 只花 4 B/键，哈希表要 8–16 B；
- **它顺带就是 canonical 顺序**：`to_sml_canonical` / 二进制编码直接按下标表遍历，不用二次排序；
- 无哈希种子、无 DoS 面、无「同 key 不同 hash」的调试噩梦（键是用户数据，哈希碰撞可被构造）。

**阈值 8 的依据**：典型配置块 2–6 个字段；线性扫描在 8 个以内通常快过任何索引（无哈希计算、无间接跳转）。**上线前用 bench 复核 4/8/16 三个点。**

**解析路径不建索引**：`push` 是 O(1) 摊还，全程零哈希、零比较、零节点分配。索引成本只在「首次读」付一次。

---

## 顺序语义（比容器更重要的部分）

### 顺序 = 语义

`to_sml` 按 `entries` 顺序输出；`Value` 的**相等性仍按集合**（顺序无关），
以保持既有语义与测试不变；需要顺序敏感时另给：

```rust
impl Value { pub fn eq_ordered(&self, other: &Value) -> bool; }
```

> 为什么不让 `PartialEq` 含顺序：现在所有 golden、契约校验、去重逻辑都按「map 相等」写；
> 改语义会一次性炸掉一大片，而收益只是「少一个方法名」。

### canonical 模式

确定性字节（哈希 / 签名 / 去重 / 二进制 / 页对齐）与「保文档顺序」是**两个需求**，必须两种输出：

```rust
pub struct DumpOptions { pub sort_keys: bool, .. }   // 默认 false（保序）
```

`sort_keys = true` 时按 §4.4 的字节序输出；二进制编码器**恒用 canonical**。

### 合并规则（必须写进规范，否则 merge 结果不确定）

| 操作 | 规则 |
|---|---|
| `&frag` 拼接 / include 合并，键**已存在** | **原位替换值**，位置不变（先出现的先占位） |
| 同上，键**不存在** | 追加到尾部 |
| 重复键（同一块内） | 解析期提升为数组（既有行为，不变） |

这条规则直接决定「两份文档合并后长什么样」，是 DSL/配置场景的高频操作，必须有测试钉住。

---

## typed block：契约定长槽位（二阶优化）

### 动机

有了 `@contract`，字段集合是**先验已知**的：

```sml
@contract Rule { action: enum [ allow deny ]  proto: enum [ tcp udp ]  port: int optional  source: str }
Rule ssh { action: allow  proto: tcp  port: 22  source: 10.0.0.0/8 }
```

此时不需要「字符串 → 槽位」的查找表：**槽位 = 契约字段序号**，一次数组下标即可。

### 布局

```rust
pub enum ObjectRepr {
    Map(Map),        // 无契约：§3 的有序条目数组
    Record(Record),  // 有契约：定长槽位 + 溢出区
}

pub struct Record {
    contract: ContractId,   // 指向契约表；布局由它决定
    slots: Box<[Value]>,    // 长度 = 契约字段数，顺序 = 契约声明序
    present: BitSet,        // 用户「显式写出」的字段位（与 GroupView::present 同口径）
    extra: Map,             // loose 字段 / 未声明字段（strict 下恒空）
}
```

**为什么需要 `present` 位图**：`default` 回填后，槽位里一定有值，但「用户到底写没写」是
`one_of` / `any_of` / `depends_on` 的判据（见 CHANGELOG「只认显式写出的键」）。位图 1 bit/字段，可忽略不计。

### 收益

| 维度 | Map | Record |
|---|---|---|
| 字段查找 | 字符串比较 / 哈希 | **一次下标**（`slot_of[field]`，解析期查一次） |
| 每实例内存 | 每字段 16B 键 + 条目开销 | 无键字符串，仅值 + 位图 |
| 校验 | 按遍历顺序 | 按 slot 顺序单遍 |
| 序列化 | 保源序 | 见 §7.4（**唯一的顺序例外**） |

### 顺序例外（需要拍板）

定长槽位天然按**契约声明序**，会丢掉源序。两个选项：

- **A（简单）**：规范规定「typed block 的输出顺序 = 契约声明序」。这是**全文唯一**不保源序的地方，写进文档即可。
- **B（保真）**：`Record` 额外存 `order: Box<[u8]>`（字段数 ≤ 255 时 1 B/字段），按源序输出。

建议 **A**：typed block 是「有 schema 的记录」，按 schema 输出反而更可预测；
真要保源序，用户用 `@is` + 普通块（Map 表示），那是保序路径。

### 契约变更 = 布局变更

`Record` 的槽位由契约决定 ⇒ 契约字段增删会改变布局。需要：

- `ContractId` 带版本（或内容哈希）；
- 二进制格式里记录 `ContractId` + 字段表（Avro 的 writer schema 思路）；
- 跨版本读时走「按名映射」慢路径（只发生在版本不匹配时）。

---

## C-ABI 契约（必须写进 `include/sml.h` 注释）

1. **指针有效期**：`sml_get*` / `sml_iter_*` 返回的 `const sml_value*` 在**树被释放**或
   **任何 mutating 调用**（若将来提供）之后失效。当前值树只读，等价于「与树同寿命」。
2. **迭代顺序**：按**源序**下标 `0..len`。C 侧不得假设按键排序（这是行为变更，需在头文件与
   CHANGELOG 标注）。
3. **不要缓存指针**：C 侧若需跨调用持有，应持有路径（`sml_get_in(root, "a.b[0]")`）或句柄。
4. **未来若加 mutation**：改用代际句柄 `struct sml_ref { uint32_t slot; uint32_t gen; }`，
   代际不匹配返回 `E-ABI-STALE`。**现在不做，但把字段位置预留出来**。
5. **线程**：值树 `Send + Sync`；跨线程只读安全（`OnceLock` 保证索引构建线程安全）。

---

## 性能与内存预算

| 项 | 估算 | 备注 |
|---|---|---|
| `size_of::<Value>()` | 40–48 B | CI 静态断言 |
| `Entry` | 16 + 48 = **64 B** | `Box<str>` + `Value` |
| 25k 键对象 | **1.6 MB** + 索引 100 KB | 排序下标表 4 B/键 |
| 现状 BTreeMap | 估 2.4–2.8 MB | 节点 + 分配器开销 |
| 解析 | 预期 **−15%～−25%** | 去掉 log n 比较与节点分配；以 bench 为准 |
| 查找（9–4096 键） | ~log₂n 次比较 | 与 BTreeMap 同量级，但比较对象更少（连续内存） |

**扩容**：解析器可用 token 数预估 `Vec::with_capacity`（`tokenize_spans` 已能给出上界）。
超大对象（>100k 键）若 memcpy 成为瓶颈，改用 **分块** `Vec<Box<[Entry; 64]>>`：块不搬移，
顺带给 ABI 一个「指针在块内有效」的稳定区。

---

## 迁移计划（每阶段可独立验收）

| 阶段 | 内容 | 验收 |
|---|---|---|
| **P0 接口先行** | 新增 `Object` + `Map` trait（`get` / `iter` / `insert` / `len` / `keys`），内部仍 `BTreeMap`；全仓改用 trait，不碰行为 | 全量测试绿、零行为差异 |
| **P1 换实现** | `Vec<Entry>` + 惰性索引；`to_sml` 保序 | golden 重录一次；bench 达 G2；fuzz 0 panic |
| **P2 canonical** | `DumpOptions::sort_keys` + 二进制编码器接排序下标表 | 同值同字节（哈希稳定）用例 |
| **P3 Record 布局** | typed block 走 `Record`；`present` 位图 | 契约用例全绿；`one_of` 等跨字段约束语义不变 |

> P0 是关键：它把「换容器」变成「换实现」，让 P1 的回滚成本降到一天。

---

## 备选方案与取舍

| 方案 | 结论 | 理由 |
|---|---|---|
| `IndexMap` | **可接受的 fallback** | 成熟、`get_index`、serde 齐活；代价 +2 依赖，破「零依赖」叙事 |
| `HashMap` | ❌ | 迭代序不确定（哈希种子），等于把不确定性写进格式 |
| `BTreeMap` + 旁挂 `Vec<String>` 记顺序 | ❌ | 双真相源；删键/覆盖/拼接必错位 |
| 指针表（存 `*const Value`） | ❌ | `Vec` 扩容即悬空；只能存下标 |
| `Rc<Node>` 链式对象树 | ❌ | 引用计数 + 指针追逐，比现状更慢 |
| `im::OrdMap` / `rpds` | ❌ | 保的是「键序」不是「插入序」，方向不对 |
| `Vec<(String, Value)>` 无索引 | ⚠️ 仅小对象 | ≤8 键时就是本方案的退化档，无需单独实现 |

---

## 验收判据（合并前必须全过）

- [ ] `size_of::<Value>() <= 48`（静态断言）
- [ ] 25k 键解析 **不快于** 现状就算失败；100k 键 ≤ 现状 × 0.85
- [ ] 25k 键峰值内存下降 ≥ 25%
- [ ] 语料全量 `parse(to_sml(v)) == v`，且 `to_sml` 输出**键序 == 源序**
- [ ] 新用例：`&frag` 覆盖（原位）/ 新增（尾部）/ include 合并 / 重复键提升
- [ ] canonical 用例：同值不同源的文档，`sort_keys=true` 输出逐字节相同
- [ ] C-ABI 用例：迭代顺序 == 源序；指针在树释放前有效
- [ ] fuzz 2 万条随机输入：0 panic、0 abort
- [ ] `docs/` 与 CHANGELOG 记录行为变更（**输出顺序变了，这是用户可见的**）

---

## 未决问题

1. **阈值**：线性/二分/哈希的切点（8 / 4096）需 bench 复核。
2. **typed block 输出序**：§7.4 选 A（契约序）还是 B（源序 permutation）。
3. **`SmallVec`**：小对象（≤3 键）占比高时是否值得引入一个依赖换掉一次堆分配。
4. **`#[non_exhaustive]`**：给 `Value` 加能避免下一次加字段的破坏性变更，但会让下游 `match` 必须带 `_`——现在加，还是 1.0 前加。
5. **兼容层**：是否保留 `Value::as_btree()`（deprecated）给老调用方过渡一个版本。
6. **二进制 canonical**：是否直接复用排序下标表作为输出序（省一次排序），需与二进制格式设计对齐。