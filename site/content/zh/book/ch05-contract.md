---
title: "第 5 章：契约系统"
translationKey: "book-ch05"
---

# 第 5 章：契约系统

前面学的片段是"值的复用"。**契约（Contract）是"形状的约束"**——它定义"一个块应该有哪些字段、各自什么类型、是否必填、默认值多少、取值范围"，并在解析期就校验，而不是等你运行程序才发现问题。

> 适用场景：用 SML 做**应用配置**时，契约就是你的 Schema。改错字段名、漏填必填项、填了超出范围的端口号——解析时就直接报错，并告诉你精确到行列。

> **适用范围**：契约在 **Rust（参考实现）、JS、C、C++、Lua** 上都能用（Lua 的契约是近期补上的），
> 同一条契约违反在这几端报**同一个错误码**（见[错误码总表](/errors/)）。还不够格的部分会**明确报错**
> 而不是假装通过 —— 例如 Lua 遇到外置类型 `@type` 与模式类型会显式拒绝，并把原因登记在实现里。

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

### 5.2.1 类型标注形式：`@is type(契约名)`

`@is` 之后也能写成"类型标注"的形式，语义与 `@is 契约名` **完全等价**，只是把契约名放进类型括号里、突出"这是一个类型约束"：

```sml
@contract 办事人 strict {
    姓名: str
    手机: str
}
窗口一 {
    @is type(办事人)        # 与 @is 办事人 等价
    姓名: 张三
    手机: "13800138000"
}
```

两种写法校验结果一致（缺字段同样报"契约字段缺失"，默认值同样回填）。当契约恰好就叫 `type` 时，`@is type` 仍按"契约原名"解析，不会误当标注解包——即 `@is type` 永远是"应用名为 type 的契约"。

### 5.2.2 块级类型标注：`<契约名> <块名> { }`

如果觉得在块里写一行 `@is` 还不够自然，可以直接**把契约名写在块名前面**，像给变量标类型一样：

```sml
@feature enable typed-block     # 该写法需显式开启

@contract 受理人 strict {
    姓名: str
    手机: 手机号
}
@type name: 手机号 {            # type 是"值的格式"，契约是"块的形状"
    序列: [ { 字面: "1" } { 名: 后续, 类: 数字, 次: 10 } ]
}

受理人 窗口一 {                 # 契约名 + 块名 = 该块即受此契约约束
    姓名: 张三
    手机: "13800138000"
}
```

量词还支持 **`最小` / `最大`** 形式（中英等价，与 `次` 同义）：

```sml
@type name: 身份证尾号 {
    序列: [ { 名: 年, 类: 数字, 次: { 最小: 4, 最大: 4 } } ]   # 对象形式
}
@type name: 邮编 {
    序列: [ { 类: 数字, 最小: 6, 最大: 6 } ]                    # 平铺形式，与上等价
}
@type name: 短码 {
    序列: [ { 类: 数字, 最大: 2 } ]                             # 只给最大 = 0..2 次
}
```

`次: { 最小, 最大 }` 与直接把 `最小`/`最大` 平铺在元素上两种写法完全等价。只写 `最大` 表示"0 到该值"次；非法组合（如 `最小` 大于 `最大`、取负值）会在解析期**直接报错**，不会静默通过。

这与既有的裸块写法 `type [name...] { }` **完全同形**——区别只在于：当首词是一个**已定义的契约名**时，它自动被当作类型约束应用，等价于在块内首行写 `@is 契约名`。非契约名字（如 `server web { }`）行为完全不变，因此既有文档零影响。该能力是 **opt-in** 的，需要 `@feature enable typed-block`。

### 括号是普通字符

在 SML 里，`( )` 只是**普通字符**，不是语法符号。这意味着裸词值可以直接包含括号，无需引号：

```sml
备注: (重要)          # 值就是字符串 "(重要)"，不会丢
优先级: (P0) 紧急
```

契约枚举也用圆括号书写：`enum(公开, 内部, 机密)`、`enum(active, disabled)`。

> 注意：JS 引擎旧版本曾把括号当作分隔符导致值被截断（如 `(重要)` 静默变成 `null`），现已修复；Rust 引擎始终正确。用一对引擎交叉验证最稳妥。

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

