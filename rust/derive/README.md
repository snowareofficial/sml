# swsml-derive

SML (SNOWARE Markup Language) 的 derive 宏：`SmlSerialize` / `SmlDeserialize`。

**English below** ｜ 中文在上方，English 在下方

---

# 中文

`swsml-derive` 提供 `#[derive(SmlSerialize, SmlDeserialize)]`，把自定义结构体 / 枚举自然地映射为 SML 值。宏生成的代码只依赖 `swsml` 主 crate 的 trait 与私有辅助，运行时零额外依赖。

通常通过 `swsml` 的 `derive` feature 引入：

```toml
[dependencies]
swsml = { version = "0.6.0", features = ["derive"] }
```

也可以单独依赖：

```toml
[dependencies]
swsml-derive = "0.6.0"
```

## 快速开始

```rust
use sml::{SmlSerialize, SmlDeserialize};

#[derive(SmlSerialize, SmlDeserialize, Debug)]
struct Server {
    host: String,
    #[sml(default)]
    port: i32,
    #[sml(rename = "tls-enabled")]
    tls_enabled: bool,
}

let s = Server {
    host: "web.example".into(),
    port: 8080,
    tls_enabled: true,
};
let text = s.to_sml();
let back = Server::from_sml(&text).unwrap();
```

## 属性

- `#[sml(rename = "...")]` 改名
- `#[sml(default)]` 缺失时用 `Default`
- `#[sml(skip)]` 跳过字段
- `#[sml(flatten)]` 并入子块
  - ⚠️ **键冲突现在报错**：提升上来的键若与父块已有字段同名，两方向都拦（flatten 盖父块 / 父块盖 flatten），`to_sml_value()` 返回 `Err`。依赖旧「后者胜」静默覆盖的代码会开始收到 `Err`；需要保留旧行为请改名或去掉 `flatten`。
- 容器级 `#[sml(rename_all = "kebab-case")]` 批量改名

## 许可

MulanPSL-2.0

---

# English

`swsml-derive` provides `#[derive(SmlSerialize, SmlDeserialize)]` to map your own structs and enums to SML values naturally. The generated code only depends on traits and private helpers from the main `swsml` crate, with zero extra runtime dependencies.

Usually brought in through the `derive` feature of `swsml`:

```toml
[dependencies]
swsml = { version = "0.6.0", features = ["derive"] }
```

It can also be used standalone:

```toml
[dependencies]
swsml-derive = "0.6.0"
```

## Quick start

```rust
use sml::{SmlSerialize, SmlDeserialize};

#[derive(SmlSerialize, SmlDeserialize, Debug)]
struct Server {
    host: String,
    #[sml(default)]
    port: i32,
    #[sml(rename = "tls-enabled")]
    tls_enabled: bool,
}

let s = Server {
    host: "web.example".into(),
    port: 8080,
    tls_enabled: true,
};
let text = s.to_sml();
let back = Server::from_sml(&text).unwrap();
```

## Attributes

- `#[sml(rename = "...")]` rename a field
- `#[sml(default)]` use `Default` when missing
- `#[sml(skip)]` skip a field
- `#[sml(flatten)]` flatten into a sub-block
  - ⚠️ **Key conflicts now error**: if a key lifted by `#[sml(flatten)]` collides with a field
    already present in the parent block, both directions are rejected (`to_sml_value()` returns
    `Err`). Code relying on the old silent "last wins" overwrite will now receive `Err`; to keep
    the old behaviour, rename or drop `flatten`.
- Container-level `#[sml(rename_all = "kebab-case")]` batch rename

## License

MulanPSL-2.0
