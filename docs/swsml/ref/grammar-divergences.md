# grammar.sml 逐特性 × 五端差异台账

> **产品线** swsml ｜ **状态** current ｜ **最后核对** 2026-11-14
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
| js | `E-CONTRACT-017` | `pair { @is Left loose Right loose }` | 多契约叠加未实现（JS 支持 `@type`，故不是它） |
| lua | `E-CONTRACT-008` | `Window.start: 时刻` | 同 C |

⚠️ **别把 C / Lua 的 `E-CONTRACT-008` 归因到「契约嵌套」**：纯契约嵌套切片（`contract-nest`：
`Address` / `Node` + 默认值回填）四端**全部 OK**。这条归因错了会把修复方向带到「实现嵌套契约」，
而四端早就支持；真正的缺口是 `@type`。

## 3. 逐特性矩阵

实测（2026-11-14，rust `smltools` debug + gcc / g++ / node / luajit）：

| 特性 | 说明 | rust | c | cpp | js | lua |
|---|---|---|---|---|---|---|
| 整份 `grammar.sml` | 全集文档 | OK | `E-CONTRACT-008` | `E-CONTRACT-001` | `E-CONTRACT-017` | `E-CONTRACT-008` |
| `contract-nest` | 契约当字段类型 | OK | OK | OK | OK | OK |
| `contract-multi` | 多契约叠加 + 就地严格度 | OK | `E-CONTRACT-017` | `E-CONTRACT-017` | `E-CONTRACT-017` | `E-CONTRACT-017` |
| `contract-requires` | `@requires when`（when 未开） | `E-CONTRACT-018` | `E-CONTRACT-018` | `E-CONTRACT-018` | `E-CONTRACT-018` | `E-CONTRACT-018` |
| `contract-requires-on` | `@requires when`（when 已开） | OK | `E-CONTRACT-018` | `E-CONTRACT-018` | `E-CONTRACT-018` | `E-CONTRACT-018` |
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

### 4.5 多契约：四端同码 `E-CONTRACT-017`

`@is Left loose Right loose` 在 C / C++ / JS / Lua 上同码同义，`grammar.sml` 的
`cross_end_notes` 只需记这一条口径（原来只写了「JS 只认单个」，其余三端同样不支持）。

### 4.6 已经一致的格子

- **契约嵌套 + 默认值回填**：四端与 Rust 同树（`country` 回填 `CN`）。
- **`typed-block` / `block-name`**：`__type` / `__name` 元数据四端都在。
- **`@when` / `@for` / `weak*`**：门控与展开行为一致，`@for` 的 `${h}` 插值也一致。
- **带点特性名**：`type.featag` / `type.weak` 四端都认（不在 `E-FEATURE-003` 名单里）。

## 5. 回写位置

| 落点 | 内容 |
|---|---|
| `grammar.sml` 的 `cross_end_notes` | 多契约（四端同码）、`@type`（C/C++/Lua 缺失 + 声明被丢弃）、featag 拒绝码漂移、`@requires` 同码两义 |
| `grammar.sml` 头部与 `@is` / `@requires` 处的行内注释 | 删掉「Lua 的 `@feature` 非法」「`weak*` 未实现」「只有 JS 拒绝 `@requires`」这类与实测不符的说法 |
| `tests/conformance/known_divergences.md` | 新增一节指向本文，并把 G-01 / G-02 记为待修条目 |
| `errors/codes.sml` | `E-FEATURE-003` 的 `impls` / 注记（三端其实有特性集）—— 需与 G-02 的修复一起动 |

## 6. 未覆盖

- **顶层数组 / include / glob / 正则引入 / 外置扩展点**：单文档无法自证，由 conformance 用例
  与 `known_divergences.md` 覆盖，不在本文范围。
- **`@version` 版本位**：各端支持范围不同（`docs/swsml/spec/version.md`），与 `grammar.sml`
  的语法特性无关，另测。
- **v5 草案里尚未实现的项**（A8 手写 `__*`、A10 `auto` 裸词、`!any` / `!strict-paths`）：
  只在 Rust 侧有意义，四端连 featag 语法都不认，切片没有区分度。
