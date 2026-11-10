# `swml` 状态

> **产品线** swml ｜ **状态** current ｜ **最后核对** 2026-11-10
>
> 一页看清门面 crate 的发布状态与仓库位置。公开面规范见 `spec/api.md`；
> 未定项见 `plan/api-draft.md`；元数据在 `docs/meta.sml`。

## 1. 一句话

`swml` **已建、未发布**：`publish = false`；随 dev2 与 `amberice` / `bamcore` / `bambin`
一起发（`docs/repo/ref/pending-decisions.md` D1 / D6）。

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

- 不单独发版：随 **dev2** 与 `amberice` / `bamcore` / `bambin` 成批发布（用户 2026-11-10 裁定，
  见 `docs/repo/ref/pending-decisions.md` D1 / D6）；v5 表 A 定稿仍是前置。
- 对外主仓是 GitHub 的 `swsml` 仓，本仓保持权威开发源，用 workflow 单向推送；
  crate 的 `repository` 用 GitHub URL（精确值待新仓名确定）。
- 发布前必须绿：`cargo test -p swml`、`cargo check -p swml --no-default-features`。

## 4. 门面源码与对外仓（2026-11-10 已裁定）

| 项 | 裁定 |
|---|---|
| 门面源码 | **留在本仓 workspace**，继续随仓编译与测试（`rust/Cargo.toml` 的 `members`） |
| 对外仓 | GitHub 上的 `swsml` 仓作主仓，workflow 单向推送，对外收 star / issue / PR |
| crate 链接 | `repository` 用 GitHub URL；精确值待新仓名确定后回填各 `Cargo.toml` |

`TODO.md` §11.1「门面不进本仓 workspace（独立仓库、依赖 crates.io）」与代码相反，
按本裁定不采纳；若日后要真把 crate 源码拆到独立仓，需另行裁决（依赖 `sml-value` 先发 crates.io）。
