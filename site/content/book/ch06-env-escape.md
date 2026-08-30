---
title: "第 6 章：环境变量与转义"
---

# 第 6 章：环境变量与转义

本章讲两件"让配置更安全、更通用"的小事：**环境变量注入**和**字符串转义**。

## 6.1 环境变量：`$env.VAR`

配置里常有敏感值（API Key、密码）或随环境变化的值（域名、端口）。把这些**写死在文件里**既不安全也不灵活。SML 用 `$env.VAR` 在解析期从环境变量读取：

```sml
secrets {
    resendApiKey: $env.RESEND_API_KEY
    dbPassword: $env.DB_PASSWORD
    optionalWebhook: $env.UNSET_WEBHOOK   # 未设置 -> 空串，不报错
}
```

要点：
- `$env.VAR` 在解析时就地替换为环境变量的值（替换为字符串）。
- 变量**未设置**时，结果为**空串**，不会报错（所以可放心写可选项）。
- 这样配置可以安全地提交进版本库，密钥只存在于运行环境的变量中。

> 等价 JSON 必须把明文写死；SML 让"配置"与"密钥"解耦。

## 6.2 字符串转义

引号字符串里支持常见转义：

| 转义 | 含义 |
|------|------|
| `\n` | 换行 |
| `\t` | 制表符 |
| `\\` | 反斜杠本身 |
| `\"` | 引号本身 |
| `\u{XXXX}` / `\uXXXX` | Unicode 码点，转为 UTF-8 |

示例：

```sml
banner: "SML \u{1F680} 上线 \n第二行\t制表"
label: "雪花 \u{2744} snow"
path: "C:\\Program Files\\app"
```

`\u{2744}` 会被解析成雪花字符 `❄`。

> 转义**只在引号字符串内**生效。裸词（不加引号的值）不支持转义，需要特殊字符就用引号包起来。

## 6.3 动手试一试

1. 写配置引用环境变量：
   ```sml
   database {
       url: $env.DATABASE_URL
       pool: 16
   }
   ```
2. 在运行解析器前设置 `DATABASE_URL=postgres://localhost/app`，确认解析结果里 `url` 被正确填充。
3. 试一个带 emoji 的字段：
   ```sml
   greeting: "你好 \u{1F44B}"
   ```

→ [第 7 章：多语言使用](./ch07-languages)
