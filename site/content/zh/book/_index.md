---
title: "SML 教科书"
translationKey: "book-index"
---

# SML 教科书 { ❄ }

欢迎来到 **SML（SNOWARE Markup Language）** 的官方教科书。

这是一本**循序渐进、面向初学者**的教材，同时也**可作工具书随时翻阅**。无论你有没有用过 JSON / YAML / TOML，都能从零开始学会用 SML 写配置、描述数据。

> 一句话理解 SML：它是一种**声明式的数据 / 配置格式**，像写便签一样写配置——引号可省、逗号可省、结构靠花括号 `{}` 定界。

## 这本书怎么读

- **零基础**：从第 1 章开始，一章一章往下读，每章都有「动手试一试」。
- **有经验**：直接跳到第 4–5 章看 include / 契约等进阶能力。
- **查用法**：用下面目录跳到对应章节，或读附录的「对照表 / 错误排查」。

## 目录

| 章节 | 主题 | 你会学到 |
|------|------|----------|
| [序章](/book/intro) | 为什么是 SML | 它解决什么问题、和 JSON/YAML 的区别 |
| [第 1 章](/book/ch01-basics) | 第一个 SML 文件 | 键值对、注释、标量类型（字符串/数字/bool/null） |
| [第 2 章](/book/ch02-blocks) | 块与嵌套 | 对象块、数组、冒号可省、嵌套结构 |
| [第 3 章](/book/ch03-fragments) | 片段继承 | `@base` 定义、`&base` 引用，配置复用 |
| [第 4 章](/book/ch04-include) | include 与命名空间 | 拆分文件、`as ns` 隔离、`import` 别名 |
| [第 5 章](/book/ch05-contract) | 契约系统 | `@contract` / `@is`，类型、默认值、枚举、区间、组合 |
| [第 6 章](/book/ch06-env-escape) | 环境变量与转义 | `$env` 注入、`\u` 与 `\n` 转义 |
| [第 7 章](/book/ch07-languages) | 多语言使用 | Rust / C / JS / Lua 如何集成 SML |
| [第 8 章](/book/ch08-project) | 实战：完整项目 | 一个接近真实的部署配置范例 |
| [第 9 章](/book/ch09-advanced) | 进阶：功能组合 | include/契约/片段/$env 组合、8 种设计模式 |
| [第 10 章](/book/ch10-features) | feature 完整参考 | 每个 feature 的开关、语法、报错、兼容矩阵 |
| [附录](/book/appendix) | 对照与排查 | 与 JSON/YAML/TOML 对照、常见错误 |

## 约定

- 所有示例都是**真实可解析**的 SML 文本。
- 代码块标注 `sml` 的是 SML 源码；标注 `rust` / `js` / `c` / `lua` 的是宿主语言调用。
- 示例中 `#`、`--`、`//` 都是注释，怎么写都行。

准备好了吗？从 [序章](/book/intro) 开始吧。

## 动手练习

读完本章，在下面的编辑器里**直接修改 SML 并点“运行”**，立刻看到解析结果或校验错误——有输出才能高效学习。

{{< sml-playground "intro" >}}

{{< sml-quiz "intro" >}}

