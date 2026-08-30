---
title: "Chapter 8: Practical Application - Complete Project Configuration"
translationKey: "book-ch08"
---
# Chapter 8: Practical Application: Complete Project Configuration

Put everything you have learned together: a deployment configuration that is close to reality, covering contracts, fragments include、 Environment variables, nested blocks, arrays.

## 8.1 File Disassembly

`common.sml` (public fragment+contract):

```sml
@contract Service loose {
    name:    str
    port:    int  default 8080 min 1 max 65535
    debug:   bool default false
    peers:   array[str] ?
}

@base {
    region: cn-north-1
    timeout: 30
}
```

`app.sml` (main configuration):

```sml
@version v1

include "common.sml"

# Top level application contract
@is Service
name: gateway
port: 9090
debug: true
peers: [ auth billing ]

# Fragment inheritance reuse
service auth { &base port: 7100 name: auth-svc }
service billing { &base port: 7200 name: billing-svc }

# Nested block+array
database: {
    url: $env.DATABASE_URL
    pool: { min: 2 max: 16 }
}
features: [ logging metrics tracing ]
```

After parsing, you will get a complete, structurally correct, and contract validated tree:

-The top layers `name="gateway"`, `port=9090`, `debug=true`, and `peers` are arrays.

-`service.auth` automatically comes with `region=cn-north-1`, `timeout=30` (from `&base`).

-`database.url` comes from environment variables, while `database.pool` is a nested block.

## 8.2 Progressive Enhancement: namespace isolation

When there are more modules, place each module in a separate file and use a namespace:

```sml
include "auth.sml" as modules.auth
include "billing.sml" as modules.billing

gateway {
    auth: modules.auth.config
    billing: modules.billing.config
}
```

In this way, any arbitrary definition of `@contract`/`@name` within `auth.sml` will not contaminate the scope of the main file.

## 8.3 Verification of Mental Models

When writing SML configuration, think in this order:

1. **Structure**: Use blocks `{}` and arrays `[]` to layer the data.

2. **Reuse**: Extract duplicate fields into `@name` fragments and reference them with `&name`.

3. **Split**: If the file is too large, `include`; To isolate, use `as ns`.

4. **Constraint**: Configuration is read to the program, using `@contract`+`@is` to lock the structure and eliminate errors during parsing.

5. **Decoupling**: Use `$env.VAR` for sensitive/environment related values.

## 8.4 Next steps

-Do you want real-time trial and error? Write on the left and view the results on the right using [online Playground](/playground/).

-Check specific grammar/errors? Please refer to the appendix (/book/appendix).

-Do you want to read the complete design specifications? List of features of [Site homepage](../).

Congratulations on reading this textbook! Now you can use SML to write production level configurations. → [Chapter 9: Advanced](/en/book/ch09 advanced)

## Hands on practice

After reading this chapter, directly modify SML in the editor below and click "Run" to immediately see the parsing results or validation errors - having output is necessary for efficient learning.

{{< sml-playground "ch08" >}}

{{< sml-quiz "ch08" >}}
