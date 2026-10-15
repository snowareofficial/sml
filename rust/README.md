# swsml

**SWE Serial `<< 19 * 99 >>`** — 1999

> **In memory of the Chinese victims of the NATO bombing of the Chinese Embassy
> in Yugoslavia on 7 May 1999.** \
> 谨以此编号纪念 1999 年 5 月 7 日（贝尔格莱德时间）北约轰炸中国驻南斯拉夫联盟
> 大使馆中遇难的三位中国记者：邵云环、许杏虎、朱颖。

**SML — SNOWARE Markup Language** for Rust：声明式数据 / 配置格式，JSON 与 YAML 的
替代方案。零依赖（serde 可选）。

> crate 名为 `swsml`（`sml-rs` 已被一个无关项目占用），但 **lib 名仍是 `sml`**，
> 所以 `use sml::{...}` 不受影响。

## One example

```sml
# firewall.sml — contracts, typed blocks, fragment reuse, env inlining
@feature enable typed-block      # 块级类型标注（Rust / JS 支持，opt-in）

@contract Rule {
    action:  enum [ allow deny ]
    proto:   enum [ tcp udp icmp ]
    port:    int min 1 max 65535 optional
    source:  str
    log:     bool optional
}

@base { log: true }              # 共享字段，按值引用

Rule ssh {                       # typed block：首词命中契约 ⇒ 自动套用，无需 @is
    &base                        # 拼接 log: true
    action: allow  proto: tcp  port: 22   source: 10.0.0.0/8
}

Rule web {
    &base
    action: allow  proto: tcp  port: 443  source: 0.0.0.0/0
}

Rule drop {
    action: deny   proto: udp             source: 0.0.0.0/0
}

alert_webhook: $env.SLACK_WEBHOOK        # 环境变量在解析期内联
```

```rust
use sml::{parse_file, Value};

let fw = parse_file("firewall.sml")?;
// 同型的 `Rule` 块按**类型**收拢到 `Rule` 键（数组），实例名在 `__name`
if let Value::Array(rules) = fw.get("Rule").unwrap() {
    assert_eq!(rules[0].get("__name").and_then(|v| v.as_str()), Some("ssh"));
    assert_eq!(rules[0].get("action").and_then(|v| v.as_str()), Some("allow"));
    assert_eq!(rules[0].get("log"), Some(&Value::Bool(true)));   // &base 拼接
}
```

无引号、无逗号、无缩进陷阱，schema 层还是**可选**的——删掉 `@contract` 它就是普通数据。

> **⚠️ 类型名必须与契约名完全一致（大小写敏感，且会静默失效）**
> 契约声明为 `Rule` 时，**必须**写 `Rule ssh { }`。若写成 `rule ssh { }`，首词不会命中
> 契约表，契约**静默不生效**——不校验、不填默认值、**也不报错**。实测：
> - `Rule ssh { action: bogus }` → `E-CONTRACT-006`（enum 违规，被拦下）
> - `rule ssh { action: bogus }` → **通过**（契约没生效）
>
> 这是当前最容易踩的坑。「首字母大小写不敏感」的开关正在讨论中（属项目决策）。

> **typed block 的适用范围**：`ContractName block { }` 由 opt-in 特性 `typed-block`
> 门控，目前**仅 Rust 与 JS** 支持；C / C++ / Lua 会降级为普通裸块（多出 `__type`
> / `__name` 两个键，契约不生效、不报错）。**跨端共享的文档请改用块内首行
> `@is Rule`** —— 那是五端等价的可移植写法。

> **版本（`Version`）**：`@version v1..v4` 声明语法版本，缺省即 v1。v2 / v3 / v4 要求
> 字符串显式引号（`name: "John"`），v1 允许裸词；`Version::CURRENT` 为 v4。上面示例
> 未声明 `@version`，走默认 v1 基线。

---

## 为什么是 SML · Why SML

JSON 引号噪音大、不能写注释；YAML 依赖缩进、隐式类型容易踩坑。SML 保留两者的可读
块结构，去掉各自的粗糙边：引号 / 逗号可选、块冒号可选、原生注释、片段复用、
`include`、环境变量内联，再加一层**可选的契约（schema）**在解析期校验。

## 安装 · Installation

```toml
[dependencies]
swsml = "0.6"

# serde 互操作：
# swsml = { version = "0.6", features = ["serde"] }
# 彻底零依赖（无 derive、无 serde）：
# swsml = { version = "0.6", default-features = false }
```

兼容 Rust 2021 与 2024 edition（`build.rs` 已处理差异）。

## 特性速览 · Features

| 特性 | 说明 |
|---|---|
| 引号可选 | 裸词即字符串：`state: NY` |
| 块冒号可选 | `address { }` ≡ `address: { }` |
| 数组灵活 | `[ a b c ]` ≡ `[ a, b, c ]`，可逐行 |
| 片段 | `@name { }` 定义，`&name` 按值引用 |
| **契约** | 可选 schema：类型、枚举、默认值、范围、组合 |
| **typed block** | `Rule ssh { }` —— 首词即契约，自动套用（需 `@feature enable typed-block`） |
| `include` | 拆分文件；文本内联，块内也可用 |
| 环境变量 | `$env.VAR` 解析期内联 |
| 版本化 | `@version v1..v4` |
| 特性开关 | `@feature enable/disable <name>`（Rust / JS 支持） |
| 注释 | `#` `--` `//` 单行；`/* */` `_* *_` 多行 |

## 值模型 · Value model

与 JSON 同构，7 种变体：

```rust
pub enum Value { Null, Bool(bool), Int(i64), Float(f64),
                 Str(String), Array(Vec<Value>), Object(BTreeMap<String, Value>) }
```

```rust
use sml::{parse, to_sml};
let v = parse("name: John\nage: 27")?;
assert_eq!(v.get("name").and_then(|x| x.as_str()), Some("John"));
println!("{}", to_sml(&v));        // 往返回 SML
```

> `get()` 支持点路径（`db.port`）。整型 / 数组等需直接 `match` 枚举——当前公开方法
> 只有 `get` / `as_str` / `as_float`。

## 契约 · Contracts

`@contract Name { }` 定义；应用方式**两种等价**：

1. 块内首行 `@is Name`（**可移植，五端一致**，推荐跨端共享的文档使用）
2. 块外首词 `Name block { }`（typed block，需 `@feature enable typed-block`，仅 Rust / JS）

校验发生在**解析期**：必填、类型、枚举、数值范围、数组元素类型；`default` 会填充。
默认拒绝契约外的字段（契约名后写 `loose` 放行）。字段类型可以是另一个契约（组合，
递归校验）。不使用契约时行为完全不变。

类型：`str` / `int` / `num` / `bool` / `any` / `[T]` / `enum [ ... ]`
修饰符：`required`（默认）/ `optional` / `default <v>` / `min <n>` / `max <n>`

**多契约叠加**：`@is A, B`（或 `A B block { }`）可叠加，规则是 **strict 优先 /
字段并集 / 冲突 ⇒ `E-CONTRACT-016`**；同一契约重复声明只生效一次。

**typed block 的结构**：`Rule ssh { }` 产出
`Rule: { __name: "ssh", __type: "Rule", ... }`；**同类型的多个块收拢成数组**（见开头
`firewall.sml` 的 `Rule: [ ... ]`），实例名靠 `__name` 区分。`__type` / `__name` 是
语法元数据，不参与 strict 契约的「未声明字段」检查。

## 片段与 include · Fragments & include

```sml
# app.sml
app: resender
database {
    include "conf.d/db.sml"   # 文本内联；相对"包含者"所在目录
    pool: 16
}
```

- `parse()` 是纯函数（无 I/O）；`include` 由 `parse_file()` 处理——可安全嵌入
  WASM / 沙箱。循环包含、文件缺失会报错；深度上限 32。
- 片段是**值级复用**：`@base { region: cn-north-1 }` 后用 `region: &base`，或在块内
  裸写 `&base` 把其字段**拼接**进当前块（拼接进来的字段同样受契约约束）。

## serde 与 derive · Serde & derive

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct Server { host: String, port: u16 }

let s: Server = sml::serde::from_str("host: web.example\nport: 8080\n")?;
let text = sml::serde::to_string(&s)?;
```

默认 `derive` feature 另提供 `#[derive(SmlSerialize, SmlDeserialize)]`，无需 serde 即可
自然映射（属性：`rename` / `skip` / `default` / `flatten` / `rename_all`）。开启
`serde` feature 后 `Value` 与 serde_json / toml 双向互通。

## 多目标导出 · Emit backends

开启 `emit-*` feature 后，解析成 `Value` 即可编译到 Markdown / LaTeX / XML / SVG /
HTML / Slint UI / 自定义格式（`sml::emit::*`，如 `to_markdown`、`to_slint`）。命令行走
[`smltools/README.md`](smltools/README.md)——它还能把 JSON / YAML / TOML / XML
**反向迁移**成 SML，并接进 Hugo / Zola。

## C-ABI（供 C / C++ 及其它语言）

`crate-type` 含 `cdylib`，提供两套 API：

- **值树 API**（`sml_loads` / `sml_load_file` / `sml_get_in` / `sml_dumps` …）：直接遍历
  结果，C 侧无需 JSON 库；错误带文件 / 行列与稳定错误码。
- **JSON 字符串 API**（`sml_parse` / `sml_parse_ex` / `sml_dump`）：直接产出 JSON 文本。

桥接头文件：[`../c/sml_rs.h`](../c/sml_rs.h)、[`../cpp/sml_rs.hpp`](../cpp/sml_rs.hpp)。
零依赖的原生 C99 / C++ 实现（`sml.h` / `sml.hpp`）是**另一套后端**，与 Rust 版二选一。

## 其它语言实现 · Other implementations

| 语言 | 位置 |
|---|---|
| Rust | 本目录（参考实现） |
| C | `../c/sml.h` |
| C++ | `../cpp/` |
| JavaScript | `../js/sml.mjs`（零依赖 ESM） |
| Lua / Soup | `../lua/`（`lib/sml.soup`） |

Rust 是唯一保证持续维护的实现，其余为实验性。五端共用同一套稳定错误码
（`E-<DOMAIN>-<NNN>`，共 144 条，唯一事实来源 `../errors/codes.sml`）。

> 能力并非五端齐平：**`@feature` 开关与 typed block 仅 Rust / JS 支持**；C / C++ / Lua
> 遇到 `@feature` 会报 `E-PARSE-005`。跨端文档请用 `@is`。

## 示例 · Examples

```bash
cargo run --example include_demo
cargo run --example include_demo --features serde   # 同时打印 JSON
cargo run --example derive_demo                     # derive 宏
cargo run --example emit_ddl                        # 原型演示：SML schema → SQL DDL
```

## License

MulanPSL-2.0
