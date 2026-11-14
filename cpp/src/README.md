# `cpp/src/` —— `cpp/sml.cpp` 的模块视图

`cpp/sml.cpp` 是 2700 行的单文件。这里按函数边界拆成八个 `.inc`，`sml.cpp` 按序
`#include` 回来。**仍然是同一个翻译单元**，预处理输出与拆分前逐行相同（59320 行代码完全一致，
差异只有模块边界空行，已用 `g++ -E` 比对）。

## 模块顺序

| 模块 | 内容 |
|---|---|
| `00_prelude_util` | 头文件、命名空间、错误码前缀 / 包装、`Value::clone` |
| `01_lex` | `tokenize`、转义解码、`looks_like_int/float` |
| `02_coerce_type` | `coerce_word` / `coerce`、`parse_type`、`parse_field`、契约校验 |
| `03_feature_scan` | 特性名表、版本字面量、`scan_features` |
| `04_weak_bare` | `weak_*`、`is_quoted_word`、`bare_block_ahead`、`parse_bare_block` |
| `05_parse` | 深度守卫、`parse_block`、`parse_value`、`parse_array`、`parse_for` |
| `06_include_api` | `expand_includes`、`parse_impl`、`Parser::parse*`、`BuiltinTable`、`apply_contract` |
| `07_dump` | `dump_*`、`fmt_float`、`Parser::to_sml` |

## 为什么是 `.inc`

`cpp/sml.cpp` 同样被多处直接编译（`cpp/run_tests.py`、conformance 适配器、`probe_grammar.py`
运行时编译）。拆成独立 TU 需要先划出公开头文件边界并改全部脚本；先做物理拆分，
收益（导航、review、diff 粒度）即可拿到。

## 验收

```bat
g++ -std=c++17 -I cpp -E cpp/sml.cpp > pp_after.i   :: 与拆分前对比（忽略 # 行）
python cpp/run_tests.py
python tests/conformance/run_conformance.py --require-all
python tools/check/probe_grammar.py grammar.sml
```

## 栈帧（2026-11-14 实测，`-fstack-usage`，x86-64 -O2）

| 函数 | 帧 | 深度 | 说明 |
|---|---|---|---|
| `parse_block` | 880 B | 递归，上限 128 | ≈ 113 KB |
| `expand_includes` | 1584 B | 递归，上限 32 | ≈ 50 KB |
| `parse_type` | 912 B | 非递归 | 一次性 |

结论：最坏路径约 113 KB，默认 1 MiB 栈有 9 倍余量。**改这两个递归函数时不要新增大的栈缓冲**
（>1 KB 的数组请 `std::vector` 或 `std::string`），否则 128 层会放大成几十 MB。
