---
title: "第 7 章：多语言使用"
---

# 第 7 章：多语言使用

SML 是格式，要"用起来"得靠宿主语言解析它。各语言实现互不耦合、可单独嵌入（WASM / 沙箱 / 编辑器都行）。下面给出最常用的四种集成方式。

## 7.1 Rust（`swsml`）

```rust
use sml::parse;
let v = parse("name: John\nage: 27").unwrap();
assert_eq!(v["name"], "John");
```

带文件 include：

```rust
use sml::parse_file;
let v = parse_file("app.sml")?;
```

serde 桥接（可选 feature）：

```toml
# Cargo.toml
sml-rs = { version = "0.2", features = ["serde"] }
```

```rust
use sml::{parse, Value};
let v = parse("name: John\nage: 27")?;
let json = serde_json::to_string(&v)?;   // {"name":"John","age":27}
```

> `Value` 手写实现了 `Serialize`/`Deserialize`，序列化为自然的 `27` 而非 `{"Int":27}`。不启用 serde 时 crate 零依赖。

## 7.2 C（`sml.c`）

```c
#include "sml.h"
char err[256] = {0};
sml_value *v = sml_parse("name: John\nage: 27", err, sizeof(err));
/* v->type == SML_STR ("John") ... 用 sml_free(v) 释放 */
```

契约系统已与 Rust **100% 对齐**，同一份 `CONFIG_CONTRACT` 四端行为一致。

## 7.3 JavaScript（`sml.mjs`）

零依赖 ESM，浏览器 / Node 通用，含契约与 Playground：

```js
import { parse, stringify } from "./sml.mjs";
const v = parse('name: John\nage: 27');
console.log(stringify(v));
```

SML ↔ JSON 互转（两者同构）：

```js
const obj = parse(smlText);              // 普通 JS 对象
const json = JSON.stringify(obj);
const sml = stringify(JSON.parse(json));
```

## 7.4 Lua / Soup（`lib/sml.soup`）

```lua
local sml = require("lib.sml")
local v, err = sml.load(text)   -- 解析
print(sml.dump(v))              -- 序列化
```

```bash
soupx lua/sml.sar config.sml     # 解析并打印
```

## 7.5 其他

- **C++**（`cpp/`）：头文件 + 单编译单元，零第三方依赖，解析失败抛 `sml::ParseError`（含行列位置）。
- **Python**：见 `rust/` 外的 py 绑定。

## 7.6 选哪个？

| 你在写 | 用 |
|--------|----|
| Rust 程序 / 命令行工具 | `swsml` |
| 嵌入式 / 系统层 | C / C++ |
| 前端 / Node 服务 | `sml.mjs` |
| Soup 生态 / 脚本 | `lib/sml.soup` |

→ [第 8 章：实战项目](/book/ch08-project)

## 动手练习

读完本章，在下面的编辑器里**直接修改 SML 并点“运行”**，立刻看到解析结果或校验错误——有输出才能高效学习。

{{< sml-playground "ch07" >}}

{{< sml-quiz "ch07" >}}

