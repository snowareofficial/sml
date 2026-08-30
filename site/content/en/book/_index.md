---
title: "SML Textbook"
translationKey: "book-index"
---
# SML Textbook{ ❄ }

Welcome to the official textbook of SML (SNOWARE Markup Language).

This is a progressive and beginner oriented textbook, which can also be used as a reference book for reading at any time. Whether or not you have used JSON/YAML/TOML, you can learn to write configurations and describe data using SML from scratch.

>One sentence understanding of SML: It is a declarative data/configuration format that writes configurations like notes - quotation marks can be omitted, commas can be omitted, and the structure is delimited by curly braces `{}`.

## How to read this book

-**Zero Foundation**: Starting from Chapter 1, read one chapter after another, with each chapter having a "hands-on try".

-**Experienced**: Skip directly to chapters 4-5 to see advanced abilities such as include/contract.

-**Check usage**: Use the following table of contents to jump to the corresponding chapter, or read the "Comparison Table/Error Troubleshooting" in the appendix.

catalogue

|Chapter | Theme | You will learn|
|------|------|----------|
|[Preface](/en/book/intro) | Why SML | What problem does it solve and the difference between it and JSON/YAML|
|[Chapter 1](/en/book/ch01-basics) | First SML file | Key value pairs, comments, scalar types (string/number/pool/null)|
|[Chapter 2](/en/book/ch02-blocks) | Blocks and Nests | Object Blocks, Arrays, Colons Can Be Saved, Nested Structures|
|[Chapter 3](/en/book/ch03-fragments) | Fragment Inheritance | `@base` Definition, `&base` Reference, Configuration Reuse|
|[Chapter 4](/en/book/ch04-include) | Include and namespace | Split files, `as ns` isolation, `import` alias|
|[Chapter 5](/en/book/ch05-contract) | Contract System | `@contract`/`@is`, Type, Default Value, Enumeration, Interval, Combination|
|[Chapter 6](/en/book/ch06-env-escape) | Environment Variables and Escaping | `$env` Injection, `\u` and `\n` Escaping|
|[Chapter 7](/en/book/ch07-languages) | Multilingual use | How to integrate SML with Rust/C/JS/Lua|
|[Chapter 8](/en/book/ch08-project) | Practical: Complete Project | A Near Real Deployment Configuration Example|
|[Chapter 9](/en/book/ch09-advanced) | Advanced: Function combination | include/contract/fragment/$env combination, 8 design patterns|
|[Chapter 10](/en/book/ch10-features) | Complete Reference for Features | Switches, Syntax, Errors, Compatibility Matrix for Each Feature|
|[Appendix](/en/book/appendix) | Comparison and troubleshooting | Comparison with JSON/YAML/TOML, common errors|

## Agreement

-All examples are **real and parsed** SML text.

-The code block labeled `sml` is the SML source code; The annotations `rust`/`js`/`c`/`lua` are host language calls.

-In the example, `#`, `--`, and `//` are all comments, which can be written in any way.

are you ready? Let's start with the prologue (/book/intro).

## Hands on practice

After reading this chapter, directly modify SML in the editor below and click "Run" to immediately see the parsing results or validation errors - having output is necessary for efficient learning.

{{< sml-playground "intro" >}}

{{< sml-quiz "intro" >}}
