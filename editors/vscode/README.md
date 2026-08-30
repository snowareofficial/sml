# SML — VSCode 扩展

为 [SML](../README.md)（SNOWARE Markup Language）提供编辑支持。

## 功能

| 功能 | 说明 |
|---|---|
| **语法高亮** | 键、字符串、数字、布尔/null、注释、指令、片段、契约关键字、类型、修饰符 |
| **错误提示** | 实时解析并定位错误到精确行列（红色波浪线 + 问题面板） |
| **补全** | 指令、契约关键字、类型、修饰符、字面量、契约名、片段名、本文档键名 |
| **悬浮说明** | 悬停 `@contract` / `@is` / `loose` / `include` 等关键字查看解释与示例 |
| **格式化** | 按 SML 规范重排（解析 → 序列化），解析失败时不改动文件 |

## 安装（从源码）

扩展未上架市场，需本地打包安装。用 **npx** 调用 `vsce`，无需全局安装：

```bash
cd editors/vscode

npm run package          # 同步解析器 + 打包（推荐，等价于下面两步）
# 或手动：
#   python scripts/sync-parser.py
#   npx --yes @vscode/vsce package

code --install-extension sml-lang-0.2.0.vsix
```

一条命令打包并安装（覆盖旧版）：

```bash
npm run install-local
```

或手动安装：VSCode → `扩展` → `...` → `从 VSIX 安装`，选择生成的 `.vsix`。

> **为什么必须 `npm run package` 而不是直接 `vsce package`**：
> 打包前需运行 `scripts/sync-parser.py`，把仓库的 `js/sml.mjs` 复制到
> `src/vendor/`。VSIX 只包含扩展目录内的文件，若桥接层直接 import
> 目录外的 `../../../js/sml.mjs`，该模块**不会被打进包**，
> 装到别的机器上会因找不到模块而完全失效。

开发时无需打包：在 VSCode 中打开 `editors/vscode` 目录，按 `F5` 启动扩展宿主即可调试。

## 为什么不用 LSP

SML 语法小、解析器（`js/sml.mjs`）零依赖且可直接 import，进程内调用更轻，
免安装、免端口协调。代价是能力局限于 VSCode。

若将来需支持其他编辑器，可把 `src/sml-parse.mjs` 包一层 LSP server 复用，
扩展主体逻辑无需重写（见 [TODO.md](../../TODO.md)）。

## 已知限制

- **契约校验不生效**：契约目前仅 Rust 实现支持（见 [TODO.md](../../TODO.md)），
  JS 实现只做语法解析。因此字段类型不符、枚举越界等**语义**错误在编辑器中不会报出；
  语法错误会正常报出。
- 解析失败时只报**第一条**错误（解析器遇错即停），后续错误需修正后再次触发。
- 补全基于文本扫描（正则），非完整语义分析。

## 文件结构

```
editors/vscode/
├── package.json                      # 扩展清单（语言/语法/配置贡献点）
├── language-configuration.json       # 注释、括号、缩进、折叠
├── syntaxes/sml.tmLanguage.json      # TextMate 语法（高亮，声明式）
└── src/
    ├── extension.js                  # 诊断 / 补全 / 悬浮 / 格式化
    └── sml-parse.mjs                 # 桥接层：复用 ../../../js/sml.mjs
```

桥接层直接复用仓库的 JS 实现，保证**插件与语言实现行为一致**：
同一份文本，插件报错的位置就是解析器真正报错的位置。
