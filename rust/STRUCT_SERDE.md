# SML Rust 结构体序列化 / 反序列化示例

> 演示 `#[derive(SmlSerialize, SmlDeserialize)]` 的「Rust struct → SML 文本 → 解析回 Rust struct」链路。
> 所有输出均来自 `swsml` crate 实测（`cargo run --example ...`）。

---

## 1. 正常往返（derive 宏自产自销）

### Rust 源码

```rust
use sml::{SmlSerialize, SmlDeserialize};

#[derive(SmlSerialize, SmlDeserialize, Debug, PartialEq)]
struct Inner {
    x: i32,
    y: i32,
}

#[derive(SmlSerialize, SmlDeserialize, Debug, PartialEq)]
struct Outer {
    a: Inner,
    name: String,
    tags: Vec<String>,
    active: bool,
}

fn main() {
    let original = Outer {
        a: Inner { x: 1, y: 2 },
        name: "web".into(),
        tags: vec!["a".into(), "b".into()],
        active: true,
    };

    // 1) serialize：Rust struct -> SML 文本
    let text = sml::to_string(&original);
    println!("{}", text);

    // 2) parse：SML 文本 -> Value
    let v = sml::parse(&text).unwrap();

    // 3) deserialize：SML 文本 -> Rust struct
    let back: Outer = sml::from_str(&text).unwrap();
    println!("equal: {}", original == back);
}
```

### 输入（Rust 实例）

```
Outer { a: Inner { x: 1, y: 2 }, name: "web", tags: ["a", "b"], active: true }
```

### 输出 1 — `to_string` 生成的 SML 文本

```
a:
{
  x: 1
  y: 2
}
active: true
name: web
tags: [
  a
  b
]
```

### 输出 2 — `parse` 解析回的 `Value`

```text
Object({
    "a": Object({
        "x": Int(1),
        "y": Int(2),
    }),
    "active": Bool(true),
    "name": Str("web"),
    "tags": Array([
        Str("a"),
        Str("b"),
    ]),
})
```

### 输出 3 — `from_str` 反序列化回的 struct

```
equal: true
Outer { a: Inner { x: 1, y: 2 }, name: "web", tags: ["a", "b"], active: true }
```

**结论**：derive 宏自产自销，`to_string` 输出永远是合法 `:` 语法，再 `from_str` 回来 struct 完全一致 ✅

---

## 2. 反例：手写 SML 文本用 `=` 会破坏结构

SML 词法里**没有 `=` token**，`=` 会被当普通字符吞进裸词。所以手写文本不能用 `=` 做分隔。

### 输入（手写的 SML 文本，非 Rust 源码）

```sml
a {
x = 1,
y=2,
}
```

### 输出 — `parse` 后得到的 `Value`（已错位）

```text
Object({
    "a": Object({
        "1": Int(1),
        "x": Str("="),
        "y=2": Str("y=2"),
    }),
})
```

### `to_sml` 再序列化出去（合法但语义已错）

```sml
a:
{
  1: 1
  x: =
  y=2: y=2
}
```

**结论**：`x = 1` 被拆成键 `"x"`=字符串 `"="` 与多余的键 `"1"=1`，`y=2` 整段变成键 `"y=2"`。
信息在**第一次 `parse` 就已丢失**，round-trip 后再多次也回不到原始意图 ❌

---

## 3. 正确手写 SML 文本（用 `:` 分隔）

### 输入

```sml
a:
{
  x: 1
  y: 2
}
```

### 输出 — `parse` 的 `Value`

```text
Object({
    "a": Object({
        "x": Int(1),
        "y": Int(2),
    }),
})
```

可直接 `sml::from_str` 映射到 `Outer` 这样的 struct。

---

## 关键区别

| 入口 | 起点 | `=` 是否可用 | 能否无损往返 |
|------|------|-------------|--------------|
| derive 宏 `to_string` | Rust struct 实例 | 不涉及（自动生成 `:`） | ✅ 能 |
| 手写 SML 文本 `parse` | SML 文本文件 | ❌ 不支持，会被吞进裸词 | ❌ 用 `=` 会破坏 |

> 一句话：**struct derive 这条路全程不碰手写 SML，因此 `=` 问题不会出现**；只有你直接手写 SML 文本时才必须用 `:`。
