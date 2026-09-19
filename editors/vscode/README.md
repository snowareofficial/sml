# SML — VSCode 扩展

为 [SML](../README.md)（SNOWARE Markup Language）提供编辑支持。

## 功能

| 功能 | 说明 |
|---|---|
| **语法高亮** | 键、字符串、数字、布尔/null、注释、指令、片段、契约关键字、类型、修饰符 |
| **错误提示** | 实时解析并定位错误到精确行列（红色波浪线 + 问题面板），**含契约语义错误** |
| **补全** | 指令、契约关键字、类型、修饰符、字面量、契约名、片段名、本文档键名 |
| **悬浮说明** | ① 悬停 `@contract` / `@is` / `loose` / `include` 等关键字看解释；② **悬停契约名看「填入默认值后的结构」**（来自解析结果，不是抄一遍声明） |
| **跳转到定义** | `@is Server` → `@contract Server`；`&base` → `@base { }`（F12 / Ctrl+点击 / 右键「转到定义」，三者同一套 provider） |
| **悬浮（块名）** | 光标停在块名上（`primary {`，或**块级类型标注** `Server primary {`）：显示**路径**（`database.primary`）、它应用的契约、以及**契约填充后的实际结构**；没有契约的块也给结构。⚠️ 块级类型标注写法需 `@feature enable typed-block`，目前 **Rust + JS 已实现，C / C++ / Lua 未实现**（后者会把该写法当普通裸块 ⇒ 契约静默不生效）—— 跨实现共享的文件请写 `@is 契约名` |
| **悬浮（字段）** | 停在契约声明里的字段名上（`port: int default 5432 min 1 max 65535`），**或**停在数据区里属于某契约的键上 → 给类型 / 枚举 / 默认值 / 区间 / 必填可选 + **行尾 `#` 说明** + 当前值（并指出这个值是显式写的还是契约填的默认值） |
| **跳转到定义** | `@is Server` → `@contract Server`；`&base` → `@base { }`；`契约名 块名 {` → `@contract 契约名`；**数据区的键 → 契约里的字段声明**（F12 / Ctrl+点击） |
| **include / import 导航** | 光标停在 `include "conf.d/db.sml"` / `import "a.sml"` 的**路径**上：**F12 跳到被包含文件**；**悬停**显示解析到的路径（✓ 找到 / ✗ 未找到）+ 该文件的**顶层键**；在路径位置**补全**会列出工作区里的 `.sml`（按**相对当前文档**的写法插入）。相对路径按 `include` 的语义（相对被包含文件所在目录）解析 —— 与解析器同一条规则 |
| **特殊颜色** | 选中一个词 → 右键「应用特殊颜色」：选颜色后写进工作区的 **`HL-cfg.sml`**（随仓库走、可直接手写编辑）。默认**只对语法单元生效**（契约 / 片段 / 类型 / 键 / 指令）—— 这样不会把注释、字符串里的同名文字一起染掉；要整篇同词都染，可选「按普通词着色」（`unit: text`） |
| **特别高亮** | 选中一个词 → 右键「特别高亮选中词（当前工作区）」：把该词在**整个工作区**里点亮（状态栏显示 N 处 / M 文件；点状态栏或对同一个词再触发一次即清除） |
| **自检** | 悬浮 / 跳转 / 高亮「没反应」时，右键「SML: 自检」：把扩展版本、**包内解析器指纹**、命令注册、语言模式、文档校验结果、光标下的词能不能跳转 —— 逐条写进「输出 → SML」 |
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

> **这个 `.vsix` 只有 VS Code 能用**：VSIX 是 VS Code 专用的打包格式，**Zed 装不了它** ——
> Zed 用 `extension.toml` + Tree-sitter grammar，是另一套技术栈（`editors/zed/`）。
> Zed 用户请看 [Zed 扩展说明](../zed/README.md)。

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

## 特别高亮（临时探照灯）

把光标放到一个词上（或选中一段**同一行**内的文字）→ 右键 →
「**SML: 特别高亮选中词（当前工作区）**」：

- 该词在**整个工作区**内的所有出现处都会被点亮；搜索范围由 `sml.specialHighlight.include`
  控制（默认 `**/*.sml`，遵循 `files.exclude`）。
- 状态栏显示 `N 处 / M 文件`；触到上限会标注**已截断**（不假装搜全了）。
  清除方式有三个：**点状态栏**、对**同一个词再触发一次**命令、右键「SML: 清除特别高亮」
  （后者仅在有高亮时出现）。
- 编辑正在高亮的文件时会**就地重扫该文件**（快），不会整工作区重搜。

⚠️ 三点是刻意为之，别「优化」掉：① **字面**匹配 —— 选中 `(`、`*`、`[` 也按字面找，
不当正则（否则轻则少命中、重则抛异常）；② **不做语义判断** —— 注释、字符串里的同名文字
同样点亮（文本级探照灯的价值在**可预期**，「聪明」在这里是负资产：用户没法预测哪处会亮）；
③ 只给**可见编辑器**上色（VSCode 的 decorations 只能作用于可见编辑器），其余文件仍计入
统计，打开时按缓存补上。

> 与「自定义高亮」的区别：`HL-cfg.sml` 那套（`sml.reloadHighlight` / `sml.setHighlightMode`）
> 是**静态配置**关键词配色；「特别高亮」是**临时**的、跟着你当前选中的词走。

## 没反应时先跑自检

悬浮不出展开、跳转没动静、特别高亮没反应 —— 先别怀疑编辑器：**右键 →「SML: 自检」**
（或命令面板搜 `SML: 自检`），结果写在「**输出 → SML**」面板。它逐条报告：

- 扩展版本与**包内解析器指纹**（`vendor/sml.mjs` 的字节数 + sha256 前缀）—— 与仓库当前版本
  对不上就说明**装的是旧包**，重打重装即可；
- 命令是否注册（缺 `sml.specialHighlight` 同样说明装的是旧包）；
- 当前文件的**语言模式**（不是 `sml` 时，所有 provider 都不生效 —— 右下角可改）；
- **文档校验结果** —— 这是「悬浮只显示声明、没有展开」的**第一大原因**：展开那半段要求
  整份文档全绿（语法 **和** 契约）；
- 光标下的词是不是契约名、有没有实例可展开、能不能跳到定义。

面板里同时记录扩展激活与解析器加载的结果 —— 那是「静默失效」的最后一环。
（悬浮自己遇到「取不到实例」时也会把原因写进该面板，同一文档版本只解释一次。）

**另一个高频真身：文件没被当成 SML。** 语言模式不是 `sml` 时，provider（按语言选择器注册）
不会被调用、右键菜单项（`when: editorLangId == sml`）不显示、TextMate 语法也不生效 ⇒
**悬浮 / 诊断 / 补全 / 右键菜单一起失效**，看起来就像"扩展坏了"。扩展现在会在启动时检查：
只要工作区里有 `.sml` 文件就会激活（`workspaceContains:**/*.sml`），发现某个 `.sml` 的语言模式
不是 SML 时**弹一个警告并提供「设为 SML」按钮**；自检面板也会报告
`SML 语言已注册：✓/✗`（✗ 就说明扩展根本没被加载：被禁用 / 受限模式 / 未安装）。

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
  ⚠️ **前置条件（最常见的「怎么没有展开」）**：展开那一半要**整份文档都能通过校验**
  （语法 **和** 契约 —— 桥接层的 `contractInstance` 内部就是 `parseSafe(text)`）。
  文档里只要有**任何**错误，悬浮就只显示契约声明、并在下面注明「未找到可展开的实例」。
  **先看问题面板有没有红字，再看悬浮**。
  ⚠️ 也要注意写法：契约标注写在**块内**（`web {` 的下一行写 `@is Server`）；
  写成 `web @is Server { }`（块名后紧跟）**不是合法语法** —— Rust 报 `E-PARSE-012`、
  JS 报「多余的结束符号 }」。
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
| **Hover** | ① hover `@contract` / `@is` / `loose` / `include` for explanations and examples; ② **hover a contract name to see the instance after the contract is applied** — defaults really filled in by the parser, not a copy of the declaration |
| **Go to definition** | `@is Server` → `@contract Server`; `&base` → `@base { }` (F12 / Ctrl+click / right-click "Go to Definition" — all three use the same provider) |
| **Hover on block names** | put the cursor on a block name (`primary {` / `Server primary {`): shows the **path** (`database.primary`), the contract it applies, and the **structure after the contract filled in defaults**; blocks without a contract still show their structure |
| **Go to definition** | `@is Server` → `@contract Server`; `&base` → `@base { }`; `Contract block {` → `@contract Contract` (F12 / Ctrl+click) |
| **include / import navigation** | put the cursor on the **path** in `include "conf.d/db.sml"` / `import "a.sml"`: **F12 jumps to the included file**; **hover** shows the resolved path (✓ found / ✗ not found) plus that file's **top-level keys**; **completion** at a path position lists the workspace `.sml` files (inserted as paths **relative to the current document**). Relative paths resolve exactly the way `include` does (relative to the including file) |
| **Special colors** | select a word → right-click "应用特殊颜色" (apply special color): pick a color and it is written into the workspace **`HL-cfg.sml`** (travels with the repo, hand-editable). By default it applies **only to syntax units** (contract / fragment / type / key / directive) so comments and strings with the same text are not recolored; choose "按普通词着色" (`unit: text`) to color every occurrence |
| **Spotlight highlight** | select a word → right-click "SML: 特别高亮选中词（当前工作区）": lights up **every occurrence in the workspace** (status bar shows N matches / M files; click it or re-run on the same word to clear) |
| **Self-check** | hover / go-to-definition / highlight "not working"? right-click "SML: 自检": reports extension version, **bundled parser fingerprint**, registered commands, language mode, document validation and whether the word under the cursor is navigable — all into the "Output → SML" panel |
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

> **This `.vsix` is VS Code only**: VSIX is a VS Code-specific package format — **Zed
> cannot install it** (Zed uses `extension.toml` + a Tree-sitter grammar, a different
> stack, under `editors/zed/`). Zed users: see the [Zed extension README](../zed/README.md).

> **Why `npm run package` and not just `vsce package`**: before packaging,
> `scripts/sync-parser.py` copies the repo's `js/sml.mjs` into `src/vendor/`.
> The VSIX only contains files inside the extension directory; if the bridge
> imported the out-of-tree `../../../js/sml.mjs` directly, that module **would
> not be bundled**, and the extension would fail on other machines.

## Why not LSP

SML has a small grammar and a zero-dependency parser (`js/sml.mjs`) that can be
imported directly, so in-process calls are lighter — no install, no port
coordination. The cost is being limited to VSCode.

## Spotlight highlight (temporary searchlight)

Put the cursor on a word (or select text **within one line**) → right-click →
"**SML: 特别高亮选中词（当前工作区）**":

- Every occurrence of that word **across the workspace** is highlighted. The search scope is
  `sml.specialHighlight.include` (default `**/*.sml`, honouring `files.exclude`).
- The status bar shows `N matches / M files`; hitting a cap is reported as **truncated**
  (it never pretends the search was exhaustive). Clear it by **clicking the status bar**,
  **re-running the command on the same word**, or right-click → "SML: 清除特别高亮"
  (the last one only appears while a highlight is active).
- Editing a highlighted file re-scans **that file only** (fast) — no workspace-wide re-search.

⚠️ Three deliberate choices, do not "optimise" them away: ① **literal** matching — selecting
`(`, `*` or `[` searches for those characters, not a regex (a regex would under-match or throw);
② **no semantic filtering** — the same text inside comments or strings is highlighted too (a
text-level searchlight is valuable because it is *predictable*; cleverness here is a liability
because users cannot predict what lights up); ③ only **visible editors** can be decorated
(VSCode API), so other files still count towards the totals and get painted from the cache when
opened.

> Difference from "custom highlighting": the `HL-cfg.sml` mechanism
> (`sml.reloadHighlight` / `sml.setHighlightMode`) is **static** keyword colouring;
> the spotlight is **temporary** and follows whatever word you select.

## When nothing happens, run the self-check first

Hover shows no expansion, go-to-definition does nothing, the spotlight does not react — before
blaming the editor: **right-click → "SML: 自检"** (or run `SML: 自检` from the command palette).
The report goes to the "**Output → SML**" panel:

- extension version and the **bundled parser fingerprint** (`vendor/sml.mjs` size + sha256 prefix) —
  a mismatch means you are running an **old package** (rebuild + reinstall);
- whether the commands are registered (a missing `sml.specialHighlight` also means an old package);
- the **language mode** of the current file (anything but `sml` and no provider fires — the language
  picker in the status bar changes it);
- **document validation** — the number-one reason for "hover shows the declaration only, no
  expansion": the expansion half requires the whole document to validate (syntax **and** contracts);
- whether the word under the cursor is a contract name, whether an instance exists, whether it is
  navigable.

The panel also records activation and parser-load results — the last link in a silent failure chain.
(The hover itself logs *why* when it cannot find an instance; once per document version.)

**The other common root cause: the file is not treated as SML.** When the language mode is not `sml`,
the providers (registered per language selector) are never called, the context-menu items
(`when: editorLangId == sml`) are hidden, and the TextMate grammar does not apply either — so
**hover, diagnostics, completion and the context menu all die together**, looking like a broken
extension. The extension now activates whenever the workspace contains any `.sml` file
(`workspaceContains:**/*.sml`) and, on finding a `.sml` file with a non-SML language mode, shows a
warning with a **"设为 SML"** (set to SML) button; the self-check also reports
`SML 语言已注册：✓/✗` (a ✗ means the extension was never loaded: disabled / restricted mode /
not installed).

## Known limitations

- ~~**Contract validation does not run**~~: this note is **outdated** (corrected
  2026-09-18). The JS implementation **does support** contracts (`@contract` /
  `@is` / default fill-in / strict vs `loose` / `[T]` array types), so semantic
  errors such as type mismatch or enum out-of-range **are reported** in the editor;
  the historic gap was that the `[T]` shorthand was unimplemented — see
  [CHANGELOG](../../CHANGELOG.md).
- On parse failure only the **first** error is reported.
- Completion is based on text scanning (regex), not full semantic analysis.
- "Hover shows the expanded contract result" covers **top-level** annotated blocks only; when no
  instance can be found the hover says so explicitly ("no expandable instance found") and shows the
  declaration alone — it never guesses.
  ⚠️ **Precondition (the usual reason people think it is broken)**: the expansion half requires the
  **whole document** to validate (syntax **and** contracts — `contractInstance` calls
  `parseSafe(text)` internally). If the document has *any* error, the hover shows the declaration
  only, plus the "no expandable instance found" note. **Check the Problems panel first.**
  ⚠️ Also mind the syntax: put `@is` **inside** the block (on the line after `web {`).
  Writing `web @is Server { }` is **not valid SML** (Rust: `E-PARSE-012`; JS: "stray closing brace").
- "Go to definition" also only resolves **same-name definitions** (document-level names) — no scope
  analysis.
