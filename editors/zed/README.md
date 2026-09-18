# SML for Zed

SML (SNOWARE Markup Language) 的 [Zed](https://zed.dev) 扩展骨架。

Zed 用 **Tree-sitter** 做高亮（不吃 TextMate scope），所以这里与 `editors/vscode/` 是
两套技术栈，不能互相复制粘贴：TextMate 语法在 `editors/vscode/syntaxes/sml.tmLanguage.json`，
Tree-sitter 语法在本目录的 `grammars/sml/grammar.js`。

## 目录结构

```
editors/zed/
  extension.toml                    # 扩展清单（含 grammar 声明，见下方「已知限制」）
  languages/sml/
    config.toml                     # 语言名 / 后缀 / 注释符 / 缩进
    highlights.scm                  # 高亮查询（Zed 就读这个位置）
  grammars/sml/
    grammar.js                      # Tree-sitter 语法（节点名契约见其文件头）
    test/parse/{basic,advanced}.sml # 语法样例，供 tree-sitter parse 查 ERROR
```

**没有** `Cargo.toml` / `src/lib.rs`：纯语法扩展不需要 Rust 代码（Zed 官方文档明确
只有 LSP / context server / debugger 三类扩展才需要）。真要加 LSP 时再补。

## 安装（本地开发）

1. 先把 grammar 的路径填对 —— `extension.toml` 里现在是**占位值**，直接装会失败：
   按该文件顶部注释改成 `file://` + 本机绝对路径（monorepo 下的 grammar 不能像独立
   grammar 仓库那样只填仓库 URL）。改完该目录必须在 Git 仓库里，`rev` 填 HEAD 短 sha。
2. Zed → Extensions → **Install Dev Extension** → 选 `editors/zed/` 目录
   （或命令面板执行 `zed: install dev extension`）。
3. 打开任意 `.sml` 文件，若无高亮：命令面板 `zed: open log` 看报错；需要更详细日志时
   关掉 Zed，从终端 `zed --foreground` 重启。
4. 提供了 grammar 的扩展需要 **wasi-sdk** 来编译 parser（Zed 会自动下载；也可用
   `WASI_SDK_PATH` 指向已有安装）。

## 与 `smltools --to highlight` 的关系（容易踩）

`smltools --to highlight` 会生成 5 份产物，其中 Zed 侧两份是：

| 生成路径 | 真实去向 |
|---|---|
| `zed/highlights.scm` | **复制**到本目录 `languages/sml/highlights.scm`（Zed 只读这里） |
| `zed/themes/sml.json` | 主题文件，用户按 Zed 的主题安装方式引入（不是扩展的一部分） |

生成路径是「暂存/待复制」而不是 Zed 直接读取的位置 —— 这是 Zed 的目录约定决定的。
本目录的 `highlights.scm` 是**不分方言**的通用版；用 `--to highlight` 得到的版本会
额外带上你那份定制里的方言指令（`@form` / `@policy` 之类）作为关键字。

### 节点名契约

`highlights.scm` 与 `grammar.js` 之间靠 **节点名** 耦合，见 `grammar.js` 文件头的表格。
最要紧的一条：`directive` 节点的文本**包含 `@`**（是 `@form`，不是 `form`），所以生成器
给出的谓词形如 `^@(form|policy)$`。改任一侧都要同步另一侧，否则谓词永远匹配不上、
高亮静默失效。

## 已知限制（都是实情，不是免责声明）

1. **Zed grammar 已通过编译验证（2026-09-19）**：用 `tree-sitter-cli@0.22.6` 跑了
   `generate`，并用 `parse test/parse/basic.sml` 与 `parse test/parse/advanced.sml`
   验证，**两份语料均 0 `ERROR`、0 `MISSING`**；生成的 `src/parser.c`（约 53 KB）、
   `src/grammar.json`、`src/node-types.json` 已随本目录一同提交进 monorepo。
   验证命令（见 `grammars/sml/test/parse/README.md`）：
   ```bash
   cd editors/zed/grammars/sml && tree-sitter generate && tree-sitter parse test/parse/*.sml
   ```
   仍**未**提交标准的 `test/corpus/*.txt`（那需要手推期望语法树，留待后续补充）。
2. **grammar 的发布仓库已定（2026-09-19）**：Zed 的 `extension.toml` 只认
   `repository` + `rev`（官方文档没有 `path` 字段），会整仓克隆，不认 monorepo 子目录。
   因此源真相留在 monorepo（`editors/zed/grammars/sml/`）的**同时**，另开独立发布仓库
   `snoware/tree-sitter-sml` 作**下游镜像**（只用于发布，不含其它内容）；`extension.toml`
   的 `[grammars.sml]` 已指向它，`rev` 已填该仓库首版 commit `b72396d…`（2026-09-19）。本地开发要立刻
   见效仍可用 `file://` + 本机绝对路径（见 `extension.toml` 顶部注释）。
3. **与权威实现的刻意差异**（Tree-sitter 正则不支持环视，`grammar.js` 文件头有完整列表）：
   - `a--b` 这里会整段当裸词，`swsml` 的 lexer 会切成 `a` + 行注释；
   - 数字同样能被裸词正则匹配，靠 `prec(1)` 让 `number` 胜出。
4. **`@version v4` / `@feature base` 的参数会被着色成类型名**：语法层分不出「指令名
   是什么」（那是文本，不是结构）。要精确限定需在查询里加谓词与辅助捕获：
   ```scheme
   ((directive) @_d (type_name) @type (#match? @_d "^@(contract|is|type)$"))
   ```
   此处没有采用 —— `@_` 前缀（不参与着色）的行为我未能在当前环境验证，宁可留个
   可见的小瑕疵，也不提交一条可能整段失效的查询。
5. 能力范围只到 **高亮 / 括号匹配 / 注释切换**。VSCode 侧的实时诊断、补全、悬浮说明
   来自那里的扩展代码（`editors/vscode/src/`），Zed 侧要同样的能力得另写 LSP 接入，
   本骨架不包含。
