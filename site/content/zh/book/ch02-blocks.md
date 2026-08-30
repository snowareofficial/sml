---
title: "第 2 章：块与嵌套"
translationKey: "book-ch02"
---

# 第 2 章：块与嵌套

上一章只有扁平的键值。真实配置是有层级的——一个人有"地址"，一个服务有"数据库设置"。本章学**对象块**和**数组**。

## 2.1 对象块（嵌套键值）

用花括号 `{}` 把一组键值包起来，就是一个"块"：

```sml
address: {
    street: "21 2nd Street"
    city: New York
    state: NY
}
```

**冒号可以省掉**——这是 SML 的特色：

```sml
address {
    street: "21 2nd Street"
    city: New York
    state: NY
}
```

上面两种写法**完全等价**。`address { }` ≡ `address: { }`，你喜欢哪种就用哪种。

## 2.2 块可以无限嵌套

```sml
database {
    primary {
        host: db1.internal
        port: 5432
    }
    replica {
        host: db2.internal
        port: 5432
    }
}
```

> 关键点：**SML 不依赖缩进来定层级**，靠的是花括号。所以你可以自由缩进，缩进错了对解析没影响（但建议保持缩进，对人友好）。

## 2.3 数组

方括号 `[]` 表示数组。元素之间**逗号可省**，换行也可：

```sml
tags: [ logging metrics tracing ]      # 裸词数组，逗号可省
ports: [ 80, 443, 8080 ]               # 带逗号也行
empty: []
```

数组元素也可以是块（对象数组）：

```sml
endpoints: [
    { path: /health method: GET }
    { path: /api/v1 method: POST }
]
```

数组里的块同样支持"冒号可省"：

```sml
users: [
    { name: alice role: admin }
    { name: bob role: user }
]
```

## 2.4 顶层三种形态

一个 SML 文件顶层可以是：

1. **键值 / 块混排**（最常见）
   ```sml
   name: gateway
   database { host: db1 }
   ```

2. **纯数组**（适合"记录列表"，如发信历史）
   ```sml
   [
     { ts: 2026-08-30T10:00 to: a@b.c status: ok }
     { ts: 2026-08-30T11:00 to: x@y.z status: fail }
   ]
   ```

3. **单个对象块**
   ```sml
   {
     name: gateway
     port: 8080
   }
   ```

> 顶层**标量**（比如单独写个 `42`）不可往返——SML 顶层必须是"容器"。这是格式固有限制。

## 2.5 动手试一试

把第 1 章的"名片"升级成有层级的结构：

```sml
name: 张三
contact {
    email: zhangsan@example.com
    phone: "138-0000-0000"
}
skills: [ rust sml linux ]
```

解析后你会得到：`name="张三"`、`contact.email=...`、`skills` 是一个含 3 个元素的数组。

→ [第 3 章：片段继承](/book/ch03-fragments)

## 动手练习

读完本章，在下面的编辑器里**直接修改 SML 并点“运行”**，立刻看到解析结果或校验错误——有输出才能高效学习。

{{< sml-playground "ch02" >}}

{{< sml-quiz "ch02" >}}

