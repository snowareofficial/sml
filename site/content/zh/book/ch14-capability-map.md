---
title: "第 14 章：能力地图"
translationKey: "book-ch14"
---

# 第 14 章：能力地图

**为什么有这一章**：`typed-block` / `@when` / `@for` 这些进阶能力只在第 10 章出现，第 1–9 章一处入口都没有 —— 读者很容易以为"SML 就会前九章那些"。本章把**所有能力**摊成一张表：**是否 opt-in**、**五端支持**、**在哪一章详解**。写跨端文档前，先来这里对一眼。

> 数据来源：第 10 章「实现面总表」（2026-09-19 真机实测）+ 契约叠加（2026-10-12，任务 C/D/F）。
> 判据是**跑出来的行为**，不是读源码猜的；`?` = 未实测（不代表没有）。

| 能力 | 默认 | Rust | JS | C | C++ | Lua | 详解章节 |
|---|---|---|---|---|---|---|---|
| 键值 / 块 / 数组 / 标量 | 开 | ✅ | ✅ | ✅ | ✅ | ✅ | [第 1–2 章](/book/ch01-basics) |
| 注释（`#` `--` `//` `/* */` `_* *_`） | 开 | ✅ | ✅ | ✅ | ✅ | ✅ | [第 1 章](/book/ch01-basics) |
| 裸词即串（`bareword-string`） | 开 | ✅ | ✅ | ✅ | ✅ | ✅ | [第 1 章](/book/ch01-basics) |
| `include`（单文件） | 开 | ✅ | ✅ ※ | ✅ | ✅ | ✅ | [第 4 章](/book/ch04-include) |
| `env`（`$env.X`） | 开 | ✅ | ✅ | ✅ | ✅ | ✅ | [第 6 章](/book/ch06-env-escape) |
| 契约（`@contract` / `@is`，单契约） | 开 | ✅ | ✅ | ✅ | ✅ | ✅ | [第 5 章](/book/ch05-contract) |
| 片段（`@base {}` + `&base`） | 开 | ✅ 展开 | ✅ | ⚠️ 不展开 | ❌ | ❌ | [第 3 章](/book/ch03-fragments) |
| `namespace`（`include … as ns`） | 开 | ✅ | ✅ | ✅ | ✅ | ❌ | [第 4 章](/book/ch04-include) |
| `implicit-ns` | 开 | ✅ | ✅ | ? | ? | ? | [第 4 章](/book/ch04-include) |
| `top-level-array`（文档根是数组） | 开 | ✅ | ❌ | ✅ | ❌ | ❌ | [第 10 章](/book/ch10-features) |
| **契约叠加（mixin，一个块多约）** | 开※ | ✅ | ❌ `017` | ❌ `017` | ❌ `017` | ❌ `017` | [§5.4.1](/book/ch05-contract) |
| `typed-block`（`契约名 块名 { }`） | **关** | ✅ | ✅ | ❌ 静默降级 | ❌ | ❌ | [§5.2.2](/book/ch05-contract) |
| `@type` 模式（Loom-in-SML） | 开 | ✅ | ✅ | ❌ 明确拒绝 | ❌ 明确拒绝 | ❌ 明确拒绝 | [§5.2.2](/book/ch05-contract) |
| `multi-include`（`include "a", "b"`） | **关** | ✅ | ✅ | ✅ | ✅ | ❌ | [第 10 章](/book/ch10-features) |
| `glob-include`（`include "inc/*.sml"`） | **关** | ✅ | ✅ | ❌ | ? | ❌ | [第 10 章](/book/ch10-features) |
| `regex-include`（`include "re:…"`） | **关** | ✅ | ✅（受限子集） | ? | ❌ | ❌ | [第 10 章](/book/ch10-features) |
| `ext-rewrite` | **关** | ✅ | ? | ? | ? | ? | [第 10 章](/book/ch10-features) |
| `@when` | **关** | ✅ | ❌ | ❌ | ❌ | ❌ | [第 10 章](/book/ch10-features) |
| `@for` | **关** | ✅ | ❌ | ❌ | ❌ | ⚠️ 待复核 | [第 10 章](/book/ch10-features) |
| `@feature` 门控机制本身 | — | ✅ | ✅ | ❌ | ❌ | ❌ | [第 10 章](/book/ch10-features) |

※ **JS 的 `include` 由宿主提供文件表**（`parse(text, { files })`），JS 侧自己**不读盘**（为编辑器 / 服务端内嵌设计）。
※ **契约叠加**无需额外 `@feature`（随 `contract` 默认开），但目前**只有 Rust 落地**，其余四端**明确报 `E-CONTRACT-017`**（绝不静默）。

**三条速记**：

1. **五端都稳的**（优先用于跨端文档）：键值 / 块 / 数组、注释、裸词串、`include`、`$env`、单契约 `@contract`/`@is`、`namespace`。
2. ** Rust（±JS）的进阶**：`@when` / `@for` 仅 Rust；`typed-block` / `@type` / `glob`·`regex-include` 为 Rust + JS。
3. **会静默或降级的，跨端要当心**：C 的 `fragment` 不展开、C / C++ / Lua 的 `typed-block` 退化成普通裸块、Lua 的 `for` 待复核 —— 这些端要么明确报码、要么留下与预期不同的树。

→ 回[教科书目录](/book/)
