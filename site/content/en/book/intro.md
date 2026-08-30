---
title: "Introduction: Why SML"
translationKey: "book-intro"
---
# Introduction: Why SML

Before you start writing the first line of SML, answer a question: * * Why is there another configuration format? **

JSON, YAML, and TOML are all very useful, but they each have their own frustrating aspects:

|Format | Pain Points|
|------|------|
|JSON | Quotation marks, commas, and curly braces must not be missing at all; Writing configuration is very verbose|
|YAML | Indent sensitive, one space misplacement will explode; `no` will be mistaken for `false`, which will startle you|
|TOML | If nested deeply, many `[table.sub]` headers need to be written, and the block structure is not intuitive|

## Design Objectives of SML

SML wants to do **"Write configurations like notes"**:

1. **Less ceremonial sense** - quotation marks can be selected, commas can be selected, and colon blocks can be omitted.

2. **Structure relies on curly braces** - without indentation, copying and pasting will not result in strange errors.

3. Prioritize readability - making it easy to understand at a glance and making diff/review easier.

4. **Customizable** - Basic capabilities are minimal, while complex capabilities (including, regular, etc.) are enabled as needed, avoiding the pitfalls of excessive YAML complexity.

5. **Built in contract** - not only "storing data", but also defining "what data should look like" and verifying it during the parsing period.

## A minimum comparison

The same configuration can be written in three ways:

```json
{
  "name": "gateway",
  "port": 8080,
  "debug": true,
  "tags": ["logging", "metrics"]
}
```

```yaml
name: gateway
port: 8080
debug: true
tags:
  - logging
  - metrics
```

```sml
name: gateway
port: 8080
debug: true
tags: [ logging metrics ]
```

The SML version has almost no punctuation noise - no quotes, no commas, no indentation traps. This is its core temperament.

## What is it not suitable for

SML is a **data/configuration format**, not a programming language:

-Cannot write `if`/`for`, cannot define functions, cannot calculate `1+1`.

-When logic is needed, read SML in the host language (Rust/JS/Lua...) and process it in the code.

>Remember one sentence: * * SML is responsible for "describing what it is", and code is responsible for "how to do it". **

In the next chapter, we will write the first real SML file. → [Chapter 1](/en/book/ch01-basics)

## Hands on practice

After reading this chapter, directly modify SML in the editor below and click "Run" to immediately see the parsing results or validation errors - having output is necessary for efficient learning.

{{< sml-playground "intro" >}}

{{< sml-quiz "intro" >}}
