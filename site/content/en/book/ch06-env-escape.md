---
title: "Chapter 6: Environment Variables and Escaping"
translationKey: "book-ch06"
---
# Chapter 6: Environment Variables and Escaping

This chapter discusses two small things that make configuration more secure and universal: **environment variable injection** and**string escaping**.

## 6.1 Environment variables: `$env.VAR`

There are often sensitive values (API Key, password) or values that vary with the environment (domain name, port) in the configuration. Writing these **in files** is neither secure nor flexible. SML uses `$env.VAR` to read from environment variables during parsing:

```sml
secrets {
    resendApiKey: $env.RESEND_API_KEY
    dbPassword: $env.DB_PASSWORD
    optionalWebhook: $env.UNSET_WEBHOOK   # 未设置 -> 空串，不报错
}
```

main points:

-`$env.VAR` is locally replaced with the value of the environment variable (replaced with a string) during parsing.

-When the variable **is not set**, the result is an empty string**, and there will be no error (so you can write optional options with confidence).

-This configuration can be securely submitted to the repository, and the key only exists in the variables of the runtime environment.

>Equivalent JSON must write plaintext to death; SML decouples "configuration" from "key".

## 6.2 String Escaping

Common escaping supported in quotation string:

|Escaping | Meaning|
|------|------|
|`\n` | Line Break|
|`\t` | Tab|
|`\\` | Backslash itself|
|`\"` | Quotation marks themselves|
|`\u{XXXX}`/`\uXXXX` | Unicode code point, converted to UTF-8|

Example:

```sml
banner: "SML \u{1F680} 上线 \n第二行\t制表"
label: "雪花 \u{2744} snow"
path: "C:\\Program Files\\app"
```

`\u{2744}` will be parsed into snowflake characters `❄`.

>Escaping **only takes effect within the quoted string**. Bare words (values without quotation marks) do not support escaping, and special characters are enclosed in quotation marks.

## 6.3 Give it a try with your hands

1. Write configuration reference environment variables:

```sml
   database {
       url: $env.DATABASE_URL
       pool: 16
   }
   ```

2. Set `DATABASE_URL=postgres://localhost/app` before running the parser, and confirm that `url` is correctly filled in the parsing result.

3. Try a field with emoji:

```sml
   greeting: "你好 \u{1F44B}"
   ```

→ [Chapter 7: Multilingual Use](/en/book/ch07 languages)

## Hands on practice

After reading this chapter, directly modify SML in the editor below and click "Run" to immediately see the parsing results or validation errors - having output is necessary for efficient learning.

{{< sml-playground "ch06" >}}

{{< sml-quiz "ch06" >}}
