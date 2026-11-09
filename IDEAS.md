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

- **Status / 状态**：`Done`（2026-10-27；**已合入 `dev`**，`feat/value-accessors` 分支已删）—— 访问器层已补齐。
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
- **Gap / 缺口**：~~Rust 原生侧反而更薄，缺 `as_object` / `as_array` / `as_i64` /
  `as_bool`（＋ `as_object_mut`）~~ —— 2026-10-27 已补齐（见上「已落地」），现无缺口。
- **Note / 备注**：~~`get` 只走 `Object`，遇数组下标即 `None`~~ —— 2026-10-27 起下标可用
  （`a.b[0].c`），但**字面键名优先**于下标语法，故路径里带 `[` 的既有键不受影响。

## I-4 注释双语化，英文在前 / bilingual comments, English first

- **Status / 状态**：`Done`（2026-11-02 归零）—— 本仓库注释**大体已是**「English ｜ 中文」，
  这一条是把**未达标的**（纯中文或格式不整）补成统一约定，**单独一笔**收尾。
  ✅ **已收口**：按下面「口径」那一节（docs.rs 可见面）扫全仓，**摘要行全部含 `｜`**；
  与之配套的「无 doc 的 `pub` 项」也同批归零（记在 HANDOFF R5 + R11）。
  ⚠️ **不计入的两类**：`sml-codes/src/{codes,msgs}.rs` 的 151 处码表 doc 是**生成物**
  （要双语须改 `errors/gen_*.py` 模板并重生成 4 份产物，属独立决定）；私有模块 `//!` 与内部 `//` 不做。
- **Convention / 约定**：`English ｜ 中文`，一行说完，不写第二段。
- **Scope / 范围**：先公开 API 注释，再按文件推进；一次只改一类，便于 review。
- **口径已定（2026-11-02，用户拍板）**：只算 **docs.rs 可见面**（crate 根 `//!` + `pub` 项摘要，
  即「会出现在文档站上的那部分」），**并扩到全部在发布的公开 crate** —— `swsml` / `swsml-derive` /
  `sml-*` 全系 / `smltools`。私有模块的 `//!`（如 `sml-value::object`）与内部普通 `//` **不做**。
- **第四批（2026-11-02）**：crate 根 `//!` 全线收口 —— 双语化 `sml-contract` / `sml-lex` /
  `sml-pattern` / `smltools` / `sml-feature` / `sml-regex` / `sml-codes` / `swsml-derive`，
  并**补写** `sml-include` 与 `sml-parse` 原本**缺失**的 crate 级 `//!`（这两处 docs.rs 首页此前是空的）；
  另落若干高频 `pub` 项摘要（`suggest_contract_name` / `ext::TypeCheck` / `Tok` / `Class` /
  `Object` / `Object::insert_or_merge` / `include::{parse,expand}` / `ext` 模块头）。
  🔬 判别实验：改动前后各跑一次 `cargo doc --no-deps`，警告集合**完全一致**
  （`sml-codes` 3 条 / `sml-parse` 5 条 / `swsml-derive` 1 条，均为既有坏链接）⇒ 本批**未引入**新警告。
- **第六批（2026-11-02）**：`sml-value` 收口 —— 14 处公开项摘要双语化（`serde::{from_value,to_value,to_string}`、
  `dump::{to_sml_with,to_sml_canonical,to_sml_checked,DumpOptions,DumpOptions::sort_keys}`、
  `ser_text::to_string_direct`、`Object::{insert,iter,sort_keys,eq_ordered,into_values}`）。
  判别实验：改动前后 `cargo doc -p sml-value --features sml,serde` 警告集合一致。
  ⚠️ 本 crate 的 7 个迭代器类型（`Iter` / `Keys` / `Values` …）**没有在 `lib.rs` 具名再导出**，
  按已定口径（docs.rs 可见面）不计入；若哪天把它们导出，会立刻变成新欠账。
- **第七批（2026-11-02，`sml-parse` 开头）**：`parse_file` / `parse_file_features` / `lib.rs` 两个
  `pub mod` 标题 / `ParseOptions` / `ParseOutput` / `Diagnostic`。🚨 顺带发现并修掉一个真 bug：
  `parse_file` 的文档是**错挂**的（讲的是「值嵌套深度上限」，而 `api.rs` 早没那个常量），
  还带悬空 intra-doc 链接 ⇒ 修后 `cargo doc` 警告 5 → 4。另修正 `Diagnostic` 里过期的 `Err(String)`。
  **第四批（同日）**：`ParseOptions` 的 4 个 builder + 4 个注册方法、`ParseOutput::contract_map`、
  `DiagnosticKind`（枚举本身原本无 doc，已补写）+ 4 个变体 ⇒ `ext.rs` 可见面收口。
  **本 crate 剩余**：`Directive` 及其方法、`contract_map.rs` 的 `AppliedContract` / `ContractMap` / `PathSeg`。
- ✅ **2026-11-02 已清零（`836cf15`）**：12 个 crate 全部归零，摘要行一律 `English ｜ 中文`。
  口径 = 「摘要行里有没有 `｜`」（旧判据「有没有英文字母」会把提到标识符的中文句算成已双语，已废弃）。
  ⚠️ **未做**：`sml-codes/src/{codes,msgs}.rs` 的 151 处码表 doc（生成物，须改
  `errors/gen_*.py` 模板并重生成 4 份产物，属独立决定）；扫描脚本的五个坑见 `HANDOFF` §6。
- **另一笔（不是双语化，是补写）**：`Object` 的 14 个方法与 `serde_bridge` 的
  `#[cfg(not(feature = "sml"))]` 版 `to_string` **完全没有 doc** —— 见 `HANDOFF` 的 **R11**。
  ✅ **2026-11-02 已补（`c661f5a`）**：连 `Value` 枚举**本身**、`Map` 两个 cfg 别名、
  `Null`/`Bool`/`Int`/`Str`/`Array` 五个变体一并从零写（共 18 处）。
  ⚠️ 刻意**不加 doctest**：桩函数在默认档下不编译，只在 `cfg(not(sml))` 里成立的例子
  会重演 W16「名义存在、从不运行」。
  ✅ **第二批（`d71b824`）**：门面 `swsml` 15 处（7 个 `Options::new`、`SvgOptions` /
  `SlintOptions` 两个结构体、`to_svg` / `to_slint` / `to_latex`、`c_abi::{jsonify,json_to_value}`、
  `sml::serde` 模块 `//!`），并修 `[`to_sml`]` 坏链接 ⇒ `cargo doc --all-features` 警告 1 → 0。
  **`sml-value` 与 `rust/src` 均已清零**。
  ✅ **第三批（`9582c25`）**：其余 crate 17 处一次收完 ⇒ **全仓 `pub` 项都有 doc，R11 归零**
  （`sml-contract` / `sml-pattern` 的 doc 警告各 1 → 0）。两条经验写进 HANDOFF §2/§6：
  生成物（`sml-codes/src/msgs.rs`）要改**生成器模板**，不能手改产物；写 doc 必须读实现，
  三批共抓到 4 处「想当然」。
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

## I-5 weakbind：插件认契约，库忽略 / plugin-only contracts

- **Status / 状态**：`Partial`（2026-11-04，**仅 Rust 单端**）——
  语法 `@weakbind` / 作用域 / 旁路表 `ParseOutput.weak_binds` / `E-PARSE-029` /
  内置表注入 `builtin_contracts` / `--dump-builtins` / `W-CONTRACT-001` 八档软校验**均已落地**
  （用例：`weakbind.rs` 8 + `builtin_contracts.rs` 3 + `weakbind_warn.rs` 10 + `dump_builtins.rs` 4 + lint 钉子 1）；
  **编辑器提示与其余端未做**（草案 §10）。
- **规范草案**：`docs/swsml/spec/weakbind.md`（v0.1，第一刀已落地 · 其余未闭合）——
  语法 / 作用域 / 码（`E-PARSE-029` + `W-CONTRACT-001`）/ 旁路表 / 内置表与导出格式 /
  多端同步清单 / 用例清单都在那里；本条只留定位与沿革。
- **优先级（2026-11-04 用户裁决）**：`weak*` 族（I-5 / I-15 / I-17）**先做**；
  `.smlb` / `smldb`（I-18）与 `smlkit`（I-19）**还需商榷**，不排期、不占先。
  开工前须先落一件事：与 I-15 `weakdata` 的边界判据（带契约 vs 纯数据；不被编译期丢弃 vs 不进主树）。
- **命名已定（2026-11-04 用户裁决）**：**`weakbind`**（原 `weakport`；`weakimport` 否决，理由见下「用户修订」）。
- **Idea / 想法**：给插件用的通道 —— 插件（宿主）认 `contract`，**库侧忽略**，不校验、不报错。
- **Done / 已完成（可复用的零件）**：外置指令的 `Outcome::Discard` = 元数据块不进主树
  （`rust/sml-parse/src/ext.rs`）；`ParseOptions` 能挂外置指令表与外置类型表；
  契约映射记录 `trace_contracts` 可让宿主自己看「谁挂了哪个契约」。
- **Gap / 缺口**：没有「**库侧不认识、插件侧照常当 contract**」的形态；
  现有 `@is` 一律由**库**强制校验，且必须先定义契约（否则 `E-CONTRACT-001`）。
- **语义纠正（2026-11-04 用户澄清，覆盖此前「跳过校验」的写法）**：
  `weakbind` **不是**削弱/降级校验，而是**把校验的归属方从库挪到插件** ——
  插件侧**仍然当正常 contract 处理**（照常校验、照常走类型 / 默认值 / 区间 / 补全）；
  只有**核心库**不认它：不查契约表、不校验、不为「未知契约」报 `E-CONTRACT-001`。
  - 推论 1：树上必须**原样保留**契约名与字段（否则插件无从校验）⇒ 只 `Discard` 的省事路线**不成立**。
  - 推论 2：没有该插件的宿主读同一份文档 ⇒ 库不报错、文档照常可用 —— 这就是本项的存在理由。
  - 推论 3：**「契约未注册」这个错的报告方是插件，不是库** —— 与码表「`impls` 记谁实现了」的口径冲突，
    需裁决：这个错进不进码表、算宿主层，还是复用 `E-CONTRACT-001` 但改记归属。
- **原始动机（2026-11-04 用户澄清，写清免得后来人读歪）**：`weak*` 一族**主要是给编辑器提示用的**
  —— 当契约**内置在解析器/库里**时（内置一套），文档里若用 `@is` 再声明一遍，等于**同一个契约写两遍**：
  库里一份、文档里一份，既重复维护，又会让库把文档那份当约束再校验一次。
  `weakbind` 让文档里那份只服务编辑器（补全 / 悬停 / 诊断提示），**库侧不重复消费** ⇒ 契约仍然只维护一份。
  - 由此定**消费者的分工**（族内区分靠这个，不靠"弱"的程度）：
    `weakbind` = 编辑器/插件按 contract 解释（authoring 期提示）；
    `weakdata` = 拓展读旁路数据（运行期）；`weakdoc` = 项目级 profile 引用（工具链）。
    三者共同点：**库侧一律不参与**，所以落地都必然要改 vsix（`TODO.md` L993）。
- **该用 / 不该用（2026-11-04 实测，`site/` 与 `editors/` 已核）**：
  - ✅ **该用**：书里**只做展示**的 ```` ```sml ```` 示例块。站点构建链**不解析**它们 ——
    `site/build_site.py` 只做「同步 `sml.mjs` + 生成两个 JSON + 跑 hugo」，
    `site/gen_epub.py` 对代码块只 `_html.escape`；`site/content/` 下 **50 个文件、125 处 `@contract`**
    构建期一处都没校验 ⇒ 这些声明存在的理由本来就只有「给编辑器看」。
  - ❌ **不该用**：`site/data/sml-lessons*.json` 里会被**浏览器真解析**的源码
    （`site/layouts/shortcodes/sml-playground.html` 里 `parseSafe(src, {files})`）；
    `tests/conformance/**`、`examples/**` 这类被工具链跑/逐字节对比的语料。
  - ⚠️ **同步节奏（2026-11-04 用户定，纠正上一版「先做五端」的写法）**：
    **多端同步是发布前的必做项**，不是当前阻塞 ⇒ 当前允许 **Rust 单端先行**
    （与 `E-CONTRACT-017` 的「Rust 先行、其余端响亮拒绝」同一模式）。
    且**同步对象不止五端**（仓内实际行为承载点）：
    `rust` / `js` / `c` / `cpp` / `lua` / `smltools` / `editors/vscode` / `editors/lsp` / `editors/zed` /
    `site/static/sml.mjs`（构建期同步自 js）；再接 5 个绑定层 `bindings/{csharp,go,java,python,ruby}`。
  - ⚠️ **开发期硬约束**：同步未到位前，`@weakbind` **不得进共享语料**
    （`site/content`、`site/data/sml-lessons*`、`examples/`、`tests/conformance/`）——
    现在各端口径是「Rust 报错 / JS 静默收成片段」，写进语料就等于让同一份文档在不同端表现不同。
    只在 Rust 单端的开发用例里用。
  - 📌 **代价，先说清**：`weakbind` 的主用途是编辑器提示，而扩展正是发布前那批同步的一部分
    ⇒ **单端先行只能验证语义，兑现不了用途**；提示真能用要等同步做完。
  - 📌 **现实提醒**：全仓**目前没有内置契约表**（Rust 侧契约全部来自文档，`sml-contract` 的
    `contracts` 是解析期收集的 `BTreeMap`，无 `*builtin*`）⇒「省掉重复那一份」的收益
    要等"内置契约"真的出现才兑现；现在能兑现的只是「只给编辑器看、不参与运行期」这一半。
- **Design / 思路（按上述纠正）**：仍落在外置指令侧，但**不能只 `Discard`** ——
  需要多一条「解析时保留、校验时跳过」的 outcome：库认出 `@weakbind` 后把该块当**普通数据块**
  （契约名 + 字段全部入树），再由宿主侧钩子把「该块 + 自己的契约表」交给插件走正常 contract 路径。
  不必改词法，只在 `ext` 层区分「丢弃」与「保留但不解释」。
- **已定案（2026-11-04）**：
  - ① **库可以读取到 `weakbind`** ⇒ 走解析树：库原样保留契约名与字段（可枚举、可获取），
    只是**不进契约映射、不当约束、不为未知契约报错**。插件不必自己重建 typed-block 逻辑。
  - ② **warning 报在库里**（不是编辑器、不是扩展）；**Rust 能判定弱声明是否匹配** ⇒ 库侧比对、
    库侧出诊断，`W-` 级、不阻断解析。
- **最小形态（草案，2026-11-04）**：`@weakbind <契约名> [<字段体>]` ——
  字段体**可选**；只写名字时形状完全由内置表提供（这才是「不用写两遍」的真正兑现）；
  写了体才做字段级比对。
- **匹配判据（库侧，待你点头）**：

  | 情形 | 是否 warning |
  |---|---|
  | 契约名不在内置表（含拼错） | ✅ 报 |
  | 体里字段名不在内置契约 | ✅ 报 |
  | 体里字段类型与内置契约不同 | ✅ 报 |
  | 内置契约有 `required` 字段、体里没写 | ⚠️ 建议**不报**（体可以只写关心的字段） |

  - **码**：新增 **一个** `W-CONTRACT-001`「弱声明与内置契约不匹配」，四类差异靠 detail 区分
    （沿用 `E-LEX-007` / `E-FEATURE-003` 的「同因同码 + detail」做法），domain `CONTRACT` 已存在、不必加域。
  - ⚠️ 连带必须改：`W-LINT-002`「契约定义从未被应用」要把弱声明**排除在外**，否则每条 `weakbind`
    都会吃一条假 warning。
- **编辑器提示是主需求（2026-11-04 用户强调）**：`weakbind` 的价值不在「库少校验一次」，
  而在**块内补全 / 悬停 / 诊断要照常工作**。要成立，缺三块：
  1. **作用域规则**：`@weakbind tls` 管哪个块 —— 建议照 `@when` 的先例：只作用于**紧邻的下一个块**，
     悬空 / 连续出现各有码（`E-PARSE-016` / `E-PARSE-017` 同族）。
  2. **形状来源（最硬的一环）**：扩展现在**完全不知道内置契约长什么样** ——
     `collectContractNames` 只收名字，形状一律来自文档里的 `@contract` 体。
     唯一不违背「不写两遍」的做法：**库导出 → 构建期生成 → 扩展打包**（生成物，不是手写第二份），
     与仓里 `errors/gen_codes.py` 生成五端码、`site/build_site.py` 同步 `sml.mjs` 同一套路。
  3. **扩展识别**：`collectContractNames`（`editors/vscode/src/sml-parse.mjs:235`）加 `@weakbind` 支路，
     并区分「弱声明（形状来自内置表）」与「真契约（形状来自文档体）」。
  - 提示具体要做到四件事：块内字段补全 / 悬停显示类型·默认值·区间 / 未知字段诊断 /
    `@weakbind` 后面的**契约名补全**（候选来自内置表）。
- **用户修订（2026-10-31，影响 Status）**：控制权在**应用层** ——
  **应用层自己设契约**、由应用层注册/解释，不是给普通数据用的旁路。
  `weakbind` 对插件来说**看起来就是普通数据**（库侧忽略，插件侧按应用层给的契约解释）。
  - ⚠️ **可内置 ≠ 不优化**：即使落成「内置/外置指令 + 库忽略」，语义上仍要保证
    **弱绑定不受编译期丢弃** —— 之后 `smlb` 编译时**不会忽略 `weakbind`**
    （即它是真数据通道，不是被 `cfg`/feature 裁掉的可丢弃物）。措辞与实现都别把它
    写成「随手 Discard」——那只适合「纯元数据」，不适合「插件要真读」的通道。
  - ⚠️ 因此与 I-15 `weakdata` 的边界需重画：两者都是「库忽略、交宿主」，
    但 **`weakbind` 带契约**（应用层设契约来解释数据）、**`weakdata` 纯数据**；
    且 weakbind 强调「不被编译期忽略」的保真，weakdata 强调「不进主树」。
    落地前先划清，别造近义术语。
- **命名沿革（2026-11-04）**：`weakport` → ~~`weakimport`~~ → **`weakbind`**。
  `import` 否决理由：它暗示「把外部东西搬进来」，而本项是**库一个字节都不搬**；
  且与既有的 `include`（真会搬）抢语义。`bind` = 声明绑定、不做校验，
  正对上「弱绑定」这个已在本文档使用的词。族内三个后缀长度也齐：`weakbind` / `weakdata` / `weakdoc`。

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

- **Status / 状态**：`Done` —— **计数三件套与跨字段依赖已齐（①②③）**；**④ 三态 + 块级覆盖已做**；
  **⑤ `Warning` 档已加**；**`x-*`（按前缀放行额外键）已以修饰符 `x_only "<prefix>"` 非破坏实现（2026-11-02，未新增语言语法）**。
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
  ✅ 「按前缀放行额外键」已实现（2026-11-02）：外置修饰符 `x_only "<prefix>"`（`sml_contract::XOnly`），
  按契约开启、与 `strict` 互斥、未注册即禁用；**未新增语言语法**，故无需五端同步。
  ⑤ 诊断已新增 `DiagnosticKind::Warning`（与 `Deprecated` / `DuplicateKey` / `KeyOverwritten` 分工明确）—— 已完成。
- **Design / 思路**：三态最划算 —— 复用 `Diagnostic` 加 `Warning` 档，`loose` 保留为静默放行，
  新增 `warn`；顺序与叠加的 strict 优先级规则要一并写清。互斥/计数这类先做成
  Modifier（外置），再考虑升为内置语法 —— 这样不必五端同时改。
- **下一棒 / 待确认（2026-10-30 ④⑤ 落地时补记）**：本棒按「Rust 先行」交付，下面 6 条
  **尚未定论或未做**，别当成已完成：
  1. ✅ **已拍板（2026-11-02）：接受** `strictness` 字段（取值 `strict`/`warn`/`loose`），
     `infer-contract --json` 输出随之变更；C / JS / Lua 侧若有黄金用例按此同步（未同步前不得
     反向把 Rust 改回 `allow_extra`）。
  2. ✅ **已拍板（2026-11-02）：暂不下沉 `Diagnostic`**。保留 `ContractWarning` + parser 转译，
     但字段名/语义与 `Diagnostic` 对齐，将来若要合并只动类型、不动行为。
  3. ✅ **已拍板（2026-11-02）：本轮不追行号**（契约告警 `line` 仍 `None`，测试不断言）；
     将来把 token 行号透传进 `sml-contract` 时再补。
  4. ✅ **已拍板（2026-11-02）：保持「绑定紧邻其前的契约名」**——`@is A warn B` ⇒ 只降 A；
     `@is A B warn` ⇒ 只降 B。已写进 `sml-contract` 文档（「整条 `@is` 统降」不采纳：
     多契约时无法表达「只放宽其中一个」）。
  5. ✅ **已拍板（2026-11-02）：保留** `@contract C strict`（与默认等价，兼容既有写法，不删）。
  6. ✅ **已落地（2026-11-02）**：`x-*` **不作为语言语法**加入；改为外置修饰符 `x_only "<prefix>"`。
     语义与取舍见本节末「x-* 评估」。

### x-* 评估（2026-11-02）

**它是什么**：`strict` 契约下，把「未声明但以 `x-` 开头」的键放行，其余未声明键仍报 `E-CONTRACT-004`：

```sml
@contract Svc x-* { name: str }
svc { @is Svc  name: gateway  x-owner: infra }
```

**结论（2026-11-02）：否决「新增语言语法」，改用外置修饰符 `x_only "<prefix>"`（已实现）。** 理由：

1. **它放走的正是契约存在的意义**：`strict` 唯一能抓的错就是「键写错/多写」；`x-*` 把「扩展键」与
   「拼错的键」混在一起 —— `x-naem`、`x-nmae` 会被静默放行，strict 就名存实亡。
2. **已有更便宜的替代**：① 注释 `#`（不产出数据）；② 要整块放宽就用 `loose` + lint 告警；
   ③ 真需要元数据就**在契约里显式声明** `meta: { ... }` 子块 —— 一次声明、处处校验。
3. **代价不成比例**：新语法 + 五端同步（JS/C++/Lua 各一套契约校验），换来一个「绕过契约」的逃生口。

**何时值得重做**：出现「第三方插件必须往文档塞标记、且无法改契约」的真实需求时（类似 OpenAPI 的
`x-` 生态），再按 **A 方案**（契约级 `x-*`、放行即告警、不向下传递、字面前缀）落地。

**与人写多次的关系**：重复手写同类文档的痛点，应由 **`default` + 片段 `&frag` + `@for` + 契约复用**
解决，而不是 `x-*` —— 后者只让「每份文档各塞各的私货」更容易，反而让重复场景更难保持一致。

**落地方式（2026-11-02）**：`sml_contract::XOnly` —— 修饰符 `x_only "<prefix>"`：

```sml
@contract Svc loose {
  name: str
  _x: str optional x_only "x-"     # 只有本契约按 x- 前缀放行
}
```

- **只在写了它的契约里生效**（别的契约完全不受影响）；
- **与 `strict` 互斥**：strict 下直接报 `E-CONTRACT-011`（不静默无效）；块级 `@is C warn`
  把有效严格度降档后合法；
- **可整体禁用**：不调 `ParseOptions::with_modifier(XOnly)` 时它就是未知修饰符（零扩展 = 零影响）；
- 校验点在 `apply_contract`，**不放** `Modifier::check_group` —— 后者只在该字段出现时才被调用，
  作者漏写 owner 字段就会漏检。
  用例：`sml-parse/src/ext.rs` 的 5 条 `x_only_*`（放行 / 拦非前缀 / strict 互斥 / 块级 warn / 未注册）。

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
  `docs/swsml/ref/capability-matrix.md`（每格附实跑命令与输出）。**它同时挖出三条此前不在任何清单上的
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

- **Status / 状态**：**`Rejected`（2026-11-03 用户裁决：不改名）** —— 原话「swsml 我不想改名，
  那会大幅增加维护成本；我想在新的渐进式 crate 采用新名称」。
  ⇒ **`swsml` 保持不动**，新增能力走**新 crate 用新名**这条路（见 `I-19` 的 `smlkit` 族）。
- **为什么否掉**（把成本列出来，供以后别再提）：改名要同时动 `Cargo.toml` 的 `name` 与 `[lib] name`、
  **11 个 crate 的依赖引用**、`packaging/` 与 `debian/` 的 **SONAME `libswsml.so.0`**、
  C-ABI 头 `sml.h`、README / 徽章 / 站点下载链接、两个远端仓库、域名 —— 而**收益只是「好搜一点」**，
  且老用户与已链接的下游会直接坏（必须给共存期）。**投入产出不成比例。**
- **替代路径（已采用）**：不动 `swsml`，把「好名字」给**新 crate**：
  `smlkit-easy`（渐进式入门层，已建）+ 将来 `smlkit`（品牌门面，瘦别名）。
  这样新用户从 `smlkit-*` 进来，老用户与 SONAME / C-ABI 全部不受影响。
- **Now / 现状**：包名 `swsml`（crates.io 上 `sml-rs` 被无关项目占用），lib 名仍是 `sml`；
  仓库名 `sml`（Gitee 权威源 + GitHub 镜像），域名 `sml.swebase.cn`。
  ⚠️ 曾经的 `IDYML.md`（`idyml` 英文稳定别名）已被 **`smlkit` 取代并否决**（`-yml` 误导成 YAML），见 `I-19`。
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

- **Status / 状态**：`Partial`（2026-11-03 三棒）—— 四类雷（3/4/5/6）的**可修项已基本修完**。
  - ✅ 雷 5 A 组（`c4077a0`）：derive 大整数超 i64 一律报错，与 serde 桥同码 `E-DERIVE-002`；
  - ✅ 雷 3「与库分叉」类（`3bff9b0`）：六条消四条，`depth_of` 与文本扫描识别未动；
  - ✅ 雷 5 A4-6（`W-PARSE-001/002` 告警带码）、雷 5 B（浮点 raw 已定性为 serde 数据模型做不到）；
  - ✅ 雷 5 B **五端口径**（2026-11-03）：C 的 `add_float` / C++ 的 `fmt_float` 与 Rust `format_float`
    同口径（整数值浮点保留 `.0`，NaN/inf 转引号串）⇒ 套件 `19` 的 C/C++ 格转 AGREE；
    **JS / Lua 结构上无法区分 `1` 与 `1.0`**，登记为结构差异（要改就动值模型，属 I-18）。
  - ✅ 雷 6 三条全清：Lua 大整数（`11-02`）、C++ 裸 `include`（`11-03` **补齐**，`08/09` 转 AGREE）、
    JS `\uXXXX`（`10-30`）。一致性基线以 `tests/conformance/known_divergences.md` 头部与
    `docs/repo/ref/scan.md` 为准（现算，不在此写死）。
  - ⏳ 仍未动：雷 1 根因（迭代式解析，已闭）、雷 3 的崩用户类（已复核 0 处）与冗余类（已修）——
    剩下的都是**已定性为不做**或**结构上做不到**的（见 `TODO.md` 排雷区与 `known_divergences.md`）。
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

- **Status / 状态**：`Partial`（2026-11-04，**仅 Rust 单端**）——
  语法 `@weakdata <名> { … }`、旁路表 `ParseOutput.weak_data`、同名后者覆盖、7 条用例已落地；
  其余端与编辑器属发布前同步。规范：`docs/swsml/spec/weakdata.md`。
- **优先级（2026-11-04 用户裁决）**：`weak*` 族**先做**（I-5 / I-15 / I-17 为一组）；
  `.smlb` / `smldb` / `smlkit` **还需商榷**，不排期。
- **Idea / 想法**：`weakdata` 声明的数据**不进入解析**（不进主树、不影响解析结果），
  库侧忽略，只作为旁路元数据，供**拓展**按需读取。
- **用途举例（用户给的）**：高亮配置、自定义提示。
- **形态（用户补充，2026-10-26）**：
  - 必须是 **`@` 指令**（`@weakdata`）—— 与 `weak*` 整族一致；
  - 必须**手动 `@feature` 开启**，不能默认生效（opt-in，保向后兼容）。
- **与 I-5 `weakbind` 的关系**：同一族思路 —— 「库侧忽略，交给宿主」；
  区别在于 **weakbind 是契约通道、weakdata 是数据通道**。两者都要和对方划清边界，
  否则又多一个近义术语（§七·二 术语混用那条债）。
- **可复用零件**：外置指令的 `Outcome::Discard`（元数据块不进主树）、
  `ParseOptions` 的外置指令表、`trace_contracts` 的记录方式。
- **Design / 思路**：先定「拓展如何读到它」与冲突规则，再谈语法。
- **📌 待办**：落地后**必须进文档**（模块文档 + 站点教程页），按 I-13 的规矩
  同步补「语义 + 例子 + 陷阱」。
- **Risk / 风险**：与 `weakbind` / `flatten` 语义相邻，命名和边界必须先写清再动手。

## I-17 `weakdoc`：`.smlex` profile 的引用指令 / project profiles, referenced, never injected

- **Status / 状态**：`Partial`（2026-11-04，**库侧已实现**）——
  `@weakdoc <名>` 语法、旁路表 `ParseOutput.weak_docs`、文档级判重（`E-PARSE-030`）、6 条用例已落地；
  **三命名空间发现（项目 `.smlex/profiles` / 用户全局 / 系统）与 `--profile-dir` 属工具侧，未做**。
  规范：`docs/swsml/spec/weakdoc.md`（含路径表）。
- **优先级（2026-11-04 用户裁决）**：`weak*` 族**先做**（I-5 / I-15 / I-17 为一组）；
  `.smlb` / `smldb` / `smlkit` **还需商榷**，不排期。
- **Idea / 想法**：项目里放一个 **`.smlex`** 文件，存**必要的 profile**
  （针对某一类文档的**特化支持**）；文档里用 `weakdoc` 关键字引用它。
- **前提**：profile 需要 **install**（先装才能引用）。
- **硬性约束（用户定，逐条钉住）**：
  1. **不能用于注入** —— 只做「声明/引用」，绝不往文档里注入内容或行为；
  2. 只能以 **`weak*`** 形态存在，且**不能叠加自身**（同一个不能重复挂）；
  3. **可以有不止一个叠加**（不同的 profile 可同时挂）；
  4. **拓展负责检查**：叠加后若冲突，**直接报错**。
- **与 I-5 `weakbind` / I-15 `weakdata` 的关系**：三者同族 —— 库侧忽略、
  交给拓展解释。**weakbind = 契约、weakdata = 数据、weakdoc = 文档 profile**。
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
  | 4 | 正式（无后缀） | 稳定 | ① 安全：`cargo audit` + `miri_check.py` + `osv_check.py` + fuzz 全绿（结果贴 `CHANGELOG.md`）；② conformance **27/9→0**；③ 性能基线达标（⚠️ 原写「`docs/swsml/archive/iter-parse.md` §1.4」，**该节不存在、文件里也没有「性能基线」字样** —— 悬空引用。基线本身要先建，见 I-23） |

  **bump 纪律**：平时不 bump，发版才重排 —— `dev` 积量期间版本号保持不变；正式发版时用
  `workspace.version` 一次性统一主 crate 与子 crate（当前 8 个 crates 版本各自漂移，属历史包袱）。
  配 `--check-version`/CI 门禁，防「没到门槛就 bump」回归（见 `TODO.md` §六·〇）。
- **关联**：I-12 改名（名字与版本通常一起发）、`TODO.md` §六·〇（bump 纪律）与 §八·二（publish 顺序）。
- **Risk / 风险**：后缀一多就成「版本号噪音」⇒ 种类要少（只 `-beta` / `-rc` 两档 + 正式），
  不设「过渡性」的第三种后缀。

---

## I-18 持久化：`.smlb` 二进制格式 + `smldb`（规范已写、**零实现**）

- **Status / 状态**：`Todo`（规范已定、零实现；**细节未定，用户 2026-11-03 明说「smlb 等的一些细节我还没定好」⇒ 只登记，不动手**）。
- **2026-11-04 用户裁决**：**还需商榷** —— 继续暂缓，不进排期；同期让位给 `weak*`（I-5 / I-15 / I-17）。
  ⚠️ 连带：`TODO.md` §六 的「发布准备」原定「等 I-18 + I-19 一起发」，前提已不成立，需重定节奏。
- **出处**：`docs/amberice/spec/format.md`（881 行，2026-11-02 入库；文件头挂「内容可能再议」横幅）+ `docs/swsml/spec/value-layout.md`（324 行）。
  这两份**长期不在 `TODO.md` / `IDEAS.md` 里**，只躺在 HANDOFF §4.5，2026-11-03 被打扫出来才登记。
- **已写清的部分**（`【已定】` 仅代表写作时的判断，不代表批准）：
  - 内存布局：标签 interning + 实体解码 + 契约定长槽位（`value-layout.md`）
  - `.smlb` 格式：magic `SWSB` + 全局字符串表 + 前序排列 + `subtree_sz`；
    实测 2 MB / 52,000 节点：**5032 K 文本 → 867 K 裸 → 540 K + zstd -3**
  - 场景分野：冷数据 mmap、传输走 zstd；`smldb` = SQLite BLOB **存原文、绝不拆行**；fsync 是真正的悬崖
  - 后端选型：SQLite（决定性理由 = Turso 没有 FTS5）；SIMD 暂缓
  - 落地顺序：`.smlb` + 零拷贝 + 选择器 → ser/de → `smldb` + rusqlite →（才）碰数据库，**`sml` 保持零依赖**
- **执行前必须先定的**（`spec` §10 五条 + 三个前提）：
  1. FTS 要 / 不要（要 → 锁死 SQLite）
  2. `:has()` 进不进 v1（建议暂缓）
  3. `.content` 是否等价 `[class~=content]`
  4. `id` 是否强制唯一
  5. 规模信封（单文档典型/最大、文档数量级、写入频率、持久性）—— 未知则按 §5.9 的 8 MB 自适应
  - ⚠️ **模型前提最要紧**：`.smlb` 现在描述的是 **HTML 式节点**（`tag_id` / `nattrs` / `text_id` / `nchildren`），
    与 `Value`（对象 / 数组 / 标量）**不对应** ⇒ 「编 `Value` 还是另建 AST」不定，前两步无法开工
  - ⚠️ `value-layout.md` 的 9 条验收**一条未打勾**、6 个未决问题，且与 `.smlb` 的 canonical 排序**尚未对齐**
- **关联**：`I-20`（长文档）、`preserve_order`（是它能否「源序进源序出」的前提）。

## I-19 品牌门面（瘦别名）/ 国际稳定发布线：`swml`（原名 `smlkit`）

- **Status / 状态**：**`swml` 已建、未发布** —— `rust/swml/src/{lib,easy}.rs`，
  `publish = false`，`smlkit-easy` 已并入它的 `easy` 模块（原 crate 不再存在）。
  发布状态与口径冲突见 `docs/swml/status.md`。
- **2026-11-04 用户裁决**：**还需商榷** —— 名字虽已定，但「建门面 crate」这一步暂缓，不进排期；
  同期让位给 `weak*`（I-5 / I-15 / I-17）。
- **2026-11-06 用户裁决（改名 + 分层）**：`smlkit` → **`swml`**，`swsml` 做**幕后 core**；
  `smltools` 保持原名，另加 `swmltool` / `swmltools` 作防御（须合规，见 `TODO.md §11.1`）。
  同时明确「**`smlkit` 和 `amberice` 定位不一样**」⇒ `amberice` 不是本条的名字，落点另记
  （`TODO.md §11.4` 待一句裁决）。
- **工具面边界（2026-11-06 用户点破，已执行）**：**`smltools` 只做用户面** —— 转换 / 迁移 / lint /
  编辑器集成；**仓库自身的治理**（文档索引 / 体检 / 门禁 / 发布检查）不进 `smltools`，
  各开 `publish = false` 的内部 crate（首个是 `rust/smldocs`，见 `TODO.md §14.3/§14.4`）。
  判据一句话：**一条命令如果换一个仓库就毫无意义，它就不属于产品 CLI**。
  这条同时是 I-19 的边界：门面/别名解决「找不到」，内部工具解决「维护本仓」，两者不得混进同一个包。
- **为什么否掉 `idyml`**：它**不是**被占用（实测 crates.io / PyPI / npm 三处全空），问题是**误导** ——
  `-yml` 会让名字主动说错话，让人以为「又一个 YAML」。⇒ 由此立下一条**排除规则**：
  **名字里不许出现 `yml` / `yaml` / `ml` / `json` / `toml` / `ini` 词根。**
- **实测占用（2026-11-03，逐个数查过）**：

  | 名字 | crates.io | PyPI | npm |
  |---|---|---|---|
  | `sml` / `sml-rs` / `idml` | **已被无关项目占用**（Standard ML / 智能电表协议） | — | — |
  | `swsml` | 本仓（已发布） | 空 | 空 |
  | ~~`idyml`~~ | 空 | 空 | 空 |
  | **`smlkit`**（已作废） | **空** | **空** | **空** |
  | **`swml`**（2026-11-06 改名，实测当天） | **空（404）** | **空（404）** | **空（404）** |
  | `swmltool` / `swmltools`（2026-11-06 实测） | **空（404）** | 未查 | 未查 |
  | `sml-easy` / `smlbase` / `smlcore` / `smldoc` / `smlx` | 空 | 空 | 空/未知 |

- **想法**：`swml` 作为**品牌门面**（瘦别名，`pub use swsml::*` 级别，零新 API），
  面向国际用户、作为比 `swsml` 更保守的发布线（只跟 swsml 稳定 minor）。
  它与 `smlkit-easy`（渐进式 API 层，见 `TODO.md` §三·十二 I 组）构成一族：
  **`swml` = 门面，`smlkit-easy` = 入门层（是否改名 `swml-easy` 待裁），`swsml` = 底层实现（不改名）**。
- **明确不做**：不 fork 实现、不翻内部注释；`swml` 这个门面**不进本仓 workspace**
  （依赖走 crates.io，独立仓库），而 `smlkit-easy` 现在先在本仓里开发（`publish = false`）。
- **落地顺序（建议）**：① `smlkit-easy` 的 L1/L2 先做完（它才是「装了不会用」的解药）；
  ② 等 `.smlb` / `smldb`（I-18）就位；③ **0.7.0 一次性发** `swml`（门面）+ `smlkit-easy` + `swml-smlb`/`db`（若做）。
- **发布前必须做**：GitHub org / 域名 / docs 子域三处一起占（否则「名字不撞」只解决一半）；
  再跑一遍三生态占用检查（crates.io / PyPI / npm，命令见下）。
  查占用用：`Invoke-WebRequest https://crates.io/api/v1/crates/<name>`（404 = 空）。
- **与 I-12 的关系**：I-12 是「**把 `swsml` 自己改名**」—— 成本最高（已发布、11 个 crate、SONAME、文档）；
  本条是「**另开一个门面**」，成本低得多。**现状：不动 `swsml`，只加 `swml` 一族。**

## I-20 长文档产出链：EPUB / 独立 HTML / 分章 / TOC / 增量 emit（**未做**）

- **Status / 状态**：`Partial`（2026-11-03）—— **P1 的目录 / 锚点已落地**，P0 的 `--to html` 早已存在
  （本条旧记录说「取值只有 md|xml|svg|latex|slint|lvgl|custom|sml」是**过期**的），
  仍缺 `--to epub`、`--split-by section`、增量 emit、emit 源定位。
- **出处**：`story/ISSUE_smltools_缺失功能.md`（110 行，2026-09；同样长期未登记）。
- **场景**：35 万字 SML 长篇小说，7 个后端（sml/md/svg/slint/lvgl/latex/xml）**全部成功 emit** ⇒
  解析与基本转译是健康的；缺的是「写长文」这条工作流。
- **缺的四件**，按源文件优先级：
  1. **P0 `--to epub` / `--to html`**：现取值只有 `md|xml|svg|latex|slint|lvgl|custom|sml`；写小说只能先转 md 再靠 pandoc，链路断、还丢 `label` 锚点
  2. **P1 `--split-by section` / `--toc`**：35 万字只产出**单个大文件**；`emit/markdown.rs` 的 `topic`/`section` 已能出 `h1..h6` + `id` 锚点，**基础设施具备、CLI 层没暴露**
  3. **P2 增量 emit**：每次整篇重转，改一处要重算 35 万字
  4. **P3 emit 失败缺源定位**：深度超限只给通用文案，没有「第几行 / 哪个块」——长文档排错极难
- **✅ 已落地（2026-11-03）**：
  - CLI 新增 `--toc`（`md` / `html`）：条目从**值树**抽（`h1..h6` / `topic` / `section`），
    **有 `label` 的才是链接**（没锚点不瞎造），markdown 生成列表、html 生成 `<nav class="toc">`
    并插在 `<body>` 之后；实现见 `rust/smltools/src/toc.rs`（4 条单测）。
  - 为让链接有着落：markdown 与 html 的 `h1..h6` **有 `label` 时**输出 `id` 锚点
    （`topic` / `section` 本来就有）。
  - 🚨 顺带修一处**破坏长文档**的既有缺陷：markdown 的 `p` 段原先跟着 `depth` 加缩进，
    行首 4 空格在 Markdown 里就是**代码块** ⇒ 整篇正文被渲染成代码块。现固定不缩进。
  - 仍未做：`--to epub`、`--split-by section`（按章节拆文件）、增量 emit、emit 失败源定位。
- **另一条遗留约定**：`@feature base` **只认版本号**，所以「在文档里声明书名」没有承载位；
  CLI 现在按顶层 `title:` 字段推断（**已是事实约定**），要正式化需单独立个约定。
- **关联**：`I-21`（部件目录）、`I-18`（长文档的持久化）。

## I-21 语义部件目录：figure / admonition / footnote / deflist / sub·sup / toc + **跨后端一致**

- **Status / 状态**：`Partial`（2026-11-03）—— **跨后端漂移三修已落地**，六类部件仍未做。
- **出处**：同 `story/ISSUE_smltools_缺失功能.md` §四 / §五。
- **markdown 后端已有**：`h1-6 / p / ul / ol / li / table / code / blockquote / hr / a / img / em / strong / del /
  topic / section / para / quote / math / theorem / proof` + 内联角色 `{ref:}{term:}{math:}{em:}{strong:}`。
- **缺的六类部件**：`figure`/`figcaption`（带编号图注 + 交叉引用）、`admonition`/`callout`、
  `footnote`（`MarkdownOptions` 注释里列为 v2 规划、**代码没有分支**）、`definition list`、`sub`/`sup`、`toc` 生成。
- **跨后端语义漂移**（2026-11-03 已修完三项，见下节；`html` 后端更早已有自己的 `apply_roles`，本条旧说法「只在 markdown」已不成立）：
  - ~~`theorem` / `proof` 只在 markdown 走 raw-HTML，latex 退化成 `description` 列表~~ ✅
    已改为 `theorem` / `proof` 环境（标题进 `[...]`），导言区自动补 `\usepackage{amsthm}`
  - ~~`math` 块在 markdown 里硬编码 `<div class="math">`，不响应 `--math`~~ ✅
    现在 `--math` 同时管 markdown（`$$…$$` 原样透传、不转义）与 latex；
    CLI 的 `--math` 也已接进 `MarkdownOptions`（此前只有 latex 收到）
  - ~~内联角色只在 markdown 实现~~ ✅ 抽了共享切分器 `emit::expand_roles`：
    latex 翻成 `\hyperref[]{}` / `\emph{}` / `\textbf{}` / `$…$`（`{math:}` 受 `--math` 控制且共用危险原语检查）；
    svg / slint 折叠成**显示文本**（`flatten_roles`，这两个后端只能放纯文本）。
    **xml / lvgl 刻意不动**：它们是**结构透传**（SML 键 → 元素名），改文本内容等于篡改数据。
- **验证（2026-11-03）**：`rust/tests/emit.rs` 新增 10 条（markdown math 三态 / latex 定理与证明环境 /
  latex 角色翻译 / 行内公式的危险原语守卫 / svg+slint 折叠），全量 `cargo test` 唯一红仍是本地探针。
- **UI 侧的「部件库」**：slint / lvgl / svg 是**纯透传**（`__type` 直接当元素名），没有 `card`/`dialog`/`tab`/`chart`/`navbar`
  这类复合部件映射 ⇒ SML 在 UI 场景停留在「另一种 XML」。建议做成 `emit-*` 的可选扩展，不改纯透传默认。
- ⚠️ **既有约束（用户已定不修）**：svg/slint/xml/lvgl 把中文键名 sanitize 成下划线 ⇒ 中文部件名在这些后端无效。

## I-22 下游方言反馈的六条语言设计问题（**待明确 · 未登记过**）

- **Status / 状态**：`Todo`（问题清单，非方案；来源是下游真实用法）。
- **出处**：仓库根下那份**未入库**的下游方言说明（122 行，2026-09-12；被 `.gitignore` 的
  `*-SML-EXTENSIONS.md` 规则排除，**文件名含内部项目名，此处刻意不引用**）。
  该下游用 Go 自实现了「表单子集」与「文章方言」，**没有改动 SML 核心语法**，但这六处想要上游给个说法：
  1. **多选枚举语法** `[enum [ a b c ]]` —— 能否上升为 SML 核心写法？（现在是在数组类型里特判 `inner.text == "enum"`）
  2. **布局项的正式化** —— 「只排版、不收集数据」的项现在只能挂在 `@form` 里当无契约键；是否该有 `@layout` / `@ui` 块？
  3. **条件必填 `required_if <expr>`** —— 现在 `visible_if` 只影响展示，服务端照旧按必填校验。**与 I-7 同族**（约束精确控制）
  4. **源码顺序语义** —— 解析器是否保证保留源序？若保证，下游可去掉 `pos` 补丁式的顺序控制。**与 `preserve_order` / R8 的 Q10 相关**
  5. **`@embed` 是否该成为一级概念** —— 跨宿主引用（表单 / 文章 / 视频 / 图片）现在是方言；若上游视作一等能力，下游愿对齐命名
  6. **与 `sml-contract::Pattern` 对齐** —— 下游的 `pattern` 是 Go RE2 方言，上游是受限正则子集；同一份文档两处行为不同怎么收敛
- **⚠️ 纪律**：源文件明写「未经明确指示不要对外发布 / 不要提交给上游」。登记进 IDEA **不等于**对外提出。

## I-23 更激进的性能优化：把「没开的 feature」从运行期判断变成不走的路径

- **Status / 状态**：`Todo`（2026-11-03 用户点题：「记下更激进的性能优化这一项，比如特定 feature 没开时提速」）。
  **方向明确、零实测数据** ⇒ 第一步是**建基准**，不是改代码。
- **Idea / 想法**：现在「特性没开」只是**判断结果为 false**，判断本身照做、代码照编、分支预测照赌。
  更激进的做法是让「没开」的能力在**热路径上根本不存在**：
  1. **编译期能省的（cargo feature 层）**：`emit-*` 后端（已这么做）、`preserve_order`
     （关掉时对象是 `BTreeMap`，省去有序插入与克隆）、`contract` / `pattern` 整段不编译。
  2. **运行期可省的**：`FeatureSet` 是**位掩码**，热路径每次 `features.has(Feature::X)` 都要探位；
     可改为**入口一次性求值**（把判定结果折算成函数指针 / 策略枚举，热路径只做一次间接调用），
     或**按指令分派**（`@when` 分支在集合不含 `when` 时直接短路到错误，不再进入实现）。
  3. **专门快路径**：最常见的一类文档是「纯键值、无指令、无 include」。可以让入口先做一次
     廉价扫描，命中就走**不含指令/包含/契约**的解析循环（物理上不存在那些判断与分支）。
- **Gap / 缺口**：~~全仓没有 benchmark~~ ✅ **2026-11-03 已补**：`rust/benches/perf.rs`
  （零依赖手写计时器，不引 criterion）+ `docs/swsml/ref/perf-baseline.md`。
  基线（release、5 轮取最小）：`small` parse **15.2 µs / 12.8 MB/s**、`large`（909 KiB）
  parse **10.9 ms / 81.4 MB/s**、`deep`（120 层）parse **81.6 µs / 8.5 MB/s**；
  `to_sml` 约为同输入 parse 的 **1/4** 耗时。
  ⇒ 最刺眼的一条：**小文档的单位成本是大文档的 6 倍**，即固定开销主导（入口的版本/特性剥离、
  `FeatureSet` 求交、`ParseOptions` 组装、词法初始分配）。
  （另：I-16 门槛里「性能基线达标（`docs/swsml/archive/iter-parse.md` §1.4）」仍是**悬空引用** ——
  该节不存在，现已在 I-16 就地标注并指向本条。）
- **前置条件（按序）**：~~① 建基准~~ ✅ 已完成；~~② 挑一处 A/B~~ ✅ **已落地（2026-11-03）**；③ 有效再推广。
- **第一处 A/B 结果**（`parse` 加「无 `@` 则不剥离」快路径）：
  small **15.2 → 7.1 µs（−53%）**、large **10.9 → 5.6 ms（−48%）**、deep **81.6 → 52.9 µs（−35%）**；
  一致性套件 AGREE 32 / DIVERGE 8 **未变**。详见 `docs/swsml/ref/perf-baseline.md`。
  **根因**：不是「入口组装」，而是 `scan::strip_features` 的 `compute_string_spans` ——
  它为判断 `@` 是否落在引号串内**无差别扫全文**，扫描常数 × 极短正文 = 固定开销。
- **第二处 A/B 结果**（`compute_string_spans` 惰性求值，只扫 `@` 候选行）：
  含 `@` 的大文档 **10.31 → 6.59 ms（−36%）**。⚠️ 这条只影响**有 `@`** 的文档，
  所以基准里必须有 `large@directive` 这个负载，否则改动量不出来。
- **第三处（深嵌套）结论：不该在这里改**。实测 `deep` 377 ns/层、`deep@60` 373 ns/层
  （每层恒定），而 `flat@120`（120 个键挤在一个 map）**433 ns/键、比嵌套更慢**
  ⇒ 瓶颈是「每个键的 `String` + map 节点分配」，与嵌套无关。真要动是**键的表示**
  （小字符串内联 / interning / inline 小 map）⇒ 转 `docs/swsml/spec/value-layout.md` 与 I-18。
- **推广**：快路径已覆盖 7 个入口（`parse` / `parse_versioned` / `parse_allowed` /
  `parse_with_features(_env)` / `parse_with` / `parse_with_warnings`）。
- **分配归因（2026-11-03）**：基准里加了零依赖 `GlobalAlloc` 计数器 —— 边际成本
  **每键 6.9 次分配**（词法 ≈2.8 + 解析 ≈4）。⇒ 每键 ≈400 ns 就是「7 次 malloc +
  一次 BTreeMap 插入」的钱。**再往下只能动表示**：`Tok::Word(String)` → 借用切片 /
  键 interning / inline 小 map —— 三条都是 lexer→parser→value 的跨 crate 变更，
  与 `docs/swsml/spec/value-layout.md` 和 I-18 的字符串表设计绑在一起，**不在性能任务里做**。
- **⚠️ 硬约束（想清楚再动手）**：**文档里的 `@feature enable/disable` 是运行期、文档级开关**，
  所以**不能**把 SML 特性位编译掉 —— 能被编译掉的只有「调用方在编译期就决定」的那部分
  （`preserve_order`、`emit-*`）。而仓库里 **cargo feature 与 SML 特性同名**（`when` / `for` /
  `contract` 两边都有），这是最容易踩混的地方：改之前必须逐项确认「这个名字在每一层指的是谁」。
- **Risk / 风险**：编译期 cfg 与 17 个 SML 特性**组合爆炸**（CI 编译时间 + 测试矩阵翻倍）；
  为一个未证明收益的优化引入双份实现，等于把维护成本永久翻倍。
- **关联**：I-16（正式版门槛要看性能，但它的引用是悬空的）、`preserve_order` 的 7 处转发
  （`rust/Cargo.toml`，任何新增「吃 `sml-value`」的 crate 都会多一个漏点）。

---

## 价值评估 / Value check（用户判断优先，成本是我估的）

| 条目 | 有用吗 | 成本 | 建议 |
|---|---|---|---|
| I-5 `weakbind` | **有用，但慢**：无宿主需求时不急 | 低 —— 落在外置指令 `Discard`，零词法改动 | 先留口子，等真有插件再说 |
| I-6 `featag` | **用户已定：有价值** —— DSL 场景的表达力与可读性提升（`rectangle size 2` 比一堆字段直白）。我的保留意见只保留为**风险**：它等于一门小宏语言，须先划清与 `&frag` / `@when` / `@for` 的边界，且别做成「只有 Rust+JS」 | 高 —— 新内置指令 + 五端一致性 + 规范页 | 按用户判断推进；**先写规范草案**，Q1–Q8 定稿再动手 |
| I-7 约束精确控制 | **最划算**：不止拼错 —— 缺失 / 最小 / 最多 / 互斥都要能表达 | 中低 —— 互斥等先做成 Modifier 就只动 Rust | 若要挑一条先做，挑这条；①②③ 已落地（2026-10-28 / 10-30），**④⑤ 已落地（2026-10-30）**，仅剩「按前缀放行额外键（`x-*`）」—— 属新语法，按「Rust 先行、五端统一同步」留到后续 |
| I-8 TOML `=` | 有用（迁移/复制粘贴友好） | 中高 —— 五端词法 + 旧文档行为可能变 | `=` 可做且要 opt-in；`[name]` 维持搁置 |
| I-9 近似契约名提示 | **便宜且立刻见效**：错误现场就能给「你是不是想写 X」 | 低 —— 不改码，只加文案；编辑器侧另算 | 可与 I-7 同批做 |
| I-10 团队 / 宣传 | 有用，但**不能靠写文档推进**：缺的是入口（CONTRIBUTING / 模板 / 上手页） | 低到中 —— 纯文档，不碰代码 | 先补入口 + 能力矩阵，再谈宣传 |
| I-11 中文可剥离 | 有用（`idyml` 与嵌入式都要轻量）；**先数据化，再谈剥离** | 中 —— 触及 `sml-codes` 全部文案与生成链路 | 分两步，别一步到位 |
| I-12 改名 | **已否决（2026-11-03 用户裁决）**：`swsml` 维护成本会大幅增加（11 个 crate + SONAME + C-ABI 头 + 两远端 + 域名），收益只是「好搜一点」 | ~~高~~ 不做；好名字改由新 crate 承载（I-19 `smlkit`） —— 原评估：包名 / lib 名 / SONAME / 头 / 域名 / 双远端 | 先定名（避开 `yml` 词根）再抢注，给共存期 |
| I-13 实现即文档 | **用户已定：有价值** —— 闪光点决定别人要不要点进来 | 低 —— 改流程，不新增文档体系 | 就近沉淀「语义 + 例子 + 陷阱」；例子必须可跑 |
| I-14 仓库排雷 | **用户已定：要做** —— 「测试全绿」不一定「验证到位」 | 中 —— 四类雷，一次只排一类 | 先出清单再修；每条补一条真能跑的测试 |
| I-15 `weakdata` | 用户点名记住；拓展侧真需要（高亮配置 / 自定义提示） | 概念阶段，设计后才知 | 先定形态与边界，与 `weakbind` 划清；落地要进文档 |
| I-17 `weakdoc` | **用户已定：要做** —— 一类文档一份特化 profile，比逐份文档重复声明干净 | 中 —— 引入 `.smlex` + install 机制 | 死守「不能注入」：解析结果对有无 `.smlex` 必须一致，且可测 |
| I-16 版本号规则 | **方案已定（2026-10-30）** —— 四级门槛 + semver `-beta`/`-rc` 后缀；平时不 bump、发版才统一 | 低 —— 判据已写进 I-16 与 `TODO.md` §六·〇 | 落地后给 §六·〇 补 bump 纪律；`--check-version` 门禁随发版一并做 |
| I-18 `.smlb` / `smldb` | **用户 2026-11-03 点名登记，但明说「细节还没定好」** ⇒ 现在要的不是实现，是**把 §10 五条 + 模型前提拍掉** | 高 —— 新二进制格式 + 新 crate + SQLite 后端 | **先拍「编 `Value` 还是另建 AST」**，这条不定，前两步（`.smlb` 与 ser/de）无从开工 |
| I-19 `smlkit` 品牌门面 | 有用：给国际用户一条**保守发布线**，成本比给 `swsml` 改名低得多。**名字已定 `smlkit`**（2026-11-03；三生态实测全空；`idyml` 因误导 YAML 被否） | 低到中 —— 一个只剩 `pub use` 的 crate + 独立仓库 + CI | 先做 `smlkit-easy` 的 L1/L2（那才是解药），**0.7.0 与 `.smlb`/`smldb` 一起发**；发布前把 GitHub org / 域名 / docs 子域一起占 |
| I-20 长文档产出链 | **有用且缺口硬**：EPUB / HTML 是「用 SML 写小说」的终点；分章 + TOC 基础设施已具备、只差 CLI 层 | 中 —— `--to epub`/`--to html` 各一个后端；分章 / TOC 轻 | `--to html` 本就有；`--toc` 与标题锚点 **2026-11-03 已落地**（顺带修了「段落被缩进成代码块」）；下一步 `--split-by section`，再 EPUB，增量 emit 与 emit 定位往后放 |
| I-21 语义部件目录 | 有用，但**先修跨后端一致**比加新部件更值（`theorem`/`math`/内联角色只在 markdown 生效，属语义漂移） | 中 —— 每个部件 × 各后端 | ①②③ 已于 **2026-11-03 全部落地**（内联角色补齐到 latex/svg/slint、`math` 响应 `--math`、latex 定理/证明环境）；下一步才是 figure/admonition/footnote/toc，UI 部件库最后 |
| I-23 更激进的性能优化 | **有用但目前不可测**：仓库里**一个 benchmark 都没有**，谈提速全是空话 | 低（先建基准）→ 中（改热路径）→ 高（编译期切分） | 只做第一步：`criterion` 三组基准 + `docs/swsml/ref/perf-baseline.md`。没有数字之前**不要**动双份实现 —— 那会把维护成本永久翻倍 |
| I-22 下游方言六问 | **只对上游有参考价值**：3（`required_if`）与 4（源码顺序）与 I-7 / `preserve_order` 直接相关，值得吸收；1/5 属方言口味，2 属「布局 DSL」倾向 | 低 —— 现在只是问题清单 | 先把 3、4 两条并入 I-7 与 `preserve_order` 讨论；其余**未经用户明确指令不得对外提出** |

> 一句话 / One line：**I-7 + I-9 是同一件事的两半**（约束能表达 / 写错能看见），性价比最高；
> I-6 按用户判断推进（DSL 可读性），但先定 Q1–Q8；I-5 便宜缺需求；I-8 别碰 `[name]`；
> I-10 / I-11 是对外层（团队与国际化）。**I-12（给 `swsml` 改名）已于 2026-11-03 否决** ——
> 维护成本远大于收益；「好名字」改由**新 crate** 承载（I-19 `smlkit` 族），老包与 SONAME / C-ABI 全不动。