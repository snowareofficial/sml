---
title: "SML { ❄ } — SNOWARE Markup Language"
---

# SML { ❄ }

**SML（SNOWARE Markup Language）** 是一种声明式数据 / 配置格式，定位为 JSON / YAML / TOML 的轻量替代品。它强调**可读性**与**少仪式感**：引号可选、块冒号可省、逗号可选、支持片段继承与契约校验。

> 仓库：[snoware/sml](https://gitee.com/snoware/sml) ｜ 多语言实现：Rust (`swsml`) · C (`sml.c`) · JavaScript (`sml.mjs`) · Lua (`lib/sml.soup`) · C++ · Python

## 特性一览

- **引号可选**：裸词即字符串；含空格 / 特殊字符才需引号
- **块冒号可省**：`address { }` ≡ `address: { }`
- **数组分隔灵活**：逗号可选 `[ a b c ]` ≡ `[ a, b, c ]`
- **片段继承**：`@base { }` 定义、`&base` 引用，实现配置复用
- **include 内联**：`include "common.sml"` 递归展开外部文件
- **环境变量**：`$env.HOME` 在解析期内联
- **契约系统**：`@contract` / `@is` 对配置做类型与结构校验（严格 / 宽松两种模式）
- **零依赖**：各实现互不耦合，可单独嵌入（WASM / 沙箱 / 编辑器）

## 多语言实现对照

| 语言 | 仓库 / 文件 | 状态 |
|------|------------|------|
| Rust | `rust/` (`swsml`) | ✅ 可用（契约系统完整，serde 桥接） |
| C | `c/sml.c` | ✅ 可用（契约系统已与 Rust 100% 对齐） |
| JavaScript | `js/sml.mjs` | ✅ 可用（零依赖 ESM，浏览器 / Node 通用，含契约与 playground） |
| Lua | `lua/lib/sml.lua` | ✅ 可用（Soup 生态 `lib/sml.soup` 同源） |
| C++ | `cpp/` | ✅ 可用 |
| Python | `rust/` 外另见 `py` 绑定 | ✅ 可用 |

> 契约系统已在 **Rust / C / JavaScript / C++** 四端对齐：同一份 `CONFIG_CONTRACT` 定义，四端解析行为一致。

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

# 片段继承
@base { region: cn-north-1 }
server web { &base port: 8080 }
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

### include 文本内联

```sml
# main.sml
include "common.sml"      # 递归展开，防环、深度 ≤ 32
app: myapp
```

`common.sml` 的内容会被内联到 `include` 所在位置。也可写 `@include "x.sml"`。

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
