---
title: "序章：为什么是 SML"
translationKey: "book-intro"
---

# 序章：为什么是 SML

在你动手写第一行 SML 之前，先回答一个问题：**为什么又一种配置格式？**

JSON、YAML、TOML 都很好用，但它们各有让人皱眉的地方：

| 格式 | 痛点 |
|------|------|
| JSON | 引号、逗号、花括号一个都不能少；写配置很啰嗦 |
| YAML | 缩进敏感，一个空格错位就炸；`no` 会被当成 `false` 吓你一跳 |
| TOML | 嵌套一深就要写很多 `[table.sub]` 头，块结构不直观 |

## SML 的设计目标

SML 想做**"像写便签一样写配置"**：

1. **少仪式感**——引号可选、逗号可选、块冒号可省。
2. **结构靠花括号**——不靠缩进，复制粘贴不会出现诡异错误。
3. **可读优先**——让人一眼看懂，也让 diff / review 更轻松。
4. **可裁剪**——基础能力极简，复杂能力（通配 include、正则等）按需开启，不重蹈 YAML 过度复杂的覆辙。
5. **契约内建**——不只是"存数据"，还能定义"数据应该长什么样"并在解析期校验。

## 一个最小对比

同样一份配置，三种写法：

```json
{
  "name": "gateway",
  "port": 8080,
  "debug": true,
  "tags": ["logging", "metrics"]
}
```

```yaml
name: gateway
port: 8080
debug: true
tags:
  - logging
  - metrics
```

```sml
name: gateway
port: 8080
debug: true
tags: [ logging metrics ]
```

SML 版本几乎没有"标点噪音"——没有引号、没有逗号、没有缩进陷阱。这就是它的核心气质。

## 它不适合什么

SML 是**数据 / 配置格式**，不是编程语言：

- 不能写 `if` / `for`、不能定义函数、不能算 `1+1`。
- 需要逻辑时，用宿主语言（Rust / JS / Lua …）读 SML、在代码里处理。

> 记住一句话：**SML 负责"描述是什么"，代码负责"怎么做"。**

下一章，我们写出第一个真正的 SML 文件。→ [第 1 章](/book/ch01-basics)

## 动手练习

读完本章，在下面的编辑器里**直接修改 SML 并点“运行”**，立刻看到解析结果或校验错误——有输出才能高效学习。

{{< sml-playground "intro" >}}

{{< sml-quiz "intro" >}}

