---
title: "第 3 章：片段继承"
translationKey: "book-ch03"
---

# 第 3 章：片段继承

真实项目里，很多配置块"长得差不多"——比如多个服务都要 `region`、`timeout`、`dns`。重复写既啰嗦又容易不一致。SML 用**片段（fragment）**解决这个问题。

> 一句话：**片段是"值的复用"**——把一组字段定义一次，到处引用展开。

## 3.1 定义片段：`@name`

用 `@名字 { }` 定义一段可复用的字段：

```sml
@net {
    region: cn-north-1
    dns: internal.swebase.cn
    timeout: 30
}
```

注意 `@net` 本身**不会出现在解析结果里**，它只是个"模板"。

## 3.2 引用片段：`&name`

用 `&名字` 把片段"展开"进来：

```sml
network: &net
```

解析后 `network` 会得到：

```sml
network {
    region: cn-north-1
    dns: internal.swebase.cn
    timeout: 30
}
```

## 3.3 在块内引用

片段常用来给多个服务注入公共字段：

```sml
@base {
    region: cn-north-1
    timeout: 30
}

service auth { &base port: 7100 name: auth-svc }
service billing { &base port: 7200 name: billing-svc }
```

`service auth` 展开后等价于：

```sml
service auth {
    region: cn-north-1
    timeout: 30
    port: 7100
    name: auth-svc
}
```

`service billing` 同理拿到同样的 `region` / `timeout`，但 `port` / `name` 各自不同。**复用 + 个性**，完美。

## 3.4 重要细节：块内裸写 `&name` 不会展开

这是新手最容易踩的坑：

```sml
server {
    &base            # ❌ 这样写，&base 被当成"键名"，不会展开字段
    port: 8080
}
```

正确写法是把片段作为**值**赋给某个键：

```sml
server {
    net: &base       # ✅ net 这个键获得 base 的全部字段
    port: 8080
}
```

或者用 `&base` 本身作为值的来源（如 3.3 的 `service auth { &base ... }` 那种"块开头直接跟 `&base` 再跟额外字段"的写法，解析器会把它当作"先展开片段再合并后续字段"）。**牢记：片段是"值"，要出现在 `键: 值` 的值位置。**

## 3.5 片段 vs 契约（先预告）

你可能会想："片段和契约（第 5 章）听起来像？" 它们的定位完全不同：

| | 片段 `@base` / `&base` | 契约 `@contract` / `@is` |
|---|---|---|
| 本质 | **值的复用** | **形状的约束** |
| 做什么 | 把一组字段"展开进来" | 校验结构、补默认值 |
| 结果 | 数据被复制填充 | 数据被检查 + 补齐 |

两者**正交**，可以一起用（先把片段展开，再用契约校验）。

## 3.6 动手试一试

为一个游戏服务器集群写配置，三个服共享 `region` 和 `max_players`：

```sml
@common {
    region: ap-east-1
    max_players: 64
}

lobby { &common port: 25565 name: 大厅 }
pvp { &common port: 25566 name: 竞技场 }
survival { &common port: 25567 name: 生存 }
```

→ [第 4 章：include 与命名空间](/book/ch04-include)

## 动手练习

读完本章，在下面的编辑器里**直接修改 SML 并点“运行”**，立刻看到解析结果或校验错误——有输出才能高效学习。

{{< sml-playground "ch03" >}}

{{< sml-quiz "ch03" >}}

