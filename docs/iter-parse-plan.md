# 迭代式解析改造计划 · TODO 雷 1 的根因

> 状态：**待开工**（2026-10-30 完成分析与方案设计，未动代码）。
> 本文是开工时的作业书 —— 含递归漏斗的**确切位置**、状态机设计、迁移清单与验收判据。

## 0. 为什么要做（实测数据，不是推测）

| 量 | 实测值 |
|---|---|
| 每层嵌套的栈成本（debug 构建） | **约 8 KB** |
| 128 层（`MAX_VALUE_DEPTH`）闭合嵌套 | ≈ 1.05 MB |
| 线程栈 2 MiB（Rust 默认） | **通过** |
| 线程栈 1 MiB | **崩**（`0xC00000FD` = STATUS_STACK_OVERFLOW） |
| 64 层 | 需 ≈ 0.6 MB |

⇒ 深度闸（128 层）**是有效的**，但「合法深度 + 小栈线程 + debug 构建」仍会 abort，
而栈溢出在 Rust 里**不可被 `catch_unwind` 捕获** ⇒ 服务端 / 小栈 worker 是真实风险面。

**目标不是提高深度上限**，而是把每层的原生栈成本降到**常数**（堆上显式栈承载深度）。

## 1. 递归漏斗：只有三个入口

`Parser::parse_value`（`rust/sml-parse/src/parser.rs:1707` 起）是值位置的唯一漏斗：

| # | 入口 | 触发条件 |
|---|---|---|
| 1 | `self.parse_bare_block(key)` | `!colon && bare_block_ahead()` —— 裸块 `Type [name] [args…] { … }` |
| 2 | `self.parse_block(Some(Tok::RBrace))` | `Tok::LBrace`（`key: { … }`） |
| 3 | `self.parse_array()` | `Tok::LBrack`（`key: [ … ]`） |

另有 `parse_array_inner`（`:1837` 起）内部的 `parse_block` / `parse_array` / `parse_bare_block`，
以及 `@for` 的 `parse_for_value` —— 都收束到上面这三个入口。

## 2. ⚠️ 关键结论：必须覆盖**裸块**，只做 `{` / `[` 等于白做

经典深嵌套形状 `a { a { a { … } } }` **没有冒号** ⇒ `bare_block_ahead()` 为真 ⇒ 走**入口 1（裸块）**。
故驱动必须把裸块一并纳入；裸块的 `__type` / `__name` / `__args` 元数据与参数收集规则
也要随 Frame 一起搬，这是工作量比"只做 `{`"大的主因。

## 3. 状态机设计（显式栈，每层 O(1) 原生栈）

```rust
/// 一个"待完成的容器"。
enum Frame {
    Block {
        node: BTreeMap<String, Value>,
        applied_contracts: Vec<(String, Option<Strictness>)>,
        pending_when: Option<bool>,
        closing: Option<Tok>,
        bare: Option<BareCtx>,   // 裸块：类型名 / 块名 / 参数（完成后写元数据）
        cont: Cont,              // 本帧完成后如何把结果交回父帧
    },
    Array {
        arr: Vec<Value>,
        cont: Cont,
    },
}

/// 子容器返回值交回父帧的方式。
enum Cont {
    Field { key: String, key_idx: usize, when: Option<bool> }, // 块字段：含同名提升规则所需上下文
    Index,                                                     // 数组元素
    Root,                                                      // 顶层：直接作为结果
}
```

驱动循环：每次只看 `stack.last_mut()`，**消费一个 token**；遇到子容器就 `push` 新帧并 `continue`；
遇到闭合符号就 `pop` 并按 `cont` 归并回父帧。原生栈深度恒定为 1 层。

## 4. 迁移清单（逐项搬，别一次全搬）

| 要搬的状态 | 出处 |
|---|---|
| `node` / `applied_contracts` / `pending_when` / `closing` | `parse_block_inner:1269-1276` |
| 闭合符号处理（匹配 / 错配 `E-PARSE-002` / 顶层多余 `E-PARSE-003`） | `parse_block_inner:1298-1329` |
| 逗号（字段分隔 vs 裸词逗号 `E-PARSE-007`） | `parse_block_inner:1330-1347` |
| `Tok::BareAt` → `E-PARSE-004`、`Tok::At` → `parse_at_directive` | `parse_block_inner:1350-1357` |
| key 读取、`${}` 键插值拦截（`E-PARSE-013`）、冒号 | `parse_block_inner:1360-1393` |
| 裸片段合并 `&name`（含 `KeyOverwritten` 诊断） | `parse_block_inner:1399-1440` |
| 子容器返回后的归并：**同名提升为数组** + `DuplicateKey` 诊断 | `parse_block_inner:1447-1474` |
| 悬空 `@when`（`E-PARSE-017`）、块结束后契约应用（单 / 多两条路） | `parse_block_inner:1481-1516` |
| 数组：`arr`、下标路径、`RBrace` 报错、裸块元素 | `parse_array_inner:1837+` |
| 裸块：类型名 / 块名 / 参数 + `__type` / `__name` / `__args` 元数据 | `parse_bare_block` |
| 递归入口替换：`parse_value` 的 1 / 2 / 3 | `parse_value:1719-1741` |

## 5. 分步与验收

**判别脚本（已就绪，不入库）**：`rust/tests/_stack_probe.rs`（下划线开头 ⇒ 被 `.gitignore` 忽略）。

```powershell
$env:CARGO_TARGET_DIR="<独立构建目录>"     # ⚠️ 共享目录会 LNK1104
$env:DEPTH=128; $env:STACK_KB=256
cargo test -p swsml --test _stack_probe -- --nocapture
```

- **改前**：`STACK_KB=2048` 通过、`1024` 崩 ⇒ 基线。
- **目标**：`STACK_KB=256`（乃至更小）下 128 层**不崩** ⇒ 每层成本已降到常数。

每迁一步必跑：

```powershell
cargo test -p swsml --test error_codes --test parser_edgecases --test contract_strictness `
           --test depth_boundary --test contract_legacy_modifier
```

全部收尾后再跑：conformance（应为 **AGREE 27 / DIVERGE 9**）、`key_conflict`、`emit`、`c_abi`、
`ser_de_direct`，以及 C / C++ / JS / Lua 四端各自回归。

## 6. 风险与禁令

- **不许改测试迎合实现**：语义漂移会伪装成"实现 bug"，一旦开始改断言就停手重审。
- `@for` / `@include` 暂留递归，并在代码注释里标注"仍递归"，不假装已完成。
- **独立 worktree + 独立 `CARGO_TARGET_DIR`**：共享构建目录在并发下会 `LNK1104`（`sml.dll` 互抢）。
- 本机 `cargo test --workspace` 跑不了（`swsml` 的 lib 与 cdylib 同名），只能按 crate / 按测试目标跑。
- 未合并进 `dev` 前，不许让半成品落地（分支上随便试，合并要等判据全绿）。

## 7. 备选方案（若迭代式代价过大再考虑）

1. **减帧**（既有路线②的延续）：继续把 `parse_block_inner` 的分支抽成 `#[inline(never)]`
   方法 —— 已有先例（`parse_at_directive`，558 → 210 行）。目标是每层 < 2 KB。
   *不彻底*，但足以让 2 MiB 栈有 8× 余量。
2. **`stacker` 栈增长**：业界通用解法（栈不足时切到新栈段）。代价是新增依赖，
   要过 OSV 审计，且与 `sml-value`「零依赖」的既有纪律冲突 —— 若要引入，先裁决放哪一层。
3. 降低 `MAX_VALUE_DEPTH`：**不做**（用户 2026-10-26 已裁决；128 是五端实测钉住的边界）。
