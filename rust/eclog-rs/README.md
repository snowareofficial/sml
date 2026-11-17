# eclog-rs

Eclog 文本格式的 Rust 实现：JSON-like 数据的解析与渲染。独立 crate，**不依赖 swsml / smlite**，
核心 `#![no_std]` + `alloc`、零硬依赖。

A Rust implementation of the Eclog text format: parsing and rendering JSON-like data.
Standalone crate, independent of `swsml` / `smlite`; core is `#![no_std]` + `alloc`
with no hard dependencies.

## 快速开始 ｜ Quick start

```rust
use eclog::parse;

let doc = parse(b"name: John\nage: 27\ntags: [ a, b, c ]").unwrap();
assert_eq!(doc.get("name").and_then(|v| v.as_text()), Some("John"));
assert_eq!(doc.get("age").and_then(|v| v.as_number()).map(|n| n.raw()), Some("27"));
```

事件流（不建树，供流式消费）：

```rust
use eclog::{parse_with, ParseHandler, Str, Value};

struct Print;
impl ParseHandler for Print {
    fn on_object_begin(&mut self) { println!("object"); }
    fn on_object_end(&mut self) { println!("object_end"); }
    fn on_array_begin(&mut self) { println!("array"); }
    fn on_array_end(&mut self) { println!("array_end"); }
    fn on_key(&mut self, key: &Str) { println!("key {}", key.text()); }
    fn on_value(&mut self, value: &Value) { println!("value {:?}", value); }
}

parse_with(b"a: 1", &mut Print).unwrap();
```

## 语法 ｜ Syntax

- 值类型：`null` / `true` / `false` / number / string / `{...}` / `[...]`；根必须是对象，顶层 `{}` 可省。
- 字符串四种记法：裸词（`[A-Za-z_]` 起）、`"..."`、`@"..."`/`@tag"..."tag`、`|TAG ... TAG`（按闭标记缩进去除公共前缀）。
- 与 `+` 拼接（可跨行）：**只有第一个片段的记法与定界符生效**。
- 数值保留原始字面量：`+0` / `-0` / `1e100` / `+inf` / `-nan` 逐字节可比。
- 分隔：`,` 或换行；两者都可省尾逗号。注释只有 `#` 行注释。
- 字段名可用引号；裸词为 `true/false/inf/nan/null` 时报错（`invalid key`）。

## 渲染 ｜ Rendering

```rust
use eclog::{parse, to_json_compatible, to_typical};

let doc = parse(b"name: John\ntags: [a, b]").unwrap();

assert_eq!(to_typical(&doc), "name: John\ntags:\n[\n\ta\n\tb\n]\n");
assert_eq!(
    to_json_compatible(&doc),
    "{\n\t\"name\": \"John\",\n\t\"tags\":\n\t[\n\t\t\"a\",\n\t\t\"b\"\n\t]\n}\n"
);
```

两种 scheme 与 Eclog-CPP 一致：`typical`（根无 `{}`、裸词、无逗号、`inf`/`nan` 裸写）与
`json_compatible`（根有 `{}`、全 quoted、逗号、`inf`/`nan` 写成字符串）。需要更多控制时用
`Renderer` + `RendererConfig`（`formatting` / `linebreak` / 缩进 / inline / 注释）。

需要 RFC 7396 Merge Patch 时用 `merge_patch(&mut target, &patch)`：`null` 删键、对象递归、
其它整体替换，补丁子树里的 `null` 成员会先被递归剔除。

> 数值渲染**按数值重新格式化**（与 C++ 值树一致）：`+0` → `0`、`-0` → `0`、`-0.0` → `-0`、
> `3.14159265358979323846` → `3.141592653589793`。`raw` 只在解析侧保真。

## 状态 ｜ Status

- 已落地：值模型 + 解析器（树与事件双入口）、双 scheme 渲染器、RFC 7396 merge patch。
- 未落地：amberice 对接模块（3c）；`Number` 的 per-value `fracDigits`（C++ `NumberDesc(v, n)` 的等价物）暂未提供。

## 特性 ｜ Features

| feature | 作用 |
|---|---|
| `std` | 为 `Error` 实现 `std::error::Error`（核心始终 `no_std`） |

## 许可 ｜ License

三许可任选其一：**MIT OR Apache-2.0 OR MulanPSL-2.0**，见 [`LICENSE`](LICENSE)。

Tri-licensed under **MIT OR Apache-2.0 OR MulanPSL-2.0**; see [`LICENSE`](LICENSE).

`tests/golden/` 下的输入与 `.check` 期望来自 [Eclog-CPP](https://github.com/Vallest/Eclog-CPP)
（MIT，Copyright (c) Vallest Systems LLC），原样引用、未做修改；`tests/golden/render/` 是
把上游源码编成 oracle 后跑出来的渲染字节，同样不作手改。

The inputs and `.check` expectations under `tests/golden/` are taken verbatim from
Eclog-CPP (MIT, Copyright (c) Vallest Systems LLC); `tests/golden/render/` holds the
bytes produced by compiling that source as an oracle, likewise never hand-edited.
