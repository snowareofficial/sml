---
title: "第 11 章：实战翻译挑战"
translationKey: "book-ch11"
---

# 第 11 章：实战翻译挑战

光看会忘记，动手才记住。这一章不给标准答案——给你**原始配置**，要你**手写等价的 SML**，再点「验证」让解析器实时判分。

这些挑战刻意选了工程里常见的格式（Caddyfile / docker-compose / nginx），它们和 SML 的「块 + 键值」思维高度相通。翻译时记住三条：

1. 块 = 块：`name { }` 两边通用；
2. 键值对：`key value`（Caddy/nginx）或 `key: value`（YAML）都对应 SML 的 `key: value`；
3. 数组：SML 用 `[ a b c ]`（逗号可省），YAML 用 `[a, b, c]`。

下面是三个难度递增的挑战。每题右侧白底编辑器里**直接写 SML**，写错或漏字段都会即时提示。

## 挑战 1：Caddyfile → SML

{{< sml-challenge "caddyfile" >}}

## 挑战 2：docker-compose → SML（高难度）

{{< sml-challenge "docker-compose" >}}

## 挑战 3：nginx.conf → SML（高难度）

{{< sml-challenge "nginx" >}}

## 为什么这么做

把别的格式翻译成 SML，最能考验你有没有真正理解「SML 就是一棵树」。当你能凭直觉把任意配置映射成 `块 / 键值 / 数组` 三件套，你就已经掌握 SML 的精髓了。

→ [附录：对照与排查](/book/appendix)
