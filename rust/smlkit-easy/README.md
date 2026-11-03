# smlkit-easy

读一份 SML 只需要 **4 个概念**：文档 → 字段 → 值 → 错误。这个 crate 只暴露这 4 个。

> 状态：**未发布**（`publish = false`）。计划与 `.smlb` / `smldb` 一起发。API 仍可能变。

## 30 秒

```rust
use smlkit_easy::Doc;

let doc = Doc::read("server { host: localhost port: 8080 }")?;
assert_eq!(doc.str("server.host"), Some("localhost"));
assert_eq!(doc.int("server.port"), Some(8080));
# Ok::<(), smlkit_easy::Error>(())
```

文件（**会展开 `include`**）：

```rust
# use smlkit_easy::Doc;
let doc = Doc::read_file("config.sml")?;
# Ok::<(), smlkit_easy::Error>(())
```

错误带**跨端稳定的错误码**，可直接分支：

```rust
# use smlkit_easy::Doc;
let err = Doc::read("42").unwrap_err();
assert_eq!(err.code(), "E-PARSE-008"); // 顶层须为容器
```

## 进阶：只有一个开关

文档里写了 `@is <契约名>` 时认不认 —— 就这一件事，用 `Contracts` 说清：

```rust
# use smlkit_easy::{Contracts, Doc};
let text = "@version v1\ndoc {\n  @is NoSuch\n  x: 1\n}\n";

// 默认（= Doc::read）契约生效：未知契约直接报 E-CONTRACT-001
assert!(Doc::read(text).is_err());
// 显式关闭：放行，且不校验类型、不回填默认值
let doc = Doc::read_with(text, Contracts::Off)?;
assert_eq!(doc.int("doc.x"), Some(1));
# Ok::<(), smlkit_easy::Error>(())
```

文件入口同理：`Doc::read_file_with(path, Contracts::Off)` 是本层**唯一**同时做到
「展开 `include`」与「关掉文档内契约」的入口（`read_file` 关不掉，`read_with` 不展开 include）。

## 为什么单独一个 crate

`swsml` 的根命名空间有 90 多个名字（13 个 `parse_*`、31 个 C-ABI 函数、include 内部件、
三个 `MAX_*`…）。那是给「要把 SML 嵌进产品、需要控制每个旋钮」的人准备的。

这一层**不做成 `swsml` 里的模块**：模块解决不了 `use swsml::*` 把 C 函数也拉进命名空间的问题，
而且会继续挤占它。独立 crate 才能做到 —— **这里根上只有 4 个名字：`Doc`、`Value`、`Error`、`Contracts`。**

（`Value` 出现是因为 `list()` / `raw()` 的返回类型就是它；它就是 swsml 的值类型本身，
不是这里另造的一层包装。）

## 这一层故意不做什么

| 不做 | 想要就去用 |
|---|---|
| 契约**内容**（定义、校验实现、诊断、`ParseOutput`） | `swsml`（`parse_with_warnings` / `ParseOutput`） |
| 逐后端输出（md / html / svg / latex / slint…） | `swsml` 的 `emit-*` |
| 特性开关、版本范围限制 | `swsml` 的 `parse_with_features` / `parse_allowed` |
| 二进制持久化与查询 | 待 `.smlb` / `smldb`（尚未实现） |

## 两个必须知道的坑

**1. 路径只能穿对象，没有下标。** `doc.str("items.0.name")` **永远取不到**；
数组用 `doc.list("items")` 取出来再自己索引。键名自带 `.` 的字段同样取不到。

**2. 「看起来是数字」不等于整数。** `0755`（前导零）和超出 `i64` 的大整数
（`9223372036854775808`）在 SML 里都是**字符串** —— 这是刻意的：前导零保住权限位语义，
大整数保住精度。所以 `doc.int("mode")` 对 `mode: 0755` 返回 `None`，不是 bug。

## 相关

- 主 crate：[`swsml`](https://crates.io/crates/swsml)（全量能力 + 逐后端输出）
- 项目主页：<https://sml.swebase.cn>
