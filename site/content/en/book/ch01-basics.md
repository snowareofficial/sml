---
title: "Chapter 1: The First SML File"
translationKey: "book-ch01"
---
# Chapter 1: The First SML File

The goal of this chapter is to write a parsed SML file that understands key value pairs, comments, and scalar types.

## 1.1 The simplest document

New `hello.sml`:

```sml
name: John
age: 27
```

That's it. Two rows of key value pairs, separated by colons for keys and values. No need for quotation marks, no need for curly braces, no need for commas.

>In SML, **keys** are always bare words (letters, numbers, underscores, hyphens);**The value** can be a naked word or enclosed in quotation marks.

## 1.2 Annotations

SML supports three types of line comments, any of which can be used:

```sml
# Well number annotation (most commonly used)
-- Double Horizontal Annotation (Soup/Lua Style)
// Slash comment (C style)

name: John   # Comments can also be written at the end of the line
```

There are also two types of block annotations (which can span multiple lines):

```sml
/* This is a comment
   Can write many lines */
_* This is also a comment，Soup Habitual writing style *_
```

>Note: `#`, `--` inside the * * quotation marks of the string will not be treated as comments, so feel free to write them.

## 1.3 Scalar Types

SML will automatically recognize the type of value:

|What you wrote | recognized as | description|
|--------|--------|------|
|`John`/`NY` | String | Bare word is a string|
|`"21 2nd Street"` | String | Quotation marks only for spaces/special characters|
|`27` | Integer ||
|`0.75` | Floating point number ||
|`true`/`false` | Boolean ||
|`null` | NULL | Equivalent to JSON null|

Hands on trial:

```sml
firstName: "John Doe"     # Containing spaces ->must be quoted
state: NY                 # Single word ->Naked word is sufficient
age: 27                   # Integer
ratio: 0.75               # Floating point
enabled: true             # Boolean
note: null                # Null value
chinese: 中文无需引号      # Chinese naked words are also acceptable
```

>When must quotation marks be added? **When the value contains characters that may interfere with parsing, such as spaces, colons, square brackets `[]`, curly brackets `{}`, and pound signs `#`, wrap them in quotation marks. If you're not sure, add quotation marks. It's never wrong.

## 1.4 String: Bare Words vs Quotation Marks

This is the most comfortable place for SML. contrast:

```sml
# The following two are completely equivalent in the parsing result, both are strings "NY"
state: NY
state: "NY"
```

But emails with `@` can also be written directly naked (because `@` is just a regular character when not at the beginning of the word):

```sml
email: alice@example.com     # Naked word, @ in the middle, safe
from: "SML Team <dev@mail.swebase.cn>"   # Containing spaces ->quotation marks
```

## 1.5 Give it a try with your hands

Write a simple 'personal business card':

```sml
name: 张三
title: 工程师
city: 北京
age: 30
active: true
```

Then read it with a parser in any language, and you will get a key value tree. In the next chapter, we will organize key values into blocks and arrays, which is the true strength of SML.

→ [Chapter 2: Blocks and Nesting](/en/book/ch02-blocks)

## Hands on practice

After reading this chapter, directly modify SML in the editor below and click "Run" to immediately see the parsing results or validation errors - having output is necessary for efficient learning.

{{< sml-playground "ch01" >}}

{{< sml-quiz "ch01" >}}
