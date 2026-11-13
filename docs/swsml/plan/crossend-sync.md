# swsml 跨端同步施工计划（切片 · 未开工）

> 把 `docs/repo/ref/unfinished.md` §2 的主线 A 切成**可独立验收、可单独回滚**的片。
> 本文是施工依据，不含代码；解析器改动一律等 §0 的裁决落定后再开工。
> 能力对等原则与现状判据见 §0.6。
>
> 数字口径：`docs/repo/ref/scan.md`（`python tools/scan_status.py --write` 生成）、
> `site/static/errors.json`（`errors/codes.sml` 的生成物）、
> `python tests/conformance/run_conformance.py --require-all` 的实测输出。
> 计数不手写，复核命令见 §5。

---

## 0. 开工门槛（状态）

| # | 门槛 | 为什么 | 裁决结果 |
|---|---|---|---|
| **G1** | 五端策略裁决表 | `docs/swsml/spec/weakbind.md`（`draft`）§8 列了四端识别 `@weakbind` 的清单；`docs/swsml/plan/featag-slices.md` S9 明确 C / C++ / JS / Lua 不实现 featag。哪些特性强制五端同形，没有统一裁决表 | ✅ 已裁：本文 §0.1 逐特性裁决表（2026-11-12）；不并入 `docs/repo/ref/pending-decisions.md` 的 D1–D6 已裁决清单 |
| **G2** | v5 定稿 | `50 "camelia" dev` 未启用、`docs/swsml/spec/v5.md` 仍 `draft`；四端实现未定语义必然返工 | ✅ 已裁「v5 没定稿，先做多端同步」：v5 保持 `draft`，表 A 逐条实测结论登记为**发布前置**，A8 / A12 实现后置，`50` 关闭；**不阻塞本批** |
| **G3** | weak* 规范定稿 | `weakbind.md` / `weakdata.md` / `weakdoc.md` 均为 `draft`，按 `AGENTS.md` 只能看不能用 | ✅ 已裁「先定稿再同步」：状态矛盾已修；weakdoc 提示定为**带码 warning**；**默认忽略口径见 §0.3（当前实现不符，属 P4 前置修复）**；三份转 `current` 后才排 P4 |

## 0.1 五端策略裁决表（2026-11-12 用户裁决）

本批范围：**五端全做**（`rust` / `js` / `c` / `cpp` / `lua`）。策略三档：
「同形」= 四端产出与 Rust 一致的值树与码；「响亮拒绝」= 明确报错而非静默吞掉；
「单端先行」= 维持 `impls: [rust]`、不排期。

| 特性 / 族 | 五端策略 | 落点 | 依据 |
|---|---|---|---|
| 老 17 位 + baseline（D 组 16 条码） | **同形** | rust / js / c / cpp / lua | 本批裁决；P2 |
| `when` / `for`（E 组 6 条码） | **四端实现**，不走响亮拒绝 | rust / js / c / cpp / lua | 本批裁决；P3 |
| `type.featag` + featag 族（A 类 10 条码） | 单端先行 | rust | featag-slices S9（已有裁决） |
| `lex.ident`（A 类 1 条码） | 单端先行 | rust | featag-slices S1b（暂不生成五端表） |
| `type.weak` + `@weakbind` / `@weakdata` / `@weakdoc`（B 组 3 条码） | **先定稿再同步**，之后五端同形（含软校验与旁路表，不按消费者裁剪） | rust / js / c / cpp / lua | 本批裁决 + §0.6 能力对等；P4 前置 G3 |
| `W-PARSE-001` / `W-PARSE-002`（C 类 2 条码） | **四端补等价 API**：各加「成功 + 诊断列表」出口 | rust / js / c / cpp / lua | 本批裁决；属公开 API 破坏，发布口径须同步；P4 |
| `@weakdoc` 引用的 profile 未命中 | **带码 warning**（`W-CLI-001`），工具侧产出、不阻断解析 | `smltools` + 8 份生成物 | 本批裁决；见 §0.4 |
| `Edition` / `@version` | 五端同形，但 **v5 未定稿 ⇒ P5 移出本批关键路径**，`50` 关闭 | rust / js / c / cpp / lua | 本批裁决「v5 没定稿，先做多端同步」 |
| `@requires`（`E-CONTRACT-018`） | 同因同码 | rust / js / c / cpp / lua | 现有五端同源断言 |
| **FFI / C-ABI / 绑定层** | **功能对等**：Rust 能交出的结果，FFI 与五个绑定层都要能交出（机制不要求同形） | `rust/src/c_abi.rs`、`c/sml_rs.h`、`cpp/sml_rs.hpp`、`bindings/*` | 2026-11-12 原则裁决；判据见 §0.6；P7 |

⚠️ 本批裁决**覆盖**两条旧暂缓口径：`T06 / T11 / T48`「先不动 Lua」与 `T43`「默认路径 = 不动 C++」。
docs 台账与 `TODO.md` 三处旧句均已回填。仍需遵守的既有裁决：featag 四端不实现（S9）、
T44「native 可先不做」、同步节奏「发布前必做」（`IDEAS.md` I-5）。

⚠️ G3 定稿完成前，`@weakbind` 不得进共享语料（`IDEAS.md` I-5 的开发期硬约束）：
`site/content`、`site/data/sml-lessons*`、`examples/`、`tests/conformance/` 一律不写。
P1 的 D / E 组用例不含 weak\*，不受此限。

### 0.2 G3 定稿剩余项（转 `current` 前）

| 文档 | 剩余项 |
|---|---|
| `docs/swsml/spec/weakbind.md` | §7 第 2 条「片段名豁免」不再是裁决项：按 §0.3 的默认忽略口径，转 `current` 时把结论写回；§7 第 1 条已被 §7.1 回答（`type.weak`），一并移出待定表 |
| `docs/swsml/spec/weakdata.md` | 无独立待裁项；进度表里「其余四端 + 编辑器 + 绑定层」的未勾项需按五端全做回填 |
| `docs/swsml/spec/weakdoc.md` | §3 规则 3 已裁「带码 warning」；`--resolve-profiles` 的未命中退出码 1 保持不变 |

三份 `status` 由 `draft` 转 `current` 要改 `docs/meta.sml` ⇒ 摘要变了必须重跑
`cargo run -p smldocs -- --index` 与 `--check`。

### 0.3 默认忽略口径（已定）与当前实现缺口

**已定设计（2026-11-13 修订）**：未开 `type.weak` 时 weak\* **响亮拒绝**（`E-FEATURE-001`，
五端同码，用例 `53_weakbind_gate`），**不做默认忽略** —— 少一个静默路径，也少一处四端与
Rust 的对照缺口。开了特性之后，弱绑定信息才经**专门 API**（`parse_with` /
`parse_with_warnings`）产出旁路表与软校验诊断。

| 层 | 入口 | 行为 |
|---|---|---|
| 默认 | `parse()` / `parse_file()` | 只交出 `Value`；`rust/sml-parse/src/api.rs:584-585` 直接取 `.value`，把整个 `ParseOutput`（诊断 / 三条 weak 旁路表 / 契约表 / 契约映射）丢掉 |
| 默认选项 | `ParseOptions::default()`（`rust/sml-parse/src/ext.rs:301-315`） | `capture_contracts` / `trace_contracts` / `collect_errors` 全关、内置契约表为空 ⇒ 软校验整段跳过 |
| 专门 API | `parse_with(opts)`（`api.rs:305`）/ `parse_with_warnings` / `parse_file_with` | 返回完整 `ParseOutput`；显式开 `type.weak`、注入 `builtin_contracts` 后才产出 `weak_binds` 与 `W-CONTRACT-001` |

⚠️ **实现与设计不符（P4 前置修复）**：`BUILTIN_DIRECTIVES` 登记 `weakbind` 后，
`rust/sml-parse/src/parser.rs:1478` 的分支对 `fname == "weakbind"` 一律接管 —— 未开
`type.weak` 报 `E-FEATURE-001`，开了就要求契约名，于是 `@weakbind { }` 报 `E-PARSE-019`、
`@weakbind type: X { }` 报 `E-PARSE-006`。语法层**先判定、先报错**，默认路径没走到「忽略」；
这与 `weakbind.md:12` 的「默认关、v1–v4 既有文档一字不变」相冲。

⇒ 因此「片段名豁免留/不留」**不是裁决项**：按默认忽略口径，`@weakbind { }` 应落回片段定义 /
普通数据（与 `parser.rs:1446-1449` 替 `@when { .. }` 保留的让路同理），不抢占名字；
只有专门 API 开了 `type.weak` 才进入弱绑定语义。P4 先修默认路径，再谈专门 API 的能力对齐。

### 0.4 weakdoc 带码 warning（已确认并登记）

| 项 | 结论 |
|---|---|
| 码 | `W-CLI-001`（CLI 域序号 001 空位） |
| 触发 | 文档显式写了 `@weakdoc`，且项目 / 用户 / 系统三处命名空间均未命中 |
| 行为 | 解析成功、退出码 0、主树逐字节不变；stderr 多一条不阻断的 warning |
| 边界 | `--resolve-profiles` 的未命中退出码 1 不变（显式查询命令）；库不读文件系统，四端 i18n 表能查到该码但不自行报 |
| 状态 | 中英文文案已登记进 `errors/codes.sml` / `errors/msg.en.json`，8 份生成物已重生成；**提示逻辑属 P4，未实现** |
| `impls` | `[ smltools ]` / `partial`（工具层码，P4 落地前标部分完成） |

### 0.5 发布口径（四端 API 破坏，2026-11-12 定）

| 项 | 口径 |
|---|---|
| 变更性质 | C / C++ / JS / Lua 各加「成功 + 诊断列表」出口 ⇒ **公开 API 破坏** |
| 版本 | 按 `CHANGELOG.md` 文首的 Cargo 0.x 语义（MINOR = 不兼容），下一个正式版为 **0.7.0**；`swsml` 与 `smltools` 版本号互相独立 |
| 记录 | `CHANGELOG.md` / `CHANGELOG.en.md` 的「未发布」段登记为**预告**；「库 API 破坏」与「文档语法破坏」分栏不混 |
| C-ABI | 新增 `CSmlDiagnostic` / weak 旁路表 / 扩展选项结构时，同步 `rust/src/c_abi.rs`、`c/sml_rs.h`、`cpp/sml_rs.hpp` 与五个绑定层；**只并存新符号、不改旧签名**（`sml_dump_err` 先例）。能力对等判据见 §0.6 |
| 不擅自做 | 不 bump 版本号、不发版、不动 `E-FEATURE-004` 启用位（bump 属发版动作，按 `TODO.md` 铁律） |

### 0.6 能力对等原则与现状矩阵（2026-11-12 定）

**原则**：`swsml`（Rust）能做的事，FFI（`rust/src/c_abi.rs` / `c/sml_rs.h` / `cpp/sml_rs.hpp`）
与其它端（含五个绑定层）**功能上都要有**。判据是**功能等价 / 可观测结果一致**，
不是 API 形态、符号名或内部机制逐一同形：同一份输入在任一端应得到同一棵值树、同一组诊断与
旁路数据、同一组错误码。**不要求照搬 Rust 特有机制** —— `with_type` / `with_modifier` / `directive`
这类传 trait 对象 / 闭包的外置注册点，宿主可用自己的惯用机制达成同样的注册与生效效果，
不必改成函数指针 + 回调。`docs/swsml/spec/weakbind.md` §8 的「绑定层需确认各自暴露的解析结果结构」
由此回答：功能一致即可。不以「当前没有消费者」为由永久裁剪；允许按切片分批落地，但每片都要逐项打勾。

⚠️ **两层别混**：Rust → C-ABI 按**功能对等**；C-ABI → 五个绑定层仍按 `bindings/README.md` 的
既有纪律**逐符号对齐**（绑定层绑定的就是 C-ABI 符号集）。P7 扩的是 C-ABI 符号集，绑定层随后
按新符号集对齐，不要求把 Rust 的 trait 对象搬进 FFI。

Rust 的能力清单在 `rust/sml-parse/src/ext.rs`：`ParseOutput` 八个字段
（`value` / `diagnostics` / `errors` / `contract_map` / `contracts` / `weak_binds` / `weak_data` / `weak_docs`）
与 `ParseOptions`（`capture_contracts` / `builtin_contracts` / `trace_contracts` / `apply_contracts` /
`collect_errors` / `warn_key_conflict` / `ordered` / `allow_top_scalar` / `directive` / `with_type` / `with_modifier`）。

| Rust 能力 | C-ABI + 两个头文件 | 原生四端 | 五个绑定层 | 落到哪片 |
|---|---|---|---|---|
| `value` / 富错误 | ✅ `sml_loads` / `sml_parse` / `CSmlError` | ✅ 各有入口 | ✅ `loads` / `load_file` / `dumps` / `SmlError` | 维持 |
| `diagnostics` | ✅ `sml_parse_with_diags` + `sml_diag_list_free`（文本入口） | ✅ C `sml_parse_ex` + `sml_diag`／C++ `Parser::parse(..., &diags)`／JS `parseSafe().warnings`／Lua `Sml.parse_with_warnings` | ✅ 五门 `loads_with_diags` 一族 | P4（✅ 原生）＋ P7（✅ 文本入口） |
| `errors`（`collect_errors`） | ✅ `sml_parse_all(..., SML_WANT_ERRORS, ...)` + `sml_result_errors`（`severity = 1`） | ✅ C `sml_parse_ex` 只报首个错误，其余四端同 | ✅ 五门 `parse_all` 的 `errors` | P4 + P7（✅ 全列） |
| `contract_map`（`trace_contracts`） | ✅ `SML_WANT_CONTRACT_MAP` + `sml_result_contract_map`（`sml_contractmap` 扁平表） | ⚠️ JS 有 `opts.traceContracts` 局部通道 | ✅ 五门 `parse_all` 的 `contractMap` | P4 + P7（✅ 全列） |
| `contracts`（`capture_contracts`） | ✅ `SML_WANT_CONTRACTS` + `sml_result_contracts`（借用）+ `sml_builtin_clone`（宿主独占） | ❌（C++ / Lua 的契约表是内部表，不导出） | ✅ 五门 `parse_all` 的 `contracts`（已克隆） | P4 + P7（✅ 全列） |
| `weak_binds`（`@weakbind` 旁路表） | ✅ `sml_parse_full_lists` 的 `sml_weaklists.binds` + `sml_weaklists_free` | ✅ C `sml_weakbind` 出参／C++ `ParseOptions::weak_binds`／JS `parseSafe().weakBinds`／Lua 第 4 返回值 | ✅ 五门 `loads_full_lists` 一族 | P4（✅ 原生）＋ P7（✅ 全列） |
| `builtin_contracts` 注入 | ✅ `sml_builtin` + `sml_parse_full`（`sml_opts`：flags / env / builtins） | ✅ C `sml_builtin`／C++ `BuiltinTable`／JS `opts.builtinContracts`／Lua `Sml.parse_with_builtins` | ✅ 五门 `loads_full` + `Builtin` | P4（✅ 原生）＋ P7（✅ 全列） |
| `weak_data` / `weak_docs` | ✅ `sml_weaklists.data`（含载荷值树）/ `.docs` | ❌（`@weakdata` / `@weakdoc` 只做识别与门控，载荷不进旁路表） | ✅ 五门 `loads_full_lists` 一族 | P4 + P7（✅ C-ABI／绑定层） |
| 文件入口 + 诊断/弱表 | ✅ `sml_load_file_full(path, opts, lists, diags, err)` | ✅ C `sml_parse_file_ex` / `sml_parse_file_full`；C++ `Parser::load_file(path, opts)`（诊断/弱表走 `opts`）；JS / Lua **设计上不读文件**（宿主读文本 + files map / base），不适用 | ✅ 五门 `load_file_full` 一族 | P7（✅ 全列） |
| 其余 `ParseOptions` 开关 | ⚠️ `flags` 位掩码 + 三字段 | ⚠️ JS 部分 / Lua 仅 `allow_top_scalar` | ⚠️ 仅 `flags` 形参 | P7 |
| `parse_file_with`（文件 + include + 选项 + 诊断） | ❌ 拆成 `sml_load_file`（无选项）与 `sml_parse_ex`（无 include） | ❌ | ❌ | P7 |

⚠️ 现状一句话：`ParseOutput` 八个字段里**只有 `value` 越过了 FFI**。`bindings/README.md` 自述
「逐符号对齐 `c/sml_rs.h` 的 v3 全集」，而该全集本身不含上述能力 ⇒ 绑定层的对齐基线要随 C-ABI
一起扩。新增符号走「并存新符号、不改旧签名」（`c_abi.rs` 的 `sml_dump_err` 先例）。

用例正文与实测输出随交付说明给出，**不落进共享语料**：G3 定稿前 `tests/conformance/` 不收 weak\*。

### 0.7 三端预处理层差异（P3 门控的前置，2026-11-12 定位）

**问题**：`@feature` 在 Rust 是**预处理**（`rust/sml-parse/src/scan.rs` 的 `strip_version` /
`strip_features` 先剥行再词法），三端是**顺序解析**（`@feature` 当普通指令在解析主循环里消费）。
这导致三个门控点在三端做不了或行为分裂：

| 门控点 | Rust | C / C++ / Lua 现状 | 结论 |
|---|---|---|---|
| `top-level-array` | 顶层 `[` 处查位 ⇒ `E-FEATURE-001` | 实测**分裂**：C / JS 静默成功、C++ / Lua 报 `E-PARSE-006` | 需要预处理才知道顶层形态 |
| `env`（引号串） | 值处理时查位 ⇒ `E-FEATURE-002` | C 的 `$env.` 内插在 `lex_run`（词法层），那时还没有特性集 | C 端要先有预处理并把特性集传进词法 |
| `include` / `namespace` | 预处理扫完再展开 | 展开在解析前（C `resolve_includes`、C++ `expand_includes`、Lua `expand_text`），`@feature` 还没读到 | 同上 |

**解法**：三端各加一个 `scan_features(text)` 预处理（扫行首 `@feature enable|disable`、算出
初始特性集），在词法 / include 展开**之前**调用；解析主循环的 `@feature` 分支保留（幂等，
重复置同一位不影响结果）。`@version` 同理，否则版本基线也无法在词法前确定。

**施工点**：

- C：`c/sml.c` 的 `sml_parse` / `sml_parse_file` 入口（`lex_run` 之前）；`lexer` 结构加
  `unsigned features`，供引号串 `$env.` 内插处判定。
- C++：`cpp/sml.cpp` 的 `parse_impl`（`tokenize` 之前）；`interp_env` 加 `features` 形参。
- Lua：`lua/lib/sml.lua` 的 `Sml.load`（`expand_text` / `tokenize` 之前）；`coerce` 加
  `features` 形参（调用点同步）。

**验收**（先落 `cases/pending/`，收敛后再移入 `cases/`）：

1. `@feature disable top-level-array` + `[1 2]` ⇒ 五端同报 `E-FEATURE-001`；
2. `@feature disable env` + `k: "$env.NOPE"` ⇒ 五端同报 `E-FEATURE-002`；
3. 默认文档（不写 `@feature`）五端值树与现在逐字节相同。

**切片**：`P3a`（本片，三端预处理）→ `P3b`（剩余门控点）→ `P3c`（`@when` / `@for`）。

## 1. 范围

| 层 | 端 | 本文是否排期 |
|---|---|---|
| 语言层 | `rust` / `js` / `c` / `cpp` / `lua` | **五端全做**（本批裁决，覆盖 Lua / C++ 旧暂缓口径）；Rust 为参考实现 |
| 工具层 | `smltools` | 随 P4 带三项：`--dump-builtins --to json`、lint 放过弱声明、`W-CLI-001` 提示 |
| 编辑器 / 站点副本 | `editors/*`、`site/static/sml.mjs` | 不单独排期：源码侧已完成（`TODO.md` F 节），四副本由 `check_js_copies.py` 守；`.vsix` 重打属发版动作 |
| C-ABI + 绑定层 | `rust/src/c_abi.rs`、`c/sml_rs.h`、`cpp/sml_rs.hpp`、`bindings/{csharp,go,java,python,ruby}` | **能力对等**（本批原则裁决）：随 C-ABI 扩展逐符号对齐，**单独成片 P7**；不再「随所绑定实现走」 |

三件不做的事：

1. **featag 四端**：S9 已裁决「不实现，遇 v5 报本端不支持」，属设计取舍，不是欠账；
2. **五端语法生成器**：`featag-slices.md` S1b 写明「等哪一端真的要接时再造生成器」，本批维持手写实现 + 共享用例对齐；
3. **不做 v5 定稿、不启用 `50`**：v5 保持 `draft`，表 A 缺口登记为发布前置，Edition 四端（P5）后置；`E-FEATURE-004` 的启用位按 `docs/repo/ref/pending-decisions.md` D1 保持关闭。

## 2. 38 条码：先分类，再排期

`errors/codes.sml` 的生成物里 `impls: [rust]` 共 **46** 条；`DERIVE` 域 **8** 条属 Rust 派生宏 / serde 桥
（宿主绑定层，不是语言层），扣除后是 **38** 条。这 38 条不是同一类工作，混排必然返工。

| 类 | 判据 | 条数 | 码 | 处理 |
|---|---|---|---|---|
| **A 单端先行（已裁决，不算欠账）** | note 指向 featag（S9）或 `lex.ident`（S1b 暂不生成五端表） | **11** | `E-CONTRACT-019`–`025`、`E-PARSE-026`–`028`、`E-LEX-007` | 维持 `impls: [rust]`，裁决表标「单端先行」，不排期 |
| **B weak*（语义未定稿）** | note 指向 weakbind / weakdoc 软校验 | **3** | `E-PARSE-029`、`E-PARSE-030`、`W-CONTRACT-001` | 先定稿三份 draft（G3），进 P4；软校验与旁路表按 §0.6 五端同形 |
| **C 诊断通道（四端无 warning 出口）** | note 明写「其余四端无诊断通道」 | **2** | `W-PARSE-001`、`W-PARSE-002` | **四端补等价 API**（§0.1 裁决），进 P4；属公开 API 破坏，单独成片 |
| **D1 同因同码（四端已有该语法）** | 四端已有触发点，只是码不同 / 静默吞掉 | **2** | `E-PARSE-004`、`E-PARSE-007` | ✅ 已收敛（`42` / `43`，默认套件 43 例全 AGREE） |
| **D2 四端缺能力（原被误并入 D）** | note 要求的机制四端根本没有：`@feature`、外置指令 / 修饰符、多契约叠加、部分引用 / glob-regex、dump 深度 | **14** | `E-CONTRACT-011`、`E-CONTRACT-016`、`E-EXT-002`/`005`/`008`、`E-FEATURE-006`/`007`/`008`/`010`、`E-INCLUDE-007`/`008`/`009`、`E-LIMIT-004`、`E-PARSE-025` | 随 P3 / P4 的前置能力一起做；先补三端 `@feature` 门控 |
| **E 四端缺语法** | `@when` / `@for` 一族 | **6** | `E-PARSE-013`–`018` | **四端实现**（§0.1 裁决），进 P3 |

D1 里两条已收敛（`E-PARSE-004` / `E-PARSE-007`）：四端按 Rust 判据补了「孤立 `@`」
（词法层区分 `At` / `BareAt`）与「裸词逗号」（`looks_like_field_sep`），默认套件 43 例全 AGREE。

**D2 的重分类依据（2026-11-12 逐条核对四端源码）**：D 组原按码表 note 归为 16 条「同因同码」，
但 14 条要求的机制四端根本没有 —— `@feature`（C / C++ / Lua 无，JS 有）、外置指令 / 修饰符
（四端都无）、多契约叠加（四端按设计报 `E-CONTRACT-017`）、部分引用 / glob-regex（四端能力不同）、
dump 深度限制（四端 dump 无上限）。对它们直接改码会变成重复实现前置能力，故并入 P3 / P4。

`unfinished.md` §2 还列了「TODO 里的跨端 7 项」：施工前以 `docs/repo/ref/todo-ideas-tables.md` 的
T02 / T03 / T04 / T05 / T06 / T11 / T21 逐条复核，状态以 T 表为准（T02 / T05 / T21 已结案，
T06 / T11 已由本批裁决纳入五端范围），不按 §2 的行号照抄。

## 3. 切片表

| 片 | 范围 | 依赖 | 验收（必须能演示） | 主要落点 | 文档影响（随片交付） |
|---|---|---|---|---|---|
| **P0** | ✅ 已收官（2026-11-12）：五端策略裁决表、G2 / G3 状态 | 无 | 本文 §0.1 逐特性裁决行已填；A 类 11 条显式标「不排期」 | 仅文档 | §0.1 / §2 分类表已回填 |
| **P1** | 用例先行：为 D / E 两组各建「触发条件 → 期望码」共享用例，先放 `cases/pending/` 跑出预期 DIVERGE 并登记 | P0 | `--pending --require-all` 的 DIVERGE 明细逐条进 `known_divergences.md`；默认 `--require-all` 保持 41 例 AGREE；修好后移入 `cases/` | `tests/conformance/cases/pending/`、`run_conformance.py` 的 `--pending`、`known_divergences.md` | 用例协议与放行项说明 |
| **P2** | **D1 2 条已收敛**（`E-PARSE-004` / `E-PARSE-007`）；D2 14 条不单列，随 P3 / P4 的前置能力一起收敛 | P1 | D1 五端同码且已移入 `cases/`；D2 在 P3 / P4 里按能力补齐后再钉用例 | `js/sml.mjs`、`c/sml.c`、`cpp/sml.cpp`、`lua/lib/sml.lua`、`errors/codes.sml` + 生成物 | `errors/README.md` 进度表、各端 README 带码口径 |
| **P3** | E 组 6 条 `when` / `for`：**四端实现**（本批裁决）。拆三片：**P3a** 三端预处理（`scan_features`，见 §0.7）✅ **已完成**（三端词法前扫描 + `top-level-array` 门控，五端同码，用例 `50_feature_top_array_gate`）→ **P3b** 剩余门控点（`env` ✅ `51_feature_env_gate`、`bareword-string` ✅ `52_feature_bareword_gate`、`include` ✅ 三端单测覆盖；`namespace` 随「三端 namespace 语法」一起做，三端 `as ns` 本就报 001）→ **P3c** `@when` / `@for`：**四端 `@when` 已完成**（裁剪 + 14/15/16/17，pending 45–48 全 AGREE 并移入 `cases/`，默认套件 50 例；C 端处理了「`=` 是独立 `T_EQ`、`==` 是两个 `T_EQ`」的词法差异，C++ 补了 `is_directive` 白名单）；四端 `@for`：**四端已完成**（有界展开 + `${var}` 只读插值 + 键名位置报 013 + 缺枚举项报 018，pending 44 / 49 五端 AGREE 并移入 `cases/`，默认套件 52 例；四端词法都加了 `${var}` 整体词保护）。已完成：三端 `@feature` 识别 + `contract` / `fragment` / `env` / `bareword-string` / `include` 门控 | P0、P1 | 同一输入五端同码且值树一致；opt-in、有界循环、只读变量与 Rust 一致；未开特性时旧文档零影响 | 四端指令分派处（含 `lua/lib/sml.lua`） | 裁决表 + 能力矩阵（`draft`，须显式点名才用） |
| **P4** | B 组 3 条 + C 组 2 条：weak* 识别 / 同码 warning / 旁路表（含软校验，不按消费者裁剪），**四端补「成功 + 诊断列表」出口**；`smltools` 三项（含 `W-CLI-001` 提示） | P0、G3 定稿、§0.4 已登记 | 按定稿后的 `weakbind.md` §9 的 14 个用例逐项核对；五端能交出带行号的 `W-PARSE-001/002` 且不阻断解析；缺 profile 时给 `W-CLI-001` 且退出码 0 | 四端公开 API + `rust/smltools`、`errors/codes.sml` + 8 份生成物 | 三份 weak* spec 转 `current`、`weakbind.md` §8 清单勾完、各端 README 的 API 变更说明 || **P5** | Edition 四端（**不启用 `50`**）：`@version` 字面量 / 代号映射 / 三态 | **v5 定稿**（本批后置） | `docs/swsml/spec/version.md` §4「需要同步的面」逐项；v1–v4 用例五端 AGREE；`50` 仍报 `E-FEATURE-004` | 四端版本判定、`E-FEATURE-004` / `010` 的 note | `version.md`、各端 README、能力矩阵 |
| **P7** | **FFI 功能对等**：C-ABI 补能交出诊断 / 多错误 / 契约表 / 三条 weak 旁路表 / 扩展选项的出口（机制按宿主惯例，不照搬 Rust trait 对象），两个头文件与五个绑定层按新符号集对齐 | P4（四端 API 定型后） | 同一份输入经 Rust 与任一绑定层，§0.6 矩阵每行的**功能结果**一致（值树 / 诊断 / 旁路数据 / 码）；新增符号不改旧签名；`bindings/README.md` 的对齐清单同步 | `rust/src/c_abi.rs`、`c/sml_rs.h`、`cpp/sml_rs.hpp`、`bindings/*` | `bindings/README.md`、CHANGELOG 的 C-ABI 栏、各绑定 README |
| **P6** | 收口：回填台账、重跑扫描、扩展侧回归 | P2–P4、P7 | §4 命令全绿；`unfinished.md` §2 与 T 表状态一致 | 仅文档 + 生成物 | `unfinished.md` / `scan.md` / `index.md` / `report.md` |

**P1 进度（2026-11-12）**：D 组 2 条已收敛并移入 `cases/`（`42` 孤立 `@`、`43` 裸词逗号，
五端同码），默认套件现为 43 例 AGREE 43 / DIVERGE 0；E 组 6 条仍在 `cases/pending/`
（`44`–`49`），`--pending --require-all` 实测 DIVERGE 6 条、退出码 0，明细已登记
`known_divergences.md`。E 组的根因是四端缺 `@feature` / `@when` / `@for`，
P3 顺序须先补三端 `@feature` 门控。

**P4 进度（原生四端已收敛；C-ABI / 绑定层留 P7）**：`W-CONTRACT-001` 七档
（`decl.*` 3 + `block.*` 4）与 `@weakbind` 旁路表在 JS / C / C++ / Lua 全部落地，
入口分别是 `parseSafe()` 的 `warnings` + `weakBinds`、
`sml_parse_with_builtins(..., sml_weakbind **, size_t *)`、
`Parser::parse_with(ParseOptions{ builtins, weak_binds })`、
`Sml.parse_with_builtins(text, builtins, base)`；内置表按各端惯例
（C 的不透明句柄 `sml_builtin` / C++ 的 `BuiltinTable` / Lua 的 table）。
软校验一律**只读**：`weakbind.md` §9 第 1 条「有 / 无 `@weakbind` 主树逐字节相同」
在四端各有回归用例（`c/tests/test_codes.c`、`cpp/tests/test_codes.cpp`、`lua/test_codes.lua`）。
仍未做：`E-PARSE-029`（悬空 / 连续 / 目标不是块）四端仍只有 Rust 报，四端把悬空项**按块丢弃**；
`weak_data` / `weak_docs` 旁路表（C-ABI 已有，四端原生语法层尚未收）。

**P4 工具侧三项收口（2026-11-13）**：`--dump-builtins --to json` 早已落地；
`W-CLI-001` 提示已实现 —— 普通解析写 stderr（`warn_missing_profiles`）、`--lint` 写进报告
（`apply_profile_checks`），两处**同一个码**，`--no-profiles` 不报，`--resolve-profiles`
保持自己的退出码 1；lint 放过弱声明是**构造性成立**的（`W-LINT-002` 只数 `@contract` 定义，
`lint.rs` 的 `builtin_directives_is_single_source` 单测钉住 `@weakbind` 不算定义）。
回归：`rust/smltools/tests/warn_missing_profile.rs`（5 例）+ 原 `profile_checks.rs` 的
「找不到」一例改判为 `W-CLI-001`（与 `E-LINT-005`「找到了但读不出来」分家）。

**P7 进度（诊断出口已贯通；其余能力待续）**：`sml_parse_with_diags(text, flags, sml_diag_list*, sml_error*)`
＋ `sml_diag_list_free` 已在 `rust/src/c_abi.rs` 落地，两个头文件同步；五个绑定层各有语言级封装
（`loads_with_diags` / `LoadsWithDiags` / `loadsWithDiags` / `loads_with_diags`），
交出「稳定码 + 文案 + 行号 + 严重度」，并都断言**值树与 `loads` 逐字节相同**。
回归钉子：`rust/tests/c_abi.rs`（3 例）、`cpp/tests/test_rs_diag.cpp`（5 项，走 `cpp/build_verify.py` 的
`RS-DIAG` 一档）、五个绑定各自的测试套件。
第二片（扩展选项 + 内置表注入）已落：`sml_builtin`（`new` / `free` / `count` / `add_field` /
`set_range` / `add_enum`，与 C 原生端同形状）+ `sml_opts`（`struct_size` / `flags` / `env` /
`builtins`）+ `sml_parse_full`；C++ 侧对应 `sml::Builtin` / `sml::Opts` / `loads_full`。
`W-CONTRACT-001` 由此在 C-ABI 上**可达**（`rust/tests/c_abi.rs` 与
`cpp/tests/test_rs_diag.cpp` 都钉了「声明侧 + 块内容侧 4 条、值树不变」）。
注意 `flags` 是**特性上限**：`type.weak` 是 opt-in、不在基线里，传 `0` 会因交集把
文档的 `@feature enable type.weak` 剔掉，随后 `@weakbind` 报 `E-FEATURE-001`
（这一格有显式用例，防「以为默认全开」）。

构建 cdylib 时必须带 `--features sml`（`cargo build -p swsml --features sml`）：
`sml` feature 才带 SML 文本发射器，缺了它 `sml_dump_err` / `sml_dumps` 会返回
`E-INTERNAL-001`「未启用 `sml` feature」，而绑定层的往返用例会在这里红。

第三片（五门对齐）也已落：Python `loads_full(text, flags, env, builtins)` + `Builtin`、
Ruby `loads_full(text, flags:, env:, builtins:)` + `Builtin`、Java `loadsFull(text, Opts)` +
`Builtin`、Go `LoadsFull(text, Opts)` + `NewBuiltin`、C# `LoadsFull(text, Opts, out diags)` +
`Builtin`；每门都钉了「建表 → 4 条 `W-CONTRACT-001` → 值树不变 → env 注入 → 基线不含
`type.weak` 报 `E-FEATURE-001`」。

第四片（弱声明三表 + 文件入口）已落：`sml_parse_full_lists`（`sml_parse_full` = 它传
`lists = NULL`）、`sml_load_file_full`、`sml_weaklists`（`binds` / `data` / `docs`，
其中 `data.value` 由表拥有）+ `sml_weaklists_free`；C++ 侧 `sml::WeakLists`（RAII）与
`loads_full(..., &lists)` / `load_file_full`。`weak_binds` / `weak_data` / `weak_docs`
三行在 C-ABI 上收口，「文件 + 诊断」那格同时补齐。

第五片（五门弱表）也已落：Python `loads_full_lists`、Ruby `Sml.loads_full_lists`、
Java `Sml.loadsFullLists`（`Parsed.lists`）、Go `LoadsFullLists`、C#
`LoadsFullLists(..., out lists)`；每门都钉「三表 1/1/1 → `weakdata` 载荷可读 →
主树无痕 → 100 次不崩」。至此 `§0.6` 只剩 `errors` / `contract_map` / `contracts`
三行（均属 P6 收口前的最后一批），以及绑定层的**文件入口**。

第六片（聚合结果）也落了：`sml_parse_all(text, opts, want, err)` + `sml_result_*` 一族
getter（value / diags / errors / weak / contract_map / contracts）。设计要点：**一次解析 →
一个结果句柄 → getter**，新增输出只加 `SML_WANT_*` 位与一个 getter，不再改既有签名
（`sml_parse_full_lists` 那种「每加一样能力就加出参」已到尽头）。C++ 侧 `sml::Result`（RAII）
+ `sml::Want` 位；`sml::Builtin::borrow()` 让 `Result::contracts()` 的结果能直接当内置表用，
于是 §6.1「用 SML 写契约表、再注入回去」在 C-ABI 上闭环（有用例）。

**P7 第七片（绑定层聚合结果）已落**：五个绑定层各有 `parse_all` 一族 —— Python
`parse_all(...)` → `ParseAll`、Ruby `Sml.parse_all`、Java `Sml.parseAll`、Go
`ParseAllOf`、C# `Sml.ParseAllOf`，交出 `value` / `diagnostics` / `errors` /
`weak` / `contract_map` / `contracts`，`want` 位与 `SML_WANT_*` 逐位一致。
配套新增 `sml_builtin_clone`：把 `sml_result_contracts` 的**借用**表变成宿主**独占**的句柄，
于是「读完结果 → 释放结果 → 继续拿表当内置表注入」不用吊着结果句柄的命（C++ 侧
`Result::contracts_owned()` 同一语义）。每门都钉了「捕获 → 释放 → 注入 → 命中
`W-CONTRACT-001`」的闭环与「多错误 2 条、行号 1/3、severity=1」。

**P7 第八片（文件入口）已落**：`sml_load_file_full` 在五门各有 `load_file_full` 一族
（Python `load_file_full`、Ruby `Sml.load_file_full`、Java `Sml.loadFileFull`、Go
`LoadFileFull`、C# `Sml.LoadFileFull`），展开 `include` 并交出诊断 + 弱表；每门都钉了
「include 已展开 / 弱表带 `@weakdoc` / 干净文件零诊断 / 不存在的路径报 `E-IO-001`」。
至此 `§0.6` 的**绑定层列整列收口**（只剩「本地四端」列上 C / C++ / JS / Lua 的文件入口
仍不交诊断 —— 那是各端 API 面的独立议题，不属于 C-ABI）。

**P6 进度（码表台账已按实测回填）**：`errors/codes.sml` 的 `impls` / `status` 不再落后于实现 ——
`W-PARSE-001` → `[ rust js c cpp lua ]` + `status: done`（五端都有「成功 + 诊断列表」出口，
触发条件与行号口径一致）；`W-CONTRACT-001` → `[ rust js c cpp lua ]`、`status: partial`
（四端缺 `block.undeclared_field`，它依赖契约 strictness）；`W-PARSE-002` 仍 `[ rust ]`，
但 note 改写成「有通道、尚未接这条告警」，替换掉已不成立的「四端无诊断通道」。
重生成时补齐一处**产物滞后**：`W-CLI-001` 早已进码表，8 份产物里却没有它
（`COUNT 166 → 167`）。

**P6 收口（2026-11-13，三份台账已对齐）**：

1. **码表**：`impls` / `status` 与实现一致（见上）；`W-CLI-001` 的产物滞后已补（167 条码）。
2. **一致性套件**：`known_divergences.md` 的「当前口径」改为 **AGREE 60 / DIVERGE 0**，
   旧的 43 / 52 两处降级为历史段 —— 此前 `scan.md` 一直照抄 43，属**生成物与实时扫描脱节**
   （本批 §1 卡点 3 的复发）。
3. **扫描器补交叉核对**：`tools/scan_status.py` 新增 `scan_cases()`，`scan.md` 的
   conformance 行现在同时写「台账 AGREE 数 + 实际用例数」，两者不等会当场标 ⚠️。
4. **`unfinished.md` §2 与 T 表逐行对齐**：错误码 44 / 语言层 36、套件 60、weak\* 五端、
   版本模型（字面量已同步，剩 `Removed` 三态等）、FFI（绑定层列整列收口）。
5. **门禁**：`python tests/conformance/run_conformance.py --require-all`（60 AGREE）、
   `--pending`（空）、C / Lua / C++ 三端、五门绑定、码表三份 `--check`、`smldocs --check`
   全部通过（明细见各片报告）。

仍未做（不属本批）：P5 的 `Removed` 三态 / 兼容检查 / 迁移工具（等 v5 定稿），
编辑器侧提示，本地四端文件入口交诊断。

**P5 进度（Edition 五端字面量一致，2026-11-13）**：四端各自补了一个**同一口径**的字面量解析器
（C `version_lit_major` / C++ 同名 helper / JS `versionLitMajor` / Lua `version_lit_major`），
接受 `vN` / `N` / `N.0` / `N0`（连写）与代号 `camelia`，`5.0` 族统一 `E-FEATURE-004`；
**版本决定特性基集**也补齐了 —— C++ 在 `scan_features` 里、Lua 在 `@version` 分支里、
C 走既有的 `ps->version`、JS 走 `scanVersionBase`，四处都实现「v1 开 `bareword-string`、
v2 起关」，与 Rust `FeatureSet::for_edition` 同构。判据是新增的 6 条一致性用例
（`55`–`60`：连写 / 点号 / `v4` / `50` / 代号 / **v2 严格串**），默认套件从 54 → **60 例全 AGREE**。
`50` 仍关闭（不启用 v5），`Removed` 三态 / 兼容检查模块 / 迁移工具仍待做（见 `version.md` §7）。

**顺序**：`P0（已收官）→ P1 → P2 / P3`（两者可并行）`→ P4`（等 G3 定稿；含 §0.3 的默认路径修复）`→ P7 → P6`；
`P5` 后置到 v5 定稿之后，不占本批关键路径。
**回滚粒度**：P2 按「一组码 × 一端」提交；P3 / P4 每步都由 P1 的用例先钉住旧行为；
P4 的公开 API 改动单独成片，P7 按「一个 C-ABI 符号 × 一组绑定」提交，都不与码表对齐混在一次提交里。

## 4. 每片的验收命令（统一）

```
python tests/conformance/run_conformance.py --require-all     # 五端都可用，DIVERGE=0
python tests/conformance/run_conformance.py --pending --require-all   # 先行用例，DIVERGE 为预期
python errors/gen_json.py --check
python errors/gen_codes.py --check
python errors/gen_js_errors.py --check
python tools/check/check_js_copies.py
python c/build_check.py --run                                  # 必须在 c/ 里跑
python cpp/build_verify.py                                     # 本机需 SML_RUST_LIB + PATH 指向 cdylib
python lua/run_check.py
node js/probe-error-codes.mjs
cargo test -p <改动的 crate>
cargo run -p smldocs -- --check                               # 在 rust/ 里跑
python tools/scan_status.py --write
```

`python tools/check/check_dump_parity.py` 需要 `SMLTOOLS` 指向本机 `smltools`；它比的是渲染字节，
与 conformance 的值树分工不同，两者不互相替代。
P7 另需绑定层自检：`bindings/python/test_sml.py`、`bindings/ruby/test_sml.rb`、
`bindings/java/src/test`（口径见 `bindings/README.md`；Go / C# 的等价自检随 P7 补齐）。

## 5. 复核命令（计数不手写）

```
python -c "import json,collections; d=json.load(open('site/static/errors.json',encoding='utf-8')); r=[x for x in d['codes'] if x.get('impls')==['rust']]; print(len(r), len([x for x in r if x['domain']!='DERIVE']), dict(collections.Counter(x['domain'] for x in r)))"
python tools/scan_status.py --json
python tests/conformance/run_conformance.py --require-all
```

## 6. 施工时不要做的事

- G3 定稿完成前不把 `@weakbind` / `@weakdata` / `@weakdoc` 写进任何共享语料；
- 不手改 `errors/codes.sml` 的 8 份生成物，也不手改 `docs/index.md` / `report.md`；
- 不为凑数把 A 类 11 条标成已实现：S9 的「不实现」是设计取舍；
- 不用 SKIP 掩盖缺端，`--require-all` 是硬闸门；
- 不在 P2 顺手改 v5 或 featag 语义；
- 不绕过 `errors/codes.sml` 直接改 8 份生成物；新增码一律先登记中英文文案再重生成；
- 不以「当前没有消费者」「这一端用不到」为由**永久裁剪** Rust 已有的能力；§0.6 按功能结果逐项打勾，允许按切片分批落地；
- 不为「逐一对齐机制」把 Rust 的 trait 对象 / 闭包注册点硬改成 FFI 函数指针：功能等价即达标。
