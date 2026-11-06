# Agent 阅读规则（改文档前先读这一页）

> 本文件是给**人和 AI** 的入口规则，不是技术文档。技术内容在 `docs/`，入口是生成的
> [`docs/index.md`](docs/index.md)。

## 1. 四条硬规则

1. **改文档前先读 `docs/index.md`** —— 它列出每份文档的 `owner` / `status` / 替代者。
   不看索引就动手，等于闭着眼睛改一个自己不知道是否作废的文件。
2. **默认只读 `status: current`** —— 那是权威、可引用、可据以施工的部分。
3. **`draft` / `stale` 必须被显式点名才读**：
   - `draft` = 未定稿，只能看不能用（当结论引用就是错的）；
   - `stale` = 已被取代，正文不再维护，去读它的 `superseded_by`；
   - `archive` = 一次性记录 / 已完成方案，**只作历史，勿照做**。
4. **正文不写历史** —— 不在文档里留「曾经如此」「此前是 X 后来改成 Y」。
   历史归 `git log` 与 [`docs/changes.sml`](docs/changes.sml)。

## 2. 新增 / 改动文档的流程

1. **先登记**：在 [`docs/meta.sml`](docs/meta.sml) 加一条
   `doc { title date updated owner status scope supersedes superseded_by }`；
   没登记的文件会被 `docs --check` 判为**未登记**，CI 直接红。
2. **改完重跑**：`cargo run -p smldocs -- --index`（刷新索引）与
   `cargo run -p smldocs -- --check`（门禁：未登记 / 断链 / `updated` 超期 /
   `stale` 无替代者，任一命中即非零退出）。
3. **搬迁/拆分/删除**：在 `docs/changes.sml` 记一条 `change { doc at commit summary }`。

## 3. 目录约定（先产品线，再类型）

```
docs/swsml/{spec,ref,plan,archive}/   语言与核心 crate（swsml）
docs/amberice/{spec,ref,plan,archive}/ .ambr 容器 / 格式 / smldb（draft）
docs/swml/{spec,plan}/ + status.md     门面 crate（swml）
docs/repo/{ref,plan,archive}/          仓库自身：清单 / 架构 / 交接与任务书
docs/index.md  docs/report.md          生成物，勿手改
docs/meta.sml  docs/changes.sml        元数据与变更记录（手写）
```

判据：**先归属产品线，再分 spec/ref/plan/archive**；不允许从仓库根平铺一堆 `*.md`。

**构建产物不落源码目录**：`c/build/`、`cpp/build/`、`js/build/`、`lua/build/`（均已 gitignore）；
一次性探针以 `_` 开头并归 `_scratch/`。源码目录里看到 `*.exe` / `*.dll` / `*_out.txt`，
说明有人在源码旁边直接编译 —— 那是要修的，不是要提交的。

## 4. 工具面边界（别再犯）

- `smltools` 是**产品 CLI**（给装了 SML 的人：转换 / 迁移 / lint）。仓库自身的治理
  （文档索引、体检、门禁、发布检查）**不进 `smltools`**，放 `publish = false` 的内部 crate
  （当前是 `rust/smldocs`）。
- 判据一句话：**一条命令如果换一个仓库就毫无意义，它就不属于产品 CLI。**

## 5. i18n

这个仓库的用户可见文字**默认双语**：crate 文档与 CLI 注释用 `中文 ｜ English`，
README 有 `.en.md` 对，错误码文案走 `errors/codes.sml`（生成五端产物）。
新增用户可见文案时按同一惯例办：**中文为主 + 英文并列**，不要只写一种；
`docs/` 正文以中文为主（`meta.sml` 的 `scope` 等结构化字段不写双语）。
