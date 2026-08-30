---
title: "Chapter 2: Blocks and Nesting"
translationKey: "book-ch02"
---
# Chapter 2: Blocks and Nesting

The previous chapter only had flat key values. The real configuration is hierarchical - one person has an 'address', and one service has a' database setting '. This chapter studies **object blocks** and**arrays**.

## 2.1 Object Blocks (Nested Key Values)

Wrap a set of key values in curly braces `{}` to form a "block":

```sml
address: {
    street: "21 2nd Street"
    city: New York
    state: NY
}
```

**Colons can omit** - this is a characteristic of SML:

```sml
address {
    street: "21 2nd Street"
    city: New York
    state: NY
}
```

The above two writing methods are completely equivalent. `address { }` ≡ `address: { }`, You can use whichever you like.

## 2.2 blocks can be infinitely nested

```sml
database {
    primary {
        host: db1.internal
        port: 5432
    }
    replica {
        host: db2.internal
        port: 5432
    }
}
```

>Key point: SML does not rely on indentation to determine hierarchy, but relies on curly braces. So you can freely indent, indentation errors have no impact on parsing (but it is recommended to keep indentation, which is friendly to people).

## 2.3 Array

Square brackets `[]` represent arrays. Commas can be omitted between elements, and line breaks can also be used:

```sml
tags: [ logging metrics tracing ]      # Naked word array, commas can be omitted
ports: [ 80, 443, 8080 ]               # Commas are also fine
empty: []
```

Array elements can also be blocks (object arrays):

```sml
endpoints: [
    { path: /health method: GET }
    { path: /api/v1 method: POST }
]
```

Blocks in the array also support "colon saving":

```sml
users: [
    { name: alice role: admin }
    { name: bob role: user }
]
```

## 2.4 Top Three Forms

The top-level of an SML file can be:

1. **Key value/block mixed sorting** (most common)

```sml
   name: gateway
   database { host: db1 }
   ```

2. **Pure array** (suitable for "record lists", such as sending history)

```sml
   [
     { ts: 2026-08-30T10:00 to: a@b.c status: ok }
     { ts: 2026-08-30T11:00 to: x@y.z status: fail }
   ]
   ```

3. * * Single object block**

```sml
   {
     name: gateway
     port: 8080
   }
   ```

>The top-level scalar (such as writing a separate `42`) cannot be round-trip - the SML top-level must be a "container". This is an inherent limitation of the format.

## 2.5 Give it a try with your hands

Upgrade the "business card" in Chapter 1 to a hierarchical structure:

```sml
name: 张三
contact {
    email: zhangsan@example.com
    phone: "138-0000-0000"
}
skills: [ rust sml linux ]
```

After parsing, you will get: `name="Zhang San"`, `contact.email=...`, `skills` is an array containing 3 elements.

→ [Chapter 3: Fragment Inheritance](/en/book/ch03-fragments)

## Hands on practice

After reading this chapter, directly modify SML in the editor below and click "Run" to immediately see the parsing results or validation errors - having output is necessary for efficient learning.

{{< sml-playground "ch02" >}}

{{< sml-quiz "ch02" >}}
