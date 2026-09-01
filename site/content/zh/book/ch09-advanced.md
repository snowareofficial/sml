---
title: "第 9 章：进阶——功能组合与设计模式"
translationKey: "book-ch09"
---

# 第 9 章：进阶——功能组合与设计模式

前 8 章分别学了键值、块、片段、include、契约、环境变量、多语言集成。本章把它们**组合**起来——真实的 SML 配置很少只用一项能力。

> 学完这一章，你会知道：一个生产级 SML 配置库应该长什么样、为什么这样写、怎么取舍。

## 9.1 一张全景图

先俯瞰 SML 的"能力栈"：

| 层 | 能力 | 解决什么 | 默认 |
|----|------|----------|------|
| 数据 | 键值、块、数组、标量 | 描述是什么 | 开 |
| 复用 | 片段 `@name` / `&name` | 值的复制 | 开 |
| 拆分 | `include` | 跨文件复用 | 开 |
| 隔离 | `as ns`（点分路径） | 命名空间 | 开 |
| 默认 | 无扩展名 ⇒ `as foo` | 隐式命名空间 | 开 |
| 批量化 | 多目标 `,`、import 别名 | 一次含多 | 关 |
| 匹配 | `*` 通配 glob | 文件名匹配 | 关 |
| 匹配 | `re:` / `/.../` 正则 | 复杂文件名 | 关 |
| 改写 | `*->*.sml` | 把任意后缀当 sml | 关 |
| 约束 | `@contract` / `@is` | 形状校验 | 开 |
| 注入 | `$env.VAR` | 环境变量 | 开 |
| 转义 | `\n` `\u{XXXX}` | 字符串转义 | 开 |
| 多语言 | Rust/C/JS/Lua 解析器 | 跨生态 | — |

**默认开**的九件套让你"开箱即用"；**默认关**的六件按需 opt-in，不重蹈 YAML 复杂化覆辙。

## 9.2 模式 1：模块化配置库

把"通用配置片段 + 契约"抽成共享库，业务项目 `include` 进来。

### 库文件（`sml-lib/net.sml`）

```sml
@contract Service {
    name:   str
    port:   int  min 1 max 65535
    region: str
}

@default-http {
    port: 80
    region: cn-north-1
}

@default-https {
    port: 443
    region: cn-north-1
}
```

### 业务项目（`app.sml`）

```sml
@version v1

# 整文件作命名空间
include "sml-lib/net" as net

gateway {
    @is net.Service
    &net.default-http
    name: api-gw
}

admin {
    @is net.Service
    &net.default-https
    name: admin-panel
    port: 8443
}
```

要点：
- `include "sml-lib/net"` 不带扩展名 ⇒ 隐式 `as net`（等价于显式 `as net`）。
- 命名空间隔离了库的 `@contract` 与 `@name`，业务里用 `net.Service` / `&net.default-http` 引用。
- 库可以独立版本化发布（甚至在 git submodule / 包管理里）。
- 同一个库被多个项目 include 时，宏/契约名字**不冲突**——因为都装在不同命名空间下。

## 9.3 模式 2：可裁剪 schema（feature flag）

契约字段也可以按需裁剪——用 `optional` / `default` 让同一个契约在不同环境有不同的"必填集"。

```sml
@contract Database strict {
    host:     str
    port:     int  default 5432 min 1 max 65535
    user:     str  default app
    password: str  ?               # 可选：本地/测试用空
    sslmode:  enum(disable, allow, require) default require
}
```

- 生产：填全部字段，`sslmode: require` 必给。
- 开发：可不填 `password`（用空连接 + 信任认证），`sslmode: disable`。
- CI：契约把**未声明的字段**当错误——所以测试环境塞的 mock 字段会被立刻发现。

> 一份契约，多个 profile。靠"optional + default + 严格模式"三件套，不需引入多个契约。

## 9.4 模式 3：按环境生成（env overlay）

同一份基础配置，在不同环境叠加不同片段——典型三段式 dev / staging / prod：

```sml
# base.sml
@base {
    region: cn-north-1
    timeout: 30
    log_level: info
}

service api { cfg: &base port: 8080 name: api }

# env/dev.sml
include "base" as cfg
service api { &cfg.base }
# 覆盖 log_level 不必写完整路径：可直接追加
service api { log_level: debug }
```

> 注意：SML 暂时不内置 "merge by name" 的复杂合并语义；上述写法是**靠契约 + include 命名空间**手写覆盖。更复杂的 merge 推荐在 Rust/JS 侧用 `sml-merge` 之类的库做（参考 https://github.com/snoware/sml-merge）。

## 9.5 模式 4：include + 契约 + $env 三件套

这是最常见也最稳的生产用法：

```sml
# common.sml
@contract Service {
    name: str
    port: int  default 8080
    debug: bool default false
}

# app.sml
@version v1

include "common" as cfg

gateway {
    @is cfg.Service
    name: gateway
    port: 9090
    debug: $env.DEBUG
}

secrets {
    api_key: $env.API_KEY
    db_password: $env.DB_PASSWORD
    webhook: $env.OPTIONAL_WEBHOOK   # 未设 -> 空串
}
```

为什么稳：
- **契约**保证结构。
- **include 命名空间**让契约不污染主文件作用域。
- **$env**让敏感值不入库。
- 缺点（开发态）也明显：`$env` 解析期才求值，IDE 调试时看不到值——解决办法是用 `sml-playground` 或自己写 mock env loader。

## 9.6 模式 5：协议级契约（"前后端共享 schema"）

SML 契约是**纯声明**，不绑语言。同一份契约可同时约束：

- 配置文件（`.sml`）
- 后端 Go/Rust 代码生成的类型
- 前端 TS 类型（自动生成）
- API 请求/响应校验

```sml
@contract User {
    id:    str
    email: str
    role:  enum(user, admin, owner) default user
    age:   int  min 0 max 150 ?
}
```

后端 Rust：

```rust
// 由 swsml-derive 宏从 .sml 自动派生
#[derive(FromSmlContract)]
struct User { id: String, email: String, role: Role, age: Option<u8> }
```

前端 TS：

```ts
// 由 swml-ts-codegen 从 .sml 自动生成
type User = { id: string; email: string; role: 'user'|'admin'|'owner'; age?: number };
```

> SML 本身就是"协议级"——一份 schema、四端共享、行为一致。

## 9.7 模式 6：契约组合 + 递归

用 `array[契约名]` 表达"契约数组"——适合列表型数据：

```sml
@contract Endpoint { host: str port: int }
@contract Service {
    name:  str
    main:  Endpoint                # 单个
    peers: array[Endpoint]         # 列表
    back:  Endpoint?               # 可选
}

@is Service
name: gateway
main:  { host: a port: 1 }
peers: [ { host: b port: 2 } { host: c port: 3 } ]
```

递归层数不限（解析期检测环引用；环引用 = 错误）。

## 9.8 模式 7：glob + 契约（"所有模块统一 schema"）

> 需要 `@feature enable glob` 开启。

```sml
@feature enable glob

@contract Module {
    name:  str
    entry: str
    deps:  array[str] ?
}

# 把 modules/ 下所有 .sml 文件作为子模块加载，并各自校验
include "modules/*.sml" as modules

# 之后可用 modules.auth / modules.billing 访问
gateway {
    primary: modules.auth
    fallback: modules.billing
}
```

适合**插件系统**：每个插件是独立 .sml 文件，统一契约校验，统一命名空间访问。

## 9.9 模式 8：re: 正则做"按命名规则"批处理

> 需要 `@feature enable regex` 开启。

```sml
@feature enable regex

# 加载所有 v 开头的 .sml（如 v1.sml / v2.sml）
include "re:^v[0-9]+\\.sml$" as versions

# 加载所有 .json 改写为 .sml 解析
include "configs/re:.*\\.json$" -> .sml
```

`re:` 前缀表明后面是正则；正则语法是**手写子集**（`. * + ? ^ $ [a-z]` 即可满足 90% 场景，避免引入完整 regex 引擎）。

## 9.10 取舍原则（什么时候用什么）

- **结构稳定、字段变化多** → 契约 + 默认值 + enum。
- **重复字段多、组合式配置** → 片段 + include。
- **多团队共享一套 schema** → 模块化库（include + 命名空间）。
- **同一份配置多个环境** → 契约为骨架，$env 注入差异。
- **插件式扩展** → glob + 命名空间。
- **复杂文件名匹配** → re: 正则。
- **混合来源**（`.json` / `.yaml` 当 `.sml` 用） → ext-rewrite `-> .sml`。

## 9.11 反模式（请避免）

- **一个巨大契约覆盖所有场景**——必填字段太多，团队成员吐槽。
- **命名空间嵌套超过 3 层**（`a.b.c.d`）——可读性下降，宁可拆文件。
- **契约里塞可执行逻辑**——SML 契约是声明，不是脚本；要算就用宿主语言。
- **`$env` 用作"动态配置源"**——它只在**解析期**求值，运行时变化无效；运行时配置请用代码。
- **include 嵌套超过 5 层**——会拖慢阅读；该抽库了。

## 9.12 动手试一试

把第 8 章的"完整项目"按本节"模式 5（include + 契约 + $env）"重构一遍：

1. 把 `common.sml` 抽成共享契约 + 共享片段。
2. `app.sml` 改成 `include "common" as cfg`。
3. 密钥全部用 `$env.*`。
4. 用 `cargo test`（或对应语言的契约测试）跑通解析与校验。

→ [第 10 章：feature 完整参考](/book/ch10-features)

## 动手练习

读完本章，在下面的编辑器里**直接修改 SML 并点“运行”**，立刻看到解析结果或校验错误——有输出才能高效学习。

{{< sml-playground "ch09" >}}

{{< sml-quiz "ch09" >}}

