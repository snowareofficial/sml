---
title: "SML { ❄ } — SNOWARE Markup Language"
translationKey: "zh-home"
---

# SML { ❄ }

**SML（SNOWARE Markup Language）** 是一种声明式数据 / 配置格式，定位为 JSON / YAML / TOML 的轻量替代品。它强调**可读性**与**少仪式感**：引号可选、块冒号可省、逗号可选、支持片段继承与契约校验。

> 仓库：[snoware/sml](https://gitee.com/snoware/sml) ｜ **参考实现：Rust (`swsml`)** ｜ 实验性实现（暂不保证）：C (`sml.c`) · JavaScript (`sml.mjs`) · Lua (`lib/sml.soup`) · C++ · Python

## 特性一览

- **引号可选**：裸词即字符串；含空格 / 特殊字符才需引号
- **块冒号可省**：`address { }` ≡ `address: { }`
- **数组分隔灵活**：逗号可选 `[ a b c ]` ≡ `[ a, b, c ]`
- **片段继承**：`@base { }` 定义、`&base` 引用，实现配置复用
- **include 内联 / 命名空间**：`include "x.sml"` 递归展开；`include "x.sml" as a.b` 以点分路径隔离进独立作用域（含宏 / 契约），冲突即报错
- **环境变量**：`$env.HOME` 在解析期内联
- **契约系统**：`@contract` / `@is` 对配置做类型与结构校验（严格 / 宽松两种模式）
- **多目标转译**：解析为 `Value` 后可编译为 Markdown / LaTeX / XML / SVG / Slint UI / 自定义格式（emit 后端）
- **零依赖**：各实现互不耦合，可单独嵌入（WASM / 沙箱 / 编辑器）

## 📖 SML 教科书

从零开始、循序渐进，也能当工具书随时查。→ **[在线阅读](/book/)**

- [序章：为什么是 SML](/book/intro)
- [第 1 章：第一个 SML 文件](/book/ch01-basics) · [第 2 章：块与嵌套](/book/ch02-blocks) · [第 3 章：片段继承](/book/ch03-fragments)
- [第 4 章：include 与命名空间](/book/ch04-include) · [第 5 章：契约系统](/book/ch05-contract) · [第 6 章：环境变量与转义](/book/ch06-env-escape)
- [第 7 章：多语言使用](/book/ch07-languages) · [第 8 章：实战项目](/book/ch08-project) · [第 9 章：功能组合与设计模式](/book/ch09-advanced) · [第 10 章：feature 完整参考](/book/ch10-features) · [第 11 章：实战翻译挑战](/book/ch11-challenges) · [附录：对照与排查](/book/appendix)
- 离线版：**[下载 EPUB](/sml-book.epub)**

## 多语言实现对照

> **实现状态说明（重要）**
>
> - **Rust（`rust/`，crate `swsml`）是 SML 的参考实现**：语法、契约系统、emit 后端、测试与安全扫描（OSV 依赖漏洞扫描 + 完整回归测试）均以它为准，也是目前**唯一持续维护并做版本保证**的实现。生产使用请选择 Rust。
> - **Rust 以外的实现（C / JavaScript / Lua / C++ / Python）目前标记为「实验性（experimental）」**：随仓库一并提供，可运行、可嵌入，但**暂不保证**与 Rust 行为完全一致、暂不保证 API 稳定、暂不纳入例行的漏洞扫描与回归测试。请自行评估后使用，发现问题欢迎提 issue。

| 语言 | 仓库 / 文件 | 状态 |
|------|------------|------|
| Rust | `rust/` (`swsml`) | ✅ **参考实现 · 推荐**（契约系统完整，serde 桥接，例行漏洞扫描） |
| C | `c/sml.c` | ⚠️ 实验性（暂不保证，契约系统曾与 Rust 对齐，后续变更不承诺同步） |
| JavaScript | `js/sml.mjs` | ⚠️ 实验性（暂不保证，零依赖 ESM，浏览器 / Node 通用，含契约与 playground） |
| Lua | `lua/lib/sml.lua` | ⚠️ 实验性（暂不保证，Soup 生态 `lib/sml.soup` 同源） |
| C++ | `cpp/` | ⚠️ 实验性（暂不保证） |
| Python | `rust/` 外另见 `py` 绑定 | ⚠️ 实验性（暂不保证） |

> 契约系统的规范与判定以 **Rust 实现**为准；C / JavaScript / C++ 曾做过对齐验证，但因非 Rust 实现已进入「暂不保证」状态，跨端一致性不再作为版本承诺。

## 快速使用

```sml
# 基础键值
firstName: John
age: 27
address:
{
    streetAddress: "21 2nd Street"
    state: NY
}

# 数组（逗号可选）
phoneNumbers: [ { type: home } { type: office } ]

# 片段继承（&name 是值引用，写作 key: &name）
@base { region: cn-north-1 }
region: &base
```

### Rust

```rust
use sml::parse;
let v = parse("name: John\nage: 27").unwrap();
assert_eq!(v["name"], "John");
```

### C

```c
#include "sml.h"
char err[256] = {0};
sml_value *v = sml_parse("name: John\nage: 27", err, sizeof(err));
/* v->type == SML_STR ("John") ... 用 sml_free(v) 释放 */
```

### JavaScript

```js
import { parse, stringify } from "./sml.mjs";
const v = parse('name: John\nage: 27');
console.log(stringify(v));
```

### C++

```cpp
#include "sml.hpp"
sml::Value v = sml::parse("name: John\nage: 27");
// v["name"].as_str() == "John" ｜ v["age"].as_int() == 27
// 解析失败时抛 sml::ParseError（含行列位置）
```

> C++ 实现 `cpp/` 为头文件 + 单编译单元（`sml.cpp`），零第三方依赖；`run_tests.py` 跑 `test_comments.cpp` / `test_contracts.cpp` 两套契约与注释测试。

## 版本演进（`@version`）

SML 通过 `@version` 声明文档遵循的语法版本，使解析器在将来引入不兼容语法时仍能正确读取旧文档。当前实现支持 **v1 / v2 / v3**（`@version` 接受 `v1`/`1`、`v2`/`2`、`v3`/`3`，超出 `v1..v3` 范围会直接报错），最新基线为 **v3**。

| 版本 | 语义 | 字符串写法 |
|------|------|-----------|
| **v1**（默认） | 初始公开版。裸词即字符串，类型自动识别 | `name: John` ✅ |
| **v2** | 草案版，引入「字符串必须显式引号」的不兼容语法 | `name: "John"` 必引号 |
| **v3**（CURRENT） | 正式版，与 v2 同语义。自由文本必须写作 `"..."` | `name: "John"` 必引号 |

> v2 与 v3 在字符串处理上**语义一致**，v2 为草案代号、v3 为正式代号。数字 / `bool` / `null` / 片段引用 `&x` / 环境变量 `$env.X` 在 v2 / v3 下**仍为裸词**——只有自由字符串需要引号。

```sml
# 默认 v1：裸词即字符串
name: John
age: 27
tags: [ a b c ]          # 数组裸词元素合法

# 显式 v3：字符串必须引号，标量仍裸词
@version v3
name: "John"
age: 27
active: true
tags: [ "a" "b" "c" ]    # 数组元素也必须引号
ref: &frag               # 片段引用仍是裸词
```

未定义片段引用在 v3 下**直接报错**（不再静默降级为字符串）。调用方还可用 `parse_allowed(docs, &[Version::V1, Version::V2, Version::V3])` 限制接受版本范围——超出范围的文档会被拒绝，防止 `@version` 成为绕过能力限制的后门。

## 契约系统（Contract）

契约是 SML 的「配置 Schema」：定义一组字段的类型、是否必填、默认值、取值范围，并在 `@is` 应用处做校验。非常适合「用 SML 做应用配置」的场景。

### 定义契约

```sml
@contract ResenderConfig loose {
    api_key:     str                # 必填字符串
    port:        int  default 8080 min 1 max 65535
    debug:       bool default false
    mode:        enum(active, disabled) default active
    tags:        array[str] ?      # 可选字符串数组
}
```

字段修饰符：

| 修饰符 | 含义 |
|--------|------|
| `str` / `int` / `num` / `bool` | 字段类型 |
| `enum(a, b, c)` | 枚举，取值须在其中 |
| `array[T]` | 数组，元素类型 `T`（如 `array[int]`、`array[str]`） |
| `?` 或 `optional` | 可选字段 |
| `required` | 显式必填（默认即必填） |
| `default <值>` | 缺失时填入该默认值（并自动视为可选） |
| `min <数>` / `max <数>` | 数值取值范围（含端点） |

### 应用契约

两种写法：

```sml
# 写法一：匿名块（顶层直接 @is）
@contract Cfg loose { api_key: str port: int default 8080 }
@is Cfg
api_key: re_abc
port: 8080
```

```sml
# 写法二：字段级 / 块级 @is
server prod {
    @is Cfg
    api_key: re_prod
    port: 9090
}
```

### 严格 vs 宽松

- **`loose`**：允许出现契约未声明的字段（容错，适合演进中的配置）
- **`strict`**：禁止任何未声明字段，否则校验失败

### 组合契约（递归引用）

```sml
@contract Endpoint { host: str port: int }
@contract Service {
    name:  str
    main:  Endpoint          # 引用另一个契约
    peers: array[Endpoint]   # 契约数组
}
@is Service
name: gateway
main: { host: localhost port: 8080 }
peers: [ { host: a port: 1 } { host: b port: 2 } ]
```

> 契约校验失败会返回带位置的精确错误（如 `contract: Service — 字段 main.port 大于最大值 65535`），方便编辑器 / CLI 直接定位。

## 进阶特性

### include 与命名空间（可裁剪设计）

> 设计哲学：**从极简到丰富，功能可裁剪**——基础能力默认开启，复杂能力（多目标 / 通配 / 正则 / 扩展名重写）必须显式 `@feature enable` 才生效，避免重蹈 YAML 过度复杂的覆辙。

**基础形态**

```sml
# 带扩展名 ⇒ 普通内联（内容直接并入当前作用域）
include "common.sml"
app: myapp

# 不带扩展名 ⇒ 默认命名空间 = 文件名（零样板隔离）
include "ui"          # 等价于 include "ui.sml" as ui
title: ui.title       # 用前缀访问

# 显式指定命名空间（覆盖默认）
include "ui.sml" as ui.form.widgets
```

规则很干净：**带扩展名 = 内联；不带扩展名 = 命名空间（以文件名为 ns）**。显式 `as` 始终优先。

**点分路径（嵌套命名空间）**

`ns` 支持 `a.b.c` 形式，等价于 Rust 的模块路径，展开为嵌套块 `a { b { c { ... } } }`。

**宏与契约也随命名空间隔离（2b）**

命名空间不止隔离数据键值，还隔离**宏与契约定义**：被包含文件里定义的 `@contract` / `@name` / `@base` 必须以 `ns.` 前缀对外引用，调用方才能找到：

```sml
# widgets.sml 内部
@contract Button { label: str }
@name primary = { label: "OK" }

# 主文件引用时必须带前缀
@is ui.form.widgets.Button
button: &ui.form.widgets.primary
```

> 被包含文件**内部**对自身宏的自引用仍按本地名解析（无需前缀），只有**对外暴露**才需要 `ns.` 前缀——解析器按"当前命名空间栈"自动给宏注册表加前缀。

**冲突即报错（不静默）**

命名空间是**独占作用域**，绝不静默覆盖：

- 宏/契约在 `as ns` 后注册为 `ns.Name`，调用方必须带前缀引用；同一命名空间内重复定义同名契约/片段 → 报错。
- 缺失文件 / 循环引用 / 超过 32 层嵌套 → 报错。
- 主文件顶层键与 `include "x" as ns` 的命名空间名冲突时，解析层应报错（Rust 实现当前以「同名键冲突提升为数组」兜底，严格冲突报错为后续版本）。

**多目标与 `import` 别名**

逗号分隔一次包含多个目标；`import` 是 `include` 的等价写法（语义一致）：

```sml
include "a.sml", "b.sml" as y, "c"          # 多目标，各自可有 as
import ui.buttons, admin.panel              # import 别名写法
```

**通配与正则（需 feature 开启）**

```sml
@feature enable glob
include "widgets/*.sml"        # glob 通配，按文件名排序逐个包含

@feature enable regex
include /plugins/.*\.sml/      # 正则匹配（/.../ 定界或 re: 前缀）
```

**零拷贝切片（性能）**

`include` 不会把文件内容深拷贝拼接成一大段文本。每个被包含文件读入后只持有其字符串切片，解析器消费一段「切片流」：遇到 `include "x" as a.b` 时插入零拷贝的开块字面量 `a { b {`、接入 x 的切片、再插入闭块 `} }`。文件内容只被解析一次，无中间大字符串，内存占用 = 各文件切片之和。

**Feature 分层（可裁剪）**

| 层 | feature | 能力 | 默认 |
|----|---------|------|------|
| 0 | `include` | 基础 `include "x.sml"` 内联（带扩展名才内联） | 开 |
| 1 | `namespace` | `as ns` + 点分路径 + 宏/契约隔离 + 冲突报错 | 开 |
| 1 | `implicit-ns` | 无扩展名 `include "foo"` ⇒ `as foo` 默认命名空间 | 开 |
| 2 | `multi-include` | 逗号多目标 `a, b, c` 与 `import` 别名 | 关 |
| 2 | `glob-include` | `*` 通配 `dir/*.sml` | 关 |
| 3 | `regex-include` | `re:` / `/.../` 正则匹配 | 关 |
| 3 | `ext-rewrite` | `-> .sml` 把非 sml 文件当 sml 解析 | 关 |

默认仅开启 `include` + `namespace` + `implicit-ns`（极简三件套）。复杂能力需 `@feature enable` 显式 opt-in。

**跨语言一致**

Rust / C / JavaScript / Lua 四端共用同一语义：点分路径、宏/契约隔离、冲突报错、切片式零拷贝、feature 分层裁剪。

> 注：以上为**规范层面的目标**；当前**仅 Rust 实现做保证**，C / JavaScript / Lua 属实验性实现，暂不保证已完整跟进（feature 分层裁剪等以 Rust 为准）。

### \u 转义

字符串支持 `\u{XXXX}` 与 `\uXXXX` Unicode 转义，解析期转为 UTF-8：

```sml
label: "雪花 \u{2744} snow"
```

### JSON 双向桥接

SML 与 JSON 同构（均为树状键值 / 数组），可无损互转：

```js
import { parse, stringify } from "./sml.mjs";
// SML -> JSON
const obj = parse(smlText);            // 普通 JS 对象
const json = JSON.stringify(obj);
// JSON -> SML
const sml = stringify(JSON.parse(json));
```

Rust 侧通过 `serde` feature 提供 `sml::serde::from_str` / `to_string`，任意 `#[derive(Deserialize)]` 结构体都能一键反序列化。

### 多目标转译（emit）

SML 不只是配置格式——解析为 `Value` 后，可经由内置后端**编译 / 转译为其它宿主格式**，把同一份数据喂给不同生态：

| feature | 目标 | 入口函数 |
|---------|------|----------|
| `emit-markdown` | Markdown / GFM | `to_markdown` |
| `emit-latex` | LaTeX 文档 | `to_latex` |
| `emit-xml` | XML / LVGL UI | `to_xml` / `to_lvgl` |
| `emit-svg` | SVG 图形 | `to_svg` |
| `emit-slint` | Slint DSL（Rust/Qt GUI） | `to_slint` |
| `emit-custom` | 用户自定义 SML 模板生成器 | `to_custom` |

默认全部开启；若只需解析 / 序列化回 SML，可 `default-features = false` 关掉所有 `emit-*`，本模块整体不参与编译。

通用约定：对象 / 块通常映射为宿主的「容器 / 元素 / 环境」，数组映射为「列表 / 序列」，字符串标量默认**自动转义**防止注入宿主保留字（如 XML 的 `<`、`&`）。裸块元数据 `__type` / `__name` 被后端用来选择语义，而非当作普通字段输出。

```rust
use sml::{parse, emit::to_markdown, emit::MarkdownOptions};

let v = parse("# 标题\nbody: 内容\nitems: [ a b c ]").unwrap();
// SML -> Markdown
let md = to_markdown(&v, &MarkdownOptions::new()).unwrap();
// SML -> Slint GUI 描述
use sml::emit::{to_slint, SlintOptions};
let slint = to_slint(&v, &SlintOptions::new()).unwrap();
```

> 转译后端对不可信输入做了递归深度上限（`MAX_VALUE_DEPTH = 128`）保护：超深嵌套会返回 `Err` 而非栈溢出崩溃，避免 DoS。

## 落地应用

SML 已被多个真实项目采用：

- **BamZap**（包管理器）：`bamzap.sml` 仓库清单使用 SML 描述依赖、源、构建脚本
- **soupmake**（Soup 构建系统）：用 SML 描述构件与依赖图
- **resender**（Resend 发信工具）：用 SML 的契约系统做 `AppConfig` 持久化

### resender 中的使用

[resender](https://gitee.com/snoware/resender) 是一个 Rhai 驱动 + Slint GUI 的 Resend 邮件发送工具，它的配置文件 `resender.sml` 直接用 SML **契约**保证结构正确：

```sml
@contract ResenderConfig loose {
    api_key:    str
    from:       str
    to:         array[str]
    subject:    str default "Hello"
    port:       int default 465  min 1 max 65535
    tls:        bool default true
}

@is ResenderConfig
api_key: re_xxxxxx
from: me@example.com
to: [ alice@example.com bob@example.com ]
subject: Weekly Report
port: 465
tls: true
```

其 Rust 端 `src/config.rs` 维护 `CONFIG_CONTRACT` 常量（即上面的契约文本），保存时把当前配置序列化回 SML 并自动附上 `@is ResenderConfig`，读取时再校验——这正是「契约即 Schema」的典型用法。

## 完整示例

一个接近真实的部署配置范例：

```sml
@version v1

@contract Service loose {
    name:    str
    port:    int  default 8080 min 1 max 65535
    debug:   bool default false
    peers:   array[str] ?
}

@base {
    region: cn-north-1
    timeout: 30
}

# 顶层应用契约
@is Service
name: gateway
port: 9090
debug: true
peers: [ auth billing ]

# 片段继承复用
service auth { &base port: 7100 name: auth-svc }
service billing { &base port: 7200 name: billing-svc }

# 嵌套块 + 数组
database: {
    url: "postgres://localhost:5432/app"
    pool: { min: 2 max: 16 }
}
features: [ logging metrics tracing ]
```

## 本地 Playground

不想装任何东西？直接在浏览器里试：**[SML Playground →](/playground/)**

左侧写 SML（含契约），右侧实时显示解析结果或精确错误位置。
