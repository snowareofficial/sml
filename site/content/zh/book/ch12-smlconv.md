---
title: "第 12 章：smlconv 多目标翻译器"
translationKey: "book-ch12"
---

# 第 12 章：smlconv 多目标翻译器

`smlconv` 是 SML 的命令行转译器：把**同一份 SML 文档**一键翻译成 Slint、LVGL、XML、SVG、LaTeX、Markdown、HTML，或直接对接 Hugo / Zola 静态站点，还能用规则表做任意文本的代码生成——全程**零胶水代码**。

> 一句话：`swsml` 是库（解析 + 翻译后端 `sml::emit::*`），`smlconv` 是把这些后端串起来的 CLI 前端。这一章学怎么用它。

> ⚠️ **实验性**：CLI 接口与 emit 后端组合仍可能随版本调整，生产关键路径请关注版本号变更。

## 12.1 安装

```bash
# 从 crates.io 安装（需 Rust 工具链）
cargo install smlconv

# 或从本仓库源码构建
cargo build --release -p smlconv
```

装好后 `smlconv --help` 能看到全部参数。

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
smlconv -i doc.sml --to md
```

输出（节选）：

```markdown
# 我的文档

## 简介

SML 写一遍，到处翻译。
```

省略 `-i` 时从 stdin 读，省略 `-o` 时写到 stdout，所以管道写法也行：

```bash
cat doc.sml | smlconv --to md
```

## 12.3 翻译到更多目标

`--to` 支持：`md`/`markdown`、`xml`、`svg`、`latex`、`slint`、`lvgl`、`html`、`custom`、`sml`。

| 目标 | 命令 | 典型用途 |
|------|------|----------|
| Markdown | `smlconv -i d.sml --to md` | 文档、README |
| XML | `smlconv -i d.sml --to xml` | 数据交换、配置导出 |
| SVG | `smlconv -i d.sml --to svg` | 图表、可视化 |
| LaTeX | `smlconv -i d.sml --to latex` | 论文、排版 |
| Slint | `smlconv -i d.sml --to slint -o ui.slint` | **用 SML 描述 UI，生成 Slint 界面** |
| LVGL | `smlconv -i d.sml --to lvgl -o ui.xml` | 嵌入式屏（LVGL v8.3+ 原生 XML） |
| HTML | `smlconv -i d.sml --to html` | 网页片段 |

## 12.4 实战：用 SML 描述界面，生成 Slint

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
smlconv -i panel.sml --to slint -o panel.slint
```

打开 `panel.slint` 即可在 Slint 设计器里预览。`smlconv` 把 SML 的块结构映射成 Slint 的组件与控件，你不再为每种 UI 框架手搓解析与模板。

> 想看 SML → Slint 的完整字段约定，可查阅 `swsml` 的 `sml::emit::to_slint` 文档。

## 12.5 文档站自动化：对接 Hugo / Zola

把 SML 直接喂给静态站点生成器，自动落盘带 front matter 的 `.md`：

```bash
# Hugo：生成 content/zh/docs/<name>.md
smlconv -i doc.sml --hugo ./site --hugo-lang zh --hugo-section docs

# Zola：生成带 TOML front matter 的 .md
smlconv -i doc.sml --zola ./content --zola-section docs
```

`--hugo` / `--zola` 模式下会忽略 `-o`，直接按输入文件名（或 `@feature base` 指定的名字）落盘，发布流水线少一个手工转换环节。

## 12.6 自定义生成器：用规则表渲染任意文本

`--to custom` 配合 `--custom-rules` 指定一份 SML 规则表，描述"匹配什么、输出什么"，就能渲染 Dockerfile、代码脚手架等任意文本，不必引入重量级模板引擎：

```bash
smlconv -i data.sml --to custom --custom-rules rules.sml -o out.txt
```

规则表本身也是 SML——你用同一种语言描述数据和生成逻辑。

## 12.7 动手试一试

拿你手头任意一份 SML 配置，试翻译成不同目标，体会"写一遍、到处用"：

```bash
smlconv -i your.sml --to xml
smlconv -i your.sml --to svg
smlconv -i your.sml --to latex
```

→ [附录：与 JSON/YAML/TOML 对照](/book/appendix)
