---
title: "SML — SNOWARE Markup Language"
---

# SML { ❄ }

**SNOWARE Markup Language**：声明式数据/配置格式，JSON/YAML 的替代品。

黑花括号 **{}** 表示语法骨架（块的边界），蓝色雪花 **❄** 表示精确的取值点。

<div class="links">
  <a class="btn" href="https://gitee.com/snoware/sml" target="_blank">Gitee 仓库 →</a>
</div>

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

| 语言 | 库 | 状态 |
|---|---|---|
| **Soup / Lua** | `lib/sml.soup`（打包 `sml.sar`，零依赖） | ✅ 原生 |
| **Rust** | `sml-rs` crate（rlib + cdylib，C-ABI） | ✅ 可用 |
| **C** | `c/sml.h` + `c/sml.c`（纯 C99 自包含，零依赖） | ✅ 可用 |
| **JavaScript** | `js/sml.mjs`（ESM，零依赖） | ✅ 可用 |

> C 库为自包含实现（非仅头文件桥），提供 `sml_parse/sml_dump/sml_obj_get/sml_get_path/sml_parse_json` 等完整 API，可直接 `gcc sml.c example.c -o demo` 编译。

## 快速使用

**Lua / Soup**
```lua
local sml = require("lib.sml")
local v, err = sml.load(text)   -- 解析
print(sml.dump(v))              -- 序列化
```

**Rust**
```rust
use sml::{parse, to_sml};
let v = parse("name: John")?;
println!("{}", to_sml(&v));
```

**C**
```c
sml_value *v = sml_parse("name: John", err, sizeof err);
printf("%s\n", sml_obj_get(v, "name")->u.s);
```

**JavaScript**
```js
import { parse, stringify } from "./js/sml.mjs";
const v = parse("name: John");
console.log(stringify(v));
```

## 落地应用

- **BamZap** 包管理器：`HetuFile.sml` 声明式部署文件（[bamzap.swebase.cn](https://bamzap.swebase.cn)）
- **soupmake** 构建系统：`LanTuFile.sml` 构建配置（与 `Soupfile` 等价）

## 仓库

- 源码：[gitee.com/snoware/sml](https://gitee.com/snoware/sml)
- 许可证：MulanPSL-2.0
