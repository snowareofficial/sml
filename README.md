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
- **契约（Contract）**：可选 schema 层，提供结构体/枚举/默认值/区间约束（见下）
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
# 片段以「值」的形式引用：region 会展开为 @base 定义的内容
region: &base
```

> **片段引用说明**：`&name` 是**值引用**——写作 `key: &name` 时展开为片段内容。
> 在块内裸写 `&base`（如 `server { &base port: 8080 }`）不会展开为字段，
> 因为块内裸词会被当作键名处理。

**顶层形态**：顶层支持键值块、`{ ... }` 对象块、`[ ... ]` 数组三种形态。
其中数组便于存放「记录列表」类数据（如发信历史）：

```sml
[
  { ts: 2026-08-30T10:00:00Z to: a@b.c status: ok }
  { ts: 2026-08-30T11:00:00Z to: x@y.z status: fail }
]
```

> 顶层**标量**（如单独的 `42`）不可往返——SML 顶层需为容器，这是格式固有限制。

**词中 `@`**：`@` 仅当位于词首时才是片段定义标记。词中间的 `@` 是普通字符，
因此邮箱等含 `@` 的裸词可直接使用、无需引号：

```sml
to: a@b.c
from: "SML Team <dev@mail.swebase.cn>"   # 含空格才需引号
```

## 契约（Contract）

SML 是**纯数据格式**，本身不具备类型系统（与 JSON/YAML 同层）。契约是在此之上的
**可选 schema 层**，为块提供结构体约束、枚举、默认值与取值区间：

```sml
@contract Server {
    host: str                               # 必填（默认 required）
    port: int default 5432                  # 缺失时填充
    tls: bool default false
    tags: [str] optional                    # 可选，元素须为字符串
    status: enum [ active standby retired ] # 取值须来自列表
    weight: num min 0 max 100                # 数值区间（含端点）
}

database {
    @is Server
    host: db1.internal
    status: active
    weight: 80
}
```

要点：

- `@contract Name { }` 定义契约（**不进解析结果**）；`@is Name` 在块内应用
- 应用时会**填充 default**，并校验必填、类型、枚举、数值区间、数组元素类型
- 校验发生在**解析期**：违反契约直接返回错误，而非留到应用侧才发现
- 契约须在 `@is` **之前**定义（顺序依赖，与片段继承一致）
- 不使用契约时解析行为完全不变 —— **向后兼容**

支持的类型：`str` / `int` / `num`（int 或 float）/ `bool` / `any` / `[T]`（数组）/ `enum [ ... ]`
修饰符：`required`（默认）/ `optional` / `default <值>` / `min <数>` / `max <数>`

### 组合（Composition），而非继承

契约之间**不共享字段定义**，而是「字段的类型是另一个契约」。写法上直接填契约名，
不引入任何新 token，且可多层嵌套：

```sml
@contract Address {
    city: str
    country: str default CN
    zip: str optional
}

@contract Server {
    host: str
    address: Address        # 组合：该字段的值须符合 Address 契约
}

db {
    @is Server
    host: db1.internal
    address { city: Beijing }   # country 缺 -> 自动填 CN
}
```

嵌套块会**递归校验并回填默认值**。被引用契约可在之后定义（引用在 `@is` 时才解析）。

### 严格模式（默认严格）

契约未声明的字段**默认被拒绝**，可立即发现 `prot` 这类拼写错误。
确需放宽时须**显式**在契约名后写 `loose`：

```sml
@contract Metrics loose {   # 允许额外字段
    latency: num min 0
}
```

`loose` 只放宽「未声明字段」，已声明字段照样校验。

> **与片段的区别**：片段（`@base` + `&base`）是**值的复用**，把一组字段展开进来；
> 契约（`@contract` + `@is`）是**形状的约束**，校验结构并补默认值。两者正交，可同时使用。

完整示例见 [`showcase_contract.sml`](showcase_contract.sml)。

**语言支持**：Rust ✅ ｜ C / JS / Lua ⏳ 待实现（见 [TODO.md](TODO.md)）。

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

## 编辑器支持

| 编辑器 | 目录 | 能力 |
|---|---|---|
| VSCode | `editors/vscode/` | 高亮、错误提示、补全、悬浮说明、格式化 |

安装与已知限制见 [`editors/vscode/README.md`](editors/vscode/README.md)。

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

[木兰宽松许可证，第2版 (Mulan Permissive Software License, Version 2)](http://license.coscl.org.cn/MulanPSL2)
—— 详见根目录 `LICENSE` 文件；各源文件头部均含 `SPDX-License-Identifier: MulanPSL-2.0` 声明。
