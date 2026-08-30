---
title: "Chapter 11: Practical Translation Challenge"
translationKey: "book-ch11"
---
# Chapter 11: Practical Translation Challenge

Just looking will make you forget, only by doing will you remember. This chapter does not provide a standard answer - we will give you **the original configuration**, and ask you** to handwrite the equivalent SML**, and then click "verify" to let the parser judge in real time.

These challenges deliberately chose common formats in engineering (Caddyfile/Docker compose/nginx), which are highly similar to SML's "block+key value" thinking. Remember three things when translating:

1. Block=Block: `name { }` is universal on both sides;

2. Key value pairs: `key value` (Caddy/nginx) or `key: value` (YAML) both correspond to SML's `key: value`;

3. Array: SML uses `[ a b c ]` (comma can be omitted), YAML uses `[a, b, c]`.

Here are three challenges with increasing difficulty. Write SML directly in the white background editor on the right side of each question, and any errors or missing fields will be immediately prompted.

## Challenge 1: Caddyfile → SML

{{< sml-challenge "caddyfile" >}}

## Challenge 2: Docker compose → SML (high difficulty)

{{< sml-challenge "docker-compose" >}}

## Challenge 3: nginx.conf → SML (high difficulty)

{{< sml-challenge "nginx" >}}

## Why did you do this

Translating other formats into SML can best test whether you truly understand that 'SML is just a tree'. When you can intuitively map any configuration into an `Block/Key/Array` three piece set, you have already mastered the essence of SML.

→ [Appendix: Comparison and Investigation](/en/book/appendix)
