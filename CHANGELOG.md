# 变更日志

本文件从 **0.6.1** 起开始记录；更早的变更见 Gitee 提交历史与各次 release 说明。

格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)。
版本号遵循 Cargo 的语义化版本解释：**0.x 阶段 MINOR 变化视为不兼容**（0.6 → 0.7 会让下游无法自动更新），
PATCH 为兼容新增 —— 因此「新增后端 / 新增 API」走 PATCH（0.6.0 → 0.6.1），只有真正的破坏性改动才动 MINOR。

`swsml`（库）与 `smlconv`（CLI）版本号互相独立，各记各的小节。

---

## [未发布]

## [0.6.1] — 2026-09-18 · swsml

### 新增

- **`emit-html` 后端**：`sml::emit::to_html` / `HtmlOptions`，把 SML 文档转成带内嵌排版 CSS 的语义 HTML5
  （`topic`→`<article class="book">`、`section`→`<section class="chapter">`、`para`→`<p>`、`quote`→`<blockquote>`、
  `img`→`<figure><figcaption>`）；`label` 生成锚点 `id`，供 `{ref: label}` 交叉引用；支持 `standalone` / `fragment` 两种形态。
- 新 feature：`emit-html`，并**已进入 default features**（与其余 `emit-*` 一致，兑现「默认全部开启」的文档承诺）。
- **外置扩展机制**（`sml::ext` / `sml::contract_ext`）：下游可注册自定义 `@指令` 与自定义契约类型，
  **无需改动 crate 源码**。用于把「方言」收编到扩展点上，而不是把它们写进 SML 规范层：
  - `ext::Directive` + `ParseOptions` + `parse_with()`：`@form` / `@policy` / `@flow` 这类
    **元数据块**（文档里写了、解析结果里不该出现）由下游自己声明语义。
    两种参数写法都收：推荐 `@xxx name: X { }`，兼容 `@xxx X { }`（后者产出弃用诊断）。
    内置指令名（contract/is/type/version/feature/when/for）不可被占用。
  - `contract_ext::TypeCheck` + `TypeSpec::Ext`：注册 `image` / `link` / `time` 这类**领域类型**，
    校验器随 `FieldSpec.ext` 传递，校验期无需全局查表。
  - `contract_ext::Modifier` + `FieldSpec.ext_data` / `mods`：注册自定义**字段修饰符**。
    解析期改写规格、校验期回调 `check()`（数组字段同样会跑）。
    典型用途 `items_max`（数组元素个数上限）—— 刻意不复用语义不同的 `max`。
  - 约束：**不注册任何扩展时，行为与既有实现完全一致**（已有单元测钉住）。
  - JS 侧同步（`js/sml.mjs` 及三处副本）：`parse(text, { directives, types, warnings })`，
    语义与 Rust 侧对齐（未注册不放行、位置参数给弃用警告、外置类型判定次序
    `@type` > 外置 > 契约引用）。
- `sml-parse` 0.1.0-alpha.1 → 0.1.0-alpha.2、`sml-contract` 0.1.0-alpha.1 → 0.1.0-alpha.2。
  后者**含不兼容改动**：`TypeSpec` 新增 `Ext` 变体、`FieldSpec` 新增 `ext` 字段。

### 修复

- `sml-parse`：移除 `parse_impl` / `parse_impl_tokens` 中从未被读取的 `version` 形参。
  版本的影响在调用层已由 `features_for(v, …)` 全部折算进特性集，挂着只会留一个 `unused` 警告，
  同时让调用方误以为「传错版本会有效果」。私有函数，公开 API 不变。

### 文档 / 工具链

- 教科书新增第 12 章「smlconv 多目标翻译器」（中英），并补进目录页。
- VSCode 扩展 **0.4.2**：引号串内的环境变量引用 `$env.NAME` 现在同样着色（此前只有 `${...}` 着色）。
- `site/public/sml.mjs` 同步至源实现 `js/sml.mjs`（此前是落后副本）：
  括号 `(` `)` **不再是分隔符**（与 Rust 词法 `sml-lex` 对齐，`备注: (重要)` 不再被切成垃圾键）、
  `enum(...)` 写法在解析层兼容重组、量词支持 `{最小,最大}` 对象式与平铺 `最小/最大`、
  `@is type(契约名)` 等价形式、块级类型标注 `typed-block`（opt-in）。
- `site/public/llms.txt`：VSCode 扩展版本号更正。

---

## [0.1.9] — 2026-09-18 · smlconv

### 新增

- `--to html`：原生产出独立 HTML5（此前长文档只能 `--to md` 再借 pandoc / Hugo 转，会丢 `label` 锚点语义）。
- 依赖 `swsml` 升至 0.6.1 并显式开启 `emit-html`。

---

## 其它（非代码）

- 新增长文档压测素材 `story/`：35 万字 SML 小说样本 `novel.sml`、生成脚本 `gen_novel.py`、
  七个后端（`sml/md/svg/slint/lvgl/latex/xml`）的产出，以及压测发现的功能缺口报告
  `ISSUE_smlconv_缺失功能.md`。用于回归「超长文档解析 + 全后端 emit」的健康度。
