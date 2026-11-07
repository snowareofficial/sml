# 剩余任务（临时，2026-11-07）

> **先读 `docs/repo/ref/unfinished.md`** —— 那是唯一的「还剩什么」清单。
> 本文件是给另一个 agent 的工作单，完成后应删除（历史归 git）。

## 已完成（不要重做）

| 项 | 状态 | 证据 |
|---|---|---|
| conformance | **41 / 0** | `python tests/conformance/run_conformance.py` |
| `.ambr` 跨端读写 | 21/21 **逐字节** | `lua/test_ambr_write.lua`（FFI + 纯 Lua 两个后端） |
| I-14 排雷 | unwrap 310→2 修复、as 230→1 修复 | 提交 `9c429b1` / `4816795` |
| clippy `missing_safety_doc` | **45 → 0** | 提交 `42ad25c` / `62c9e8d` / `b2cc482` |
| 32 位编译门 | `compile_error!` + `allow-32bit` | 提交 `763da80` |
| clippy 自动修复 | 24 文件（`needless_return` 等） | 提交 `59e9fc5` |

## 剩余（按优先级）

### P1 — clippy 其它 lint

```bash
cd rust
cargo clippy --workspace --all-targets --message-format=json 2>&1 \
  | Select-String '"code":"(clippy::[a-z_]+)"' -AllMatches
```

当前剩余约 **153** 个（`missing_safety_doc` 已清零）：

| lint | 约数 | 性质 |
|---|---|---|
| `doc_lazy_continuation` | 29 | 文档缩进，纯格式 |
| `collapsible_match` | 19 | 嵌套 match 可合并 |
| `doc_overindented_list_items` | 13 | 文档缩进 |
| `useless_format` | 11 | 机械 |
| `type_complexity` | 11 | 可能加 `#[allow]` 或抽 type alias |
| `large_enum_variant` | 9 | 需判断（可能 `Box` 大字段） |

**顺序建议**：文档类 42 → `collapsible_match` 19 → `useless_format` 11 → 其余逐个判断。

### P2 — FFI 安全加固

| 手段 | 做法 | 收益 |
|---|---|---|
| ASAN | `gcc -fsanitize=address` 编 `c/tests/*.c`，跑 21 例 | 抓 use-after-free / double-free |
| cbindgen | 生成 `sml.h`，与 `c_abi.rs` 的导出比对 | 防 ABI 漂移 |
| FFI fuzz | 垃圾指针 / 长度错配 | 抓边界 UB |

**不要做**：类型化句柄（newtype 区分借用/所有权）——改动面覆盖全部 `sml_*` 签名，发布前不动。

### P3 — `13.2 ④` 的分配优化

`load` 对 `flat@4000` 有 8173 次分配（节点级）。降下来只有两条路：

1. Arena / bump 分配（`Value` 改成索引式，API 大改）
2. `SmallString`（键 < 22 字节免分配）

两条都是**重构**，不是「提升 15%」的小步。单独立项。

## 约束

- **不改 `docs/` 除 `unfinished.md`**（文档由用户维护）
- **不碰 `rust/amberice/` 核心**，除非有明确 bug
- 所有「完成」必须给**实测输出**，不许只写代码
- 判据：`tests/vectors/amberice-container.v1.json` 的 21 例
- 文档纪律见 `TODO.md §9`：`unfinished.md` 是唯一清单，不手工维护第二份

## 可直接复制的提示词

```
仓库：C:\Users\sakeen\Desktop\sml

先读（只读）：
- docs/repo/ref/unfinished.md   ← 唯一「还剩什么」清单
- docs/repo/archive/task-2026-11-07-remaining.md  ← 本工作单

已完成，不要重做：
- conformance 41/41；.ambr 跨端读写 21/21 逐字节
- I-14 排雷（unwrap 310、as 230）已修
- clippy missing_safety_doc 45→0；32 位编译门已加

任务：清理 clippy 剩余 lint（约 153 个），按此顺序：
1. doc_lazy_continuation (29) + doc_overindented_list_items (13) —— 纯文档缩进
2. collapsible_match (19)
3. useless_format (11)
4. 其余逐个判断（type_complexity / large_enum_variant 可能加 #[allow]）

约束：
- 一次只做一类 lint，做完跑 cargo test 再继续
- 不改 docs/（除 unfinished.md）
- 不碰 rust/amberice/ 核心
- 每类完成给「修复前数量 → 修复后数量」的实测输出
- 用 cargo clippy --fix 处理机械项，但每次 fix 后必须跑测试

验收：
- cargo clippy --workspace --all-targets 的警告数逐类下降
- cargo test --workspace 通过（注意：本机有 LNK1104 文件占用问题，
  用 cargo test -p <crate> 逐个跑）
```
