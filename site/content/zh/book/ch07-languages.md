---
title: "第 7 章：多语言使用"
translationKey: "book-ch07"
---

# 第 7 章：多语言使用

SML 是格式，要"用起来"得靠宿主语言解析它。各语言实现互不耦合、可单独嵌入（WASM / 沙箱 / 编辑器都行）。下面给出最常用的几种集成方式。

> **先说清楚适用范围**：**只有 Rust 实现（`swsml`）是参考实现并持续维护**——语法、契约、漏洞扫描与回归测试都以它为准，生产请用 Rust。
> **C / JavaScript / Lua（Soup）/ C++ / Python 属实验性实现，暂不保证**与 Rust 行为一致、不保证 API 稳定、不纳入例行漏洞扫描与回归测试。本章中标注「已与 Rust 对齐」的内容为**历史对齐结论**，仅作参考。

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

契约系统曾与 Rust **100% 对齐**（同一份 `CONFIG_CONTRACT` 四端行为一致）；请注意 C 实现现为**实验性、暂不保证**。

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

## 错误码：五端同码

同一类错误在 **Rust / C / C++ / JS / Lua** 里报**同一个码**（形如 `E-CONTRACT-002`）。
所以不必记各端的措辞：记码就够了 —— [错误码总表](/errors/) 是唯一入口。
码的唯一事实来源是仓库里的 `errors/codes.sml`（当前 **141 条**），由生成脚本分发到各端常量；
**取**码的方式则各不相同：Rust 是 `Display` 后缀（`文案 [E-XXX-NNN]`），C / C++ / Lua 把码
写在消息**前缀**，JS 是错误对象的 `e.code` 字段。

由此带来的一类变化值得先知道：**一批此前「能解析」的输入现在会带码报错**。它们过去之所以
"能解析"，是因为解析出的树本来就是错的（未闭合字符串、数组里多余的 `}`、未定义片段引用、
顶层标量，都会静默给出一颗错树）。宁可响亮地拒绝，也不要静默给假数据。

本轮各端的进展（同一件事的几个工作面）：

- **Lua**：补上了契约（`@contract` / `@is` + 默认值回填）与 `include`（沙箱根、链栈环检测、
  嵌套 32 层 / 全局展开 10000 次上限）。此前它把 include 当普通键、把后面到第一个 `{` 的内容
  全吞进片段体 —— 不报错，但树是错的。
- **C++**：`@include` 恢复可用 —— 改为**解析前**递归展开，补上链栈环检测与深度 / 展开次数
  两道闸。此前不仅被包含文件的字段全部丢失，还会连带吞掉 includer 自己的字段。
- **C**：嵌套数组此前丢数据、还会因内层 `]` 被外层当成结束符而**凭空造键**
  （`m: [ 1, [2, 3], 4 ]` 得到 `{"m":[1,2,3],"4":4}`）；现已与 Rust / JS 逐字一致。
  `sml_dump` 的文本输出也与 Rust `to_sml` 对齐（「`键:` 后接对象体」不再留行尾空格）。

## 7.6 选哪个？

| 你在写 | 用 | 保证 |
|--------|----|------|
| Rust 程序 / 命令行工具 | `swsml` | ✅ 参考实现，生产推荐 |
| 嵌入式 / 系统层 | C / C++ | ⚠️ 实验性，暂不保证 |
| 前端 / Node 服务 | `sml.mjs` | ⚠️ 实验性，暂不保证 |
| Soup 生态 / 脚本 | `lib/sml.soup` | ⚠️ 实验性，暂不保证 |

→ [第 8 章：实战项目](/book/ch08-project)

## 动手练习

读完本章，在下面的编辑器里**直接修改 SML 并点“运行”**，立刻看到解析结果或校验错误——有输出才能高效学习。

{{< sml-playground "ch07" >}}

{{< sml-quiz "ch07" >}}

