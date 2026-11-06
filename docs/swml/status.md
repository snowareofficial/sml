# `swml` 状态

> **产品线** swml ｜ **状态** current ｜ **最后核对** 2026-11-06
>
> 一页看清门面 crate 的发布状态与仓库位置。公开面规范见 `spec/api.md`；
> 未定项见 `plan/api-draft.md`；元数据在 `docs/meta.sml`。

## 1. 一句话

`swml` **已建、未发布**：`publish = false`，与 `amberice` / v5 一起发。

## 2. 事实（可复核）

| 项 | 值 | 复核命令 |
|---|---|---|
| crate 名 / 版本 | `swml` `0.1.0-dev.0` | `cargo metadata --no-deps --format-version 1` |
| 发布状态 | **未发布**，`publish = false` | `grep -n 'publish' rust/swml/Cargo.toml` |
| 依赖 | `sml = { package = "swsml", path = "..", version = "0.6.9-dev.0" }`（默认档） | `rust/swml/Cargo.toml` |
| 默认 feature | 6 个 `emit-*`（markdown / html / latex / xml / svg / slint） | 同上 |
| `preserve_order` | 转发到 `swsml`，**不进 default**（Q10） | 同上 |
| 代码 | `rust/swml/src/{lib,easy}.rs`；测试 `rust/swml/tests/{quickstart,contracts,report}.rs` | `git ls-files rust/swml` |

不手写版本号：以上每次复核都以命令输出为准。

## 3. 发布线

- 不单独发版：等 `amberice`（`.ambr`）与 v5 定稿后**一次性**给出成批能力
  （用户 2026-11-03 / 2026-11-06 两次确认）。
- 发布前必须绿：`cargo test -p swml`、`cargo check -p swml --no-default-features`。

## 4. ⚠️ 待裁决：门面到底在不在本仓 workspace

| 说法 | 出处 | 事实 |
|---|---|---|
| 「门面**不进**本仓 workspace（独立仓库、依赖 crates.io）」 | `TODO.md` §11.1 | 与代码相反 |
| `swml` 是 workspace member，随仓编译与测试 | `rust/Cargo.toml` 的 `members` | ← 现状 |

两种口径不能同时成立，**未裁决**。影响：`cargo test --workspace` 是否覆盖门面、
发布拓扑里 `swml` 的位置、以及「独立仓库」是否需要拆出去。裁决前按现状（在 workspace 内）执行。
