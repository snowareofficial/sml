# Ideas — 想法记录

> 2026-10-25 · **记录专用，不动代码** / record only, no code changes
>
> 写法约定：英文在前、中文在后，一行说完。 / English first, Chinese after, one line each.

状态 / Status：`Done` 已落地 · `Partial` 部分落地 · `Todo` 未做 · `TBD` 待定 · `Blocked` 等确认

**Rule / 规矩**：任何一条**开工前**先附「理解待确认」，逐条问过用户再写代码。
Confirm the reading with the user before writing any code.

---

## I-1 自定义结构体的序列化 trait / trait-based serializer

- **Status / 状态**：`Done`（2026-10-27）—— 两半都落地：
  - 读：容器级 `#[sml(...)]`（`path` / `file` / `default` / `deny_unknown_fields` / `rename`）；
  - 写：`sml::serde::to_string_direct`（直出 SML 文本，**不经 `Value` 中转**，保留声明顺序；
    超 i64 的整数与 `Value` 同口径**拒收**，2026-11-02 起）。
  附带破坏性变更：`to_sml_value` 改返回 `Result`，`flatten` 非块由 `panic!` 改 `Err`。
  详见下方「决策（2026-10-26）」与「Progress」。
- **Idea / 想法**：给自定义结构体实现一个 trait，做到类似 serde-serializer 的效果。
- **Done / 已完成**：`SmlSerialize` / `SmlDeserialize`
  （`rust/src/derive_macro.rs`）；`#[derive(SmlSerialize, SmlDeserialize)]`
  （`rust/derive/`）；覆盖 struct / enum / newtype / tuple / `Option` / `Vec` /
  `BTreeMap` / `HashMap`；属性 `rename` / `default` / `skip` / `flatten` / `rename_all`；
  顶层函数 `to_string` / `from_str`（toml-rs 风格）；另有可选 `serde` feature 桥接。
- **Gap / 缺口**：现状是「每个类型各自生成 `to_sml_value` / `from_sml_value`」（代码生成），
  不是 serde 的「`Serializer` trait 驱动 + 类型只实现 `Serialize`」。真要 serde 形态，
  需新增 `Serializer` / `Deserializer` 一层 —— 属**能力扩展**，不是缺陷。
- **TBD / 待定**：保持现状只补文档，还是加 Serializer 层。
- **决策（2026-10-26）**：走「给 serde 补一个**直出 SML 文本**的 `Serializer`」
  （B1 子路线），另加「struct 标签」这条读方向需求，二者合并为 I-1 一次做完。
  - 前提修正：serde 双向桥（`ValueSerializer` / `ValueDeserializer`）**早就在**，
    不是缺口；真正缺的是「不经 `Value` 中转」的那一层。
  - 直出的三条真实收益：① 保留**字段声明顺序**（现走 `BTreeMap` 被字典序重排）；
    ② 支持 i128 / u128；③ 大 `u64` 保真。性能是次要的。
  - ⚠️ 约束：`to_sml` 在 `sml` feature 下且零依赖，**不能**让它依赖 serde，
    否则零依赖卖点没了 ⇒ 直出只能是**平行新增**的第二份发射器。
  - ⚠️ 约束 2：`dump.rs` 的排版由 `is_flat`（只看一层）决定，流式拿不到该信息
    ⇒ 直出必须**按容器缓冲子项**再决定排版，不是真流式。
- **Progress / 进展（2026-10-26）**：
  - ✅ 容器级属性（原只认 `rename_all`）：`path` / `file` / `default` /
    `deny_unknown_fields`（含 `= "strict"`）/ `rename`。
    测试 `rust/tests/derive_container.rs`（9 项）。
  - ✅ `SmlSerialize::to_sml_value` 改为**返回 `Result`**（与 `toml::to_string` 口径一致），
    `flatten` 目标非块由 `panic!` 改为 `Err` —— panic 不可接受。
    影响：trait 签名变更（破坏性），同步改了 `derive_macro.rs` 全部基础类型 impl、
    derive 生成代码、`macro_roundtrip` / `derive_demo` 调用点。
  - ✅ `sml::serde::to_string_direct`（`sml-value/src/ser_text.rs`，feature `serde` + `sml`）：
    不经 `Value` 中转，保留声明顺序、支持 i128 / u128 / 超 i64 的 `u64`。
    实现要点：`Node` 中间树（`Object` 是 `Vec` 不是 `BTreeMap`）+ 复刻 `dump.rs` 排版
    （共享 `quote_if_needed` / `format_float`），**靠字节一致性测试锁死两份发射器**。
    深度超限返回 `Err`（对齐 `to_sml_checked`，不像 `to_sml` 静默写占位符）。
    测试 `rust/tests/ser_de_direct.rs`（10 项）。
  - 📌 **已知差异（已写进文档）**：i128 / u128 / 超 i64 的 `u64` 直出可写，
    但 SML 回读按 `coerce_word` B10 规则得到 `Str` ⇒ **往返不对称**；
    浮点没有 `raw`，故不保留 `1e10` 这类书写形式。

## I-2 Rust 原生 contract，可选无视文件内契约 / native contracts with opt-out

- **Status / 状态**：`Done`（opt-out 开关，2026-11-02）；周边能力 `Done`。
- **Idea / 想法**：Rust 侧直接声明契约（struct / enum 映射出 SML 值），并**可选**忽略
  `.sml` 文件里写的 `@contract` / `@is`。
- **Done / 已完成**：`@contract` / `@is` 五端校验 + 默认值回填（Rust 为参考实现）；
  契约**组合**（字段类型是另一契约）与 **叠加 / mixin**（`@is a, b`，strict 优先）；
  外置扩展点 `sml_contract::ext`（`TypeCheck` / `ContractExt` / `TypeSpec::Ext`）；
  `sml_contract::infer` 从数据反推契约（`smltools --infer-contract`）；
  `ParseOptions`（`rust/sml-parse/src/ext.rs`）已承载外置指令/类型表与契约映射记录。
- **Done / 已完成（2026-11-02）**：`ParseOptions::apply_contracts(false)`（默认 `true`，向后兼容）
  —— 解析期跳过 `@is` / typed-block 的契约应用与校验（不填默认值、未知契约不报错），
  `@contract` 定义仍收集且**不进结果树**；子解析器（`@for` 体）继承该开关。
  覆盖点：块闭合 `apply_contract(s)`、`@is` 逐词 resolve、typed-block 前缀（`finish_bare_block`）。
  用例 `rust/tests/contract_optout.rs`（4 条）。
  残余：文件入口 `parse_file` 暂不接收 `ParseOptions`，opt-out 目前只能走文本入口 `parse_with`。
- **Note / 备注**：「用 struct / enum 当契约来源」＝**由图派生契约**，
  与 `infer`（从数据反推）是两件事，别混。

## I-3 `get_object` 之类的方法？ / object accessors?

- **Status / 状态**：`Done`（2026-10-27，**待合入 `dev`**）—— 访问器层已补齐。
- **Answer / 答案**：**没有**。`Value` 只有 `get(path)`（点路径）、`as_str`、`as_float`
  （`rust/sml-value/src/value.rs`）。
- **Done / 已落地（2026-10-27，分支 `feat/value-accessors`）**：新增
  `as_object` / `as_array` / `as_i64` / `as_bool` / `as_object_mut` / `as_array_mut`
  与 `iter` / `len` / `is_empty`；`get()` 另支持 `[idx]` 下标（可链式、可后再接点路径）。
  ⚠️ 副作用与处置：`get()` 属**既有行为变更** —— 已加「字面键名优先」兜底
  （键名本身含 `[` 的文档不从 `Some` 变 `None`），并由回归测试
  `bracket_in_key_name_is_not_rewritten` 钉住。
- **Done / 已完成（更早）**：C-ABI 侧接口不缺 —— `sml_get` / `sml_get_path` / `sml_key_at` /
  `sml_str_dup` / `sml_int_value` / `sml_bool_value`（`rust/src/c_abi.rs`）。
- **Gap / 缺口**：Rust 原生侧反而更薄，缺 `as_object` / `as_array` / `as_i64` / `as_bool`
  （＋ `as_object_mut`）；补这一层零风险。
- **Note / 备注**：~~`get` 只走 `Object`，遇数组下标即 `None`~~ —— 2026-10-27 起下标可用
  （`a.b[0].c`），但**字面键名优先**于下标语法，故路径里带 `[` 的既有键不受影响。

## I-4 注释双语化，英文在前 / bilingual comments, English first

- **Status / 状态**：`Partial`（2026-10-31 开工）—— 本仓库注释**大体已是**「English ｜ 中文」，
  这一条是把**未达标的**（纯中文或格式不整）补成统一约定，**单独一笔**收尾。
- **Convention / 约定**：`English ｜ 中文`，一行说完，不写第二段。
- **Scope / 范围**：先公开 API 注释，再按文件推进；一次只改一类，便于 review。
- **纪律 / 2026-10-31**：此笔**只动注释、不动逻辑**；改动与任何逻辑 diff 拆开独立提交。
- **进展（2026-10-31 第一批，核心公开面）**：✅ `sml-value/src/lib.rs` 模块 doc + 公开项注解
  已由纯中文改为「English ｜ 中文」双语（中文照留，符合「还是支持中文」）；
  ✅ **第二批（2026-10-31）**：`sml-parse/src/api.rs` 全部公开函数摘要 + `rust/src/lib.rs`
  的模块头 / re-export / serde 公开项 已双语化；编译通过、无诊断。
  🔍 ✅ **第三批（2026-10-31）**：`sml-value/src/value.rs` 全部公开项摘要
  （`Value` 变体 `Object`、`float`/`float_with_raw`/`as_float`/`float_raw`/`as_str`/
  `as_object*`/`as_array*`/`as_i64`/`as_bool`/`iter`/`len`/`is_empty`/`get`/
  `sort_keys_deep`/`sorted_by_key`/`Drop::iterative`）首行改为「English ｜ 中文」双语；
  示例/详解中文照留。无源级诊断。待后续批次：`object.rs`、`smltools` 公开面。
  ⚠️ 复核中发现既有 feature 组合裂口：`preserve_order` + `serde` 下
  `serde_bridge.rs` 用 `pop_first`/`Into<BTreeMap>` 与 `Map=Object` 冲突（非本轮引入）。
  - **✅ 已排雷（2026-10-31）**：修 `serde_bridge.rs` —— `MapDeserializer.map` 字段类型
    `BTreeMap`→`Map`、`pop_first`→`IntoIterator`（`mem::take`+`collect` 逐弹）、enum 单键
    解构 `pop_first`→`keys().next()+remove`、各处 `Value::Object(btree)`→`.into()`。
    语义：默认档维持字母序（与旧 `pop_first` 一致，无回归）；`preserve_order` 档反序列化
    改按**源序**喂 visitor（更对齐）。⚙️ 性能：`MapDeserializer` 由 O(log n) 逐弹变为
    O(n) take+collect，serde 反序列化非热路径，暂未跑基准。
    ✅ 验证：`preserve_order+serde` 下 `preserve_order` 14/14、`serde_bridge` 10/10 全绿；
    默认档 `serde_bridge` 10/10 全绿。
- **异步与多线程（2026-10-31，用户「先记着」，待二次确认方向）**：
  - 现状：`Value`/`Map`/`Object` 均为 **`Send + Sync`**（自动推断）；索引用 `std::sync::OnceLock`
    而非自身持锁 ⇒ 只读并发（多线程 `get`/`to_sml`）**无互斥、天然并行**，async 下是普通 `Send`
    数据、无阻塞点。
  - 缺口：并发**写**需 `&mut self`，`Object` 不内建锁（对标 std `BTreeMap`，锁归调用方）；
    无内部异步协调（这层也无必要 —— 无 IO 无锁）。
  - 可优化方向（未动手，等用户定）：A) 索引预热 / 并发建索引（收益在噪声带，需基准）；
    B) `Arc<Value>` 共享读（调用方 DIY，非 Value 强制）；C) 内建分片锁（破坏零依赖，不建议）。
  - **用户尚未裁决要不要动**，仅存档。

---

## I-5 weakport：插件认契约，库忽略 / plugin-only contracts

- **Status / 状态**：`Todo` —— 没有这个语法；最接近的两块能力都在。
- **Idea / 想法**：给插件用的通道 —— 插件（宿主）认 `contract`，**库侧忽略**，不校验、不报错。
- **Done / 已完成（可复用的零件）**：外置指令的 `Outcome::Discard` = 元数据块不进主树
  （`rust/sml-parse/src/ext.rs`）；`ParseOptions` 能挂外置指令表与外置类型表；
  契约映射记录 `trace_contracts` 可让宿主自己看「谁挂了哪个契约」。
- **Gap / 缺口**：没有「按块/按指令声明契约、但跳过校验」的形态；现有 `@is` 一律**强制**校验，
  且 `@is` 前必须有已定义契约（否则 `E-CONTRACT-001`）。
- **Design / 思路**：`weakport` 落在**外置指令**侧最省事 —— 宿主注册一条指令，
  返回 `Discard`，库只当元数据；不必改契约层，也不必改词法。
- **用户修订（2026-10-31，影响 Status）**：这个名字更该叫 **`weakimport`** ——
  **应用层自己设契约**、由应用层注册/解释，控制权在**应用层**，
  不是给普通数据用的旁路。weakport 对插件来说**看起来就是普通数据**（库侧忽略，
  插件侧按应用层给的契约解释）。
  - ⚠️ **可内置 ≠ 不优化**：即使落成「内置/外置指令 + 库忽略」，语义上仍要保证
    **弱绑定不受编译期丢弃** —— 之后 `smlb` 编译时**不会忽略 `weakport`/`weakimport`**
    （即它是真数据通道，不是被 `cfg`/feature 裁掉的可丢弃物）。措辞与实现都别把它
    写成「随手 Discard」——那只适合「纯元数据」，不适合「插件要真读」的通道。
  - ⚠️ 因此与 I-15 `weakdata` 的边界需重画：两者都是「库忽略、交宿主」，
    但 **`weakimport` 带契约**（应用层设契约来解释数据）、**`weakdata` 纯数据**；
    且 weakimport 强调「不被编译期忽略」的保真，weakdata 强调「不进主树」。
    落地前先划清，别造近义术语。

## I-6 featag：带参的块头标签 / param'd header tags

> ⛔ **Before implementing: confirm the reading below with the user first.**
> **实现前必须先逐条确认下方「理解待确认」，不得自行开工。**
>
> Status / 状态：`Blocked` —— 等确认；未确认不开工。**用户补描述中（2026-10-31）**。

- **Idea / 想法**：类似函数，形如 `rectangle size 2`；在块内**填空**（`{}` 内级），
  「不止填充」；前提是目标 contract 已声明、`typed-block` 已开启、featag 事先声明。
- **用户补充（2026-10-31，见下「用户修订」）**：featag 可以**应用层内置**
  （不经核心词法/内置换），并**选择不参与语料处理** —— 即它不是 SML 语法层全局之物，
  是应用层声明、应用层解释的标签形态。**全量描述由用户补写中，未定稿，勿开工**。

### Understanding to confirm / 理解待确认（我的读法，待你点头或纠正）

块头从左到右四个槽位 / four slots, left to right:

```
<featag 名 = 契约名>  <args…>  <块名>  <其他契约 other contract…>  { … }
rectangle            size 2    rect1   tls                      { … }
```

- **Q1 槽位顺序**：`契约名 → args → 块名 → 其他契约`，这个顺序对吗？
  （我上一轮按 `契约名 → 块名 → args` 理解，被否 ⇒ 现已改成上面这版。）
- **Q2 args 形态**：`size 2` 是「名字 + 值」的**命名参数**（对应 `rectangle(size=2)`）吗？
  可多组（`size 2 color red`）？值可以是块/数组/字符串吗？
- **Q3 可省槽位**：args / 块名 / 其他契约 三者哪些可省略？省了怎么消除歧义
  （现在靠「首词命中契约表」判定，args 里的词若也是契约名会撞车）？
- **Q4 「其他契约」语义**：是**叠加 / mixin**（同块多份约束，字段取并集，strict 优先），
  还是只表示「另一个可引用的目标」？
- **Q5 featag 声明**：用 `@featag rectangle { … }` 这种新内置指令吗？声明里写什么 ——
  参数表 / 字段模板 / 默认值 / 展开规则？
- **Q6 「填空（{} 内级）」具体指**：往块里**插字段**？**改字段值**？还是**校验后补默认值**？
  「不止填充」（beyond filling）还包括什么 —— 展开成多个字段？改名？条件插入？
- **Q7 与既有机制的边界**：它和 `&frag` 行级 splice、`@when` / `@for`、`@is` 叠加
  各自负责什么？重叠部分以谁为准？
- **Q8 门控**：沿用 `typed-block` + `contract` 双门控，还是给 featag 单独一个特性名？

### Rules now / 现有规则（实现时可复用，`rust/sml-parse/src/parser.rs::parse_bare_block`）

- 形状与裸块同形：`<契约名…> [块名] [args…] { }`，首词命中契约表才生效；
- 双门控：需 `typed-block` **且** `contract`（`Feature::TypedBlock` / `Feature::Contract`）；
- 契约须**先声明**，按裸名→当前命名空间前缀→`case-insensitive` 兜底解析（`E-CONTRACT-001`）；
- 首个未命中契约的词 = 块名；未开 `block-name` 时多出的词**直接报错**（不静默当块名）；
- 未命中契约表时行为不变：只写 `__type` / `__name` / `__args` 元数据；
- 单契约逐字保持旧行为，**契约词 ≥ 2** 才走叠加（`@is` 是 `a, b`）。
- 备注：`BUILTIN_DIRECTIVES`（`ext.rs`）是硬表，新增内置名须同步五端文案。

## I-7 契约约束的精确控制 / 让数据是我们想要的样子 · precise shape control

- **Status / 状态**：`Done` —— **计数三件套与跨字段依赖已齐（①②③）**；**④ 三态 + 块级覆盖已做**，仅剩 `x-*` 按前缀放行额外键留待下一棒；**⑤ `Warning` 档已加**。
  （严格性的 warn/silent 三态、诊断缺 `Warning` 档）。
- **Idea / 想法**：不止「拼错字段」这一件事，还要能表达**字段缺失 / 最小 / 最多 / 互斥**等约束，
  目标是让数据长成我们想要的样子，而不是只有 `loose` ↔ `strict` 两个极端。
- **Done / 已完成（可直接用）**：
  - 字段缺失：`required` / `optional` / `default` 三态（`FieldSpec`）；
  - 数值最小 / 最大：类型区间（`TypeSpec` 的 min/max），数组**逐元素**校验；
  - 数组元素**个数**上限：外置修饰符 `items_max`（`sml_contract::ext` 的 Modifier）；
  - **互斥字段组**（2026-10-28 落地）：外置修饰符 `one_of [ a b c ]`（至多一个）与
    `exactly_one [ a b c ]`（恰好一个），实现为 `sml_contract::OneOf` / `sml_contract::ExactlyOne`，
    经 `ParseOptions::with_modifier` 注册即用；判据看**用户显式写出的键**（`default` 回填不算），
    组成员名写错会响亮报错；    报码复用 `E-CONTRACT-011`。新增钩子 `Modifier::check_group(&GroupView)`；
  - 数组元素**个数下限**（2026-10-30 落地）：外置修饰符 `items_min <n>`（`sml_contract::ItemsMin`），
    与文档示例里的 `items_max` 成对；
  - **「至少一个字段」**（2026-10-30 落地）：外置修饰符 `any_of [ a b c ]`（`sml_contract::AnyOf`），
    与上面的 `one_of` / `exactly_one` 凑成完整的**至多一 / 恰好一 / 至少一**三件套：

    | 修饰符 | 0 个 | 1 个 | ≥2 个 |
    |---|---|---|---|
    | `one_of` | 通过 | 通过 | 报错 |
    | `exactly_one` | 报错 | 通过 | 报错 |
    | `any_of` | 报错 | 通过 | 通过 |

  - **跨字段依赖**（2026-10-30 落地）：外置修饰符 `depends_on b` / `depends_on [ b c ]`
    （`sml_contract::DependsOn`）。挂在**依赖方**字段上：**本字段出现 ⇒ 所列字段必须出现；
    本字段未出现 ⇒ 恒通过**（本条不约束依赖目标单独出现）。
    ⚠️ 刻意**不叫 `requires`** —— 仓库里已有块级指令 `@requires`，那是**特性门控**
    （应用契约前必须已开启某 `@feature`），与本修饰符的「字段陪伴要求」是两回事；
    同名会让读者分不清 `@requires X` 里的 `X` 是特性名还是字段名。
  - 枚举取值、嵌套契约组合、叠加 / mixin（strict 优先）。
- **语义取舍（② ③ 的边界，均已写在 `sml-contract/src/ext.rs` 的文档注释里）**：
  - `any_of`：一个都没有 ⇒ 报错；有多个 ⇒ 通过（这是它与 `one_of` 的分水岭）；
  - `depends_on`：依赖目标缺失 ⇒ 报错；**依赖方自己缺失 ⇒ 通过**（缺席与否是
    `required` / `optional` / `default` 三态的活，两个语义不互相代劳）；
  - 与 `default` 回填的交互：跨字段约束只认用户**显式写出**的键（`GroupView::present`，
    `default` 回填的值不计）⇒ **组内成员全靠 `default` 时 `any_of` 仍算「一个都没有」并报错**、
    **依赖目标只靠 `default` 时 `depends_on` 同样报错**。这是**刻意**的 —— 作者给的兜底策略
    不是「用户确实填了一种联系方式」的意图表达，**别当成 bug 修**；
  - `items_min` 与互斥组的口径**不同**：它走 `Modifier::check`，只见这一个字段的**生效值**
    （无从得知是否 `default` 回填），且回填后的值就是要往下流的数据 —— 它不满足约束必须报。
    当前契约语法里 `default` 只接受裸词 / 引号串，数组字段**写不出** `default [ ... ]`，
    故该交互暂不可触发（有用例钉住，将来支持数组默认值时要回来重订）。
- **Gap / 缺口**：
  ① ~~**互斥**（`oneOf` / 至多一个 / 互斥字段组）~~ **已完成 2026-10-28**（`one_of` / `exactly_one`，见上 Done）；
  ② ~~数组**长度下限**（`items_min`）与「至少一个字段」~~ **已完成 2026-10-30**（`items_min` / `any_of`，见上 Done）；
  ③ ~~跨字段依赖（`a` 存在则 `b` 必须存在）~~ **已完成 2026-10-30**（`depends_on`，见上 Done）；
  ④ 严格性三态（strict / warn / loose）与块级覆盖（`@is X warn|loose`）**已完成**；
  ⏳ 仍缺「按前缀放行额外键」（如 `x-*`）—— 属新语法、需五端同步，留待下一棒。
  ⑤ 诊断已新增 `DiagnosticKind::Warning`（与 `Deprecated` / `DuplicateKey` / `KeyOverwritten` 分工明确）—— 已完成。
- **Design / 思路**：三态最划算 —— 复用 `Diagnostic` 加 `Warning` 档，`loose` 保留为静默放行，
  新增 `warn`；顺序与叠加的 strict 优先级规则要一并写清。互斥/计数这类先做成
  Modifier（外置），再考虑升为内置语法 —— 这样不必五端同时改。
- **下一棒 / 待确认（2026-10-30 ④⑤ 落地时补记）**：本棒按「Rust 先行」交付，下面 6 条
  **尚未定论或未做**，别当成已完成：
  1. **推断 JSON 字段名变了**：`allow_extra: bool` ⇒ `Strictness` 后，`to_json` 由 `"loose": true`
     变为 `"strictness": "loose"`（`rust/sml-contract/src/infer.rs` 的 `to_json`），影响
     `smltools infer-contract --json`。若 C / JS / Lua 侧有对应黄金用例需同步 —— **待拍板：
     这个格式变更是否接受**。
  2. **`ContractWarning` 是新建中间类型**：`sml-contract` 若依赖 `sml-parse` 会成环（现状是
     `sml-parse → sml-contract`），故无法真正复用 `Diagnostic`，改为在 `sml-contract` 定义
     `ContractWarning`，由 parser 转译成 `Diagnostic{kind: Warning}`。这属于「不新建就会成环」，
     不是「不改就表达不了」—— **待拍板：是否把 `Diagnostic` 下沉到 `sml-codes` 或新共享 crate**。
  3. **告警 `line` 目前是 `None`**：`apply_contract` 在 `sml-contract` 里拿不到 token 行号。
     要定位到具体行须把行号信息透传进 `sml-contract` —— 下一棒（本棒测试未断言 `line`）。
  4. **块级覆盖的绑定规则是本棒自定的**：覆盖绑定在**紧邻其前**的那个契约名上 ——
     `@is A warn B` ⇒ A=warn、B 默认；`@is A B warn` ⇒ A 默认、B=warn。**待确认**是否要
     改成「整条 `@is` 统一下降一档」（语法不变、语义不同）。
  5. 契约级 `strict` 关键字保留（`@contract C strict`，与默认等价），沿用既有显式写法。
  6. **`x-*` 按前缀放行额外键本棒未做**（D5）：属新语法、需五端同步，留待下一棒 —— 见上面 ④。

## I-8 simple TOML-style support / 简易 TOML 风格

- **Status / 状态**：`Todo`（`=` 分隔）；`[name]` 作块名 **搁置**（用户已定）。
- **Idea / 想法**：支持 TOML 风格写法，降低迁移成本。
- **Now / 现状**：`=` **不是 token**，会被吞进裸词 —— `x = 1` 解析成键 `"x"`＝`"="`、
  再凭空多出键 `"1"`（`rust/STRUCT_SERDE.md` 有实测），静默错树。
  `[ ]` 是**数组**语法，所以 `[name]` 当块名与既有语法直接冲突。
- **Gap / 缺口**：做 `=` 要**动词法**（五端词法表：`rust/sml-lex`、`c/sml.c`、`cpp/sml.cpp`、
  `js/sml.mjs`、`lua/lib/sml.soup`），并按 `@feature` 惯例给一个 opt-in 名字
  （如 `toml-eq`），默认关闭以保向后兼容。
- **Risk / 风险**：`x = 1` 现在是「静默错树」，开特性前必须先明确「特性未开启时是否报错」，
  否则旧文档的行为会因新 token 而变。

## I-9 近似契约名扫描与标注 / near-miss contract name hints

- **Status / 状态**：`Done`（2026-10-27）—— 运行时已落地；**编辑器侧仍未做**。
- **Idea / 想法**：扫描「可能和 contract 名相似」的写法（如 `Securty` vs `Security`），
  当作**可能的错误**标出来，而不是只报一句「未定义的契约」。
- **Done / 已落地（2026-10-27）**：`e16c54e` —— 未命中契约时在错误文案里追加
  「你是不是想写 `X`？」（`X` 为候选契约名）。改 `sml-contract/src/lib.rs` +
  `sml-parse/src/parser.rs`，新增 `rust/tests/contract_typo.rs`（4 条测试）。
  **码不变**（仍是 `E-CONTRACT-001`）⇒ 五端可分批跟进。**仍未做**：编辑器侧标黄
  （`editors/lsp`、VSCode）、外置契约类型与外置指令名进候选池。
- **Now / 现状（此为落地前的情况，留作对照）**：只有 `contract_by_first_letter` 的首字母兜底（`case-insensitive` 特性，
  `rust/sml-parse/src/parser.rs`）；未命中即 `E-CONTRACT-001`，文案不含候选；
  全仓无编辑距离 / did-you-mean 实现（`levenshtein` / `edit_distance` 零命中）。
  `typed-block` 已把「拼错的契约名被静默当块名」修掉（未开 `block-name` 直接报错），
  但**报错只说"不是已定义的契约"，不给提示**。
- **Design / 思路**：编辑距离 ≤ 2（或 ≤ 长度/3）＋ 同前缀 ＋ 大小写差异 → 文案追加
  `你是不是想写 `Security`？`；编辑器侧（`editors/lsp`、VSCode）同样标黄。
  注意：**码不变**（仍 `E-CONTRACT-001`），只加候选文案 ⇒ 五端可分批跟进，不必同日。
- **Note / 备注**：外置契约类型（`image` / `link` / `time`）与外置指令名也应纳入候选池，
  否则只提内置名会误导。
- **决策（2026-10-27，用户定）：运行时只加中文，选 C**
  备选：A 运行时内联双语 / B 同步 `codes.sml` + `msg.en.json` / C 仅运行时中文 / D C + 静态描述同步。
  定 C 的三条理由：
  1. **候选文案是动态的**（含具体契约名），静态目录装不下 ——
     `codes.sml` / `msg.en.json` 只能承载「本码会给出候选」这种静态描述，
     承载不了「你是不是想写 `Security`？」这条文本本身 ⇒ B 的字面做法不可行。
  2. **Rust 运行时目前没有英文层**：`msg.en.json` 不进二进制，它是给网站/JS 用的，
     且是 `gen_js_errors.py` 的输入。要在运行时出英文 = 新建一层 i18n ⇒ 那是
     **I-11「先数据化，再谈剥离」** 的范围，价值表写明「分两步，别一步到位」。
  3. **契约名本身是 Latin**，中英文读者都看得懂；且全仓运行时（含 C 端先例）
     都是中文硬编码 —— A 会插入一条既不属于中文层也不属于英文层的混合串，
     等于给 I-11 预支技术债。
  ⇒ 英文侧的覆盖留给 I-11。若要让英文目录也知道此功能，另起一笔只改**静态描述**，
  并跑 `gen_codes.py` + `gen_json.py` + `gen_js_errors.py`（别和功能改动混在一笔）。

## I-10 团队搭建与宣传 / team & outreach

- **Status / 状态**：`Partial`（2026-10-27）—— **前置（能力矩阵）已产出，入口文档仍未做**。
- **Idea / 想法**：项目要从「单人 + AI」长成有团队的样子，需要一套**倡议**（怎么参与、从哪下手）
  与**宣传**（怎么让人知道、为什么值得看）。
- **Now / 现状**：`.github/` 只有两个 workflow（`ci.yml`、`sync-from-gitee.yml`）——
  **没有 `CONTRIBUTING.md`、没有 issue / PR 模板、没有 good-first-issue 分类**；
  徽章有（`badge/`、`tools/gen_badge.py`）；对外文档有 README 中英 + 教科书 + 示例；
  crates.io 的 `description` / `keywords` 专门按「5 个 × 20 字符」上限调过。
- **Gap / 缺口**：缺**入口级**文档（3 分钟上手 + 「为什么不用 JSON/YAML/TOML」一页）；
  缺 benchmark（体积 / 可读性 / 解析速度对比）；缺 issue 引导与公开 roadmap。
- **Risk / 风险**：宣传口径必须与五端能力矩阵对齐 —— **不得**宣传只有 Rust 有的
  `@feature` / `@when` / `typed-block`（见 `TODO.md` §7.3 的实测差异）。
- **Order / 顺序**：先有能力矩阵 → 再有宣传语；反过来就是长期负债。
- **Progress / 进展（2026-10-27，分支 `feat/capability-matrix`）**：矩阵已产出 ——
  `docs/capability-matrix.md`（每格附实跑命令与输出）。**它同时挖出三条此前不在任何清单上的
  静默缺陷**（Lua 大整数丢精度 / C++ 无 include / JS `\uXXXX` 少跳一位），已登记为 `TODO.md` 雷 6。
  ⚠️ 该文档 §5 列明的差异项在修完之前**不得**写成「SML 支持」。
  **仍未做**：`.github/` 的 CONTRIBUTING / issue 模板 / PR 模板（本轮那条任务没起）。

## I-11 中文支持完善，且可剥离 / zh support, detachable

- **Status / 状态**：`Partial`（中文能用）；查表层 `Done`（2026-10-31 复核）；**剥离（表层）= Done**（2026-11-02，`sml-codes` 的 `zh` feature）；**调用侧硬编码中文 → 改走文案表**：**十二批已落地（Rust 侧清零，2026-11-02）**——第一批 `SmlError::from_code_title`/`from_code_msg`/`with_detail` + `smltools` 4 处静态文案；第二批 `sml-include` **全量 24 处**；第三批 `sml-lex` **全量 13 处**；第四批 `sml-parse/src/scan.rs` **11 处**；第五批 `sml-parse/src/cond.rs` + `api.rs` **25 处**；第六批 `sml-parse/src/parser.rs` **结构族 6 处**；第七批 `parser.rs` **契约/指令/类型族 9 处**；第八批 `parser.rs` **片段/扩展指令族 13 处**；第九批 `parser.rs` **收尾 31 处**；第十批 `sml-contract` **全量 22 处**；第十一批 `swsml` emit 层 **8 处**；第十二批 `smltools` **全量 37 处**（按决策 **B**：语言随 `sml-codes` 的 `zh` feature，动态细节仍经 `with_detail` 保留）。`sml-lex` / `sml-parse` / `sml-contract` / `sml-include` / `swsml` emit / `smltools` 的报错文案**已全部走码表**；剩余仅非 Rust 端（JS/C/Lua 的硬编码文案）若要做需另开一轮。
- **Idea / 想法**：中文支持再补完善；同时把中文这一层做成**可剥离**的
  （英文用户 / 嵌入式场景可以不带，只留语言中立的错误码）。
- **Now / 现状**：
  - 中文是**源语言**：错误文案硬编码在 Rust 各 crate 的 `format!` 里，没有语言层；
  - 英文层**只有 JS**：`errors/msg.en.json` →（`errors/gen_js_errors.py`）→
    `js/sml-errors.mjs`；Rust 侧此前**完全不读** `msg.en.json`，但 **2026-10-31 复核已变**：
    `sml-codes` 已有 `msgs::error_info` 查表（`zh_title/zh_msg` + `en_title/en_msg` 双列、
    `errors/gen_js_errors.py` 从 `site/static/errors.json` + `errors/msg.en.json` 生成），
    `SmlError::render_locale("en")` 已能取英文规范文案、缺英文回退中文 ⇒ **查表层具备**；
  - 中文标识符 / 键名可用（编辑器与路径处理按 `\p{L}`），BOM 五端已统一跳过；
  - 术语表、全角标点 / 数字排版没有约定。
- **Gap / 缺口**：① 想把「码 → 文案」数据化（`errors/codes.sml` 已是中文唯一来源，
  但它同时被当作文档与码表用，职责混着）—— **查表层已数据化（`msgs.rs`）**，
  真正剩的是**调用侧逐步接上 `render_locale` 语义**与「句级上下文细节」的整句翻译；
  ② Rust 无 `zh` / `en` 切换 —— **`render_locale(locale)` 已提供按 locale 出口**，
  但**硬编码 `format!` 里直拼中文的调用点**仍是「仅中文」，未逐一改走模板，属待办
  （2026-11-02 第一批：新增 `from_code_title` / `from_code_msg` / `with_detail`，先把
  `smltools` 的 4 处静态文案改走表；其余调用点按同一模式渐进）；
  ③ 无法「不带中文」构建 —— ✅ **表层已做**（2026-11-02）：`sml-codes` 增 `[features] default = ["zh"]`，
  `--no-default-features` 时 `msgs.rs` 的中文列整列不编译、`error_info` 一律走英文
  （缺英文退回码本身），`render_locale` 剥离后对任何 locale 都取英文；子 crate 本就用
  `default-features = false` 依赖它 ⇒ 天然不带中文。⚠️ 但各 crate 里 `format!` 直拼的中文**仍在**
  （第 ② 项），要「零中文二进制」还需把调用侧逐步改走文案表。
- **Design / 思路**：先数据化再剥离 —— 把中英文案都做成表（`msg.zh.json` / `msg.en.json`），
  代码只按码取串；然后 `sml-codes` 按 feature 分层（`zh` 默认 / `en`），
  `idyml` 只开 `en`。**查表层已数据化**（`msgs.rs` 两列齐备），下一步是把
  «各 crate 直拼中文的 `SmlError` / `String` 文案调用点» 逐处确认：能走码的走码，
  需保留动态上下文的在码模板外拼接 —— **剥离是最后一步，别一步到位**。
- **Risk / 风险**：文案一致性（五端 + JS 生成物）与「两处手维护」（`codes.sml` 中文、
  `msg.en.json` 英文）会放大漂移风险；改前先补 `--check` 闸门。

## I-12 改名工作（会多出一个新 crate）/ rename spawns a new crate

- **Status / 状态**：`TBD` —— 名称未定，影响面已可列。
- **Idea / 想法**：项目 / 包名要改；改名后**会多出一个新生的 crate**（新名占位）。
- **Now / 现状**：包名 `swsml`（crates.io 上 `sml-rs` 被无关项目占用），lib 名仍是 `sml`；
  `IDYML.md` 已规划 `idyml` 作**英文稳定别名**（瘦别名 `pub use swsml::*`，不复制实现）；
  仓库名 `sml`（Gitee 权威源 + GitHub 镜像），域名 `sml.swebase.cn`。
- **Impact / 影响面**（改名要一并动）：`Cargo.toml` 的 `name` 与 `[lib] name`、
  `use sml::...` 的兼容（只能靠 alias 保住）、README / 徽章 / 站点下载链接、
  `packaging/` 与 `debian/` 的 SONAME（`libswsml.so.0`）、C-ABI 头 `sml.h`、两个远端仓库、域名。
- **Decision / 决策点**：新增 alias crate（不破坏老用户）还是迁移 + 弃用期？
  新名字必须在 crates.io **先抢注** —— 这就是改名真正多出来的那个 crate。
- **Risk / 风险**：crate 名是**永久占位**，改错要陪跑整个 1.x；SONAME / 头文件名一改，
  已链接的下游直接坏，必须给共存期。
- **Name clash / 重名（2026-10-25 实测）**：`idyml` 与已存在的 **`yml`**（v0.1.0、下载 265、
  描述 `"A yml or yaml"`、无 repository）近似，`cargo add` 易手误、搜索被词根稀释；
  `idyml` 本身在 crates.io **未占用**（404）。crates.io 不校验名字相似度 ⇒
  「能注册」≠「不会混」。详见 `IDYML.md` 的「命名风险」一节。

## I-13 实现即文档，闪光点要看得见 / docs while implementing, make the highlights visible

- **Status / 状态**：`Partial`（2026-10-31 开工）—— 落地第一个「语义 + 例子 + 陷阱」示范；
  流程承诺随后续每次交付逐步兑现。
- **示范 / 2026-10-31**：为 `sml-value::Value::eq_ordered` 补了**可跑 doctest**（顺序敏感相等：
  声明 `PartialEq` 不含顺序的「陷阱」，配例子，两个 feature 档 `--doc` 均通过）；
  **再补 `insert_or_merge`**（合并规则入口：键存在不动位置调 merge / 键不存在追加尾部不调 merge，
  配可跑例子）。后续每落一个能力按此收尾。
- **Idea / 想法**：实现过程中产出的那些解释（语义对比表、反例、边界）本身就是**很好的教程素材**，
  应在实现时就顺手沉淀，而不是等做完再补 —— 人们要先看到**闪光点**才会点进来。
- **Now / 现状**：文档是**事后补**的（`STRUCT_SERDE.md` / `IDEAS.md` 都是补记）；
  `site/` 有教科书与示例、README 中英双份，但缺「能力 → 例子」的**可发现路径**。
- **Gap / 缺口**：① 实现时写的语义说明（如 flatten 的「整个父块 vs 剩余键」对比、
  panic vs Err 的取舍）散落在对话与代码注释里，没进文档；
  ② 没有「一条能力配一个可跑例子」的沉淀位置。
- **Design / 思路**：改**流程** —— 每落地一个能力，同步补「语义 + 例子 + 陷阱」三件套；
  位置就近（模块文档 / `site/` 教程页），不另起文档体系。
- **Risk / 风险**：文档与实现脱节（写的时候对、后来改了）；
  对策是**例子必须可跑**（进 doctest 或测试），不写跑不起来的示例。

## I-14 仓库排雷行动 / repo-wide landmine sweep

- **Status / 状态**：`Partial`（2026-10-27 二轮）—— 清单已出四类（雷 3/4/5/6），**改代码做了两批**。
  - ✅ 雷 5 A 组（`c4077a0`）：derive 大整数超 i64 一律报错，与 serde 桥同码 `E-DERIVE-002`；
  - ✅ 雷 3「与库分叉」类（`3bff9b0`）：六条消四条，`depth_of` 与文本扫描识别未动；
  - 🆕 新增**雷 6**（`docs/capability-matrix.md` 挖出，三条跨实现静默缺陷，**未动**）；
  - ⏳ 雷 1 根因（迭代式解析）、雷 2（坏样例）、雷 3 的崩用户类与冗余类、雷 5 A4-6 与 B 组 —— 仍未动。
- **Idea / 想法**：做一次**系统性**扫描，把「看起来在测、实际没测」「静默改数据」
  「panic / 栈溢出」这类**隐性雷**一次性挖出来，而不是等实现时偶然踩到。
- **Why / 由来**：I-1 施工时顺手撞上 `deep_nesting_exceeding_limit_is_rejected`
  —— 一条**本意是防栈溢出、自己却栈溢出**的测试（已记在 `TODO.md` 的排雷区）。
  ⇒ **「测试全绿」不等于「验证到位」**，这条测试从来没验证过它想验证的东西。
- **Scope / 要排的雷**（先列四类）：
  ① **假绿测试**：早退 / 崩溃 / 断言恒真（本条即实例）；
  ② **静默改数据**：溢出饱和、精度丢失、键被覆盖（W16 / B10 修过一批，未必扫全）；
  ③ **不可恢复崩溃**：库代码里的 `panic!` / `unwrap()` / `expect()`
  （I-1 已消灭 flatten 那一处，Rust 侧其余的待扫）；
  ④ **递归深度**：无守卫的递归（栈溢出是 abort，接不住）。
- **Design / 思路**：先**扫**出清单，**不要边扫边修**；按「会不会改数据 / 会不会崩」排优先级，
  再逐条修，且每条补一条**真能跑**的测试。
- **Risk / 风险**：扫描面大容易失控 ⇒ **一次只排一类**，每类先出清单再动手。

## I-15 `weakdata`：不进解析的扩展数据通道 / data channel extensions read, core ignores

- **Status / 状态**：`Todo` —— **概念阶段**，用户点名记住的关键词，尚未设计。
- **Idea / 想法**：`weakdata` 声明的数据**不进入解析**（不进主树、不影响解析结果），
  库侧忽略，只作为旁路元数据，供**拓展**按需读取。
- **用途举例（用户给的）**：高亮配置、自定义提示。
- **形态（用户补充，2026-10-26）**：
  - 必须是 **`@` 指令**（`@weakdata`）—— 与 `weak*` 整族一致；
  - 必须**手动 `@feature` 开启**，不能默认生效（opt-in，保向后兼容）。
- **与 I-5 `weakport` 的关系**：同一族思路 —— 「库侧忽略，交给宿主」；
  区别在于 **weakport 是契约通道、weakdata 是数据通道**。两者都要和对方划清边界，
  否则又多一个近义术语（§七·二 术语混用那条债）。
- **可复用零件**：外置指令的 `Outcome::Discard`（元数据块不进主树）、
  `ParseOptions` 的外置指令表、`trace_contracts` 的记录方式。
- **Design / 思路**：先定「拓展如何读到它」与冲突规则，再谈语法。
- **📌 待办**：落地后**必须进文档**（模块文档 + 站点教程页），按 I-13 的规矩
  同步补「语义 + 例子 + 陷阱」。
- **Risk / 风险**：与 `weakport` / `flatten` 语义相邻，命名和边界必须先写清再动手。

## I-17 `weakdoc`：`.smlex` profile 的引用指令 / project profiles, referenced, never injected

- **Status / 状态**：`Todo` —— **概念阶段**，用户点名记住的关键词，尚未设计。
- **Idea / 想法**：项目里放一个 **`.smlex`** 文件，存**必要的 profile**
  （针对某一类文档的**特化支持**）；文档里用 `weakdoc` 关键字引用它。
- **前提**：profile 需要 **install**（先装才能引用）。
- **硬性约束（用户定，逐条钉住）**：
  1. **不能用于注入** —— 只做「声明/引用」，绝不往文档里注入内容或行为；
  2. 只能以 **`weak*`** 形态存在，且**不能叠加自身**（同一个不能重复挂）；
  3. **可以有不止一个叠加**（不同的 profile 可同时挂）；
  4. **拓展负责检查**：叠加后若冲突，**直接报错**。
- **与 I-5 `weakport` / I-15 `weakdata` 的关系**：三者同族 —— 库侧忽略、
  交给拓展解释。**weakport = 契约、weakdata = 数据、weakdoc = 文档 profile**。
- **待定**：`.smlex` 的位置（项目根？）、发现规则、install 的落点、
  冲突判据由谁定义（profile 自己声明？拓展硬编码？）。
- **📌 待办**：落地后**必须进文档**（同 I-15）。
- **Risk / 风险**：`.smlex` 一旦能影响解析行为，就离「注入」只有一步之遥 ——
  约束 1 必须写成**可测的判据**（例如：解析结果对有无 `.smlex` 必须完全一致），
  否则会退化成隐式方言。

## I-16 版本后缀与发布门禁：`a` / `s` / release gates

- **Status / 状态**：`方案已定（2026-10-30 用户确认）` —— 此前迭代太快、版本号零门槛，
  规则重立为**四级门槛表 + 发版才统一 bump**（见下）。
- **Idea / 想法**：版本号加后缀区分成熟度（原拟 `a` 结尾 / `s` 结尾；改用 semver 标准的
  `-beta.N` / `-rc.N`，crates.io 原生支持、工具链友好）。
- **Gap / 缺口（已补）**：此前没有「什么版本够格标正式」的**可执行判据** ⇒ 落成准入清单。
- **Design / 思路（2026-10-30 定）**：四级门槛，每级一道可执行 gate，没过不进下一档：

  | 档 | 版本后缀 | 语义 | 准入（全满足才进） |
  |---|---|---|---|
  | 1 | 现状 / `-dev` | 功能自由迭代 | 无 |
  | 2 | `-beta.N` | 特性冻结，只修不增 | feature 收口；五端四主要语法对齐 |
  | 3 | `-rc.N` | 候选发布 | 排雷 1/2/3/4/5 开口清零 |
  | 4 | 正式（无后缀） | 稳定 | ① 安全：`cargo audit` + `miri_check.py` + `osv_check.py` + fuzz 全绿（结果贴 `CHANGELOG.md`）；② conformance **27/9→0**；③ 性能基线达标（`docs/iter-parse-plan.md` §1.4） |

  **bump 纪律**：平时不 bump，发版才重排 —— `dev` 积量期间版本号保持不变；正式发版时用
  `workspace.version` 一次性统一主 crate 与子 crate（当前 8 个 crates 版本各自漂移，属历史包袱）。
  配 `--check-version`/CI 门禁，防「没到门槛就 bump」回归（见 `TODO.md` §六·〇）。
- **关联**：I-12 改名（名字与版本通常一起发）、`TODO.md` §六·〇（bump 纪律）与 §八·二（publish 顺序）。
- **Risk / 风险**：后缀一多就成「版本号噪音」⇒ 种类要少（只 `-beta` / `-rc` 两档 + 正式），
  不设「过渡性」的第三种后缀。

---

## 价值评估 / Value check（用户判断优先，成本是我估的）

| 条目 | 有用吗 | 成本 | 建议 |
|---|---|---|---|
| I-5 `weakport` | **有用，但慢**：无宿主需求时不急 | 低 —— 落在外置指令 `Discard`，零词法改动 | 先留口子，等真有插件再说 |
| I-6 `featag` | **用户已定：有价值** —— DSL 场景的表达力与可读性提升（`rectangle size 2` 比一堆字段直白）。我的保留意见只保留为**风险**：它等于一门小宏语言，须先划清与 `&frag` / `@when` / `@for` 的边界，且别做成「只有 Rust+JS」 | 高 —— 新内置指令 + 五端一致性 + 规范页 | 按用户判断推进；**先写规范草案**，Q1–Q8 定稿再动手 |
| I-7 约束精确控制 | **最划算**：不止拼错 —— 缺失 / 最小 / 最多 / 互斥都要能表达 | 中低 —— 互斥等先做成 Modifier 就只动 Rust | 若要挑一条先做，挑这条；①②③ 已落地（2026-10-28 / 10-30），**④⑤ 已落地（2026-10-30）**，仅剩「按前缀放行额外键（`x-*`）」—— 属新语法，按「Rust 先行、五端统一同步」留到后续 |
| I-8 TOML `=` | 有用（迁移/复制粘贴友好） | 中高 —— 五端词法 + 旧文档行为可能变 | `=` 可做且要 opt-in；`[name]` 维持搁置 |
| I-9 近似契约名提示 | **便宜且立刻见效**：错误现场就能给「你是不是想写 X」 | 低 —— 不改码，只加文案；编辑器侧另算 | 可与 I-7 同批做 |
| I-10 团队 / 宣传 | 有用，但**不能靠写文档推进**：缺的是入口（CONTRIBUTING / 模板 / 上手页） | 低到中 —— 纯文档，不碰代码 | 先补入口 + 能力矩阵，再谈宣传 |
| I-11 中文可剥离 | 有用（`idyml` 与嵌入式都要轻量）；**先数据化，再谈剥离** | 中 —— 触及 `sml-codes` 全部文案与生成链路 | 分两步，别一步到位 |
| I-12 改名 | 必须做但要一次做对：crate 名永久占位；且 `idyml` 与 `yml` 近似（已实测） | 高 —— 包名 / lib 名 / SONAME / 头 / 域名 / 双远端 | 先定名（避开 `yml` 词根）再抢注，给共存期 |
| I-13 实现即文档 | **用户已定：有价值** —— 闪光点决定别人要不要点进来 | 低 —— 改流程，不新增文档体系 | 就近沉淀「语义 + 例子 + 陷阱」；例子必须可跑 |
| I-14 仓库排雷 | **用户已定：要做** —— 「测试全绿」不一定「验证到位」 | 中 —— 四类雷，一次只排一类 | 先出清单再修；每条补一条真能跑的测试 |
| I-15 `weakdata` | 用户点名记住；拓展侧真需要（高亮配置 / 自定义提示） | 概念阶段，设计后才知 | 先定形态与边界，与 `weakport` 划清；落地要进文档 |
| I-17 `weakdoc` | **用户已定：要做** —— 一类文档一份特化 profile，比逐份文档重复声明干净 | 中 —— 引入 `.smlex` + install 机制 | 死守「不能注入」：解析结果对有无 `.smlex` 必须一致，且可测 |
| I-16 版本号规则 | **方案已定（2026-10-30）** —— 四级门槛 + semver `-beta`/`-rc` 后缀；平时不 bump、发版才统一 | 低 —— 判据已写进 I-16 与 `TODO.md` §六·〇 | 落地后给 §六·〇 补 bump 纪律；`--check-version` 门禁随发版一并做 |

> 一句话 / One line：**I-7 + I-9 是同一件事的两半**（约束能表达 / 写错能看见），性价比最高；
> I-6 按用户判断推进（DSL 可读性），但先定 Q1–Q8；I-5 便宜缺需求；I-8 别碰 `[name]`；
> I-10 / I-11 是对外层（团队与国际化），I-12 是命门 —— 改名要一次做对，先抢注。