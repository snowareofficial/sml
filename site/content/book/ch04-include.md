---
title: "第 4 章：include 与命名空间"
---

# 第 4 章：include 与命名空间

当配置越来越长，你会想把不同部分拆到不同文件。SML 用 `include` 把多个文件拼成一个逻辑整体。

> 设计哲学：**从极简到丰富，功能可裁剪**。基础 include 默认开启；复杂的多目标 / 通配 / 正则需 `@feature enable` 显式开启，避免重蹈 YAML 过度复杂。

## 4.1 基础内联：`include "文件"`

```sml
# app.sml
include "common.sml"
app: myapp
```

`common.sml` 的内容会被**原样贴进来**，就像你手写在那一样。也可以出现在块内部（局部注入字段）：

```sml
database {
    include "conf.d/db.sml"   # 只给 database 块注入字段
    pool: 16
}
```

要点：
- 相对路径按**被包含文件自身目录**解析（和 C 预处理器一致），嵌套包含也符合直觉。
- 循环引用、文件缺失都会**报错**，不会静默跳过。
- 嵌套层级上限 32 层。

## 4.2 命名空间：把文件装进独立作用域

如果 `common.sml` 里有很多键，直接内联可能和主文件**撞名**。用 `as` 给它一个命名空间：

```sml
include "ui.sml" as ui
# 现在 ui.sml 里的键都被收在 ui 下面
title: ui.title
```

`as ui` 等价于把 ui.sml 的内容包进 `ui { ... }` 块。

### 不带扩展名 = 默认命名空间

**干净规则**：带扩展名 = 内联；不带扩展名 = 命名空间（名字取文件名）。

```sml
include "ui"          # 等价于 include "ui.sml" as ui
include "ui.sml"      # 内联（带扩展名）
include "ui.sml" as ui.form.widgets   # 显式指定，优先
```

## 4.3 点分路径（嵌套命名空间）

`as` 后面支持 `a.b.c`，等价于 Rust 的模块路径：

```sml
include "widgets.sml" as ui.form.widgets
```

展开后逻辑结构是 `ui { form { widgets { ... } } }`。用 `ui.form.widgets.Button` 这样的前缀去访问。

## 4.4 宏与契约也随命名空间隔离

命名空间不止隔离数据，还隔离**片段与契约定义**。在被包含文件里定义的 `@contract` / `@name`，对外必须用 `ns.` 前缀引用：

```sml
# widgets.sml 内部
@contract Button { label: str }
@name primary = { label: "OK" }

# 主文件引用时必须带前缀
@is ui.form.widgets.Button
button: &ui.form.widgets.primary
```

> 文件**内部**对自身宏的自引用仍按本地名解析（不用前缀），只有**对外暴露**才需要 `ns.` 前缀。解析器按"命名空间栈"自动加前缀。

## 4.5 多目标与 `import` 别名

逗号分隔一次包含多个目标；`import` 是 `include` 的等价写法：

```sml
include "a.sml", "b.sml" as y, "c"
import ui.buttons, admin.panel
```

> 多目标 / `import` 别名属于"丰富层"，需 `@feature enable multi` 开启（见 4.7）。

## 4.6 冲突即报错（不静默）

命名空间是**独占作用域**，绝不静默覆盖：
- 同一命名空间内重复定义同名契约 / 片段 → 报错。
- 缺失文件 / 循环引用 / 超 32 层嵌套 → 报错。

## 4.7 Feature 分层（可裁剪）

| 层 | feature | 能力 | 默认 |
|----|---------|------|------|
| 0 | `include` | 基础 `include "x.sml"` 内联 | 开 |
| 1 | `namespace` | `as ns` + 点分路径 + 宏/契约隔离 | 开 |
| 1 | `implicit-ns` | 无扩展名 `include "foo"` ⇒ `as foo` | 开 |
| 2 | `multi` | 逗号多目标、`import` 别名 | 关 |
| 2 | `glob` | `*` 通配 `dir/*.sml` | 关 |
| 3 | `regex` | `re:` / `/.../` 正则匹配 | 关 |
| 3 | `ext-rewrite` | `-> .sml` 把非 sml 文件当 sml 解析 | 关 |

开启方式：

```sml
@feature enable glob
include "widgets/*.sml"
```

## 4.8 零拷贝（性能小贴士）

`include` 不会把文件内容深拷贝拼成大字符串。每个文件读入后只持有其切片，解析器消费一段"切片流"，遇到 `as a.b` 时插入零拷贝的开块 / 闭块字面量。文件内容只解析一次，内存 = 各文件切片之和。你无需关心，但知道它很高效就好。

## 4.9 动手试一试

1. 建 `common.sml`，写 `@net { region: cn-north-1 timeout: 30 }`。
2. 建 `app.sml`：
   ```sml
   include "common.sml" as cfg
   service {
       region: cfg.net.region
       name: gateway
   }
   ```
3. 解析 `app.sml`，确认 `service.region` 取到 `cn-north-1`。

→ [第 5 章：契约系统](./ch05-contract)
