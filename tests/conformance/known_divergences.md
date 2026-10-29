# 跨实现一致性：已知分歧台账

本目录是 W5 的产出：**一份共享用例，五端（Rust / C / C++ / JS / Lua）各跑一遍，比较值树**。

## 用法

```bat
python tests/conformance/run_conformance.py
python tests/conformance/run_conformance.py --case 05 --end lua
python tests/conformance/run_conformance.py --show 3
python tests/conformance/run_conformance.py --no-build      :: 不自动构建 smltools
```

- 缺工具链的端标 `SKIP` 并说明缺什么（**不会**悄悄当成通过）。
- 退出码：`0` = 无 DIVERGE / PARSE_FAIL；`1` = 有。**本台账还没清空 ⇒ 现在是 1，属预期**。
  那是**有意的红灯**，不是套件坏了。
- 环境变量：`SMLTOOLS`（指定 Rust 侧可执行文件）、`SML_REPO`（覆盖仓库根）。
  用例 `24~26` 依赖 `SML_CONF_TOKEN=conf-token-1`，driver 自己会设，无需手工准备。

## 判据：怎么比（先看这段，否则会误读 AGREE）

每一端**只用本端的解析器**解析，再 dump 成本端的 SML 文本（这些能力本来就有）：
Rust `to_sml` ／ C `sml_dump` ／ C++ `to_sml` ／ JS `stringify` ／ Lua `Sml.dump`。

**归一化只有一份实现**：把该文本再喂回 `smltools`，得到 ① JSON 值树 ② 规范的 SML 文本。比较两层：

- **JSON 层** —— 结构与类型。键序由 Rust 侧统一，所以「C 保源序、Rust 排序」这类纯粹渲染差异不会误报。
- **token 层** —— 把规范文本切成 token，**数字按「整数 / 浮点 + 数值」归一**
  （`1` → int 1、`1.0` → float 1.0、`1e10` 与 `10000000000` → 同一个 float），字符串比解码后的内容。

两层都必要：

- 只比 JSON 会漏 —— `smltools --to json` 把 `1.0` 写成 `1`，「浮点身份」这条**点名要钉的**差异会被载体静默吃掉。
- 逐字节更不行 —— `1e10` / `1e+10` / `10000000000`、`3.14` / `3.1400000000000001` 是**同一个值**的
  不同写法（`docs/capability-matrix.md` §3 #7/#8 已定性为渲染差异）。
  逐字节比对只对 C↔Rust 成立，那是 W4 `tools/check_dump_parity.py` 的职责，两者分工不同。

## 基线（2026-10-28 全量跑）

工具链五端全可用（rust / c / cpp / js / lua）。用例 33 个：

| 端 | AGREE | DIVERGE | PARSE_FAIL | SKIP |
|---|---|---|---|---|
| rust（参考实现） | 32 | 0 | 1 | 0 |
| c | 28 | 4 | 1 | 0 |
| cpp | 24 | 5 | 4 | 0 |
| js | 26 | 5 | 2 | 0 |
| lua | 23 | 4 | 6 | 0 |

合计：**AGREE 17 / DIVERGE 16**（W24 修 C 单词修饰符前为 AGREE 15 / DIVERGE 18：
`c` 的 AGREE 25→28、PARSE_FAIL 4→1；`13/14` 整体转 AGREE、`16` 的 C 格转 A、`33` 仍 DIVERGE 属预期）。

口径三条（照这个读表，否则会读错）：

- `PARSE_FAIL` = 该端**没产出值树**（解析失败 `ERR`，或 dump 不能被 Rust 回读 `NC`），本来就没有树可比。
  它和 `DIVERGE` 都算「不一致」，都不许静默。
- **「两端都失败」不算一致**：判分先判「有没有产出值树」，再比分组。
  所以 rust 那 1 个 `PARSE_FAIL` 来自 `33_contract_modifier_alt` —— 参考实现自己也不认那套记号。
- **空用例是假绿**：0 字节的用例会让五端一起产出空树、一起 AGREE。
  driver 现在把 0 字节用例当**硬错误**拦下（`[!] 以下用例是 0 字节`），本目录也确实被并发写入清空过一次
  （`03` / `31`），已补回内容。

## 分歧清单（18 条，逐条登记，不许静默忽略）

「类别」三档：**缺陷** = 实现与既定口径不符，应修；**已知取舍** = 仓库已有定论、不接受修改或用例本身在钉既有口径；
**待定性** = 还没查到根因，先不判。**本轮已无「待定性」条目**（#12 / #17 / #18 都已查到根因）。

| # | 用例 | 分歧 | 类别 | 备注 / 关联 |
|---|---|---|---|---|
| 1 | `05_nest_array` | Lua 解析失败 `E-PARSE-006`（键位置遇到 `[`） | **缺陷** | Lua 侧嵌套数组不支持。C 侧同类问题已由 W17 修（`c/sml.c` 的 `parse_array` 加 `[` 递归 + 深度守卫），Lua 未同步 |
| 2 | `06_nest_array_mixed` | 同上（键位置遇到 `{`） | **缺陷** | 同 #1 |
| 3 | `07_nest_array_deep` | Lua 解析失败 `E-PARSE-003`（未匹配的 `]`） | **缺陷** | 同 #1 |
| 4 | `08_include_basic` | C++ 把 `include "..."` 当键值，不展开 | 已知取舍 | 即 TODO 排雷区「雷 6」；`docs/capability-matrix.md` 已登记 |
| 5 | `09_include_chain` | 同 #4（链式 include 的 `lib` / `chain` 全部缺失） | 已知取舍 | 同 #4 |
| 6 | `11_fragment_splice` | C++ 行级 `&frag` 未展开（多出 `&base` 键）；Lua 解析失败 `E-PARSE-001` | 已知取舍 + **缺陷** | C++ 那半是 TODO §五「W4-片段行级展开」（✋ 待裁决），**实测只剩 C++ 一格**（C 已实现，该行措辞需更新）；Lua 那半是新缺陷 |
| 7 | `13_contract_default` | ~~C 报 `E-CONTRACT-003`~~ **C 半 W24 已修**：C 现认 `optional`/`default`，该格转 A，用例整体 AGREE | **已修复（W24）** | 见下方「#7–#9 + #18 是同一件事」 |
| 8 | `14_contract_enum_range` | ~~C 报 `字段 default 必填但缺失`~~ **C 半 W24 已修**：转 A，用例整体 AGREE | **已修复（W24）** | 同 #7 |
| 9 | `16_contract_compose` | **C 半 W24 已修（转 A）**；JS 仍丢 `o.inner.tag` | **缺陷（JS 半）** | C 那半同 #7；JS 那半见下方「#9 的 JS 半」 |
| 10 | `19_num_one_dot_zero` | Rust 保 `1.0` 为 float，其余四端当 int | 已知取舍 | `docs/capability-matrix.md` 已登记「`1.0` 只有 Rust 保留为浮点」；雷 5 B 组 `format_float` |
| 11 | `20_num_bigint` | Lua 丢精度：`330106201503071234` → `...070000`，`i64::MAX` → `...8000000` | 已知取舍 | 能力矩阵已登记「大整数 >2^53 分裂；Lua 转 float 丢精度」；雷 6 #1。⚠️ 属**数据保真**级，但 LuaJIT 值模型只有 double，改不了。**JS 那格的 AGREE 是载体盲区**，见 §载体盲区 B-01 |
| 12 | `22_escape_basic` | C++ 的 dump 不能被 Rust 回读 | **已修复（fix/needs-quote-escaping）** | 根因：C++ 的 dump 只对「含空格」加引号（`v->s.find(' ')`），**含换行/制表/回车的串原样写出**，`b` 被读成新键 ⇒ `E-PARSE-006`。本任务（fix/needs-quote-escaping）在 `cpp/sml.cpp` 新增 `dump_needs_quote`（空白/`#{}`/`"`/`\` 均判为需引号，**换行一并纳入**）+ `dump_escape_str`（转义串内 `"`/`\`），`dump_value`/`dump_inline` 的 Str 分支改用之。现状：该用例五端全 AGREE |
| 13 | `23_escape_unicode` | JS 的 `\uXXXX` 多出数字：`aAb` → `aA1b`、`xéy` → `xé9y`；C++ 报 `E-LEX-005` 位数不足 | **缺陷** | 能力矩阵已登记「`\uXXXX` JS 有 bug」（根因 `js/sml.mjs` 的 `i += 4` 应为 `i += 5`）；C++ 只认 `\u{...}`。本用例是两者的可复现实证 |
| 14 | `25_env_quoted_exact` | C 在引号内不替换 `$env`（得到空串） | 已知取舍 | 能力矩阵「引号内三档行为：Rust 真插值 / C 不替换 / C++·JS·Lua 仅整串锚定」 |
| 15 | `26_env_quoted_embed` | 只有 Rust 做真插值，其余四端原样保留 `$env.SML_CONF_TOKEN` | 已知取舍 | 同 #14（三档口径的中段）。三档要不要收敛、收敛到哪一档，属 ✋ |
| 16 | `30_typed_block_meta` | JS / Lua 缺 `__type` / `__name` 元数据 | 已知取舍（Lua）／ **缺陷**（JS） | Lua 已登记（能力矩阵 三·十「Lua 连元数据都丢」）；**JS 这半未登记**：能力矩阵只说「typed-block 只有 Rust + JS 实现」，没说「未开特性时 JS 也不给元数据」 |
| 17 | `32_escape_quote` | 除 Rust 外四端 dump 都无法被回读（`E-LEX-001` 字符串未闭合） | **已修复（fix/needs-quote-escaping）** | 根因：四端的 `needs_quote` / `quoteIfNeeded` 只判空白与 `:#{}`，**不判 `"` 与 `\`** ⇒ `quote"in` 原样写出、非法 SML。本任务（fix/needs-quote-escaping）四端同修：`c/sml.c` `needs_quote`、`cpp/sml.cpp` `dump_needs_quote`、`js/sml.mjs` `quoteIfNeeded`（正则加 `"\\`）、`lua/lib/sml.soup` `sml_needs_quote` 均纳入 `"`/`\`；**转义两端也一并补**：C 与 Lua 此前加引号却漏转义串内 `"`/`\`（已补 `escape`/`sml_escape_str`），JS 原已有转义。现状：该用例五端全 AGREE |
| 18 | `33_contract_modifier_legacy_rejected` | ~~只有 C 接受 `?` / `=` 记号~~ **已废弃（2026-10-30）**：原 C 与 JS 支持符号式，现五端统一按 `E-PARSE-006` 拒绝 | **已修复** | 裁决：废弃符号形式、统一到单词式 `optional` / `default`。C 删 T_QMARK/T_EQ 分支；JS 删 `?` 分支；C++/JS 对 `?`/`=` 的报错从 `E-CONTRACT-003` 对齐到 `E-PARSE-006`（C++ 还修了「`parse_field` 失败被 `st.err` 吞掉」——现在失败即中断解析）。用例已改名并改判为 `expect-error E-PARSE-006`，五端全 AGREE |

### #7–#9 + #18 是同一件事：契约修饰符有**两套不兼容的记号**

| 修饰符 | Rust | C++ | JS | Lua | **C** |
|---|---|---|---|---|---|
| 可选 | 单词 `optional` | 同 | 同 | 同 | **`?` / `optional`**（W24 起 `optional` 也认） |
| 默认值 | 单词 `default <v>` | 同 | 同 | 同 | **`= <v>` / `default <v>`**（W24 起 `default <v>` 也认） |
| 区间 | `min` / `max` | 同 | 同 | 同 | 同 ✅ |
| 放宽 | `loose` | 同 | 同 | 同 | 同 ✅ |

- **实跑证据（修复前）**：`zip: str optional` 在 C 里被读成「字段 `zip`」+「字段 `optional`」⇒
  `E-CONTRACT-003 字段 'optional' 必填但缺失`；`port: int default 5432` 同理。
  反方向：`country: str = CN` 在 Rust 里 ⇒ `E-PARSE-006 契约字段 '=' 后须有冒号`。
- **根因（修复前）**：`c/sml.c` 的 `parse_contract_body` 只认 `?`（`T_QMARK`）/ `=`（`T_EQ`）/ `min` / `max` / `loose`；
  `optional` / `default` 这两个单词那里**没有分支**（全文零命中）。
  而 README / `showcase_contract.sml`、教科书里的契约示例写的都是**单词形式** ⇒
  **照文档写的契约在 C 上必然失败**。
- **W24 修复（C 半）**：`c/sml.c` 的 `parse_contract_body` 已补 `optional`（置 `required=0`）/
  `default <v>`（复用 `coerce_word`/`sml_new_str` 回填，与 `=` 同路径）两个分支。纯增量：
  C 原不认这两个词，新增分支不让任何原本成功解析的文档变失败；`?`/`=` 符号形式**保留未动**。
  修复后 `13/14` 整体 AGREE、`16` 的 C 格转 A（余 JS 半仍 DIVERGE，见 #9 的 JS 半）。
- ⚠️ 别和 IDEAS I-8 混起来：I-8 说的是**数据行**的 TOML 风格 `x = 1`（`=` 还不是 token，需要 opt-in）。
  这里是**契约体内**的 `=`，C 有独立的 `T_EQ` token —— 两件事。
- **✅ 已裁决并落地（2026-10-30）**：废弃符号形式，统一到单词式 `optional` / `default`。
  W24 先补了 C 的单词形式（零风险的一半）；随后本棒废弃了符号形式：
  C 删 `?`/`=`、JS 删 `?`，C++/JS 报错码对齐到 `E-PARSE-006`，`33` 转 AGREE。
  ⚠️ 这是**破坏性变更**（下游若用 `?` / `=` 写过契约会报错）；迁移：`x: str?` → `x: str optional`、
  `p: int = v` → `p: int default v`。

### #9 的 JS 半：组合契约的**子契约默认值不回填**

- 期望 `o.inner = { x: 1, tag: t }`（`tag` 由子契约 `default t` 回填），JS 只给 `{ x: 1 }`。
- 根因方向：`js/sml.mjs` 的 `applyDefaults` 只在**当前层**按契约表补字段，
  字段类型是另一个契约（组合）时没有递归下去。父契约自己的字段没问题。
- 能力矩阵只说「契约五端都实现」，没细分「组合 + 默认值回填」的交叉 ⇒ 未登记。

## 载体的已知盲区（**重要**：这几格 AGREE 不能当成「已对齐」）

本套件用**文本载体**（各端 dump → Rust 归一化）还原值树，有两处表达不了：

### B-01 「数字样子的字符串」回读会变成数字

- **格子**：`20_num_bigint` 的 **JS**
- **实情**：JS 解析 `id: 330106201503071234` 得到的是**字符串**（`typeof === "string"`），
  但 `stringify` **不加引号** ⇒ 归一化回读成 int ⇒ 与 Rust 的 int 一模一样 ⇒ 判 **AGREE**。
- **结论**：AGREE ≠ 对齐。真相同能力矩阵 §1.3（JS 升为字符串）；
  同时它暴露了 JS dump 的另一个缺陷：**数字形状的字符串写成裸词 ⇒ 类型信息丢失**（与 #17 同族）。
- **直接取证**（不经套件）：
  ```
  node -e "import('./js/sml.mjs').then(m=>{const v=m.parse('id: 330106201503071234\n');console.log(typeof v.id, m.stringify(v));})"
  ```
- **根治方向**：让每端交出「带类型标注」的值树（而不是 SML 文本）。
  那要 C / C++ / Lua 各写一个 JSON 发射器，会引入另一批写法差异 ⇒ 本轮刻意不做，记在这里。

### B-02 `\0`（NUL）这一格无法承载

- `s_nul: "nul\0end"` 解码出真正的 0x00 字节。实测：
  - **Rust** 的 `to_sml` 把 NUL **原样写出**（不转义成 `\0`）⇒ 写进文件后自己读不回来；
  - **C** 的 `sml_dump` 走 C 字符串 ⇒ 在 NUL 处**截断**。
- ⇒ 该格**未覆盖**，已从 `22_escape_basic.sml` 移出并在用例注释里写明原因。
- 顺带结论（该单独修，不属「两端不一致」）：**Rust 的 dump 不转义 NUL**、**C 的 dump 在 NUL 处截断**。

## 顺带发现（不是两端分歧，但该记账）

- **F-01 README 的顶层数组示例本身解析不过。**
  `README.md` 那段 `[ { ts: 2026-08-30T10:00:00Z to: a@b.c status: ok } … ]` 里，
  裸写的 `2026-08-30T10:00:00Z` **含 `:`** ⇒ 五端一律 `E-PARSE-006`（实测）。
  文档要么给时间戳加引号，要么换值。（`02_top_array_object.sml` 因此刻意不用时间戳。）
- **F-02 能力矩阵 §1.1「`top-level-array` 只有 Rust + C」已过时。**
  `02` / `03` 五端**全部 AGREE**（C++ / JS / Lua 现在都支持顶层数组）。
- **F-03 同矩阵 三·十「`fragment` 的 `&base` 展开：C 不展开」已过时。**
  `11_fragment_splice` 里 **C 与 Rust 同组**，只剩 C++。
- **F-04 矩阵缺一格**：Lua 的嵌套数组缺陷（#1–#3）在能力矩阵里没有对应行。
- **F-05 `31_dup_key` 五端一致**（同名键提升为 `k: [1, 2]`）——
  TODO P2「同名键冲突（Rust 提升为数组）」这条待核实项，就「提升」行为本身**已一致**；
  各端**是否给告警**不在本套件判据内，仍待单独核对。
- **F-06 T9 收紧：顶层值型标量后跟容器，五端统一报 `E-PARSE-008`（`34_top_scalar_then_block`，AGREE）。**
  改判据前 `42\na { b: 1 }` 在五端**静默成功**（`42` 被重解释成块类型名，凭空造键 `"42"`，
  重新序列化 ≠ 原文）——那是「全员一致的静默错」，本套件抓不到，由单端错误码回归钉住。
  现判据（五端逐字同，Rust 唯一事实来源）：首个顶层结构是**值型标量**（数字/引号串/bool/null）
  且其后紧跟块/数组/键值 ⇒ 报；裸词标识符作块类型名/块名**不算**值型
  （`server web { }` 合法，见 `30_typed_block_meta`），`hello world`、`@version v1`+`42`、
  `42 { a: 1 }` 均不报。用例登记在 `errors/codes.sml` 的 E-PARSE-008 注记。

## 后续动作（按建议顺序）

1. **修 #1–#3**（Lua 嵌套数组）：与 W17 的 C 修复同族，可照抄做法（加 `[` 递归分支 + 与块共用深度闸）。
2. **定契约修饰符的规范**（#7–#9 + #18）：先裁决记号，再改一侧 + 同步全部文档。这是 ✋ 级。
3. ~~**修 #12 / #17**（dump 的引号策略）~~ **已修（fix/needs-quote-escaping）**：判据是「自己 dump 出来的东西必须能自己读回去」，
   四端已让 `needs_quote` / `quoteIfNeeded` 把 `"`、`\` 与换行也算作「需要引号」，并补上串内 `"`/`\` 转义。
4. **修 #13 的 JS 半**（`i += 4` → `i += 5`）。
5. **修 #9 的 JS 半**（组合契约递归回填默认值）。
6. #4/#5/#6 的 C++ 半属 ✋ 待裁决（雷 6 / W4-片段行级展开），不在自动化范围。
7. #10/#11/#14/#15/#16（Lua 半）是**已知取舍**，建议保持现状，只在规范文档里写清
   「这几格跨端不一致是有意的」。
8. **B-01 的载体盲区**：等有 JSON 直发通道后再补判据；在那之前别忘了这几格 AGREE 的含义。

## 待清理的临时探针（本轮**刻意不删**）

理由：套件刚立起来，这些探针还是唯一的对照物；**等套件稳定（全 AGREE + 台账收敛）后单独一棒清理**，
否则中途删了就没有对照物。清单（均被 `.gitignore` 的 `**/_*` 覆盖、未跟踪）：

- 仓库根：`_probe_js.mjs`、`_probe_js_cover.mjs`、`_test_gov_demo.mjs`、`_audit_gov.mjs`、
  `_probe2.sml`、`_probe3.sml`、`_for_probe.sml`、`_probe_for.sml`、`_rust_test.bat`、`_scratch/`
- `c/`：`_audit_c_probe.c`、`_audit_num.c`、`_audit_rs_bridge.c`、`_minload.c`、`_probe_c.c`(+`.exe`)、
  `_w10_probe{1..4}.c`、`_w4_dump.c`(+`.exe`)、`_w16_dump.c`、`_w16_head_sml.c`、`_w16_one.c`、
  `_w16_probe.c`、`_w16_scan.c`、`_w16_sml_before.c`
- `cpp/`：`_mine_probe.cpp`、`_probe_cpp.cpp`(+`.exe`)、`_smlcpp_before.cpp`、`_w10_cycle_probe.cpp`、
  `_w10_errno_probe.cpp`、`_w10_head_sml.cpp`、`_w10_include_probe.cpp`、`_w10_inc_chain.cpp`、
  `_w10_min_probe.cpp`、`_w10_probe.cpp`、`_w16_probe.cpp`、`_w16_scan.cpp`
- `js/`：`_probe_against_head.mjs`、`_probe_contract_types.mjs`、`_probe_inc_line.mjs`、`_probe_regex_inc.mjs`、
  `_test_*.mjs`、`_typed_check*.mjs`、`_w16_*.mjs`、`_head_sml.mjs`
- `lua/`：`_probe_lua.lua`、`_w16_probe.lua`、`_w16_scan.lua`、`_w16_uchk.lua`、`_sml_before.soup`
- 其它运维脚本（未必该删，逐个判）：`_build_wasm.py`、`_sync_wasm.py`、`_sync_playground.py`、
  `_du.py`、`_dump_ascii.py`

- ⚠️ TODO §三 提到的 `_verify_showcase.*` 现在**已经不在仓库里**（实测未找到），无需处理。
- 删之前先确认它们**不再被任何脚本引用**（`grep -r`），且对应能力已被本套件覆盖
  （`_probe_contract_types.mjs` 的 19 个用例里，契约/类型相关的格子已被 `13~16` / `33` 覆盖）。

## 新增用例的约定

1. 放进 `tests/conformance/cases/`，文件名 `<两位序号>_<语义>.sml`。
2. 文件头用 `#` 注释写清「这个用例钉的是什么」；若是已知分歧，指向本台账的编号。
3. 负向用例（必须以某个码失败）在头部加一行：`# conformance: expect-error E-PARSE-008`。
4. 辅助文件放 `cases/inc/`（driver 只扫 `cases/` **顶层**，子目录不会被当成用例）。
5. **不许留 0 字节用例**（会让五端一起 AGREE，属假绿）；driver 会直接拦下。
6. 跑出来是 DIVERGE ⇒ **先判真假**：是「某端实现不同」还是「用例写错了 / 载体承载不了」？
   前者登记进本台账；后者改用例并在注释里写明原因。
