---
title: "SML — SNOWARE Markup Language"
---

# SML { ❄ }

**SNOWARE Markup Language**：声明式数据/配置格式，JSON/YAML 的替代品。

黑花括号 **{}** 表示语法骨架（块的边界），蓝色雪花 **❄** 表示精确的取值点。

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
| Soup / Lua | `lib/sml.soup`（`sml.sar`） | ✅ 原生 |
| Rust | `sml-rs` crate（`src/rust/sml-rs/`） | ✅ 孵化 |
| C | `sml.h` C-ABI 头（链接 `sml-rs` cdylib） | ✅ 孵化 |
| JavaScript | `src/js/sml.mjs`（ESM，零依赖） | ✅ 孵化 |

## 相关项目

- [**BamZap 包管理器**](bamzap/) — SML 的落地应用：HetuFile 声明式部署 + `LanTuFile.sml` 构建配置
