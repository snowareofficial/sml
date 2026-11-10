# docs 体检报告

> **本文件由 `cargo run -p smldocs -- --report` 生成，勿手改。**
> 数据源 `docs/meta.sml` 摘要 `cfffde3d8e026714`，检查 52 份登记文档。任一项命中时 `docs --check` 非零退出。
> 本文件不记生成日期 / HEAD，避免跨天或提交后假漂移。

## 合计

**0 处问题**。

### 未登记（在 docs/ 下但不是 meta.sml 的 doc）（0）

- 无

### 登记的文档不存在（0）

- 无

### 元数据缺字段（0）

- 无

### 字段值非法（owner / status 不在枚举内）（0）

- 无

### status = stale 但没有 superseded_by（0）

- 无

### superseded_by 指向不存在的文件（0）

- 无

### updated 超过 90 天未核对（0）

- 无

### 断链（Markdown 相对链接指向不存在的路径）（0）

- 无

## 改哪里

- 未登记 → 在 `docs/meta.sml` 加一条 `doc { … }`；
- 超期 → 真核对一遍再改 `updated`（不是把日期往后挪）；
- 断链 → 改链接或改目标；两个都不对就删掉这条引用；
- `stale` 无替代者 → 要么写 `superseded_by`，要么把正文并入替代者后删掉本文。
