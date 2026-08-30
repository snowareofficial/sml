---
title: "Chapter 3: Fragment Inheritance"
translationKey: "book-ch03"
---
# Chapter 3: Fragment Inheritance

In real projects, many configuration blocks look similar - for example, multiple services require `region`, `timeout`, `dns`. Repetitive writing is both verbose and prone to inconsistency. SML uses fragments to solve this problem.

>One sentence: **Fragment is "value reuse"** - defining a set of fields once and expanding them everywhere by referencing.

## 3.1 Definition fragment: `@name`

Define a reusable field using `@名字 { }`:

```sml
@net {
    region: cn-north-1
    dns: internal.swebase.cn
    timeout: 30
}
```

Note that `@net` itself **will not appear in the parsing results**, it is just a "template".

## 3.2 Quoted excerpt: `&name`

Expand the fragment using `&Name`:

```sml
network: &net
```

After parsing, `network` will obtain:

```sml
network {
    region: cn-north-1
    dns: internal.swebase.cn
    timeout: 30
}
```

## 3.3 References within Blocks

Fragments are commonly used to inject common fields into multiple services:

```sml
@base {
    region: cn-north-1
    timeout: 30
}

service auth { &base port: 7100 name: auth-svc }
service billing { &base port: 7200 name: billing-svc }
```

`service auth` is equivalent to:

```sml
service auth {
    region: cn-north-1
    timeout: 30
    port: 7100
    name: auth-svc
}
```

`service billing` also obtained the same `region`/`timeout`, but `port`/`name` were different from each other. **Reuse+individuality**, perfect.

## 3.4 Important Details: `&name` will not be unfolded when written naked within the block

This is the easiest pit for beginners to step into:

```sml
server {
    &base            # ❌ 这样写，&base 被当成"键名"，不会展开字段
    port: 8080
}
```

The correct way to write it is to assign a fragment as a value to a key:

```sml
server {
    net: &base       # ✅ net 这个键获得 base 的全部字段
    port: 8080
}
```

Alternatively, `&base` itself can be used as the source of the value (such as the writing style of `service auth { &base ... }` in 3.3, which "starts the block directly with `&base` and then with additional fields", the parser will treat it as "expanding the fragment first and merging subsequent fields"). * * Remember: Fragments are "values" and should appear at the value position of `键: 值`. **

## 3.5 Fragment vs Contract (Preview)

You may wonder, "Do fragments and contracts (Chapter 5) sound like each other?" Their positioning is completely different:

|| Fragment `@base`/`&base` | Contract `@contract`/`@is`|
|---|---|---|
|Essence | **Value Reuse**|** Shape Constraints**|
|What to do | Expand a set of fields | Validate the structure and add default values|
|Result | Data copied and filled | Data checked and filled in|

The two are **orthogonal** and can be used together (first unfold the fragment, then use contract verification).

## 3.6 Give it a try with your hands

Write configuration for a game server cluster, with three servers sharing `region` and `max_players`:

```sml
@common {
    region: ap-east-1
    max_players: 64
}

lobby { &common port: 25565 name: 大厅 }
pvp { &common port: 25566 name: 竞技场 }
survival { &common port: 25567 name: 生存 }
```

→ [Chapter 4: Include and namespaces](/en/book/ch04 include)

## Hands on practice

After reading this chapter, directly modify SML in the editor below and click "Run" to immediately see the parsing results or validation errors - having output is necessary for efficient learning.

{{< sml-playground "ch03" >}}

{{< sml-quiz "ch03" >}}
