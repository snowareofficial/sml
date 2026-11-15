# grammar.sml 逐特性 × 五端差异台账

> **产品线** swsml ｜ **状态** current ｜ **最后核对** 2026-11-15
>
> 探针脚本：`tools/check/probe_grammar.py`（切片清单内置，五端探针 + 值树比对；不进 CI ——
> 本文登记的差异未清零前它必然非零退出）。
> 复跑：`python tools/check/probe_grammar.py grammar.sml`（退出码 0 = 全部与 Rust 一致）。
> 用例级一致性（60 用例 × 5 端）看 `tests/conformance/known_divergences.md`；本文只管
> `grammar.sml` 这份**全集**文档：按特性切片后，各端的第一处拒绝在哪、码是否与 Rust 相同。

## 1. 判据

探针程序就是套件在用的那一份（`tools/gen/dump_c.c` 与 `tests/conformance/adapters/`），
判据也复用套件那一份（JSON 值树 + token 层，数字按 int/float 身份归一）。三种格：

| 格 | 含义 |
|---|---|
| `OK` | 解析成功，且值树与 Rust 逐格相同 |
| 错误码 | 该端拒绝；码是稳定契约，文案不是 |
| `DIFF` | 解析成功但值树与 Rust 不同 —— **静默差异** |
| `NC` | 解析成功，但该端 dump 出的文本 Rust 读不回去 ⇒ 判据无法成立（载体问题，见 §4.5d） |

「四端明确报错」不是静默差异：那是已登记的能力缺口，用户能看见。要清零的是 `DIFF` 与
「指令被接受后丢弃」这两类。本轮实测 `DIFF` **0 格**，但查出另一类静默：`@type` 声明被
三端接受后丢弃（§4.2）。

切片共 20 条，覆盖 grammar.sml 用到而版本位分层的语法：契约嵌套 / 多契约 / `@requires` /
`@type`（合规值、不合规值、只声明不使用）/ featag（块头使用、只声明）/ 带点特性名 /
未知特性名 / `typed-block`（含 `typed-block-default`：契约带 `default`，否则「有没有应用契约」
看不出区别）/ `block-name` / 嵌套带名裸块的 `__name` 校验 / `@when` / `@for` / `weak*`。
`python tools/check/probe_grammar.py --list` 打印清单与出处。

## 2. 整份 grammar.sml：第一处拒绝

| 端 | 码 | 触发构造 | 归因 |
|---|---|---|---|
| rust | —— | `rc=0` | 参照实现 |
| c | `E-CONTRACT-008` | `Window.start: 时刻` | `@type` 未被实现 ⇒ `时刻` 回落成契约名，再按块校验 |
| cpp | `E-CONTRACT-001` | 同上 | 同上，报「unknown contract `时刻`」 |
| js | `NC` | `Deploy !tls replicas[3] zone[cn-north-1], api {` | 解析层已通；dump 把嵌套的带 featag 块写成块头形式 ⇒ Rust 读不回去（§4.5d） |
| lua | `E-CONTRACT-008` | `Window.start: 时刻` | 同 C |

⚠️ **别把 C / Lua 的 `E-CONTRACT-008` 归因到「契约嵌套」**：纯契约嵌套切片（`contract-nest`：
`Address` / `Node` + 默认值回填）四端**全部 OK**。这条归因错了会把修复方向带到「实现嵌套契约」，
而四端早就支持；真正的缺口是 `@type`。

## 3. 逐特性矩阵

实测（2026-11-14，rust `smltools` debug + gcc / g++ / node / luajit）：

| 特性 | 说明 | rust | c | cpp | js | lua |
|---|---|---|---|---|---|---|
| 整份 `grammar.sml` | 全集文档 | OK | `E-CONTRACT-008` | `E-CONTRACT-001` | `NC` | `E-CONTRACT-008` |
| `contract-nest` | 契约当字段类型 | OK | OK | OK | OK | OK |
| `contract-multi` | 多契约叠加 + 就地严格度 | OK | OK | OK | OK | OK |
| `contract-requires` | `@requires when`（when 未开） | `E-CONTRACT-018` | `E-CONTRACT-018` | `E-CONTRACT-018` | `E-CONTRACT-018` | `E-CONTRACT-018` |
| `contract-requires-on` | `@requires when`（when 已开） | OK | OK | OK | OK | OK |
| `type-dir` | `@type` 合规值 | OK | `E-CONTRACT-008` | `E-CONTRACT-001` | OK | `E-CONTRACT-008` |
| `type-dir-check` | `@type` 不合规值 | `E-CONTRACT-009` | `E-CONTRACT-008` | `E-CONTRACT-001` | `E-CONTRACT-009` | `E-CONTRACT-008` |
| `type-dir-alone` | `@type` 只声明不使用 | OK | OK | OK | OK | OK |
| `featag` | 块头 `Service !critical !obey[4], prod` | OK | OK | OK | OK | OK |
| `featag-decl` | featag 只在契约体内声明 | OK | OK | OK | OK | OK |
| `featag-gate` | `@feature enable type.featag` | OK | OK | OK | OK | OK |
| `feature-unknown` | `@feature enable no.such` | `E-FEATURE-003` | `E-FEATURE-003` | `E-FEATURE-003` | `E-FEATURE-003` | `E-FEATURE-003` |
| `typed-block` | `Service { }`（无块名） | OK | OK | OK | OK | OK |
| `block-name` | `Service prod { }` | OK | OK | OK | OK | OK |
| `when` | `@when $env.X == prod` | OK | OK | OK | OK | OK |
| `for` | `@for h in web api { "${h}" }` | OK | OK | OK | OK | OK |
| `weak-star` | `@weakbind` / `@weakdata` / `@weakdoc` | OK | OK | OK | OK | OK |
| `type-weak-gate` | `@feature enable type.weak` + 普通块 | OK | OK | OK | OK | OK |

`when` / `for` / `weak*` 三格是**值树一致**（不只是不报错）：四端都实现了门控与展开，
`__type` / `__name` 等元数据也在（`typed-block` / `block-name` 两格同理）。

`contract-multi` 已清零：五端同为 `OK`，且五端**值树相同**（字段并集 + 默认值回填）。
剩下 3 格不一致（`whole:grammar.sml` / `type-dir` / `type-dir-check`）全部由 `@type` 引起，
`type-dir-check` 的 JS 半边已与 Rust 同码（`E-CONTRACT-009`）。

## 4. 逐条结论

### 4.1 G-01 `@type` 声明被三端静默丢弃

`type-dir-alone`（只写 `@type` 声明，不给任何字段用它）五端**同树**：C / C++ / Lua 把整条
`@type` 指令接受后丢弃，既不报「不支持」，也不产出任何东西。等到有字段的类型写 `时刻`，
才因为「查不到这个类型名 ⇒ 当作契约名」而炸，且炸出来的码是契约码而非类型码：

| 端 | 不合规值 `"9:30"` 时的码 | 说明 |
|---|---|---|
| rust | `E-CONTRACT-009` | 自定义类型格式不符 |
| js | `E-CONTRACT-009` | 同上，同因同码 |
| c | `E-CONTRACT-008` | 把 `时刻` 当契约名 ⇒ 「字段应为块」 |
| lua | `E-CONTRACT-008` | 同上 |
| cpp | `E-CONTRACT-001` | 「unknown contract `时刻`」 |

影响：照 `grammar.sml` 写的文档在 C / Lua 上不会在 `@type` 处得到提示，而是在**第一次用到该类型
的字段**处报一个「应为块」的契约错误，用户很难反推到 `@type` 未实现。修复有两条路，二选一即可
消除静默：**实现 `@type`**（与 JS 同族），或**在 `@type` 处明确拒绝**（`E-FEATURE-001` / `E-PARSE-005`）。

### 4.2 G-02 未知特性名：五端同为 `E-FEATURE-003`（已对齐）

`@feature enable no.such` 五端都报 `E-FEATURE-003`。这条曾经是「同一条件两个码」：C / C++ / Lua
借用 `E-FEATURE-002`（定义是「环境变量被禁用」），编辑器与调用方会判错类别。现在三端在
`feature_index` 失败处改报 003，`lua/test_codes.lua` 的期望值一并改正。

`errors/codes.sml` 里 `E-FEATURE-003` 的 `impls` 与注记同步更正：三端**有**特性集，
`@feature enable/disable` 可用（`featag-gate` / `type-weak-gate` 两格实测通过），
「`@feature` 本身非法 ⇒ `E-PARSE-005`」不成立。同一批还对齐了 `@feature` 的另外三个码
（`006` 未知子命令 / `007` 缺参数 / `008` mode 参数非法）。

### 4.3 G-03 `E-CONTRACT-018` 同码两义

| 切片 | 条件 | rust | 四端 |
|---|---|---|---|
| `contract-requires` | `when` 未开 | `E-CONTRACT-018`（要求未满足） | `E-CONTRACT-018`（指令不支持） |
| `contract-requires-on` | `when` 已开 | OK | `E-CONTRACT-018`（指令不支持） |

第一格在 conformance 里是 AGREE（同码），但两边的**含义不同**：Rust 是「契约声明的前提没满足」，
四端是「我根本不认这条指令」。第二格才把这层差异暴露出来。建议四端改用「不支持」类的码
（`E-FEATURE-001` 或新增码），把 018 留回「要求未满足」——否则同一个码无法区分「文档漏开特性」
与「本端没实现」。

### 4.3b `contract-multi` 的实施口径（五端同源）

Rust 侧在 `sml-contract/src/lib.rs::apply_contracts`，语义是**合并视图**而不是「叠加校验两遍」：

1. **有效严格度**：每条契约取「块级覆盖 `unwrap_or` 契约级声明」，多契约取**最严**一档
   （`Strict < Warn < Loose`，取 `min`）⇒ 任一严格即整体严格；
2. **顺序无关**：字段按名取并集（BTreeMap 天然有序），与书写先后无关；
3. **冲突即错**：同名字段但**类型 / 默认值 / 区间**任一不同 ⇒ `E-CONTRACT-016`。
   类型比较用 `TypeSpec::name()` 的规范串（外置类型即注册名，同名即同校验器）。
   **不允许「后者覆盖」** —— 那会让结果依赖书写顺序；
4. 实现方式：构造一份临时 `Contract`（并集 + 最严），**复用既有 `apply_contract`**
   做默认值回填 / 类型校验 / 组合递归 —— 不另写一条校验路径，保证与单契约同源。

四端照同一口径落地（构造临时契约 ⇒ 复用各端既有的单契约入口），三处应用点：
`@is` 多契约 / 字段级 `@is` 多契约 / 块级 `A B item { }`。

| 端 | 合并入口 | 复用的单契约校验 |
|---|---|---|
| C | `c/src/04_coerce.inc::merge_contracts` | `apply_contract_rec` |
| C++ | `cpp/src/06_include_api.inc::apply_contract_set` | `apply_contract_obj` |
| JS | `js/sml.mjs::mergeContractSet` | `applyDefaults` + `checkContract` |
| Lua | `lua/lib/sml.lua::merge_contract_set` | `apply_contract_obj` |

两处口径要钉住，否则会与 Rust 分叉：

- **`required` 不在冲突预检内**，取**或**（任一份要求即必填）—— 放进预检就成了「默认值必须一致」
  之外的额外冲突，等于凭空多一类 016；
- **`warn` 在四端按 `loose` 记**：四端没有「放行 + 告警」通道，而 warn 与 loose 的**值树**逐字相同
  （Rust 只多一条诊断），故树对齐、诊断不齐。

### 4.4 G-04 featag：S2–S5b 五端一致，S6–S8 仍是缺口

四端已实现 **块头 `!` 区 + `__featags` + 契约内 `!name` 声明**（S2–S4），`featag` 与
`featag-decl` 两个切片五端 AGREE；此前「拒绝码随位置漂移」（C 报 `E-CONTRACT-003`、
C++ 报 `E-CONTRACT-001`、JS/Lua 报 `E-PARSE-006`）已消失。

**S5 / S5b 也已五端一致**（2026-11-14）：未声明即拒、个数不符都是 `E-CONTRACT-021`；
参数**类型**不符是 `E-CONTRACT-002`（Rust 复用 `check_value` 的产物，不是 021）。
探针 `featag-undeclared` / `featag-args-on-tag` / `featag-args-missing` / `featag-arg-type`
五端同码。四端只做标量这一层，枚举 / 区间 / 外置类型仍只在 Rust 侧校验。

仍只有 Rust 实现的部分：**S6 落点填充**（`-> PATH`、`!strict-paths`、`auto` 取默认值、
数组下标语义）、**S7 `from-featag`**、**S8 `!any`**。四端对落点**忽略** —— 所以 `grammar.sml` 里
`Deploy !tls replicas[3] zone[cn-north-1]` 那几段在四端会校验参数，但不会写入 `spec.*`（静默少值）。
这块没有探针覆盖（要测就得比对落点后的树），属下一步的施工项。

### 4.5 多契约：五端一致（已清零）

`@is Left loose, Right loose` 五端值树相同，`grammar.sml` 的 `cross_end_notes` 不再需要记「四端同码
`E-CONTRACT-017`」这一条。合成严格度、顺序无关、同名字段冲突 ⇒ `E-CONTRACT-016` 三件都按 §4.3b
的口径落地，逐例对拍（合并成功 / 类型不同 016 / 默认值不同 016 / 区间不同 016 / 最严取 strict /
块级覆盖 `loose`·`warn`）五端全绿。

**写法**：多契约一律以 `,` 分隔（`@is A loose, B`），与块头「契约词 → featag 区 → `,` → 块名」
同一阅读习惯 —— 逗号之后是「新的一段」。省略逗号的连续词写法仍按「连续契约词」解析（兼容既有
文档），但不作为规定写法：它让「这个词是契约还是块名/键」只能靠别处信息判断，正是本项最初
难读的原因。

### 4.5b 块头 featag 的无 `!` 带参写法：五端一致（已清零）

`docs/swsml/spec/featag.md` §2.2 规定**带参项写成 `名[ … ]`**（`obey[1, 2]`，靠 `[` 与块名区分），
`!` 只有无参项才必须有。四端按这条对齐：块头扫描把「词 + 紧跟 `[`」也认成 featag 项，并且
这笔也算「窗口里有 featag 区」—— 否则紧随的终止逗号会被当成「裸词里的逗号」而拒
（`Menu density[auto], main { }` 就是这条的最小复现）。

探针的 `featag` 切片用的是带 `!` 的 `Service !critical !obey[4], prod`，所以这一格一直是绿的
—— 是**取样**漏了无 `!` 那一半，不是两边行为相同。

### 4.5c 块头契约后跟块名但未开 `block-name`：五端同码 001（已清零）

`A B item { }`（`typed-block` 已开、`block-name` 未开）里，契约词之后还剩一个词，那只能是块名，
而块名要 `@feature enable block-name`。四端补齐了这条「拆雷」检查，与 Rust 同码同序；
`A item { }`（单契约）同样报 001，`A plain { }` 开了 `block-name` 则照常通过。

### 4.5d JS / Lua 的 dump：嵌套的带 featag 块写成块头形式，Rust 读不回去（`NC`）

树里一个块同时有 `__type` 与 `__featags` 时，JS / Lua 的 `stringify` 会在**值位置**写成块头形式
（`S: S { … }` / `Menu density[auto], main { … }`），而值位置的「裸块头」不是合法 SML —— Rust 回读
报 `E-PARSE-006`（期望键，得 `{`）。后果：这类用例在套件里变成 `NC`（判据无法成立），而不是
「解析不同」。

这条与解析无关，可用**不含本片任何改动**的输入复现：`@feature enable type.featag, typed-block` +
`@contract S { name: str  !t }` + `d { S !t { name: g } }` —— 五端解析都成功，JS / Lua 的 dump
回读失败。修法：值位置的带元数据块一律写扁平对象（`{ __type: …, __name: …, … }`），与 Rust
`to_sml` 同形；改动落在 dump 层，会牵动全部 `NC` 判定，单独成片。

### 4.6 已经一致的格子

- **契约嵌套 + 默认值回填**：四端与 Rust 同树（`country` 回填 `CN`）。
- **`typed-block` / `block-name`**：`__type` / `__name` 元数据四端都在。
- **`@when` / `@for` / `weak*`**：门控与展开行为一致，`@for` 的 `${h}` 插值也一致。
- **带点特性名**：`type.featag` / `type.weak` 四端都认（不在 `E-FEATURE-003` 名单里）。

## 5. 回写位置

| 落点 | 内容 |
|---|---|
| `grammar.sml` 的 `cross_end_notes` | 多契约（五端一致，不再记差异）、`@type`（C/C++/Lua 缺失 + 声明被丢弃）、featag 拒绝码漂移、`@requires` 同码两义 |
| `grammar.sml` 头部与 `@is` / `@requires` 处的行内注释 | 删掉「Lua 的 `@feature` 非法」「`weak*` 未实现」「只有 JS 拒绝 `@requires`」这类与实测不符的说法 |
| `tests/conformance/known_divergences.md` | 新增一节指向本文，并把 G-01 / G-02 记为待修条目 |
| `errors/codes.sml` | `E-FEATURE-003` 的 `impls` / 注记（三端其实有特性集）—— 需与 G-02 的修复一起动 |

## 6. 未覆盖

- **顶层数组 / include / glob / 正则引入 / 外置扩展点**：单文档无法自证，由 conformance 用例
  与 `known_divergences.md` 覆盖，不在本文范围。
- **`@version` 版本位**：各端支持范围不同（`docs/swsml/spec/version.md`），与 `grammar.sml`
  的语法特性无关，另测。
- **v5 草案里尚未实现的项**（A8 手写 `__*`、A10 `auto` 裸词、`!any` / `!strict-paths`）：
  `!any` / `!strict-paths` 只在 Rust 侧有完整语义（四端只认语法、S6–S8 未实现），
  切片没有区分度。
