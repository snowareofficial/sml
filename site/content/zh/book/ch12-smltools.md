---
title: "第 12 章：smltools 多目标翻译器"
translationKey: "book-ch12"
# 本章原名 ch12-smlconv（crate 改名 smlconv → smltools），保留旧地址跳转
aliases:
  - "/book/ch12-smlconv/"
---

# 第 12 章：smltools 多目标翻译器

`smltools` 是 SML 的命令行转译器：把**同一份 SML 文档**一键翻译成 Slint、LVGL、XML、SVG、LaTeX、Markdown、HTML，或直接对接 Hugo / Zola 静态站点，还能用规则表做任意文本的代码生成——全程**零胶水代码**。

> 一句话：`swsml` 是库（解析 + 翻译后端 `sml::emit::*`），`smltools` 是把这些后端串起来的 CLI 前端。这一章学怎么用它。

> ⚠️ **实验性**：CLI 接口与 emit 后端组合仍可能随版本调整，生产关键路径请关注版本号变更。

## 12.1 安装

```bash
# 从 crates.io 安装（需 Rust 工具链）
cargo install smltools

# 或从本仓库源码构建
cargo build --release -p smltools
```

装好后 `smltools --help` 能看到全部参数。

## 12.2 最简用法：SML → Markdown

新建 `doc.sml`：

```sml
title: 我的文档
section {
  name: 简介
  body: SML 写一遍，到处翻译。
}
```

翻译为 Markdown：

```bash
smltools -i doc.sml --to md
```

输出（节选）：

```markdown
# 我的文档

## 简介

SML 写一遍，到处翻译。
```

省略 `-i` 时从 stdin 读，省略 `-o` 时写到 stdout，所以管道写法也行：

```bash
cat doc.sml | smltools --to md
```

## 12.3 翻译到更多目标

`--to` 支持：`md`/`markdown`（默认）、`json`、`toml`、`xml`、`svg`、`latex`、`slint`、`lvgl`、
`html`、`custom`、`sml`，以及编辑器用的 `tmlanguage` / `highlight`。

| 目标 | 命令 | 典型用途 |
|------|------|----------|
| Markdown | `smltools -i d.sml --to md` | 文档、README |
| JSON | `smltools -i d.sml --to json` | 对接 jq / 只吃 JSON 的工具链 |
| TOML | `smltools -i d.sml --to toml` | 对接 Cargo / pyproject 等生态 |
| XML | `smltools -i d.sml --to xml` | 数据交换、配置导出 |
| SVG | `smltools -i d.sml --to svg` | 图表、可视化 |
| LaTeX | `smltools -i d.sml --to latex` | 论文、排版 |
| Slint | `smltools -i d.sml --to slint -o ui.slint` | **用 SML 描述 UI，生成 Slint 界面** |
| LVGL | `smltools -i d.sml --to lvgl -o ui.xml` | 嵌入式屏（LVGL v8.3+ 原生 XML） |
| HTML | `smltools -i d.sml --to html` | 网页片段 |
| SML | `smltools -i d.sml --to sml` | 规范化 / 重新排版（幂等） |

## 12.4 迁移：把存量配置搬进 SML

`--from` 支持 `sml`（默认）/ `json` / `toml` / `yaml` / `xml`，**缺省按扩展名推断**：
`.json` → json，`.toml` → toml，`.yaml`/`.yml` → yaml，`.xml`/`.svd` → xml，其余按 sml。

```bash
smltools -i app.json  --to sml > app.sml     # JSON  → SML
smltools -i conf.yaml --to sml > conf.sml    # YAML  → SML
smltools -i Cargo.toml --to sml > c.sml      # TOML  → SML
smltools -i chip.svd   --to sml > chip.sml   # XML / CMSIS-SVD → SML（--from 可省）
```

反向同样成立（`--from sml --to json` 等）。JSON 一侧键按**字典序**输出
（`Value::Object` 基于 `BTreeMap`），原始书写顺序不保留 —— 这不是缺陷而是**语言约定**：
对象键序**不保证**（Rust 排序、C / C++ / JS / Lua 保源序），要保序请用**数组**。
详见 README 的「跨实现差异与约定」。

拿一份真实文件感受规模：432 KB 的 CMSIS-SVD（CH32V103xx）迁成 SML 约 277 KB / 6500 行，
每个寄存器、每个位域都在，可读可 diff；`xml→json` 与 `xml→sml→json` 结果**逐字节一致**，
说明这趟转换没有丢信息。

### XML / SVD 的映射约定

- 根元素 → 顶层的一个键；子元素 → 键，**同名兄弟合并为数组**（保序）
- **纯文本元素折叠为字符串**：`<name>PWR</name>` → `name: PWR`（不套 `_text`）
- 属性 → `_attrs`；元素**同时**有属性或子元素时，文本才进 `_text`
- 空元素 → `{}`；**叶子一律是字符串**（XML 没有类型，不猜数字/布尔）
- 命名空间前缀保留（`xs:name`、`xmlns:xs`）；CDATA 原样；按规范先做行尾归一（`\r\n` → `\n`）

> 为什么不做「自动把重复结构收成片段」：SML 的片段是**无参数的值拷贝**，
> 芯片手册里「7 个通道、偏移与描述各不相同」的寄存器块之间没有可共享的参数，
> 强行合并只会改数据形状而不减内容。缩放靠的是上面的折叠与排版，不是魔法。

## 12.5 工具箱的其它能力

```bash
# 目录批量：整目录迁进 SML（同名平铺到 -o 目录，不复刻子目录结构）
smltools -i conf.d -o out/ --from json --to sml

# 静态检查：不产出转换结果；有 error 级问题时退出码 1
smltools --lint -i doc.sml

# 剥离 SML 专有痕迹，使输出能被 JSON 等格式无损消化
smltools -i doc.sml --to json --strip
```

`--lint` 只检查 SML 文档（配 `--from json` 会直接报错）。`--strip` 清掉内部标记键
`__name`/`__type` 与浮点的原始字面量 —— 片段 / 契约 / `include` / `$env` / `@when` 在
**解析期**就已消解，解析结果本身已是纯数据。

**用 SML 定制编辑器高亮**是这一章里最特别的一项：输入不是数据，而是一份「高亮声明」。

```bash
smltools -i my-dialect.sml --to tmlanguage         # 升级后的 TextMate 语法（写 stdout）
smltools -i my-dialect.sml --to highlight -o out/  # 一套 5 份产物（写到目录）
```

`--to highlight` 产出 `syntaxes/sml.tmLanguage.json`、`vscode/settings.fragment.json`
（项目内就地生效）、`themes/`、`zed/highlights.scm`、`zed/themes/sml.json`。
于是「给自己的方言配色」这件事也只需要写 SML —— 与前面所有后端同一套输入格式。

## 12.6 实战：用 SML 描述界面，生成 Slint

SML 把"配置、数据、UI 结构"统一成同一份事实。比如一个设置面板：

```sml
Window {
  title: 设置
  width: 480
  height: 320
  controls {
    name: 用户名
    type: text
    placeholder: 请输入
  }
  controls {
    name: 启用通知
    type: toggle
  }
}
```

```bash
smltools -i panel.sml --to slint -o panel.slint
```

打开 `panel.slint` 即可在 Slint 设计器里预览。`smltools` 把 SML 的块结构映射成 Slint 的组件与控件，你不再为每种 UI 框架手搓解析与模板。

> 想看 SML → Slint 的完整字段约定，可查阅 `swsml` 的 `sml::emit::to_slint` 文档。

## 12.7 文档站自动化：对接 Hugo / Zola

把 SML 直接喂给静态站点生成器，自动落盘带 front matter 的 `.md`：

```bash
# Hugo：生成 content/zh/docs/<name>.md
smltools -i doc.sml --hugo ./site --hugo-lang zh --hugo-section docs

# Zola：生成带 TOML front matter 的 .md
smltools -i doc.sml --zola ./content --zola-section docs
```

`--hugo` / `--zola` 模式下会忽略 `-o`，直接按输入文件名（或 `@feature base` 指定的名字）落盘，发布流水线少一个手工转换环节。

## 12.8 自定义生成器：用规则表渲染任意文本

`--to custom` 配合 `--custom-rules` 指定一份 SML 规则表，描述"匹配什么、输出什么"，就能渲染 Dockerfile、代码脚手架等任意文本，不必引入重量级模板引擎：

```bash
smltools -i data.sml --to custom --custom-rules rules.sml -o out.txt
```

规则表本身也是 SML——你用同一种语言描述数据和生成逻辑。

## 12.9 编辑器：VS Code 扩展提供什么

`smltools` 负责「文件 → 别的格式」；**编辑器扩展**负责「你正在写 SML 的时候」。
0.4.2 的 VS Code 扩展（VSIX 手动安装，未上架市场）除语法高亮、诊断、补全、格式化外，
还有几项**只在 SML 里说得通**的能力：

| 能力 | 怎么用 | 为什么有用 |
|---|---|---|
| 悬浮契约 | 光标停在契约名（`@is Server`） | 显示契约声明 **+ 解析器按契约填充默认值后的实际结构** —— 是「真跑出来的结果」，不是把声明复述一遍 |
| 悬浮块名 | 光标停在块名（`primary {`） | 显示块的**路径**（`database.primary`）、它应用的契约与填充后的实际结构 |
| 跳转到定义 | `@is Server` → `@contract Server`；`&base` → `@base { }` | 片段 / 契约是**文档级名字**，同名即跳，不做作用域分析 |
| 特殊颜色 | 选中一个词 → 右键「应用特殊颜色」 | 颜色写进工作区的 `HL-cfg.sml`（**它自己也是 SML 文件**，可直接手写编辑、随仓库走）。默认**只对语法单元生效**（契约 / 片段 / 类型 / 键 / 指令），不会把注释与字符串里的同名文字一起染掉 |
| 特别高亮 | 选中一个词 → 右键「特别高亮选中词」 | 在整个工作区把它点亮（改字段名 / 重构前先看清影响面） |
| 自检 | 右键「SML: 自检」 | 悬浮 / 跳转 / 高亮「没反应」时，把每一环（扩展版本、**包内解析器指纹**、命令注册、语言模式、文档校验结果、能不能跳转）写进「输出 → SML」面板 |

> 为什么这些能力值得为它单独做：SML 的**契约是可选叠加**，同一份数据「加不加契约」是两个样子。
> 悬浮直接显示**契约应用之后**的结果，等于把「解析期到底发生了什么」摊在眼前 ——
> 这是纯文本编辑器给不了的。
>
> 反过来，如果它「没反应」，先跑 **SML: 自检**：历史上最坑的两个真身是「扩展根本没激活」
> 与「文件的_语言模式_不是 SML」—— 两者都表现为「悬浮 / 右键菜单全都没反应」，与「扩展坏了」无法区分。

## 12.10 动手试一试

拿你手头任意一份 SML 配置，试翻译成不同目标，体会"写一遍、到处用"：

```bash
smltools -i your.sml --to xml
smltools -i your.sml --to svg
smltools -i your.sml --to latex
```

再反过来走一遍 —— 找一个你现有的 JSON / YAML / TOML / XML，把它迁进来：

```bash
smltools -i your.json --to sml | head -40     # 先看前 40 行再决定
smltools -i your.json --to sml -o your.sml
smltools -i your.sml --to json | diff - <(smltools -i your.json --to json)   # 往返自检
```

最后一条是关键：**迁移不丢信息**这件事不用信文档，自己 diff 一遍就知道。

→ [附录：与 JSON/YAML/TOML 对照](/book/appendix)
