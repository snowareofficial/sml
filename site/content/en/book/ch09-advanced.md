---
title: "Chapter 9: Advanced - Function Combination and Design Patterns"
translationKey: "book-ch09"
---
# Chapter 9: Advanced - Function Combination and Design Patterns

The first 8 chapters respectively learned key values, blocks, fragments include, Contract, environmental variables, and multilingual integration. This chapter combines them - real SML configurations rarely use only one capability.

>After completing this chapter, you will know what a production level SML configuration library should look like, why it should be written this way, and how to make choices.

## 9.1 A panoramic view

First, let's take a look at SML's' capability stack ':

|Layer | Ability | Solution | Default|
|----|------|----------|------|
|Data | Key value, block, array, scalar | Description | Open|
|Reuse | Fragment `@name`/`&name` | Copy of values | Open|
|Split | `include` | Cross file Reuse | Open|
|Isolation | `as ns` (dotted path) | namespace | open|
|Default | No extension ⇒ `as foo` | Implicit namespace | Open|
|Batch | Multi objective `,`, import alias | Multiple in one go | Related|
|Matching | `*` universal glob | File name matching | Off|
|Match | `re:`/`/.../` Regular | Complex File Name | Off|
|Rewrite | `*->*.sml` | Treat any suffix as sml | Off|
|Constraint | `@contract`/`@is` | Shape Verification | Open|
|Injection | `$env.VAR` | Environment Variables | On|
|Escaping | `\n` `\u{XXXX}` | String Escaping | Open|
|Multilingual | Rust/C/JS/Lua Parser | Cross Ecological | -|

**The default seven piece set allows you to use it out of the box;** Default four on demand opt in for**, avoiding the pitfalls of YAML complexity.

## 9.2 Mode 1: Modular Configuration Library

Extract the 'General Configuration Fragment+Contract' into a shared library and bring in the business project `include`.

### Library file (`sml-lib/net.sml`)

```sml
@contract Service {
    name:   str
    port:   int  min 1 max 65535
    region: str
}

@default-http {
    port: 80
    region: cn-north-1
}

@default-https {
    port: 443
    region: cn-north-1
}
```

### Business Project (`app.sml`)

```sml
@version v1

# Create a namespace for the entire file
include "sml-lib/net" as net

gateway {
    @is net.Service
    &net.default-http
    name: api-gw
}

admin {
    @is net.Service
    &net.default-https
    name: admin-panel
    port: 8443
}
```

main points:

-`include "sml-lib/net"` without extension ⇒ Implicit `as net` (equivalent to explicit `as net`).

-The namespace isolates the `@contract` and `@name` of the library, which are referenced by `net.Service`/`&net.default-http` in the business.

-Libraries can be independently released (even in Git submodule/package management).

-When the same library is included by multiple projects, the macro/contract names **do not conflict** - because they are all installed in different namespaces.

## 9.3 Mode 2: Customizable schema (feature flag)

Contract fields can also be trimmed as needed - using `optional`/`default` allows the same contract to have different 'required sets' in different environments.

```sml
@contract Database strict {
    host:     str
    port:     int  default 5432 min 1 max 65535
    user:     str  default app
    password: str  ?               # Optional: may be empty for local/test
    sslmode:  enum(disable, allow, require) default require
}
```

-Production: Fill in all fields, `sslmode: require` is required.

-Development: `password` (empty connection+trust authentication) is optional, `sslmode: disable`. 

-CI: The contract treats **undeclared fields** as errors - so mock fields inserted in the testing environment will be immediately detected.

>One contract, multiple profiles. By relying on the "optional+default+strict mode" three piece set, there is no need to introduce multiple contracts.

## 9.4 Mode 3: Generate by Environment (env overlay)

Same basic configuration, stacking different segments in different environments - typical three-stage dev/staging/prod:

```sml
# base.sml
@base {
    region: cn-north-1
    timeout: 30
    log_level: info
}

service api { cfg: &base port: 8080 name: api }

# env/dev.sml
include "base" as cfg
service api { &cfg.base }
# Covering log_1evel does not require writing the complete path: can be directly appended
service api { log_level: debug }
```

>Note: SML currently does not have built-in complex merge semantics for "merge by name"; The above writing method relies on **contract+include namespace** handwritten overlay. For more complex merges, it is recommended to use libraries such as `sml-merge` on the Rust/JS side (see reference) https://github.com/snoware/sml-merge ).

## 9.5 Mode 4: Three piece set of include+contract+$env

This is the most common and stable production usage:

```sml
# common.sml
@contract Service {
    name: str
    port: int  default 8080
    debug: bool default false
}

# app.sml
@version v1

include "common" as cfg

gateway {
    @is cfg.Service
    name: gateway
    port: 9090
    debug: $env.DEBUG
}

secrets {
    api_key: $env.API_KEY
    db_password: $env.DB_PASSWORD
    webhook: $env.OPTIONAL_WEBHOOK   # Unset -> empty string
}
```

Why Stable:

-**Contract** Guarantee Structure.

-**include namespace** ensures that contracts do not pollute the scope of the main file.

-**$env** prevents sensitive values from being stored.

-The disadvantage (in development state) is also obvious: `$env` is evaluated only during the parsing period, and the value cannot be seen during IDE debugging - the solution is to use `sml-playground` or write a mock env loader yourself.

## 9.6 Mode 5: Protocol level contract ("front-end and back-end shared schema")

The SML contract is a pure declaration without language binding. The same contract can simultaneously bind:

-Configuration file (`.sml`)

-Types generated by backend Go/Rust code

-Front end TS type (automatically generated)

-API request/response verification

```sml
@contract User {
    id:    str
    email: str
    role:  enum(user, admin, owner) default user
    age:   int  min 0 max 150 ?
}
```

Backend Rust:

```rust
// Automatically derived from. sml by swsml derive macro
# [derive(FromSmlContract)]
struct User { id: String, email: String, role: Role, age: Option<u8> }
```

Front end TS:

```ts
// Automatically generated from. sml by swml ts codegen
type User = { id: string; email: string; role: 'user'|'admin'|'owner'; age?: number };
```

>SML itself is a 'protocol level' - a schema, shared across four endpoints, and consistent behavior.

## 9.7 Mode 6: Contract combination+recursion

Using `array[ContractName]` to express a "contract array" — suitable for list data:

```sml
@contract Endpoint { host: str port: int }
@contract Service {
    name:  str
    main:  Endpoint                # Single
    peers: array[Endpoint]         # List
    back:  Endpoint?               # Optional
}

@is Service
name: gateway
main:  { host: a port: 1 }
peers: [ { host: b port: 2 } { host: c port: 3 } ]
```

Unrestricted number of recursion layers (parsing period detects loop references; loop references=errors).

## 9.8 Pattern 7: glob+contract ("Unified schema for all modules")

>`@feature enable glob` needs to be enabled.

```sml
@feature enable glob

@contract Module {
    name:  str
    entry: str
    deps:  array[str] ?
}

# Load all. sml files under modules/as submodules and verify them separately
include "modules/*.sml" as modules

# Afterwards, modules. auth/modules. billing can be used for access
gateway {
    primary: modules.auth
    fallback: modules.billing
}
```

Suitable for **plugin system**: Each plugin is an independent. sml file, with unified contract verification and unified namespace access.

## 9.9 Mode 8: re: Regularly perform batch processing according to naming rules

>`@feature enable regex` needs to be enabled.

```sml
@feature enable regex

# Load all. sml files starting with 'v' (such as v1. sml/v2. sml)
include "re:^v[0-9]+\\.sml$" as versions

# Load all. json files and rewrite them to. sml for parsing
include "configs/re:.*\\.json$" -> .sml
```

The `re:` prefix indicates that it is followed by regularization; The regular syntax is **handwritten subset** (`. * + ? ^ $ [a-z]` can meet 90% of the scenarios, avoiding the introduction of a complete regex engine).

## 9.10 Principle of Choice (When to Use What)

-Stable structure and frequent field changes → Contract+default value+enum.

-Multiple repeated fields, combined configuration ->fragment+include.

-**Multiple teams share a schema** → modular library (include+namespace).

-**Same configuration for multiple environments** → Contract as skeleton, $env injected with differences.

-**Plug in extension** → glob+namespace.

-**Complex file name matching** → re: regular.

-**Mixed source** (`.json`/`.yaml` used as `.sml`) → ext rewrite `-> .sml`.

## 9.11 anti pattern (please avoid)

-**One huge contract covers all scenarios** - there are too many required fields, and team members roast.

-Nesting namespaces more than 3 layers (`a.b.c.d`) - Decreased readability, it is better to split files.

-The executable logic in the contract - SML contract is a declaration, not a script; To calculate, use the host language.

-`$env` is used as a "dynamic configuration source" - it is only evaluated during the parsing period, and runtime changes are invalid; Please use code for runtime configuration.

-**Include nested more than 5 layers** - will slow down reading; It's time to draw inventory.

## 9.12 Give it a try with your hands

Refactor the "Complete Project" in Chapter 8 according to the "Mode 5 (include+contract+$env)" in this section:

1. Extract `common.sml` into a shared contract and shared fragments.

2. Change `app.sml` to `include "common" as cfg`.

3. Use `$env.*` for all keys.

4. Use `cargo test` (or corresponding language contract testing) to run parsing and verification.

→ [Chapter 10: Complete Reference to Features](/en/book/ch10-features)

## Hands on practice

After reading this chapter, directly modify SML in the editor below and click "Run" to immediately see the parsing results or validation errors - having output is necessary for efficient learning.

{{< sml-playground "ch09" >}}

{{< sml-quiz "ch09" >}}
