# smlconv (EXPERIMENTAL)

> ⚠️ **实验性 (EXPERIMENTAL)**：本站点的 CLI 接口与 emit 后端组合仍可能随用户反馈调整，暂不做语义化稳定性承诺。生产关键路径请勿依赖其精确行为，请关注版本号变更日志。

SML (SNOWARE Markup Language) 命令行转换器 / 多目标翻译器，从 `swsml` 主 crate 拆分出的独立二进制 crate。

复用 `swsml` 的解析器与 `sml::emit::*` 翻译后端，仅负责 CLI 组装与多目标翻译驱动。

## 安装

```bash
cargo install smlconv
# 或从源码（本仓库）
cargo build --release -p smlconv
```

## 用法

```bash
# SML -> JSON（默认）
smlconv input.sml --format json -o out.json

# SML -> Slint
smlconv input.sml --format slint -o ui.slint

# SML -> LVGL C 源码
smlconv input.sml --format lvgl -o ui.c

# SML -> XML / SVG / LaTeX / Markdown
smlconv input.sml --format xml
smlconv input.sml --format svg
smlconv input.sml --format latex
smlconv input.sml --format markdown

# SML -> Hugo（需 --hugo-root）
smlconv input.sml --format hugo --hugo-root ./site \
    --hugo-section docs --hugo-lang all

# 自定义生成器：规则表 + 模板
smlconv input.sml --rule rules.sml --template '{{section.name}}'
smlconv input.sml --rule rules.sml --template-file tmpl.txt
```

省略 `INPUT` 时从 stdin 读取，省略 `OUTPUT` 时写到 stdout。

## 与 swsml 的关系

- `swsml`：库（解析 + 翻译后端 `sml::emit::*`），零依赖主库。
- `smlconv`：独立打包的 CLI 前端，依赖 `swsml` 的 `emit` 特性。

## 许可

MulanPSL-2.0
