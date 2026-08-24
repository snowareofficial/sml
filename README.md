# sml { ❄ } — SNOWARE Markup Language

声明式数据/配置格式，JSON/YAML 的替代品。Logo：黑花括号 `{}` 表示语法骨架（块的边界），蓝色雪花 `❄` 表示精确的取值点。

**独立仓库**（snoware/sml）：soup 主仓内保留本目录作为副本/镜像源。

## 特性

- **引号可选**：裸词即字符串（`state: NY`）
- **块冒号可省**：`address { }` ≡ `address: { }`
- **数组分隔灵活**：`[ a b c ]`、每行一个、逗号可选
- **片段继承**：`@name { }` 定义，`&name` 引用
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
