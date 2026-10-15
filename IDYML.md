# idyml — swsml 的国际化稳定别名（计划书 · 待建）

> 状态：**计划中，尚未开工**。本文件是任务书。
> idyml 真正实现时是 **GitHub 上独立的原生仓库**（标准 Rust crate 布局，**不进 sml 仓库的 workspace**）；
> 本文件先留在 sml 仓库文档库，建新仓库时连同 README / CI 一并迁过去。

## 一句话
`idyml` 是 `swsml` 的**瘦别名（thin alias）**：`cargo add idyml` 之后，`use idyml::{...}` 与
`use sml::{...}` **完全一样**；但它是**全英文门面**、面向**国际用户**、作为比 swsml **更稳定的发布线
（stable branch）**。目的是用更国际化的门面**拿到更多使用者**。

## 形态：瘦别名，不是 fork 代码
- `idyml` 依赖 crates.io 上的 `swsml`，`src/lib.rs` 里 `pub use swsml::*;`（或等价再导出）——API 与 `sml` 一字不差。
- **不复制** swsml 的任何实现代码，**不翻译** swsml 的内部注释。
- `idyml` 自身**全英文**：README、`Cargo.toml` 的 `description`/`keywords`、crate 级 `//!` 文档。
- 错误消息：swsml 已有 i18n 英文层（`errors/msg.en.json`），国际用户可拿到英文错误文本——该层归 swsml，不靠 idyml。

## 与 swsml 的关系
| | swsml | idyml |
|---|---|---|
| 门面语言 | 中文为主 | **全英文** |
| 定位 | 上游 / 开发线（中文主） | **稳定线 / 国际版** |
| 版本 | 0.6.x，自主演进 | 跟踪 swsml 的**稳定** minor（如 idyml 0.6.x ↔ swsml 0.6.x） |
| 代码 | 唯一实现 | 无（再导出 swsml） |
| 发布节奏 | 较激进 | **保守**——只在 swsml 出稳定版时跟发，不抢跑 |

"更稳定（branch）" = idyml 只跟随 swsml 的稳定版本发布，给国际用户一个保守、可预期的通道。

## 独立 GitHub 仓库
- 单独原生仓库（建议 `github.com/snowareofficial/idyml`），标准 crate 布局，**不进 sml 的 workspace**。
- 依赖写 crates.io 稳定版 `swsml`（**不是 path**），保证用户 `cargo add idyml` 直接可用。
- 迁过去时带上：`README.md`（英）、`LICENSE`（MulanPSL-2.0）、CI（build/test/clippy）、publish 流程。

## 骨架参考（建仓库时）
```toml
# Cargo.toml
[package]
name = "idyml"
version = "0.6.0"                 # 对齐 swsml 稳定 minor；策略见 checklist
description = "idyml — declarative data/config format (SNOWARE Markup Language); the stable, English-first international alias of swsml. A readable JSON/YAML alternative."
license = "MulanPSL-2.0"
repository = "https://github.com/snowareofficial/idyml"
readme = "README.md"
authors = ["Sal <sal@mail.swebase.cn>"]
keywords = ["config", "data", "markup", "serialization", "declarative", "json", "yaml"]
categories = ["config", "parser-implementations", "no-std"]

[dependencies]
swsml = "0.6"                     # crates.io 稳定版（勿用 path）

[lib]
name = "idyml"
path = "src/lib.rs"
```
```rust
//! src/lib.rs
//! idyml — SNOWARE Markup Language (English-first international alias of `swsml`).
//!
//! A declarative data / config format, a readable alternative to JSON and YAML.
//! `use idyml::{...}` is exactly `use sml::{...}` — idyml re-exports the `swsml` engine.
//!
//! ```toml
//! idyml = "0.6"
//! ```
//!
//! ```rust,no_run
//! use idyml::{parse, to_sml};
//! let v = parse("name: John\nage: 27")?;
//! assert_eq!(v.get("name").and_then(|x| x.as_str()), Some("John"));
//! # Ok::<(), idyml::ParseError>(())
//! ```
pub use swsml::*;
```

## 开工前 checklist
- [ ] `cargo search idyml` 确认 crates.io 名字未被占用。
- [ ] 定版本对齐策略：idyml 版本 = swsml 版本，还是独立 0.x 起、依赖写 `swsml = "0.6"`。
- [ ] 再导出写法：`pub use swsml::*`（最简）vs 对关键项（`parse`/`parse_file`/`Value`/`to_sml`/`serde`…）加英文 `#[doc]` 再导出（docs.rs 更全英文）。
- [ ] swsml 的 `[lib] name = "sml"`：确认 idyml 内引用路径（`swsml::*` 是否可用，还是要 `sml = { package = "swsml" }` 重命名依赖）。
- [ ] 英文 `README.md`：定位、install、quick example、features、"stable international alias of swsml"、链接 swsml 深文档 / 官网。
- [ ] 新仓库 CI（build / test / clippy）+ `cargo publish` 流程 + LICENSE（MulanPSL-2.0）。
- [ ] 是否给 idyml 单独 crates.io badge / 官网下载入口。

## 明确不做
- **不 fork swsml 代码、不翻译其内部注释**——那是"全 fork"，成本与长期维护都极高，违背"瘦别名"初衷。
- 不改 swsml 本身；swsml 继续中文主、自主演进。
- 不把 idyml 塞进 sml 的 workspace（依赖必须走 crates.io，用户才 `cargo add idyml` 可用）。
