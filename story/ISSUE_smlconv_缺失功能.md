# Issue：smlconv 功能缺失报告（部件 / 长文档产出 / 跨格式一致性）

- 组件：smlconv（`rust/smlconv`）+ emit 后端（`rust/src/emit/*`）
- 现象来源：以一份约 35 万字（`novel.sml`，351,217 非 ASCII 字符）的 SML 长篇科幻小说
  压测 smlconv，7 个后端（sml/md/svg/slint/lvgl/latex/xml）均**能解析并成功 emit**，
  说明长文档的解析与基本转译能力是健康的；但在"写小说 / 写结构化长文"这一真实场景下，
  暴露出以下**功能缺失与不一致**。按用户要求，重点报告「部件（组件）」相关缺口。

---

## 一、文档写作约定与解析器不一致（真实 bug）

**`@feature base <name>` 写法被解析器直接拒绝，但 CLI 的标题推断却依赖它。**

- `rust/smlconv/src/main.rs` 的 `infer_title()` 注释明确写：「尝试从 SML 文本中提取
  `@feature base <name>` 作为标题」。
- 但 `sml-parse` 的 `@feature` 指令只接受版本号：`@feature base 需要 v1/v2/v3/v4，收到 ...`
  （实测报错）。即 `@feature base "书名"` 在解析阶段就 `parse error`，根本到不了 emit。
- 后果：`--hugo` / `--zola` 的标题自动推断分支是**死代码**；用户按文档/记忆写
  `@feature base "..."` 会整篇解析失败。

**建议**：要么解析器支持 `@feature base <name>`（作为文档命名/标题声明），要么 CLI
删除该死分支并改用 `@title`/`title:` 字段推断。二选一，先统一约定再统一实现。

---

## 二、长文档（小说）缺少直接产出格式：EPUB / 独立 HTML

- `--to` 取值：`md|markdown|xml|svg|latex|slint|lvgl|custom|sml`，**没有 EPUB，也没有独立 HTML**。
- 写一部长篇小说的自然目标是电子书（EPUB）或自带样式的 HTML 站点，但 smlconv 只能先
  `--to md` 再依赖外部工具（pandoc）转 EPUB，链路断裂、且丢失 SML 的 `label` 锚点/
  多视图等语义。
- 现有 `--hugo` / `--zola` 是"落盘成 front-matter 的 .md"，仍需外部 `hugo`/`zola build`，
  且只生成单文件，不是成品电子书。

**建议**：新增 `--to epub`（可直接打包章节为 EPUB，章节由 `section` 自动分页）+ `--to html`
（单文件 HTML，带由 `label` 生成的锚点与目录）。这是"用 SML 写小说"场景的硬缺口。

---

## 三、缺章节分页 / 目录（TOC）/ 分卷输出

- 35 万字文档 `--to md` 只产出**单个大文件**；没有按 `section` 拆多文件、没有 TOC、没有分卷。
- `emit/markdown.rs` 的 `topic`/`section` 已能映射 `h1..h6` 并带 `id` 锚点（基础设施已具备），
  但 smlconv 层没有把"章节 → 文件"或"章节 → TOC"的能力暴露出来。

**建议**：`--split-by section`（每章一个 .md/文件）、`--toc`（生成目录）、`--hugo` 已支持，
补齐 Zola/Hugo 之外的原生分章与 TOC。

---

## 四、语义文档「部件」覆盖不全（markdown 后端）

emit/markdown.rs 已支持：`h1-6 / p / ul / ol / li / table / code / blockquote / hr / a / img /
em / strong / del / topic / section / para / quote / math / theorem / proof` 以及内联角色
`{ref:}/{term:}/{math:}/{em:}`。

但对照"写小说 / 写教科书"的真实需要，仍缺以下**常用部件（组件）**：

1. **`figure` / `figcaption`（图 + 图注 + 图号）**：`img` 已支持，但无带编号图注的语义包裹，
   长文档插图文无法自动编号与交叉引用。
2. **`admonition` / `callout`（提示框 / 警告框 / 注意框）**：文档站（Hugo/Docsify）标配，
   smlconv 无对应块类型，只能退化成 generic object 渲染成难看的 `### key` 列表。
3. **`footnote`（脚注）**：`MarkdownOptions` 注释里把脚注列为 v2 规划特性，但**代码未实现**
   （实测无 `footnote` 分支）。
4. **`definition list`（定义列表）**：无语义块，只能靠 generic object 近似。
5. **`sub` / `sup`（上下标）**：无对应内联角色（现有角色仅 ref/term/math/em/strong）。
6. **`toc`（目录）生成**：无。

另外，**语义块跨后端不一致**：
- `theorem` / `proof` 只在 markdown 后端以 raw-HTML（`<article class="theorem">`）渲染，
  且依赖 Hugo `unsafe=true`；**latex 后端无定理环境映射**，直接走 generic object 渲染成列表。
- `math` 块在 markdown 里硬编码为 `<div class="math">\[..\]`，**不响应 `--math` 开关**
  （该开关仅对 latex 生效）。
- 内联角色 `apply_roles` **仅在 markdown 后端实现**；latex / svg / slint / html_passthrough
  后端不处理 `{ref:}` 等角色，导致同一份 SML 跨格式语义漂移。

**建议**：把"部件目录"作为 emit 层的明确范围补齐（figure/admonition/footnote/deflist/
sub/sup/toc），并让 `theorem/proof/math` 与内联角色在 latex/svg 等后端有一致映射。

---

## 五、UI 后端（slint / lvgl / svg）缺「部件库 / 标准组件目录」一层抽象

当前 slint/lvgl/svg 后端是**纯透传**：任何 `__type` 直接当元素/部件名（如 `lv_button` →
`button`），没有内置"复合部件"映射。

- 没有常见 UI 复合部件的 SML→宿主映射：`card`（卡片）、`dialog`（对话框）、`tab`/`tabbar`
  （标签页）、带样式的 `list`、图表 `chart`、导航栏 `navbar` 等。
- 用户每做一个界面都要从原子部件手写全部结构，SML 文档无法直接表达"常见 UI 模式"。

这与"声明层"定位本身不冲突，但缺少"部件目录"这一层中抽象，使 SML 在 UI 场景下停留在
"另一种 XML"，没发挥出"声明即组件"的优势。

**建议**：在 emit 层引入可选的"部件目录"：一组约定好的 SML 块类型（如 `card`/`dialog`/
`tab`）→ 各后端展开为对应宿主复合结构。可作为 `emit-*` 的扩展特性，不影响现有纯透传行为。

> 相关已知约束（用户已明确不修）：svg/slint/xml/lvgl 后端把中文键名 sanitize 成下划线
> （`emit/mod.rs` 的 `sanitize_xml_name` / `sanitize_slint_ident`），故中文命名的部件/键名
> 在这些后端无效。本文不重复提议，仅标注其限制了"中文部件命名"的可能性。

---

## 六、无增量 emit

- `smlconv` 每次都是**整篇重 emit**；改一处也要重转全部 35 万字。
- 对长文档，局部改动后整篇重转既慢又浪费；smlconv 不提供"只重转变更块"的能力。

**建议**：emit 层按块做内容哈希缓存，只重算源文本变更过的块，支持"增量 emit / 增量写出"。

---

## 七、长文档排错：emit 失败缺源定位

- 解析错误来自 sml-parse（有位置信息尚可）；但 emit 阶段报错（如深度超限
  `递归深度超过上限 128`）只给通用信息，没有"第几行源文本 / 哪个块"的定位。
- 35 万字文档一旦 emit 失败，定位极困难。

**建议**：emit 错误携带源位置（文件路径 + 行号 + 块路径），至少把触发深度超限的块路径打出。

---

## 优先级建议（按"写小说 / 写长文"场景）

| 优先级 | 缺口 | 理由 |
|--------|------|------|
| P0 | 一、`@feature base` 解析/CLI 不一致 | 真实 bug，整篇解析失败 |
| P0 | 二、EPUB / HTML 直接产出 | 小说场景硬缺口 |
| P1 | 三、章节分页 / TOC | 长文档工作流断裂 |
| P1 | 四、figure/admonition/footnote 等部件 + 跨后端一致 | 教科书/小说常用 |
| P2 | 五、UI 部件库抽象层 | 提升声明层价值 |
| P2 | 六、增量 emit | 长文档性能 |
| P3 | 七、emit 源定位 | 排错体验 |

---

## 复现

```text
# 生成长文档（见同目录 gen_novel.py -> novel.sml，约 35 万字）
python gen_novel.py

# 全后端 emit 均成功（验证解析/基本转译健康）
smlconv -i novel.sml --to md   -o novel.out.md
smlconv -i novel.sml --to svg  -o novel.out.svg
smlconv -i novel.sml --to slint -o novel.out.slint
smlconv -i novel.sml --to lvgl -o novel.out.lvgl
smlconv -i novel.sml --to latex -o novel.out.latex
smlconv -i novel.sml --to xml  -o novel.out.xml
smlconv -i novel.sml --to sml  -o novel.out.sml

# 暴露问题一的写法（解析失败）：
# 在 novel.sml 顶部加  @feature base "星海回响"  -> smlconv: parse error
```
