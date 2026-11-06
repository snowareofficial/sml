# 文档缺陷盘点（P1 · **只列不改**）

> 自动扫描，2026-11-04。**本文件不含任何内容改动**。
> 范围：用户可见文档（`site/content` / `editors` / `README*` / `llms.txt`）。内部记录（`HANDOFF*` / `CHANGELOG*` / `TODO` / `docs/*`）单列在末尾。

**判据**：A/B/D 类全文扫（表格与散文里也会写错）；`符号式可选 ?` 只在代码块内扫（英文问号太常见，避免误报）。总扫描 134 个 md，其中用户可见 **78** 个。

## 汇总

| 类别 | 项 | 命中 |
|---|---|---|
| A 与代码不符 | array[T] 写法 | 4 |
| B 版本口径 | v1..v4 旧口径 | 9 |
| C 示例不可跑 | ```sml 块（无 doctest 保护） | 209 块 / 36 文件 |

**A+B+D 合计**：13 处。

## 结论（先看这个）

**用户可见文档没有「硬错误」。** 13 处里：

| 处置 | 数量 | 说明 |
|---|---|---|
| **误报，无需改** | 4 | `array[T]` 那 4 处**正是在解释**「`array[T]` 只是报错文案的回显、不是输入写法」—— 写法正确 |
| **等 v5 定案后统一** | 9 | `v1..v4` 旧口径：现在**仍然正确**（当前实现确实到 v4），等 `edition` 模型落地后一起改，**现在改反而会写错** |
| **D 类（数字漂移）** | **0** | 说明前几轮的「码数统一改为读 `errors.json`」等清理是有效的 |
| **真正的工作量** | —— | 不在「错」，而在 **C 类：209 个 ```sml 代码块无 doctest 保护** + **结构（P2）** |

⚠️ 另外：`CHANGELOG.md` / `HANDOFF*.md` 里对 `parse_block_inner` 等已删符号的提及是**历史记录**，
**不要改**（改了等于篡改历史）；只在它们**指导用户怎么做**时才需要更新。

## 明细

### A 与代码不符 · array[T] 写法（4 处 —— **全部为误报**，见上表）

判据：实测 `array[str]` 报 E-PARSE-006，正确写法 `[T]`

- `site/content/en/book/ch05-contract.md`（2 处）
  - L41: `> ⚠️ **`[T]` is the only accepted array syntax** (`[int]`, `[str]`). `array[T]` is the form you see`
  - L42: `> echoed in **error messages**, not an input syntax: Rust and Lua reject `array[str]` with`
- `site/content/zh/book/ch05-contract.md`（2 处）
  - L40: `> ⚠️ **数组只有 `[T]` 一种合法写法**（`[int]`、`[str]`）。`array[T]` 是**报错文案里的回显形式**，`
  - L41: `> 不是输入写法 —— Rust 与 Lua 见到 `array[str]` 会直接报 `E-PARSE-006`（"契约字段期望键，得 `[`"），`

### B 版本口径 · v1..v4 旧口径（9 处）

判据：v5 / edition 模型已定稿，需统一口径

- `rust/README.md`（3 处）
  - L100: `> **Versioning (`Version`):** `@version v1..v4` declares the syntax version; absent`
  - L105: `> **版本（`Version`）**：`@version v1..v4` 声明语法版本，缺省即 v1。v2 / v3 / v4 要求`
  - L149: `| Versioned 版本化 | `@version v1..v4` |`
- `rust/smltools/README.md`（2 处）
  - L168: `其它参数（`--math` 放行 LaTeX 数学块透传、`--feature v1..v4`、`--title` 等）见 `smltools --help`。`
  - L357: `Other flags (`--math` to pass LaTeX math blocks through, `--feature v1..v4`, `--title`, …) are`
- `site/content/en/_index.md`（1 处）
  - L133: ``v2`/`2`, `v3`/`3`; anything outside `v1..v4` errors outright), with **v4** as`
- `site/content/en/downloads.md`（1 处）
  - L96: `<tr><td><b>libswsml0</b></td><td><code>0.6.3</code></td><td>Runtime: Rust cdylib C-ABI (<code>libswsml.so.0</code>), ful`
- `site/content/zh/_index.md`（1 处）
  - L119: `SML 通过 `@version` 声明文档遵循的语法版本，使解析器在将来引入不兼容语法时仍能正确读取旧文档。当前实现支持 **v1 / v2 / v3 / v4**（`@version` 接受 `v1`/`1`、`v2`/`2`、`v3``
- `site/content/zh/downloads.md`（1 处）
  - L95: `<tr><td><b>libswsml0</b></td><td><code>0.6.3</code></td><td>运行时：Rust cdylib C-ABI（<code>libswsml.so.0</code>），v1..v4 全集能`

### C 示例不可跑 · ```sml 块最多（全部列出）

- `site/content/zh/book/ch05-contract.md`：15 块
- `site/content/en/book/ch05-contract.md`：14 块
- `site/content/zh/_index.md`：13 块
- `site/content/en/_index.md`：11 块
- `site/content/en/book/ch02-blocks.md`：10 块
- `site/content/zh/book/ch02-blocks.md`：10 块
- `site/content/en/book/ch04-include.md`：9 块
- `site/content/en/book/ch09-advanced.md`：9 块
- `site/content/zh/book/ch04-include.md`：9 块
- `site/content/zh/book/ch09-advanced.md`：9 块
- `README.en.md`：8 块
- `README.md`：8 块
- `site/content/en/book/ch03-fragments.md`：8 块
- `site/content/zh/book/ch03-fragments.md`：8 块
- `site/content/en/book/ch01-basics.md`：7 块
- `site/content/zh/book/ch01-basics.md`：7 块
- `site/content/en/book/ch10-features.md`：5 块
- `site/content/zh/book/ch10-features.md`：5 块
- `site/content/en/book/ch06-env-escape.md`：4 块
- `site/content/en/book/ch13-extension.md`：4 块
- `site/content/zh/book/ch06-env-escape.md`：4 块
- `site/content/zh/book/ch13-extension.md`：4 块
- `site/content/en/book/ch08-project.md`：3 块
- `site/content/en/book/ch11-ai-recommend.md`：3 块
- `site/content/zh/book/ch08-project.md`：3 块
- `site/content/zh/book/ch11-ai-recommend.md`：3 块
- `editors/vscode/README.md`：2 块
- `rust/README.md`：2 块
- `site/content/en/book/appendix.md`：2 块
- `site/content/en/book/ch12-smltools.md`：2 块
- `site/content/zh/book/appendix.md`：2 块
- `site/content/zh/book/ch12-smltools.md`：2 块
- `editors/vscode/README.en.md`：1 块
- `errors/README.md`：1 块
- `site/content/en/book/intro.md`：1 块
- `site/content/zh/book/intro.md`：1 块

### 附：内部记录里对**已删除符号**的引用（不是用户可见缺陷，但需同步）

- `CHANGELOG.md`：L1612
- `HANDOFF-2026-10-26.md`：L26、L80、L82、L84、L187
- `HANDOFF-2026-10-30-2.md`：L66
- `HANDOFF-2026-10-30-3.md`：L37、L55、L56、L64、L93
- `HANDOFF-2026-11-02.md`：L235、L292、L302、L519、L521
- `HANDOFF.md`：L970、L971
- `TASK-agent-w24.md`：L215
- `TODO.md`：L523、L557、L570、L600
- `docs/iter-parse-plan.md`：L9、L39、L82、L83、L84、L85、L86、L87、L88、L89、L90、L127
- `rust/AUDIT_REPORT.md`：L113、L380、L398、L411、L420

## 下一步

1. **P2**：建 `docs/spec|plan|archive`，13 份 docs 归位，补 `docs/README.md` 索引（哪份权威）；
2. **P3**：按 A/B/D 逐条修，**与代码切片并行**（每片交付自己的文档影响）；
3. **C 类**是风险面不是错误：把可执行示例抽成 zh/en 共用片段并挂 doctest；
4. **D 类**统一改成引用生成物（码数读 `errors.json` 的 `count`）并加 `--check` 门禁。