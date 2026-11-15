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
| c | —— | `rc=0` | **整份读通**，值树与 Rust 逐格相同 |
| cpp | —— | `rc=0` | 同上 |
| js | `OK`（除数字身份） | —— | 解析与载层都通；`whole` 的判定差异只剩 int/float 身份（§0 R-2） |
| lua | `OK`（除数字身份） | —— | 同上（dump 的块头/数组串两处已在 §4.5d 修掉） |

⚠️ 曾经的坑：C / Lua 的「第一处拒绝」落在 `Window.start: 时刻` 上，很容易被归因到「契约嵌套」。
`contract-nest` 切片（`Address` / `Node` + 默认值回填）四端**一直**是 OK —— 真正的缺口是 `@type`
（见 §4.1）。

## 3. 逐特性矩阵

实测（2026-11-14，rust `smltools` debug + gcc / g++ / node / luajit）：

| 特性 | 说明 | rust | c | cpp | js | lua |
|---|---|---|---|---|---|---|
| 整份 `grammar.sml` | 全集文档 | OK | OK | OK | OK（数字身份除外） | OK（数字身份除外） |
| `contract-nest` | 契约当字段类型 | OK | OK | OK | OK | OK |
| `contract-multi` | 多契约叠加 + 就地严格度 | OK | OK | OK | OK | OK |
| `contract-requires` | `@requires when`（when 未开） | `E-CONTRACT-018` | `E-CONTRACT-018` | `E-CONTRACT-018` | `E-CONTRACT-018` | `E-CONTRACT-018` |
| `contract-requires-on` | `@requires when`（when 已开） | OK | OK | OK | OK | OK |
| `type-dir` | `@type` 合规值 | OK | OK | OK | OK | OK |
| `type-dir-check` | `@type` 不合规值 | `E-CONTRACT-009` | `E-CONTRACT-009` | `E-CONTRACT-009` | `E-CONTRACT-009` | `E-CONTRACT-009` |
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

### 4.1 G-01 `@type`：四端已落地（`E-CONTRACT-009` 五端同码）

曾长期是「C / C++ / Lua 把 `@type` 声明**接受后丢弃**」：`type-dir-alone` 五端同树（看不出问题），
一旦有字段的类型写 `时刻`，就因为「查不到该类型名 ⇒ 当作契约名」而炸 —— C / Lua 报 `008`
（字段应为块）、C++ 报 `001`（unknown contract），而 Rust / JS 报 `009`。用户很难从「应为块」
反推到「这个类型没被实现」。

现在四端各有一台**模式匹配器**（与 Rust `sml-pattern` 同构）：

| 端 | 引擎 | 实现位置 |
|---|---|---|
| rust | Thompson NFA（权威） | `rust/sml-pattern` |
| c | Thompson NFA（逐段移植） | `c/src/02b_pattern.inc` |
| cpp | Thompson NFA（逐段移植） | `cpp/src/02b_pattern.inc` |
| lua | Thompson NFA（逐段移植） | `lua/lib/sml.lua`（引擎一节） |
| js | **RegExp**（不是 NFA） | `js/sml.mjs` 的 `compilePatternToRe` |

三件事必须五端一致，否则会出现「同一份文档一端通过、另一端报 009」：

1. **字符类按 Unicode 判定**（`数字` 不止 `0-9`，全角与罗马数字也算）。若四端各写各的区间，
   边界必然漂移 —— 故由 `tools/gen/gen_unicode_tables.py` 从 Rust 的
   `char::is_numeric / is_alphabetic / is_whitespace` **导出同一张码位区间表**
   （`c/sml_pattern_unicode.h` 给 C / C++，`lua/sml_pattern_unicode.lua` 给 Lua）。
2. **无回溯**：量词编译成 ε-转移，模拟时所有线程并行推进，不存在 ReDoS；单量词上限 1000、
   指令总数上限 100000、匹配步数预算 1000 万（与 Rust 同数同义）。
3. **码的同因同码**：模式**编译失败**（循环引用 / 未定义规则 / 无法识别的元素 / 量词过大）
   ⇒ `E-CONTRACT-013`；值**不符合格式**（或不是字符串）⇒ `E-CONTRACT-009`。
   没被任何字段引用过的坏模式**不报错**（编译发生在字段类型解析处，与 Rust 同）。

### 4.7 `@type` 的剩余差异（已登记）

四端引擎落地后，与 Rust 仍有三处差别，都不在「解析对错」这一层：

| # | 差异 | 现状 | 说明 |
|---|---|---|---|
| 1 | `正则/regex` 逃生舱 | 四端**响亮拒绝**（`013`） | Rust 把内联正则编译进同一个 IR；四端没有那台受限正则解析器。与其静默「永不匹配」（会把格式约束变成「全都报 009」），不如拒绝 |
| 2 | JS 的错误码漂移 | `用:` 环/未定义 ⇒ `014`（Rust `013`）；量词过大 ⇒ `E-LIMIT-009`（Rust `013`） | JS 走 RegExp 路线，错误来自编译期预算，码没对齐 |
| 3 | JS 的数组元素 | `[D]` 的非法元素**不报**（Rust `009`） | C / C++ / Lua 已按 009 校验（带下标 `v[1]`） |

另有一条**既有**差异（与 `@type` 无关，但在同一批用例里撞见）：Lua 把数组里的引号数字串
（`[ "12" "3" ]`）解析成 **number**，Rust 保持 string。等价用例 `v: [ str ]` 同样可复现。

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

### 4.5b featag 项的 `!` 与「区的位置」：五端一致（已清零）

早先 Rust 允许**带参项省掉 `!`**（`名[ … ]`，靠紧跟的 `[` 与块名区分），四端只认 `!`。把四端
补成前者之后立刻撞上一条更根本的问题：**多契约叠加时同一个裸词被两套判据争用** ——

```sml
A B rep[3], prod { }      # rep 既是 A 的 featag 名，又有一份 @contract rep
```

Rust 报 `E-PARSE-012`、四端却把 `rep[3]` 当 featag 收下。「连续命中契约表的词算契约词」与
「`名[` 算 featag」在同一条扫描线上互相干扰，只能靠先后次序猜 —— 于是**语言层面收敛**而不是
在解析器里打补丁：

- **featag 项一律以 `!` 开头**（`!critical`、`!obey[1, 2]`）—— `!` 成为 featag 的**唯一**标记，
  不带 `!` 的裸词只剩两种身份（契约词 / 块名），与「命中契约表」这条判据正交；
- **featag 区必须紧跟契约首词**，其后才是其余契约词与块名：`A !t B, name { }`。区之外的 `!` 词
  由 B2 拒（`E-PARSE-027`），故 `A B !t, prod { }` 与 Rust 同码；
- 区里的项收完后若不是 `{`，报兜底 `E-PARSE-012` 且**不回退**旧路径（Rust `bare_child_frame`
  同位置同码）—— 否则 `A B rep[3]` 会在四端静默改判成「键 + 标量」。

规范原文同步改为「**首项必须写 `!`**；区内后续项可省，判据是命中当前契约的 featag 声明」（`featag.md` §2.2，2026-11-15 按 I-27 调整；`A !t B, name { }` 的 `B` 不在声明里 ⇒ 仍是契约词），`grammar.sml` 的活示例与
`errors/codes.sml` 的 `E-PARSE-026` 文案一并修正。

### 4.5c 块头契约后跟块名但未开 `block-name`：五端同码 001（已清零）

`A B item { }`（`typed-block` 已开、`block-name` 未开）里，契约词之后还剩一个词，那只能是块名，
而块名要 `@feature enable block-name`。四端补齐了这条「拆雷」检查，与 Rust 同码同序；
`A item { }`（单契约）同样报 001，`A plain { }` 开了 `block-name` 则照常通过。

### 4.5d JS / Lua 的 dump：值位置写成块头形式（已修）

树里一个块同时有 `__type` 时，JS / Lua 的 `stringify` 会在**值位置**也写成块头形式
（`S: S prod { … }`），而值位置的「裸块头」不是合法 SML —— Rust 回读报 `E-PARSE-006`
（期望键，得 `{`）。后果是整份文档 dump 之后自己读不回来，套件里表现为 `NC`（判据无法成立），
而不是「解析不同」。

修法：**块头形式只在键位置成立**，值位置一律写扁平对象 `{ __name: …, __type: …, … }`
（与 Rust `to_sml` 同形）。落点两处：

| 端 | 位置 |
|---|---|
| JS | `js/src/dump.mjs`：`dumpValue` 加 `allowHead` 参数，只有 `stringify` 的单键还原传 true |
| Lua | `lua/lib/sml.lua`：`dump_value` 同参数，只有单键还原与顶层 typed block 传 true |

顺带修的两个同层问题（都在 dump，都会让回读变形）：

1. **Lua 的数组字符串绕过了引号判据**：`dump_value` 的数组分支直接 `tostring(e)`，于是
   `live: [ "@version / @feature …" ]` 这类元素裸写，回读时被当成 `@version` 指令
   （Rust 报 `E-FEATURE-004`）—— `grammar.sml` 整份 dump 读不回来就是它。改成与对象值位置
   同判据（`needs_quote` + `escape`）。
2. **键序按字典序**输出（Rust 用 `BTreeMap`，`__name` < `__type` < 普通键），三端 dump 逐字可比。

修完 `whole:grammar.sml` 五端全部 `OK`；剩下的唯一差异是**数字身份**（§0 R-2：Lua 的
float `1.0` dump 成 `1`，Rust 读回是 int），属值层既有放行项，不是解析或载体的新问题。

### 4.5e 数组位置的裸块：C / C++ / Lua 已对齐，JS 仍缺

```sml
xs: [ A B item { a: 1, b: 2 } ]
```

Rust 把它读成「一个元素 = 应用 A、B 两份契约的块」（`__type: A`、`__name: item`、两契约的
字段都在）—— 数组位置与键位置走的是**同一个** `bare_child_frame`，所以 featag 区、多契约、
拆雷、012、`__name` 起点全都一视同仁。

先前四端各不相同：C / C++ 报 `E-CONTRACT-017`（数组位置是独立实现，没接多契约与 featag 区）、
JS / Lua **根本不把这个位置当裸块**（拆成 `A`、`B`、`item`、`{…}` 四个元素 —— 静默错树）。

C / C++ / Lua 已按键位置的同一套逻辑补齐（三端都是「块头 + 收尾」两段，与键位置逐字同构）：

| 端 | 实现位置 | 备注 |
|---|---|---|
| C | `c/src/05_parse.inc::try_parse_array_bare_block` | 定义从 `04_coerce.inc` 挪到 05（要用 S6/S7 的 featag helper）；04 只留声明 |
| C++ | `cpp/src/05_parse.inc::parse_bare_block` | 同上（04 留声明） |
| Lua | `lua/lib/sml.lua::parse_array_bare_block` | 新函数；`parse_array` 加一条分支 |

⚠️ 深度守卫：数组里的块是**数组的子级**，`parse_array_bare_block` 内部已经 `parse_block(depth+1)`，
外层不能再 `+1`（实测多算一层会把「128 层放行」变成 64 层）。

**JS 仍是缺口**：它的数组分支没有 `bare_block_ahead` 识别，`[ A B item { } ]` 会被拆成标量；
遇到 `[ A !t item { } ]` 则在值位置撞上 `!` 而报 `E-PARSE-027`（响亮，但码不同）。
另：C 端此前在「认出裸块却拒绝」之后会**段错误**（`coerce_word` 拿到 EOF 的 `v`）——
已在 `parse_array_inner` 加 `if (ps->failed) break;` 兜住，并把 `coerce_word(NULL)` 变成返回 null。

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
