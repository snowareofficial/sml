---
title: "Chapter 5: Contract System"
translationKey: "book-ch05"
---
# Chapter 5: Contract System

The previous paragraph was about 'value reuse'. **A contract is a "shape constraint" - it defines "what fields a block should have, what types they should have, whether they are required, how many default values they should have, and the range of values", and verifies them during parsing, rather than waiting for you to run the program to discover the problem.

>Applicable scenario: When using SML for **application configuration**, the contract is your schema. Correction of incorrect field names, omission of required fields, and filling in port numbers beyond the specified range - an error message will be generated during parsing, and you will be informed of the accuracy to the line or column.

>**Availability**: contracts run on **Rust (the reference implementation), JS, C, C++ and Lua**
>(Lua support was added recently), and the same contract violation raises **the same error code**
>on all of them — see the [error code reference](/en/errors/). Whatever is not implemented yet
>**fails loudly** instead of pretending to pass: Lua, for example, rejects external types (`@type`)
>and pattern types outright, and records why in the implementation.

## 5.1 Definition of Contract: `@contract`

```sml
@contract ResenderConfig loose {
    api_key:     str                # Required string
    port:        int  default 8080 min 1 max 65535
    debug:       bool default false
    mode:        enum(active, disabled) default active
    tags:        array[str] ?       # Optional string array
}
```

List of field modifiers:

|Modifier | Meaning|
|--------|------|
|`str`/`int`/`num`/`bool` | Field Type|
|`enum(a, b, c)` | enumeration, the value must be in one of them|
|`array[T]` | Array, element type `T` (such as `array[int]`, `array[str]`)|
|`?` or `optional` | Optional fields|
|`required` | Explicitly required (default is required, optional)|
|`Default<value>` | Fill in default value when missing (also automatically considered optional)|
|`Min<number>`/`Max<number>` | Numerical value range (including endpoints)|

## 5.2 Application Contract: `@is`

Two ways of writing:

```sml
# Style 1: @is at the top level of an anonymous block
@contract Cfg loose { api_key: str port: int default 8080 }
@is Cfg
api_key: re_abc
port: 8080
```

```sml
# Style 2: block-level @is
server prod {
    @is Cfg
    api_key: re_prod
    port: 9090
}
```

Verification occurs during **parsing period**: a precise error with location is returned directly in violation of the contract, such as `contract: Service — field main.port is greater than the max 65535`.

### 5.2.1 Type-annotation form: `@is type(ContractName)`

After `@is` you can also write a "type annotation" form. It is **exactly equivalent** to `@is ContractName` — only the contract name is wrapped in type parentheses to emphasize "this is a type constraint":

```sml
@contract Clerk strict {
    name: str
    phone: str
}
window-one {
    @is type(Clerk)        # equivalent to @is Clerk
    name: Zhang San
    phone: "13800138000"
}
```

Both forms verify identically (missing fields still report "contract field missing", defaults still fill in). If a contract happens to be named `type`, then `@is type` is still resolved as "the contract named type" and is never misinterpreted as an annotation unwrap.

### 5.2.2 Block-level type annotation: `<ContractName> <blockName> { }`

If writing a line of `@is` inside a block still feels unnatural, you can put the contract name **in front of the block name**, like typing a variable:

```sml
@feature enable typed-block     # this form must be turned on explicitly

@contract Clerk strict {
    name: str
    phone: phone-num
}
@type name: phone-num {         # type = "shape of a value", contract = "shape of a block"
    序列: [ { 字面: "1" } { 名: 后续, 类: 数字, 次: 10 } ]
}

Clerk window-one {              # contract name + block name = this block is constrained
    name: Zhang San
    phone: "13800138000"
}
```

This is **exactly the same shape** as the existing bare-block form `type [name...] { }`. The only difference: when the first word is an **already-defined contract name**, it is automatically applied as a type constraint, equivalent to writing `@is ContractName` on the block's first line. Non-contract words (e.g. `server web { }`) behave exactly as before, so existing documents are unaffected. This capability is **opt-in** and requires `@feature enable typed-block`.

### Parentheses are ordinary characters

In SML, `( )` are **ordinary characters**, not syntax symbols. So a bare-word value can contain parentheses directly, without quotes:

```sml
note: (important)          # the value is the string "(important)", not lost
priority: (P0) urgent
```

Contract enums are also written with parentheses: `enum(public, internal, confidential)`, `enum(active, disabled)`.

> Note: an older JS engine version treated parentheses as separators and silently truncated values (e.g. `(important)` became `null`); this is now fixed. The Rust engine was always correct. Cross-checking with both engines is the safest.

## 5.3 Strict vs Loose

-**Default Strict** (Nothing written after the contract name): Prohibit any undeclared fields, spelling errors `prot` will be immediately detected.

-**`loose`**: Allow fields that are not declared in the contract, suitable for evolving configurations.

```sml
@contract Metrics loose { latency: num min 0 }
```

`loose` only relaxes the requirement for "undeclared fields", while fields that have been declared will still be checked for type/interval/required fields.

## 5.4 Combination Contract (Recursive Reference)

Contracts do not share fields, but rather 'the type of field is another contract' - simply fill in the contract name without introducing new syntax:

```sml
@contract Endpoint { host: str port: int }
@contract Service {
    name:  str
    main:  Endpoint          # Quoting another contract
    peers: array[Endpoint]   # Contract array
}

@is Service
name: gateway
main: { host: localhost port: 8080 }
peers: [ { host: a port: 1 } { host: b port: 2 } ]
```

Nested blocks will recursively verify and backfill with default values. The referenced contract is allowed to be defined after `@is` (the reference is only resolved when `@is`).

## 5.5 Real Example: Resender Email Tool

[resender]( https://gitee.com/snoware/resender )Using SML contract for `AppConfig` persistence:

```sml
@contract ResenderConfig loose {
    api_key:    str
    from:       str
    to:         array[str]
    subject:    str default "Hello"
    port:       int default 465  min 1 max 65535
    tls:        bool default true
}

@is ResenderConfig
api_key: re_xxxxxx
from: me@example.com
to: [ alice@example.com bob@example.com ]
subject: Weekly Report
port: 465
tls: true
```

The Rust version maintains the `CONFIG_CONTRACT` constant, serializes the configuration back to SML and automatically attaches `@is ResenderConfig` when saving, and verifies it when reading - a typical usage of "contract is schema".

## 5.6 Give it a try with your hands

Add a contract to your game server cluster (Chapter 3):

```sml
@contract Server strict {
    name: str
    port: int min 1024 max 65535
    region: str
}

@common {
    region: ap-east-1
    max_players: 64
}

lobby {
    @is Server
    &common
    port: 25565
    name: 大厅
}
```

Try writing `port: 80` (less than 1024) into it and see if the parser reports an error.

→ [Chapter 6: Environment Variables and Escaping](/en/book/ch06-env-escape)

## Hands on practice

After reading this chapter, directly modify SML in the editor below and click "Run" to immediately see the parsing results or validation errors - having output is necessary for efficient learning.

{{< sml-playground "ch05" >}}

{{< sml-quiz "ch05" >}}
