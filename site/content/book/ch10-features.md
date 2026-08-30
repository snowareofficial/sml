---
title: "第 10 章：feature 完整参考"
---

# 第 10 章：feature 完整参考

SML 的设计原则是"**从极简到丰富，功能可裁剪**"——基础七件套默认开启；复杂能力默认关闭，需要时用 `@feature enable` 显式开启。

本章是**每个 feature 的权威参考**：开启方式、语法、报错信息、与其它 feature 的关系。

## 10.1 怎么开/关 feature

文件开头用 `@feature` 指令：

```sml
@version v1
@feature enable glob regex
@feature disable ext-rewrite
```

- `enable` 名字列表：用空格分隔。
- `disable` 名字列表：同上。
- 可以多次出现，按出现顺序**叠加**生效。
- 写在 `@version` 之后、其它内容之前。

启用后，本文件内（或被 include 的子文件）即可使用对应能力；不同文件可独立声明。

## 10.2 内置 feature 清单

### `include`（**默认开**）

最基础的"插文件"能力。

| 项 | 值 |
|----|----|
| 状态 | 默认开，无法 disable |
| 语法 | `include "path.sml"` |
| 作用 | 把目标文件内容原样插入 |
| 路径基准 | 被包含文件自身所在目录 |
| 嵌套上限 | 32 层（超限报错） |
| 循环引用 | 报错（不静默） |

错误码：`include.circular`、`include.depth-exceeded`、`include.not-found`。

### `namespace`（**默认开**）

`as` 命名空间、点分路径、宏/契约隔离。

| 项 | 值 |
|----|----|
| 状态 | 默认开 |
| 语法 | `include "x" as a.b.c` |
| 作用 | 把内容包进 `a { b { c { ... } } }` 嵌套块 |
| 宏隔离 | 是——`@name` / `@contract` 也按命名空间隔离 |
| 外部引用语法 | `a.b.c.MacroName`（限定名） |
| 内部引用语法 | `MacroName`（本地名，解析器自动加前缀） |

错误码：`ns.invalid-path`、`ns.duplicate-symbol`、`ns.unresolved-prefix`。

### `implicit-ns`（**默认开**）

不带扩展名的 `include` 自动作命名空间。

| 项 | 值 |
|----|----|
| 状态 | 默认开 |
| 语法 | `include "ui"` 等价 `include "ui.sml" as ui` |
| 隐式命名空间名 | 文件名（去扩展名） |
| 显式 `as` 覆盖 | 显式 `as foo` 优先于隐式 |

> 关闭它：`@feature disable implicit-ns`——之后 `include "ui"` 必须带扩展名。

### `multi`（**默认关**）

一次 include 多个目标 + `import` 别名。

| 项 | 值 |
|----|----|
| 状态 | 默认关 |
| 开启 | `@feature enable multi` |
| 语法 | `include "a.sml", "b.sml" as y, "c"` |
| 别名形式 | `import ui.buttons, admin.panel` |
| 同名冲突 | 报错（不静默） |

错误码：`multi.dup-name`、`multi.empty`。

### `glob`（**默认关**）

`*` 通配匹配目录里多个文件。

| 项 | 值 |
|----|----|
| 状态 | 默认关 |
| 开启 | `@feature enable glob` |
| 语法 | `include "widgets/*.sml"` |
| 通配符 | `*`（一段字符，不跨 `/`）、`?`（单字符） |
| 排序 | 字典序确定，跨平台一致 |
| 隐式 `as` | 适用——`include "widgets/*"` ⇒ `as widgets` |

错误码：`glob.not-found`（若 0 匹配）、`glob.malformed`。

### `regex`（**默认关**）

`re:` / `/.../` 前缀触发正则匹配。

| 项 | 值 |
|----|----|
| 状态 | 默认关 |
| 开启 | `@feature enable regex` |
| 语法 | `include "re:^v[0-9]+\\.sml$" as versions` |
| 正则子集 | `.` `*` `+` `?` `^` `$` `[a-z]`、`\\.` `\\d` `\\w` |
| 性能 | 手写递归回溯，O(n·m) 足够文件名短串 |
| 注意 | 路径分隔符用 `\\.`（`\` 在 md 里也要双写） |

> 完整正则特性（lookahead、backref）**不支持**——刻意保持简单，复杂匹配请用 shell 通配 + glob。

### `ext-rewrite`（**默认关**）

把任意后缀的文件当 `.sml` 解析。

| 项 | 值 |
|----|----|
| 状态 | 默认关 |
| 开启 | `@feature enable ext-rewrite` |
| 语法 | `include "*.json" -> .sml`、`include "conf" -> .sml` |
| 典型用法 | 把 `.json` / `.yaml` / `.conf` 改写后用 SML 解析器处理 |
| 风险 | 错误地把二进制文件当 sml 解析会爆栈；建议加 glob 限制 |

### `contract`（**默认开**）

`@contract` / `@is` 校验与回填。

| 项 | 值 |
|----|----|
| 状态 | 默认开 |
| 语法 | `@contract Name [loose] { field: type [default v] [min n] [max n] [enum(a,b)] [?] [required] }` |
| 引用类型 | `str` `int` `num` `bool` `array[T]` `enum(...)` 或另一个契约名 |
| 严格度 | 默认严格；`loose` 允许未声明字段 |
| 嵌套 | 无限；递归契约在被引用时检测环 |
| 错误位置 | 精确到行列 |

错误码：`contract.required-missing`、`contract.type-mismatch`、`contract.enum-invalid`、`contract.out-of-range`、`contract.unknown-field`、`contract.recursive`。

### `env`（**默认开**）

`$env.VAR` 环境变量注入。

| 项 | 值 |
|----|----|
| 状态 | 默认开 |
| 语法 | `$env.VAR_NAME` |
| 缺失行为 | 替换为空串（不报错） |
| 类型 | 始终是字符串（数字也要引号） |
| 转义 | 名字里 `_` `.` `-` 可用；首字符必须是字母或 `_` |
| 嵌套 | `$env` 里不能再写 `$env` |

错误码：`env.bad-name`。

### `escape`（**默认开**）

引号字符串内的转义。

| 项 | 值 |
|----|----|
| 状态 | 默认开 |
| 支持 | `\n` `\t` `\r` `\\` `\"` `\'` `\0` `\u{XXXX}` `\uXXXX` |
| 不支持 | 八进制 `\077`、十六进制裸 `\x41`（避免歧义） |
| 作用域 | 仅引号字符串；裸词不转义 |

### `fragment`（**默认开**）

`@name` / `&name` 片段继承。

| 项 | 值 |
|----|----|
| 状态 | 默认开 |
| 语法 | 定义 `@name { ... }`、引用 `key: &name` |
| 作用域 | 跟随命名空间（`@feature enable namespace` 时） |
| 冲突 | 同一作用域重复 `@name` 报错 |

### `top-array`（**默认开**）

文件顶层是数组（不是对象）的允许。

| 项 | 值 |
|----|----|
| 状态 | 默认开 |
| 语法 | `[{...} {...} ...]`（顶层即数组） |
| 典型场景 | 配置项是有序列表（如监控规则、路由表） |

### `bareword-str`（**默认开**）

裸词自动识别为字符串。

| 项 | 值 |
|----|----|
| 状态 | 默认开 |
| 关闭 | `@feature disable bareword-str`——之后所有字符串必须引号 |
| 触发 | 当版本 `v.strict-strings()` 设定时 |

## 10.3 兼容矩阵

| feature ↓ \ → | 与 namespace | 与 multi | 与 glob | 与 regex |
|---|---|---|---|---|
| `include` | ✅ 直接组合 | ✅ | ✅ | ✅ |
| `namespace` | — | ✅ | ✅ | ✅ |
| `multi` | ✅ | — | ⚠️ 见注1 | ⚠️ |
| `glob` | ✅（隐式 `as`） | ⚠️ | — | ❌ 互斥见注2 |
| `regex` | ✅ | ⚠️ | ❌ 互斥 | — |
| `ext-rewrite` | ✅ | ✅ | ✅ | ✅ |
| `contract` | ✅（引用契约用限定名） | — | — | — |

> **注 1**：`include "a, b" as y` 这种"逗号即分隔"的语法与 `multi` 一起用，列表里单个目标不能再含 `,`（即使在引号内也不行——引号内逗号当字面量）。
>
> **注 2**：`include "re:^.*\\.sml$"` 已经覆盖所有 .sml，没必要再 `include "*.sml"`。同时用可能被解释为"glob 优先"或"regex 优先"，跨实现行为可能不同——SML 规定**显式前缀优先**：`re:` 走 regex；`*.sml` 走 glob。

## 10.4 feature 的实现层（架构小贴士）

SML 解析器按"feature bitmask"运行：

```text
FeatureSet = (include | namespace | implicit-ns | contract | env | escape
              | fragment | top-array | bareword-str
              | multi | glob | regex | ext-rewrite)
```

- 核心层（默认开）7 个 bit 默认 = 1。
- 进阶层（默认关）4 个 bit 默认 = 0。
- 解析器对未开启的 feature 直接拒绝对应语法（解析期报错，**不静默跳过**）。

这就是"功能可裁剪、不要步入 YAML 覆辙"的实现基础。

## 10.5 性能与可移植性

| feature | 对解析时间影响 | 跨平台差异 |
|---------|----------------|------------|
| `include` | O(总文件大小) | 路径分隔符归一化（`/` ↔ `\`） |
| `namespace` | 极小（路径编译一次） | 无 |
| `multi` | 线性叠加 | 无 |
| `glob` | O(n) 文件枚举 | 隐藏文件（`.foo`）默认**不**包含 |
| `regex` | O(n·m) 短串 | 无 |
| `ext-rewrite` | 视被改写文件大小 | 内容编码假设 UTF-8 |
| `contract` | 与字段数线性 | 递归契约需 memoize |

## 10.6 未来 feature（roadmap）

以下**尚未实现**，仅作路线图预告，避免读者误用：

- `@import once`（去重 include，避免同文件被含多次）
- `feature from "another.sml"`（从另一文件继承 feature 设置）
- `with contract=loose`（块级放宽，覆盖文件级设置）
- `?` 三元简写（`a ? b : c` 在值位置的语义）——目前 `?` 是 optional 标记

## 10.7 动手试一试

为你的项目写一份 `features.sml`：

```sml
@version v1
@feature enable glob multi
@feature disable ext-rewrite

include "modules/*.sml" as modules
import modules.auth, modules.billing
```

用 `sml check features.sml` 之类的命令（参考 [ch07 多语言](/book/ch07-languages)）跑通解析与契约校验。

→ [附录：对照与排查](/book/appendix)

## 动手练习

读完本章，在下面的编辑器里**直接修改 SML 并点“运行”**，立刻看到解析结果或校验错误——有输出才能高效学习。

{{< sml-playground "ch10" >}}

{{< sml-quiz "ch10" >}}

