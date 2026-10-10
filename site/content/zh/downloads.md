---
title: "下载"
---

# 下载 SML { ❄ }

## VS Code 扩展（0.4.2）

为 VSCode 提供 SML 的语法高亮、错误提示、补全、悬浮说明、跳转与格式化，以及一批
「只在 SML 里说得通」的编辑期能力：

- **悬浮**：契约名（`@is Server`）显示契约声明 + **解析器按契约填充默认值后的实际结构**；
  块名（`primary {`）显示**路径**（`database.primary`）、它应用的契约与填充后的结构
- **跳转到定义**：`@is Server` → `@contract Server`，`&base` → `@base { }`
- **include / 命名空间导航**：路径上 F12 跳到被包含文件、悬停给「该文件的顶层键」；
  命名空间别名（`as db`）悬停给「这个命名空间里**实际有哪些键**」，F12 跳到被包含文件；
  引用（`db.host`）悬停告诉你它来自哪个文件的什么定义，F12 直接跳过去
- **特殊颜色**：选中一个词 → 右键「应用特殊颜色」，写进工作区的 `HL-cfg.sml`
  （随仓库走、可直接手写编辑）；默认**只对语法单元生效**（契约 / 片段 / 类型 / 键 / 指令），
  不会把注释与字符串里的同名文字一起染掉；**也认命名空间前缀**（`@is ui.base.Service`
  这类引用处照样着色）。**写 DSL / 做分析时这套东西的用法**见扩展 README 的同名小节
- **特别高亮**：把某个词在**整个工作区**里点亮（状态栏显示 N 处 / M 文件）
- **自检**：右键「SML: 自检」把「为什么没反应」逐条摊开写进「输出 → SML」
  （扩展版本、**包内解析器指纹**、命令注册、语言模式、文档校验结果、能不能跳转）

- 下载：[sml-lang-0.4.2.vsix](/dl/sml-lang-0.4.2.vsix)
- 安装：VSCode → 扩展 → `...` → 从 VSIX 安装，选择下载的文件
- 说明：扩展目录内含 `README.md`（中英双语）与 `README.en.md`（英文）

> **本版为重新打包**：修掉了「在 VS Code 1.13x 上扩展**根本无法激活**」的致命问题
> （模块顶层读取了新版宿主里已移除的 `vscode.InsertTextFormat`）。若你装过更早的
> 0.4.2，请**重新安装本文件** —— 旧包在新版 VS Code 上表现是「悬浮 / 右键菜单全都没反应」。
> 装完可用右键「SML: 自检」核对包内解析器指纹（应为 `96110 B / c7a5054d…`）。
>
> 扩展未上架 Marketplace，仅提供 VSIX 手动安装（VSIX 是 VS Code 专用格式，Zed 装不了它 —— Zed 用户请看仓库里的 `editors/zed/README.md`）。
> 契约（`@contract` / `@is`）在 Rust、JS、C、C++、Lua 上都已支持，因此**编辑器内的语义错误也会报**（早前「仅 Rust 支持契约」的说法已过时，详见扩展 README 的「已知限制」）。

**本版（`96110 B / c7a5054d` 解析器）新增 / 修正**：

- **命名空间导航**：`as db` 别名的悬停（该命名空间下**实际**有哪些键）与 F12；`db.host`
  这类引用的悬停（来自哪个文件的什么定义）与 F12。
- **按语法单元着色现在认命名空间前缀**：`@is ui.base.Service`、`ui.base.net.region:`、
  `&ui.base.base` 这些**引用处**也着色（此前只有被包含文件里的定义处有颜色）。
- **五端一致**：`import "x.sml"` ≡ `include "x.sml"`（此前 Rust 端会多挂一层点分命名空间，
  与 JS 得到不同的树）；JS 端补齐 `glob-include`（`include "dir/*.sml"`）与
  `import ui.buttons` 这类**点分模块名**。
- **诊断位置修正**：include 目标缺失时，红线不再画到几十行外的注释上；被包含文件里的错误
  归位到那条 include 指令，并在文案里带上目标文件名。

## 库与命令行工具（`swsml` 0.6.2 · `smltools` 0.2.1）

- **Rust 库**：[`swsml` 0.6.2](https://crates.io/crates/swsml) —— `cargo add swsml`
- **命令行工具**：[`smltools` 0.2.1](https://crates.io/crates/smltools) —— `cargo install smltools`
  （`smltools` 是 `smlconv` 改名后的**首次发布**；旧的 `smlconv` 不再更新）
- **其他语言**：C（C-ABI 头文件 `c/sml.h`）、JavaScript（零依赖 ESM，可直接 `import`）、
  C++ 与 Lua 的实现随仓库源码提供

> **0.6.2 的要点**：错误码体系全量落地（137 条码，五端实现 + 命令行工具都带码，
> 本站可查：[错误码](/errors/)）；include / import 五端对齐；五端「静默通过」改为带码报错；
> 安全修复（块注释里伪造的 include 不再读盘、C 的 `err=NULL` 越界写、深度守卫绕过）；
> `smltools` 新增 `--from xml`（CMSIS-SVD 可直接读入）与 `--to html`。
> 完整清单见 [CHANGELOG.md](https://gitee.com/snoware/sml/blob/main/CHANGELOG.md)。

各语言参考实现（Rust `swsml` 等）随仓库源码提供，见 [首页](/)。
