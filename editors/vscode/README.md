# SML — VSCode 扩展

为 [SML](../README.md)（SNOWARE Markup Language）提供编辑支持。

## 功能

| 功能 | 说明 |
|---|---|
| **语法高亮** | 键、字符串、数字、布尔/null、注释、指令、片段、契约关键字、类型、修饰符 |
| **错误提示** | 实时解析并定位错误到精确行列（红色波浪线 + 问题面板），**含契约语义错误** |
| **补全** | 指令、契约关键字、类型、修饰符、字面量、契约名、片段名、本文档键名 |
| **悬浮说明** | ① 悬停 `@contract` / `@is` / `loose` / `include` 等关键字看解释；② **悬停契约名看「填入默认值后的结构」**（来自解析结果，不是抄一遍声明） |
| **跳转到定义** | `@is Server` → `@contract Server`；`&base` → `@base { }`（F12 / Ctrl+点击） |
| **格式化** | 按 SML 规范重排（解析 → 序列化），解析失败时不改动文件 |

## 安装（从源码）

扩展未上架市场（**上架暂缓**，见 [TODO.md](../../TODO.md) §六），需本地打包安装。
用 **npx** 调用 `vsce`，无需全局安装：

```bash
cd editors/vscode

npm run package          # 同步解析器 + 打包（推荐，等价于下面两步）
# 或手动：
#   python scripts/sync-parser.py
#   npx --yes @vscode/vsce package

code --install-extension sml-lang-0.4.2.vsix
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

- ~~**契约校验不生效**~~：此说明**已过时**（2026-09-18 更正）。JS 实现的契约
  （`@contract` / `@is` / 默认值填充 / 严格与 loose / `[T]` 数组类型）**已经支持**，
  因此字段类型不符、枚举越界、未声明字段等**语义**错误会随解析一起报出。
  历史缺口是 `[T]` 数组类型简写曾未实现（照文档写 `tags: [str] optional` 会假报错），
  已修（见 [CHANGELOG](../../CHANGELOG.md)）。
- 解析失败时只报**第一条**错误（解析器遇错即停），后续错误需修正后再次触发。
- 补全基于文本扫描（正则），非完整语义分析。
- 「悬浮显示契约展开结果」只覆盖**顶层**标注块：嵌在别的块里的标注块取不到实例时，
  悬浮会明说「未找到可展开的实例」，只显示契约声明 —— 不猜、不编。
- 「跳转到定义」同理只做**同名定义**（文档级名字），不做作用域分析。

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

---

## English

An English version of this document is available at
[README.en.md](./README.en.md).

# SML — VSCode Extension

Provides editing support for [SML](../README.md) (SNOWARE Markup Language).

## Features

| Feature | Description |
|---|---|
| **Syntax highlighting** | keys, strings, numbers, bool/null, comments, directives, fragments, contract keywords, types, modifiers |
| **Diagnostics** | real-time parse with errors located to exact line/column (red squiggles + Problems panel) |
| **Completion** | directives, contract keywords, types, modifiers, literals, contract names, fragment names, in-document keys |
| **Hover** | hover `@contract` / `@is` / `loose` / `include` to see explanations and examples |
| **Formatting** | reformat per SML spec (parse → serialize); no change if parse fails |

## Install (from source)

The extension is not on the Marketplace; build and install locally. Use **npx**
to call `vsce` without a global install:

```bash
cd editors/vscode
npm run package          # sync parser + package (recommended)
code --install-extension sml-lang-0.4.2.vsix
```

One command to package and install (overwrites the old version): `npm run install-local`.
Or install manually: VSCode → `Extensions` → `...` → `Install from VSIX`.

> **Why `npm run package` and not just `vsce package`**: before packaging,
> `scripts/sync-parser.py` copies the repo's `js/sml.mjs` into `src/vendor/`.
> The VSIX only contains files inside the extension directory; if the bridge
> imported the out-of-tree `../../../js/sml.mjs` directly, that module **would
> not be bundled**, and the extension would fail on other machines.

## Why not LSP

SML has a small grammar and a zero-dependency parser (`js/sml.mjs`) that can be
imported directly, so in-process calls are lighter — no install, no port
coordination. The cost is being limited to VSCode.

## Known limitations

- **Contract validation does not run**: contracts are currently only supported in
  the Rust implementation; the JS implementation only does syntax parsing. So
  semantic errors (type mismatch, enum out-of-range) **will not be reported** in
  the editor; syntax errors are reported normally.
- On parse failure only the **first** error is reported.
- Completion is based on text scanning (regex), not full semantic analysis.
