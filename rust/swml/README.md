# swml

SML facade: the **full `swsml` API** at the root, plus a four-concept `easy` path for reading documents.
｜ SML 门面：根上是 `swsml` 的**完整 API**，另加一条「四个概念」的 `easy` 入门路径。

```rust
// Progressive ｜ 入门
use swml::easy::Doc;

let doc = Doc::read("server { host: localhost port: 8080 }")?;
assert_eq!(doc.str("server.host"), Some("localhost"));

// Expert ｜ 专家：同一个 crate 的根就是 swsml 的全部公开面
let value: swml::Value = swml::parse("a: 1")?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

> Status ｜ 状态：**unpublished / 未发布**（`publish = false`），与 `.ambr`、`swml` 0.1.0 独立发布线一起发。
> The API may still change ｜ API 仍可能变。

## Two doors, one install ｜ 一装两用

| Door ｜ 入口 | What you get ｜ 拿到什么 | Who it is for ｜ 给谁 |
|---|---|---|
| `swml::easy` | `Doc` / `Value` / `Error` / `Contracts` — four names | 「读一份配置」的人 |
| crate root ｜ 根 | `parse_*` / `Value` / `emit::*` / C-ABI / contract internals — everything | 要把 SML 嵌进产品、需要每个旋钮的人 |

No wrapper types ｜ 没有包装类型：`swml::Value` **is** `sml::Value`, so values cross between the two doors
without conversion. ｜ `swml::Value` 就是 `sml::Value`，两条路之间传值不需要转换。

## 30 seconds ｜ 30 秒

```rust
use swml::easy::Doc;

let doc = Doc::read("server { host: localhost port: 8080 }")?;
assert_eq!(doc.str("server.host"), Some("localhost"));
assert_eq!(doc.int("server.port"), Some(8080));
# Ok::<(), swml::easy::Error>(())
```

From a file — **expands `include`** ｜ 读文件会展开 `include`：

```rust
# use swml::easy::Doc;
let doc = Doc::read_file("config.sml")?;
# Ok::<(), swml::easy::Error>(())
```

Errors carry a **cross-implementation stable code** ｜ 错误带跨端稳定的错误码：

```rust
# use swml::easy::Doc;
let err = Doc::read("42").unwrap_err();
assert_eq!(err.code(), "E-PARSE-008"); // top level must be a container ｜ 顶层须为容器
```

## One switch ｜ 只有一个开关

Whether `@is <contract>` inside the document is enforced ｜ 文档里的 `@is` 认不认：

```rust
# use swml::easy::{Contracts, Doc};
let text = "@version v1\ndoc {\n  @is NoSuch\n  x: 1\n}\n";

// Default (= Doc::read): unknown contract fails ｜ 默认：未知契约直接报错
assert!(Doc::read(text).is_err());
// Off: parse anyway, no type check, no default filling ｜ 关掉：放行，不校验、不回填
let doc = Doc::read_with(text, Contracts::Off)?;
assert_eq!(doc.int("doc.x"), Some(1));
# Ok::<(), swml::easy::Error>(())
```

`Doc::read_file_with(path, Contracts::Off)` is the only entry that both expands `include` and turns
document contracts off ｜ 本层唯一同时「展开 include」与「关掉文档内契约」的入口。

## Diagnostics and output ｜ 诊断与输出

```rust
# use swml::easy::Doc;
let doc = Doc::read_with_report("a: 1\na: 2")?;
assert_eq!(doc.diagnostics().len(), 1); // duplicate key promoted to array ｜ 同名键被提升为数组
# Ok::<(), swml::easy::Error>(())
```

```rust
# use swml::easy::Doc;
let doc = Doc::read("p { text: \"正文\" }")?;
let md = swml::easy::out::markdown(&doc)?;
let html = swml::easy::out::html(&doc)?;
let _ = (md, html);
# Ok::<(), swml::easy::Error>(())
```

Each `out::*` function is gated by the same-named `emit-*` feature: turning it off makes the function
**not exist** rather than fail at runtime. ｜ 关掉 feature 即函数不存在，不是运行时错误。
For knobs like `math` / `standalone`, go to `swml::emit` ｜ 要调旋钮请去 `swml::emit`。

## What this layer deliberately omits ｜ 这一层故意不做什么

| Not here ｜ 不做 | Use instead ｜ 想要就去用 |
|---|---|
| Contract **contents** (definitions, checks, `contract_map` / `errors`) | `swml` (`parse_with` / `ParseOutput`) |
| Per-backend **knobs** (`math` / `standalone` / templates…) | `swml::emit` options |
| Feature switches, version allow-lists | `swml` `parse_with_features` / `parse_allowed` |
| Binary persistence and queries | `.ambr` / `smldb` (I-18, still in progress) |

## Two traps ｜ 两个必须知道的坑

**1. Paths cross objects only; there is no index syntax.**
`doc.str("items.0.name")` never resolves — pull the array out with `doc.list("items")` and index it
yourself. A key that itself contains `.` is equally unreachable.
｜ 路径只能穿对象，没有下标；键名自带 `.` 的字段同样取不到。

**2. "Looks like a number" is not an integer.**
`0755` (leading zero) and integers beyond `i64` (`9223372036854775808`) are **strings** in SML —
deliberately: leading zeros keep permission-bit semantics, big integers keep precision. So
`doc.int("mode")` returning `None` for `mode: 0755` is not a bug.
｜ 前导零与大整数在 SML 里都是字符串，`None` 不是 bug。

## Related ｜ 相关

- Underlying crate ｜ 主 crate: [`swsml`](https://crates.io/crates/swsml)
- Project home ｜ 项目主页: <https://sml.swebase.cn>
