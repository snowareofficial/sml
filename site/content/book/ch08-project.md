---
title: "第 8 章：实战——完整项目配置"
---

# 第 8 章：实战：完整项目配置

把所有学到的拼起来：一个接近真实的部署配置，覆盖契约、片段、include、环境变量、嵌套块、数组。

## 8.1 拆文件

`common.sml`（公共片段 + 契约）：

```sml
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
```

`app.sml`（主配置）：

```sml
@version v1

include "common.sml"

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
    url: $env.DATABASE_URL
    pool: { min: 2 max: 16 }
}
features: [ logging metrics tracing ]
```

解析后你会得到一棵完整、结构正确、且经过契约校验的树：
- 顶层 `name="gateway"`、`port=9090`、`debug=true`、`peers` 是数组。
- `service.auth` 自动带上 `region=cn-north-1`、`timeout=30`（来自 `&base`）。
- `database.url` 来自环境变量，`database.pool` 是嵌套块。

## 8.2 渐进增强：命名空间隔离

当模块变多，把每个模块放独立文件并用命名空间：

```sml
include "auth.sml" as modules.auth
include "billing.sml" as modules.billing

gateway {
    auth: modules.auth.config
    billing: modules.billing.config
}
```

这样 `auth.sml` 内部随便定义 `@contract` / `@name`，都不会污染主文件作用域。

## 8.3 校验心智模型

写 SML 配置时，按这个顺序想：

1. **结构**：用块 `{}` 和数组 `[]` 把数据分层。
2. **复用**：重复的字段抽成 `@name` 片段，用 `&name` 引用。
3. **拆分**：文件太大就 `include`；要隔离就 `as ns`。
4. **约束**：配置是给程序读的，用 `@contract` + `@is` 锁住结构，把错误消灭在解析期。
5. **解耦**：敏感 / 环境相关的值用 `$env.VAR`。

## 8.4 下一步

- 想要实时试错？用 [在线 Playground](/playground/) 左边写、右边看结果。
- 查具体语法 / 报错？看 [附录](/book/appendix)。
- 想读完整设计说明？回 [站点首页](../) 的特性一览。

恭喜你读完这本教科书！现在你已经能用 SML 写生产级配置了。→ [第 9 章：进阶](/book/ch09-advanced)

## 动手练习

读完本章，在下面的编辑器里**直接修改 SML 并点“运行”**，立刻看到解析结果或校验错误——有输出才能高效学习。

{{< sml-playground "ch08" >}}

{{< sml-quiz "ch08" >}}

