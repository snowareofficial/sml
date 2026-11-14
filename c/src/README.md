# `c/src/` —— `c/sml.c` 的模块视图

`c/sml.c` 原本是 3600 行的单文件。这里按它自带的九节横幅（错误写入 / 值 / 词法 / 契约 /
coerce / 解析 / 序列化 / JSON / include）拆成九个 `.inc`，`sml.c` 按顺序 `#include` 回来。

## 为什么是 `.inc` 而不是独立 `.c`

`c/sml.c` 被至少六处构建与测试脚本**直接编译**：

- `.github/workflows/ci.yml`
- `c/build_check.py`、`c/tests/*`（`gcc … c/sml.c`）
- `tests/conformance/adapters/`（C dumper）
- `tools/gen/dump_c.c`（W4 序列化比对）
- `tools/check/check_dump_parity.py` / `probe_matrix.py`（运行时编译探针）

拆成独立 TU 要同时改全部脚本、并给每个模块划出 `static` 边界（当前模块间大量共用
`parser` / `sml_value` 的内部结构与 static 辅助函数）。先做「物理拆分 + 原位置 include」，
拿到导航与 review 的收益，语义零变化。

## 验收（每次改完都要跑）

```bat
gcc -std=c99 -I c -E c/sml.c > pp_after.i     :: 与拆分前的预处理输出对比（忽略 # 行号）
python c/build_check.py                        :: 编译 + 语法
python tests/conformance/run_conformance.py --require-all
python tools/check/probe_matrix.py
```

拆分当天做过一次 `gcc -E` 逐行比对：**5383 行代码完全相同**，差异只有模块边界的空行。

## 要改成真 TU 时

1. 先给模块划头文件：`parser` / `sml_value` / `contract` 的内部结构目前是 `static` 与
   匿名 `struct`，得先提升成 `c/src/*.h` 里的不透明类型 + 访问函数（否则跨 TU 用不了）；
2. 按依赖顺序搬：`00_errors` → `01_value` → `02_lex` → `03_contract` → `04_coerce` →
   `05_parse` → `06_dump` → `07_json` → `08_include`；
3. 每搬一个就更新上面六处构建脚本里的源文件列表（一次全改会同时失去可编译状态）；
4. `sml_free` / `sml_dump` 的递归深度与 `MAX_INC_DEPTH` 的栈用量见下节，别在搬运时把
   已经降到堆上的大缓冲又挪回栈上。

## 栈帧与解析深度（2026-11-14 实测）

`gcc -fstack-usage` 的口径（x86-64，`-O2`）：

| 函数 | 拆分前 | 现在 | 说明 |
|---|---|---|---|
| `parse_file_core` | 35008 B | **1216 B** | `char stack[33][1024]` 是 include 环检测栈，改 `calloc` |
| `json_to_value_depth` | 4224 B | **384 B** | 递归函数；4 KB 解码缓冲抽到非递归的 `json_read_string` |
| `parse_block_inner` | 720 B | 720 B | 递归主体，128 层 ≈ 90 KB，安全 |
| `resolve_includes` | 2752 B | 2752 B | 递归上限 `MAX_INC_DEPTH`=32 ⇒ ≈ 86 KB |

结论：递归路径（`parse_block_inner` / `parse_array` / `json_to_value_depth`）现在最深约
**90 KB**，默认 1 MiB 栈有 10 倍余量；入口帧从 35 KB 降到 1.2 KB，小栈宿主（WASM / 8 KB
线程）也能调用。
