# sml { ❄ } — SNOWARE Markup Language

声明式数据/配置格式，JSON/YAML 的替代品。Logo：黑花括号 `{}` 表示语法骨架（块的边界），蓝色雪花 `❄` 表示精确的取值点。

**独立仓库**（snoware/sml）：soup 主仓内保留本目录作为副本/镜像源。

## 特性

- **引号可选**：裸词即字符串（`state: NY`）
- **块冒号可省**：`address { }` ≡ `address: { }`
- **数组分隔灵活**：`[ a b c ]`、每行一个、逗号可选
- **片段继承**：`@name { }` 定义，`&name` 引用
- **`include` 指令**：拆分配置文件，可嵌套、可在块内注入字段
- **环境变量内联**：`$env.VAR`
- **类型自识别**：`true/false/null` / 数字 / 字符串

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

## include 指令

把庞大的配置拆成多个文件。语法 `include "path"`，`@include "path"` 等价。

```sml
# app.sml
app: resender
database {
    include "conf.d/db.sml"   # 在块内注入一组字段
    pool: 16
}
```

要点：

- 相对路径按**被包含文件自身所在目录**解析（同 C 预处理器），嵌套时行为可预期
- 语义是**文本内联**而非对象合并，因此可出现在块内部
- 循环引用、文件缺失都会报错，不会静默跳过
- 嵌套上限 32 层
- 引号内的 `#` 不会被误判为注释

```rust
use sml::parse_file;
let v = parse_file("app.sml")?;
```

> `parse()` 仍是纯函数（不做 IO），include 由 `parse_file()` / `resolve_includes()` 处理，
> 便于在无文件系统的环境（WASM / 沙箱）中嵌入。

## 多语言实现

| 语言 | 目录 | 状态 |
|---|---|---|
| Soup / Lua | `lua/`（`lib/sml.soup`，打包 `sml.sar`） | ✅ 原生 |
| Rust | `rust/`（`sml-rs` crate，rlib+cdylib，C-ABI） | ✅ 孵化 |
| C | `c/sml.h`（链接 sml-rs cdylib） | ✅ 孵化 |
| JavaScript | `js/sml.mjs`（ESM，零依赖） | ✅ 孵化 |

## 使用

```lua
local sml = require("lib.sml")
local v, err = sml.load(text)   -- 解析
print(sml.dump(v))              -- 序列化
```

```bash
soupx lua/sml.sar                # 自检 + 演示
soupx lua/sml.sar config.sml     # 解析并打印
```

```rust
use sml::{parse, to_sml};
let v = parse("name: John")?;
println!("{}", to_sml(&v));
```

### serde 支持（可选 feature）

```toml
sml-rs = { version = "0.2", features = ["serde"] }
```

`Value` 实现 `Serialize`/`Deserialize`，可与任意 serde 后端互操作：

```rust
use sml::{parse, Value};

let v = parse("name: John\nage: 27")?;
let json = serde_json::to_string(&v)?;   // {"name":"John","age":27}
let back: Value = serde_json::from_str(&json)?;
```

采用手写实现而非 `#[derive]`，以保证**数据形状自然**：
`Value::Int(27)` 序列化为 `27`，而非 derive 会产生的 `{"Int":27}`。

不启用该 feature 时 crate 保持**零依赖**。

```js
import { parse, stringify } from "./js/sml.mjs";
const v = parse("name: John");
console.log(stringify(v));
```

## 落地应用

- **BamZap**：`HetuFile.sml` 声明式部署文件
- **soupmake**：`LanTuFile.sml` 构建配置（与 Soupfile 等价）

## 站点

`site/` 为独立官网（Hugo）：`python site/build_site.py`。

## License

MulanPSL-2.0
