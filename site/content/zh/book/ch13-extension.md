---
title: "第 13 章：外置扩展（自定义指令与类型）"
translationKey: "book-ch13"
---

# 第 13 章：外置扩展（自定义指令与类型）

SML 的第 13 章讲一件**不常用但关键时刻能救命**的事：当你需要的`@指令`、字段类型或修饰符
SML 没有时，**不用改 SML 的源码**——注册一个扩展就行。

> 一句话：核心语法保持小而稳，领域差异交给**下游注册的扩展点**。

## 13.1 为什么需要它

真实场景长这样：某个项目想给文档挂一块「表单描述」的元数据——

```sml
@contract BugReport {
    title: str
}

@form BugReport {
    title: { label: "标题", widget: text, required: true }
}
```

`@contract` SML 认识，但 `@form` 不认识。**于是项目只能 fork 出自己的解析器**，
写一份只认自己子集的实现。结果是：同一份 `.sml` 文档，在你的解析器里能读、在 SML 里整篇失败，
两端再也互不相通——**方言就此诞生**。

而这类需求（"给文档挂带类型的元数据块，且不进主数据树"）其实是**通用的**，
不是某个项目独有。扩展点就是把这条口子补上：**方言定义留在你的仓库，SML 核心保持通用**。

## 13.2 三个扩展点

| 扩展点 | 解决什么 | 例子 |
|---|---|---|
| `Directive` | 自定义 `@指令` | `@form` / `@policy` / `@flow` |
| `TypeCheck` | 自定义契约类型 | `image` / `link` / `time` |
| `Modifier` | 自定义字段修饰符 | `items_max`（数组元素个数上限） |

三者都遵守同一条铁律：**不注册任何扩展时，解析行为与原来完全一致**（有单元测试钉着）。

## 13.3 自定义指令（Rust）

```rust
use sml::{ext::Outcome, parse_with, ParseOptions, Value};

struct Form;

impl sml::ext::Directive for Form {
    fn name(&self) -> &str { "form" }

    // 是否接受位置参数写法 `@form Name { }`。默认 false（推荐 `@form name: Name { }`）。
    // 置 true 可兼容既有文档，但会产出一条弃用诊断。
    fn positional(&self) -> bool { true }

    // arg: 参数名；body: `{ ... }` 块（没写块时为 Value::Null）
    fn call(&self, _arg: Option<&str>, _body: Value) -> Result<Outcome, String> {
        // 元数据块：文档里写了，解析结果里不出现
        Ok(Outcome::Discard)
    }
}

let opts = ParseOptions::new().directive(Form)?;
let out = parse_with(text, opts)?;
let value = out.value;              // 主数据树
let warns = out.diagnostics;        // 弃用提示等非致命诊断
```

想把指令展开成字段，返回 `Outcome::Emit(对象)` 即可——对象的字段会合并进指令所在的块。

**两种参数写法都收**：

```sml
@form name: BugReport { ... }   -- 推荐：与片段参数同形，无歧义
@form BugReport { ... }         -- 位置参数：v4 起废弃，仅当 positional() 为 true 时接受，并报警
```

**内置指令名不可占用**：`contract` / `is` / `type` / `version` / `feature` / `when` / `for`
注册时直接报错，避免方言改写核心语义。同名重复注册同样报错，不静默覆盖。

## 13.4 自定义契约类型（Rust）

```rust
use sml::contract_ext::TypeCheck;
use sml::Value;

#[derive(Debug)]
struct Image;

impl TypeCheck for Image {
    fn name(&self) -> &str { "image" }
    fn check(&self, v: &Value) -> Result<(), String> {
        match v {
            Value::Str(s) if s.starts_with("ev-") => Ok(()),
            _ => Err("须是以 ev- 开头的证据 id".into()),
        }
    }
}

let opts = ParseOptions::new().with_type(Image)?;
```

之后契约里就能直接用，包括数组：

```sml
@contract Report { images: [image] }
```

报错会自动带上**字段路径**与外置类型的理由，例如 `images[0] 不符合扩展类型 image：须是以 ev- 开头的证据 id`。

为什么值得这么做：`image` / `link` / `time` 这类领域类型如果直接加进 SML 的类型系统，
规范层就要回答"`image` 是什么"——那是业务问题，不该由数据格式回答。

## 13.5 自定义字段修饰符（Rust）

```rust
use sml::contract_ext::Modifier;

#[derive(Debug)]
struct ItemsMax;

impl Modifier for ItemsMax {
    fn name(&self) -> &str { "items_max" }

    // 解析期：校验取值合法性，并可改写字段规格
    fn apply(&self, _spec: &mut sml::FieldSpec, v: &Value) -> Result<(), String> {
        match v { Value::Int(n) if *n >= 0 => Ok(()), _ => Err("须为非负整数".into()) }
    }

    // 校验期：内置校验（类型 / 枚举 / 区间）全部通过后才调用
    fn check(&self, spec: &sml::FieldSpec, v: &Value) -> Result<(), String> {
        let limit = match spec.ext_data.get("items_max") { Some(Value::Int(n)) => *n, _ => return Ok(()) };
        match v {
            Value::Array(items) if items.len() as i64 > limit =>
                Err(format!("最多 {limit} 项，实得 {} 项", items.len())),
            _ => Ok(()),
        }
    }
}
```

```sml
@contract Report { tags: [str] items_max 2 }
```

> ⚠️ **刻意不复用 `max`**：`max` 在 SML 契约里是**数值上界**语义，且数组不参与 min/max 校验。
> 把 `max` 当"个数上限"用会让同一写法在两端含义不同。扩展点请**新起名字**。

## 13.6 crate 现成提供哪些 Modifier（无需自己写）

上节是你**自己写**一个 `Modifier`。其实 `sml-contract` crate 已经**随包提供**一批现成的，覆盖最常见的几类约束——直接 `ParseOptions::with_modifier(...)` 注册即可（公共注册入口；底层走 `ContractExt::register_modifier`，见 `rust/sml-contract/src/ext.rs`）。

### 内建修饰符（固化在核心，不能由外置占用）

`BUILTIN_MODIFIERS`（`ext.rs:124`）列出了核心已占用的名字，**外置扩展不可与之重名**：

`optional` / `required` / `default` / `min` / `max`

### 外置 Modifier 结构体（crate 已提供，注册即用）

| 结构体 | `name()` | 语义 | `ext.rs` 出处 |
|--------|----------|------|---------------|
| `OneOf` | `one_of` | 组内**至多一个**字段出现（≥2 报错） | `ext.rs:377`，判定 `check_group` 在 `ext.rs:386` |
| `ExactlyOne` | `exactly_one` | 组内**恰好一个**字段出现（0 / ≥2 报错） | `ext.rs:398`，判定 `ext.rs:407` |
| `AnyOf` | `any_of` | 组内**至少一个**字段出现（0 报错） | `ext.rs:429`，判定 `ext.rs:438` |
| `DependsOn` | `depends_on` | 本字段出现 ⇒ 所列字段必须出现 | `ext.rs:474`，判定 `ext.rs:483` |
| `ItemsMin` | `items_min` | 数组**元素个数下限**（< n 报错） | `ext.rs:237`，判定 `check` 在 `ext.rs:249` |

> ⚠️ `one_of` / `exactly_one` / `any_of` / `depends_on` 都是**跨字段**约束，挂在某个字段上、却要看**整个块**——
> 它们走的是 `Modifier::check_group(&GroupView)` 钩子（`ext.rs:73`，默认放行）。
> 与之相对，`check(&FieldSpec, &Value)`（`ext.rs:61`）**只见这一个字段的值**，
> 适合 `items_min` 这种「只看本字段生效值」的约束。
>
> `GroupView::present`（`ext.rs:84` 结构体，字段在 `ext.rs:104`）**只算用户显式写出的键**，
> `default` 回填的值**不算出现**——这是互斥 / 计数 / 依赖这类跨字段判据的口径。
> 成员名未在同一份契约里声明过（拼错字段名）会**响亮报错**（见 `resolve_members`，`ext.rs:316`）。
>
> 这些外置修饰符**全部复用 `E-CONTRACT-011`**，不新增码。

### 内建类型表（参考）

`BUILTIN_TYPES`（`ext.rs:30` 起）列出了核心已占用的类型名，**外置类型不可与之重名**：

`str` / `int` / `num` / `bool` / `any` / `enum`（共 6 个，见 `ext.rs:30`–`ext.rs:32`）。

## 13.7 JS 侧

同样的三个能力，JS 侧用选项传入：

```js
import { parse } from "./sml.mjs";

const warnings = [];
const value = parse(text, {
  directives: {
    // discard = 元数据块；emit = 展开（须是对象）
    form: { positional: true, call: (arg, body) => ({ discard: true }) },
  },
  types: {
    // 返回 true 通过 / false 失败 / 字符串 = 失败原因
    image: (v) => (typeof v === "string" && v.startsWith("ev-"))
      ? true
      : "须是以 ev- 开头的证据 id",
  },
  warnings,   // 传入数组，弃用提示会 push 进去
});
```

## 13.8 边界与现状（重要）

- **零扩展 = 零影响**：不传任何 `directives` / `types` 时，行为与未扩展时逐字一致。
- **C-ABI 不支持**：扩展是 trait 对象，跨不了 C 边界。C / C++ 调用方只能用无扩展路径，
  带方言的文档请注明「需 Rust / JS 侧并注册扩展」。
- **Lua 侧暂无**：契约与扩展都还没落地。
- **文档可移植性会下降**：一份带 `@form` 的 `.sml`，在没有注册该指令的环境里读不了
  （Rust 侧会明确报错）。这是扩展机制**固有的权衡**——方言越方便，通用性越弱。

## 13.9 什么时候不该用它

- 只是想表达数据 → 用普通块和数组就够了，不要造指令。
- 想让**所有人**都能读你的文档 → 别用扩展，用核心语法。
- 你的"类型"其实是通用概念（时间、URL、邮箱）→ 优先考虑 `@type` 自定义模式，它不需要注册方。

## 动手练习

注册一个 `link` 外置类型，要求值必须是带 `http://` 或 `https://` 前缀的字符串，
然后在契约里以 `[link]` 使用它，并故意传一个非法值，看报错信息里有没有字段下标。
