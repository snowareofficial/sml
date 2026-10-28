# 跨实现一致性：已知分歧台账

本目录是 W5 的产出：**一份共享用例，五端（Rust / C / C++ / JS / Lua）各跑一遍，比较值树**。

## 用法

```bat
python tests/conformance/run_conformance.py
python tests/conformance/run_conformance.py --case 05 --end lua
python tests/conformance/run_conformance.py --show 3
```

- 缺工具链的端标 `SKIP` 并说明缺什么（**不会**悄悄当成通过）。
- 退出码：`0` = 无 DIVERGE；`1` = 存在 DIVERGE / 归一化失败。
- 比对的是**值树**（各端 dump → 由 Rust `--to json` 归一化 → 结构比较），不是字节：
  逐字节比对只对 C↔Rust 成立（W4 的 `tools/check_dump_parity.py` 已做到 0 差异），
  跨到 JS/Lua 会因为下面这些既有差异必然误报。

## 基线（2026-10-28 首次全量跑）

工具链五端全可用（rust / c / cpp / js / lua）。用例 33 个：

| 端 | AGREE | DIVERGE | PARSE_FAIL | SKIP |
|---|---|---|---|---|
| rust（参考实现） | 33 | 0 | 0 | 0 |
| c | 25 | 4 | 4 | 0 |
| cpp | 24 | 5 | 4 | 0 |
| js | 26 | 5 | 2 | 0 |
| lua | 23 | 4 | 6 | 0 |

合计：**AGREE 15 / DIVERGE 18**。

## 分歧清单（18 条，逐条登记，不许静默忽略）

「类别」三档：**缺陷** = 实现与既定口径不符，应修；**已知取舍** = 仓库已有定论、不接受修改或用例本身在钉既有口径；**待定性** = 还没查到根因，先不判。

| # | 用例 | 分歧 | 类别 | 备注 / 关联 |
|---|---|---|---|---|
| 1 | `05_nest_array` | Lua 解析失败 `E-PARSE-006`（键位置遇到 `[`） | **缺陷** | Lua 侧嵌套数组不支持。C 侧同类问题已由 W17 修（`c/sml.c` 的 `parse_array` 加 `[` 递归 + 深度守卫），Lua 未同步 |
| 2 | `06_nest_array_mixed` | 同上（键位置遇到 `{`） | **缺陷** | 同 #1 |
| 3 | `07_nest_array_deep` | Lua 解析失败 `E-PARSE-003`（未匹配的 `]`） | **缺陷** | 同 #1 |
| 4 | `08_include_basic` | C++ 把 `include "..."` 当键值，不展开 | 已知取舍 | 即 TODO 排雷区「雷 6」；`docs/capability-matrix.md` 已登记 |
| 5 | `09_include_chain` | 同 #4（链式 include 的 `lib` / `chain` 全部缺失） | 已知取舍 | 同 #4 |
| 6 | `11_fragment_splice` | C++ 行级 `&frag` 未展开（多出 `&base` 键）；Lua 解析失败 `E-PARSE-001` | 已知取舍 + **缺陷** | C++ 那半是 TODO §五「W4-片段行级展开」（✋ 待裁决）；Lua 那半是本表 #1 同族 |
| 7 | `13_contract_default` | C 报 `E-CONTRACT-003 字段 optional 必填但缺失` | **缺陷** | C 把修饰符 `optional` 当成了字段名 —— 契约字段解析在有修饰符时错位 |
| 8 | `14_contract_enum_range` | C 报 `字段 default 必填但缺失` | **缺陷** | 同 #7（`default` 被当字段名） |
| 9 | `16_contract_compose` | C 报 `字段 default 必填但缺失`；JS 丢 `o.inner.tag` | **缺陷** | C 那半同 #7；JS 那半是组合契约的默认值回填丢字段 |
| 10 | `19_num_one_dot_zero` | Rust 保 `1.0` 为 float，其余四端当 int | 已知取舍 | `docs/capability-matrix.md` 已登记「`1.0` 只有 Rust 保留为浮点」 |
| 11 | `20_num_bigint` | Lua 丢精度：`330106201503071234` → `...070000`，`i64::MAX` → `...8000000` | 已知取舍 | 能力矩阵已登记「大整数 >2^53 分裂；Lua 转 float 丢精度」。⚠️ 属**数据保真**级，但 Lua 值模型只有 float，改不了 |
| 12 | `22_escape_basic` | C++ 的 dump 不能被 Rust 回读（`E-PARSE-006` 期望键得 Colon） | 待定性 | 需判断是 C++ dumper 转义问题还是解析问题 |
| 13 | `23_escape_unicode` | JS 的 `\uXXXX` 多出数字：`aAb` → `aA1b`、`xéy` → `xé9y`；C++ 报 `E-LEX-005` 位数不足 | **缺陷** | 能力矩阵已登记「`\uXXXX` JS 有 bug」——本用例是它的可复现实证。C++ 那半需单独确认 |
| 14 | `25_env_quoted_exact` | C 在引号内不替换 `$env`（得到空串） | 已知取舍 | 能力矩阵「引号内三档行为：Rust 真插值 / C 不替换 / C++·JS·Lua 仅整串锚定」 |
| 15 | `26_env_quoted_embed` | 只有 Rust 做真插值，其余四端原样保留 `$env.SML_CONF_TOKEN` | 已知取舍 | 同 #14（三档口径的中段） |
| 16 | `30_typed_block_meta` | JS / Lua 缺 `__type` / `__name` 元数据 | 已知取舍 | 能力矩阵已登记；Lua 干脆不产该元数据 |
| 17 | `32_escape_quote` | 除 Rust 外四端 dump 都无法被回读（`E-LEX-001` 字符串未闭合） | 待定性 | **疑似四端 dumper 都不转义双引号**，若确认为真则是「序列化不可往返」级缺陷。先定性再定类 |
| 18 | `33_contract_modifier_alt` | 只有 C 接受该写法，Rust/C++/JS/Lua 全按 `E-PARSE-006`（契约字段 `=` 后须有冒号）或 `E-CONTRACT-003` 拒 | 待定性 | 用例钉的是 `=` 写法。按 IDEAS I-8，TOML 风格 `=` **尚未实现**（要 opt-in）⇒ 四端拒绝是对的，**C 过宽**。需确认 C 是否有意放宽 |

## 后续动作（按建议顺序）

1. **先定性 #17**（`32_escape_quote`）：四端 dumper 是否都不转义 `"`。它决定这条是「序列化层缺陷」还是「适配器写错」。
2. **修 #1–#3**（Lua 嵌套数组）：与 W17 的 C 修复同族，可照抄做法（加 `[` 递归分支 + 深度守卫共用）。
3. **修 #7–#9**（C 契约修饰符错位）：契约字段解析把 `optional` / `default` 当字段名，影响所有带修饰符的契约。
4. **修 #13 的 JS 半**（`\uXXXX`）。
5. #4/#5/#6 的 C++ 半属 ✋ 待裁决（W4-片段行级展开 / 雷 6），不在自动化范围。
6. #10/#11/#14/#15/#16 是**已知取舍**，建议保持现状，只在规范文档里写清「这五格跨端不一致是有意的」。

## 待清理的临时探针（本轮**刻意不删**）

套件稳定（全 AGREE + 台账收敛）之后单独一棒清理，届时以本套件为对照物：

- `c/`：`_audit_c_probe.c`、`_audit_num.c`、`_audit_rs_bridge.c`、`_minload.c`、`_probe_c.c`、`_w10_probe{1..4}.c`、`_w16_dump.c`、`_w16_head_sml.c`、`_w16_one.c`、`_w16_probe.c`、`_w16_scan.c`、`_w16_sml_before.c`、`_w4_dump.c`
- `cpp/`：`_mine_probe.cpp`、`_probe_cpp.cpp`、`_smlcpp_before.cpp`、`_w10_cycle_probe.cpp`、`_w10_errno_probe.cpp`、`_w10_head_sml.cpp`、`_w10_include_probe.cpp`、`_w10_inc_chain.cpp`、`_w10_min_probe.cpp`、`_w10_probe.cpp`、`_w16_probe.cpp`、`_w16_scan.cpp`
- 以及任何 `_verify_showcase.*` 残留

⚠️ 删之前先确认它们**不再被任何脚本引用**（`grep -r`），且对应能力已被本套件覆盖。
