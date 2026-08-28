# swsml

**SML — SNOWARE Markup Language** 的 Rust 实现：声明式数据/配置格式，JSON/YAML 的替代品。

SML 源于 **eclog**，在其基础上演进为独立格式。

- 仓库：<https://gitee.com/snoware/sml>
- 许可：MulanPSL-2.0

> 包名是 `swsml` 而非 `sml-rs`——后者已被无关项目占用
> （Smart Message Language 智能电表协议解析器）。
> lib 名仍为 `sml`，因此 `use sml::{...}` 不受影响。

Logo：黑花括号 `{}` 表示语法骨架（块的边界），蓝色雪花 `❄` 表示精确的取值点。

## 特性

| 特性 | 说明 |
|---|---|
| 引号可选 | 裸词即字符串（`state: NY`） |
| 块冒号可省 | `address { }` ≡ `address: { }` |
| 数组分隔灵活 | `[ a b c ]`、每行一个、逗号可选 |
| 片段继承 | `@name { }` 定义，`&name` 引用 |
| **include 指令** | 拆分配置，可嵌套、可在块内注入字段 |
| **版本声明** | `@version v1`，便于将来演进不破坏旧文档 |
| 环境变量内联 | `$env.VAR` |
| 类型自识别 | `true/false/null` / 数字 / 字符串 |

## 版本声明

```sml
@version v1
app: resender
```

- `v1` 为当前版本；未声明时默认按当前版本处理，**既有文档不受影响**
- 声明了实现不支持的版本会报错，而非静默按错误语法解析
- 允许多次声明（include 进来的文件可各自声明），但必须一致
- `version` 是保留字，不可作为片段名

```rust
use sml::{parse_versioned, Version};

let (v, ver) = parse_versioned("@version v1\nname: John")?;
assert_eq!(ver, Version::V1);

// 只需数据时用 parse 即可，它会自动剥离版本声明
let v = parse("@version v1\nname: John")?;
```

文件版本：`parse_file_versioned("app.sml")`。

```sml
firstName: John
age: 27
address:
{
    streetAddress: "21 2nd Street"
    state: NY
}
phoneNumbers: [ { type: home } { type: office } ]
@base { region: cn-north-1 }
server web { &base port: 8080 }
```

## 用法

```toml
[dependencies]
swsml = "0.1"

# 需要 serde 互操作时：
# swsml = { version = "0.1", features = ["serde"] }
```

```rust
use sml::{parse, to_sml};

let v = parse("name: John\nage: 27")?;
assert_eq!(v.get("name").and_then(|x| x.as_str()), Some("John"));
assert_eq!(v.get("age"), Some(&sml::Value::Int(27)));
println!("{}", to_sml(&v));
```

## include 指令

```sml
# app.sml
app: resender
database {
    include "conf.d/db.sml"   # 在块内注入一组字段
    pool: 16
}
```

```rust
use sml::parse_file;
let v = parse_file("app.sml")?;
```

- 相对路径按**被包含文件自身所在目录**解析（同 C 预处理器），嵌套时行为可预期
- 语义是**文本内联**而非对象合并，因此可出现在块内部
- 循环引用、文件缺失均返回错误，不静默跳过；嵌套上限 32 层
- 引号内的 `#` 不会被误判为注释

> `parse()` 是纯函数（不做 IO），include 由 `parse_file()` / `resolve_includes()` 处理。
> 这样在无文件系统的环境（WASM / 沙箱）中仍可安全嵌入 `parse()`。

运行示例：

```bash
cargo run --example include_demo
cargo run --example include_demo --features serde   # 额外打印 JSON
```

## serde 支持（可选）

启用 `serde` feature 后，`Value` 实现 `Serialize`/`Deserialize`，可与任意 serde 后端互操作：

```rust
let v = parse("name: John\nage: 27")?;
let json = serde_json::to_string(&v)?;      // {"name":"John","age":27}
let back: sml::Value = serde_json::from_str(&json)?;
```

采用**手写实现**而非 `#[derive]`，以保证数据形状自然：
`Value::Int(27)` 序列化为 `27`，而非 derive 会产生的 `{"Int":27}`。

不启用该 feature 时，本 crate 为**零依赖**。

## 多语言实现

| 语言 | 位置 |
|---|---|
| Soup / Lua | `../lua/`（`lib/sml.soup`） |
| Rust | 本目录（`sml-rs`） |
| C | `../c/sml.h` |
| JavaScript | `../js/sml.mjs` |

## License

MulanPSL-2.0
