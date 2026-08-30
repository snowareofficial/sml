---
title: "第 5 章：契约系统"
translationKey: "book-ch05"
---

# 第 5 章：契约系统

前面学的片段是"值的复用"。**契约（Contract）是"形状的约束"**——它定义"一个块应该有哪些字段、各自什么类型、是否必填、默认值多少、取值范围"，并在解析期就校验，而不是等你运行程序才发现问题。

> 适用场景：用 SML 做**应用配置**时，契约就是你的 Schema。改错字段名、漏填必填项、填了超出范围的端口号——解析时就直接报错，并告诉你精确到行列。

## 5.1 定义契约：`@contract`

```sml
@contract ResenderConfig loose {
    api_key:     str                # 必填字符串
    port:        int  default 8080 min 1 max 65535
    debug:       bool default false
    mode:        enum(active, disabled) default active
    tags:        array[str] ?       # 可选字符串数组
}
```

字段修饰符一览：

| 修饰符 | 含义 |
|--------|------|
| `str` / `int` / `num` / `bool` | 字段类型 |
| `enum(a, b, c)` | 枚举，取值须在其中之一 |
| `array[T]` | 数组，元素类型为 `T`（如 `array[int]`、`array[str]`） |
| `?` 或 `optional` | 可选字段 |
| `required` | 显式必填（默认即必填，可不写） |
| `default <值>` | 缺失时填入默认值（同时自动视为可选） |
| `min <数>` / `max <数>` | 数值取值范围（含端点） |

## 5.2 应用契约：`@is`

两种写法：

```sml
# 写法一：匿名块顶层直接 @is
@contract Cfg loose { api_key: str port: int default 8080 }
@is Cfg
api_key: re_abc
port: 8080
```

```sml
# 写法二：块级 @is
server prod {
    @is Cfg
    api_key: re_prod
    port: 9090
}
```

校验发生在**解析期**：违反契约直接返回带位置的精确错误，例如 `contract: Service — 字段 main.port 大于最大值 65535`。

## 5.3 严格 vs 宽松

- **默认严格**（契约名后什么都不写）：禁止任何未声明字段，拼写错 `prot` 立即被发现。
- **`loose`**：允许出现契约未声明的字段，适合演进中的配置。

```sml
@contract Metrics loose { latency: num min 0 }
```

`loose` 只放宽"未声明字段"，已声明字段照样校验类型 / 区间 / 必填。

## 5.4 组合契约（递归引用）

契约之间不共享字段，而是"字段的类型是另一个契约"——直接填契约名即可，不引入新语法：

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

嵌套块会**递归校验并回填默认值**。被引用契约允许在 `@is` 之后才定义（引用在 `@is` 时才解析）。

## 5.5 真实范例：resender 邮件工具

[resender](https://gitee.com/snoware/resender) 用 SML 契约做 `AppConfig` 持久化：

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

其 Rust 端维护 `CONFIG_CONTRACT` 常量，保存时把配置序列化回 SML 并自动附上 `@is ResenderConfig`，读取时再校验——"契约即 Schema" 的典型用法。

## 5.6 动手试一试

给你的游戏服务器集群（第 3 章）加契约：

```sml
@contract Server strict {
    name: str
    port: int min 1024 max 65535
    region: str
}

@common {
    region: ap-east-1
    max_players: 64
}

lobby {
    @is Server
    &common
    port: 25565
    name: 大厅
}
```

试着把 `port: 80`（小于 1024）写进去，看解析器是否报错。

→ [第 6 章：环境变量与转义](/book/ch06-env-escape)

## 动手练习

读完本章，在下面的编辑器里**直接修改 SML 并点“运行”**，立刻看到解析结果或校验错误——有输出才能高效学习。

{{< sml-playground "ch05" >}}

{{< sml-quiz "ch05" >}}

