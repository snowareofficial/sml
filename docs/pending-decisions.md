# 待用户裁决（2026-11-06）

> 用户出门前指示：把待裁决项先写在文件里。**每条都给了「不裁决时的默认路径」**——
> 实现者按默认走，不阻塞；用户回来改一句话即可覆盖。
> 权威仍是 `TODO.md` / `IDEAS.md`；本文件只记录「卡在用户」的四条。

---

## D1 · 发布节奏 + v5 定稿（T18 / T50）

**问题**：谁先发、什么条件发；`@version 50 "camelia" dev` 什么时候不再报 `E-FEATURE-004`。

| 选项 | 含义 |
|---|---|
| 甲 | 先发 `swsml 0.7.0`（含 `Tok<'a>` 破 API），`swml 0.1.0` / `amberice` 随后 |
| 乙 | 三个一起发，tag 同一天 |
| 丙 | 先 `amberice` + `swml`（新东西），`swsml` 最后 |

**默认路径（不裁决）**：按 `docs/perf-plan.md §3` 与 `T31` 的依赖顺序准备 `release.yml`，
但不执行发布；v5 文档表 A 回填照做（不改 `E-FEATURE-004` 的启用位）。

**为什么需要你**：发布是不可逆动作，且 `swsml` 已发 18 版，0.7.0 的破坏性变更需要对外口径。

---

## D2 · 双授权口径（T09）

**问题**：`swsml` / `swsml-derive` / `swml` 是否从「仅 MulanPSL-2.0」改为「MIT 或 Apache-2.0 双授权 + MulanPSL-2.0」。

| 选项 | 含义 |
|---|---|
| 甲 | 保持 MulanPSL-2.0 单授权（改动最小，但国际采用有摩擦） |
| 乙 | `MIT OR Apache-2.0` 双授权 + 保留 MulanPSL-2.0（Rust 生态惯例） |
| 丙 | `Apache-2.0 OR MulanPSL-2.0`（专利条款更强） |

**默认路径（不裁决）**：不动任何 `license` 字段与 LICENSE 文件；只在 `docs/unfinished.md` 标注「待授权口径」。

**为什么需要你**：授权是法律口径，实现者不能替你选。

---

## D3 · `@feature base` 的「书名」承载位（T13）

**问题**：`@feature base <版本>` 只认版本号，文档无法声明自己的书名/标题；CLI 现在按顶层 `title:` 推断（事实约定）。

| 选项 | 含义 |
|---|---|
| 甲 | 新增指令 `@title <字符串>`（与 `@version` 同级，五端都要认） |
| 乙 | 沿用 `title:` 顶层键，写进规范（零改动，但与普通字段混在一起） |
| 丙 | `@feature base` 扩成 `@feature base <版本> <书名>`（改既有指令语义） |

**默认路径（不裁决）**：维持现状（`title:` 顶层键），只在文档里把这条事实约定写清，不改解析器。

**为什么需要你**：这是语言层的语法扩展，一旦发布就要五端同步。

---

## D4 · `deploy.yml` 的 VPS 部署内容（T36）

**问题**：`deploy.yml` 要「构建 → Pages → VPS → 健康检查 → 失败回滚」，但**VPS 上跑什么仓库/进程无记录**；
Cloudflare 侧已确认是 Pages（`sml-site`），不是 Worker。

| 需要你补的事实 | 影响 |
|---|---|
| VPS 上部署的是什么（站点静态文件？反代？别的东西？） | 决定 rsync 目标与健康检查 URL |
| 是否已有 systemd 服务名 / 端口 | 决定回滚时重启什么 |
| 用哪把密钥（现有 SSH key 还是新建） | 决定 secrets 清单 |

**默认路径（不裁决）**：只写 `deploy.yml` 的 Pages 半边（构建 + 发布 + 健康检查），
VPS 半边留 `TODO(用户补事实)` 注释，不猜目标。

**为什么需要你**：猜错会把文件推到错误的机器上。

---

## 附：本文件不收录的

- **T43「动不动 C++」**：已在 `TODO.md:1181` 标 ✋；默认路径 = 不动 C++，用例保持 DIVERGE 并在台账注明。
- **T06 / T11 / T48（Lua 三项）**：已定「先不动 Lua」，不重复登记。
- **T22–T26 / T30 / T14 / T49**：都是「只有你能点的按钮」，属操作而非裁决，列在 `docs/todo-ideas-tables.md` 的「卡在谁 = 用户」。

---

## D5 · 加密文件里的 `DIR` / `TYPED`（**已裁决：丙**，2026-11-06）

**裁决**：丙 —— 加密状态必须自洽，宁可拒读也不猜。

**已实现**（`rust/amberice/src/container.rs`）：
- 加密文件：除 `MANIFEST` 外每段必须 `enc != 0`，否则 `PlaintextSegmentInEncryptedFile`；
- 明文文件：出现 `enc != 0` 的段，否则 `EncryptedSegmentInPlainFile`；
- `DIR` 已并入 seal/加密流水线；加密文件里它是密文，`Store::get` 用不上索引（安全优先于速度）。

**白名单：甲（已实现）**——段头 `flags` 位 0 = 「本段声明明文」（`SEG_FLAG_PLAINTEXT`）；
写侧 `DumpOptions.crypto_plain: Vec<u8>`，读侧要求 `enc == 0` 与声明位一致，不一致即拒读。
不动 manifest 布局。

⚠️ **这条现在收益为零，必须记清楚**：`Store` 没有接收密钥的入口，加密文件上连 `VALUE`
都读不了（`EncryptedNotZeroCopy`），所以「声明明文 `DIR`」目前不产生任何加速。
它要等**带密钥的 Store**（`open_with_keys` 一类）才有消费者；在那之前写这条只是格式许可。
