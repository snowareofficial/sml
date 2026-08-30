---
title: "附录：对照与排查"
---

# 附录：对照与排查

本附录把 SML 和常见格式做对照，并列出新手最常遇到的错误及解决办法。可作工具书随时翻。

## A.1 与 JSON / YAML / TOML 对照

| 能力 | SML | JSON | YAML | TOML |
|------|-----|------|------|------|
| 引号可选 | ✅ 裸词即字符串 | ❌ 必须 | ✅（但有类型推断坑） | ✅ |
| 逗号可选 | ✅ | ❌ | ✅ | ❌ |
| 块冒号可省 | ✅ `a { }` ≡ `a: { }` | — | ✅ | — |
| 缩进敏感 | ❌ 靠 `{}` | ❌ 靠 `{}` | ✅ 易错 | 部分 |
| 片段复用 | ✅ `@base`/`&base` | ❌ | ❌（锚点复杂） | ❌ |
| 命名空间 include | ✅ `as ns` | ❌ | ❌ | ❌ |
| 契约 / Schema | ✅ 内建 | ❌ | ❌ | ❌ |
| 环境变量注入 | ✅ `$env` | ❌ | ❌ | ❌ |

## A.2 常见错误排查

| 现象 | 原因 | 解决 |
|------|------|------|
| 字符串被截断 | 值含空格却用了裸词 | 加引号 `"..."` |
| 片段没展开 | 块内裸写 `&base` | 写成 `key: &base`（值位置） |
| 契约报"未声明字段" | 默认严格模式 | 契约名后加 `loose`，或移除多余字段 |
| `port` 超范围报错 | `min/max` 约束生效 | 改正值，或放宽区间 |
| `$env.X` 变空串 | 变量未设置（正常） | 运行时确认环境变量已导出 |
| include 找不到文件 | 路径相对被包含文件目录 | 检查相对路径；确认文件存在 |
| 循环 include 报错 | A 包含 B，B 又包含 A | 打破循环依赖 |
| `@is` 报契约未定义 | 契约在 `@is` 之后才写 | 把 `@contract` 移到 `@is` 之前 |
| 命名空间冲突 | 同 ns 重复定义 `@name`/`@contract` | 改名，或用不同 `as ns` |

## A.3 语法速查

```
键值：      key: value
裸词串：    state: NY
引号串：    name: "John Doe"
整数：      age: 27
浮点：      ratio: 0.75
布尔：      on: true
空值：      x: null
对象块：    a { b: 1 }    ≡   a: { b: 1 }
数组：      list: [ a b c ]
行注释：    #  --  //
块注释：    /* ... */     _* ... *_
片段定义：  @name { ... }
片段引用：  key: &name
契约定义：  @contract Name loose { field: type ... }
契约应用：  @is Name
include：   include "x.sml"        （内联）
            include "ui" as ui     （命名空间）
            include "a", "b" as y  （多目标，需 feature）
环境变量：  secret: $env.API_KEY
转义：      "line1\nline2 \u{2744}"
```

## A.4 版本与 feature

文件开头可声明版本：

```sml
@version v1
```

复杂能力按需开启：

```sml
@feature enable glob regex
include "widgets/*.sml"
```

默认开启 `include` + `namespace` + `implicit-ns`（极简三件套）；`multi` / `glob` / `regex` / `ext-rewrite` 默认关，需显式 opt-in。

---

全书完。回到 [教科书首页](./)。
