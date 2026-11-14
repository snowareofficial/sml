# `lua/lib/sml/` —— `lib/sml.lua` 的子模块

`lib/sml.lua` 是 ~3000 行的单文件，且大量函数依赖**模块级状态**（`_FEATURES`、`_LINES`、
`_LOOP_VARS`、解析游标 `i` 通过参数传递）。直接按行切会把 upvalue 切断，所以按
「先纯函数、后状态」推进。

## 已抽出

| 模块 | 内容 | 依赖 |
|---|---|---|
| `strings.lua` | `looks_number` / `needs_quote` / `escape` | 只用 `string` / `tonumber` |

`lib/sml.lua` 顶部：

```lua
local sml_strings = require("lib.sml.strings")
local sml_looks_number = sml_strings.looks_number
local sml_needs_quote = sml_strings.needs_quote
local sml_escape_str = sml_strings.escape
```

import 路径要求 `package.path` 含 `lua/?.lua`（`run_check.py`、conformance 适配器、
`probe_matrix.py` 都这么设）。

## 后续切片（按风险从低到高）

1. `ident.lua`：标识符 / 特性名表（纯数据 + 查表）；
2. `value.lua`：`deep_copy` / `is_array` / `ARRAY_MT` / `NULL` 哨兵（值模型内核，先搬它才能搬 dump）；
3. `dump.lua`：`dump_value`（依赖上一项）；
4. `lexer.lua`：`tokenize`（依赖 `_FEATURES`，需要把状态显式传入或提供 setter）；
5. `parser.lua`：最大一块，需要先把游标 `i` / `tokens` / `frames` 收进一个 `Ctx` 表，
   属**行为重构**，单独一轮做，用 conformance 兜底。

## 验收

```bat
python lua/run_check.py
python tests/conformance/run_conformance.py --require-all
python tools/check/probe_matrix.py --suite values
```

## 数值模型提醒

LuaJIT 只有 `double`：`1.0` 与 `1` 不可区分、超过 2^53 的整数已按字符串保留（与 JS 同口径）。
`probe_matrix` 的 `19` / `20` 两条因此长期放行，别再尝试在解析层“修”它。
