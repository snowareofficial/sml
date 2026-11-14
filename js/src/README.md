# `js/src/` —— `js/sml.mjs` 的模块

`js/sml.mjs` 是 ~2700 行的单文件；`parse()` 内部用了大量闭包共享游标，整体拆成 ESM 需要先
把 `parse` 的局部状态提成显式对象。因此按「**先抽纯函数段、再考虑拆闭包**」的顺序推进。

## 已抽出

| 模块 | 内容 | 依赖 |
|---|---|---|
| `dump.mjs` | `needsQuote` / `escapeStr` / `quoteIfNeeded` / `dumpValue` / `dumpInline` / `stringify` / `dump` | 无（只用内置） |

`sml.mjs` 在原位置换成 `export { stringify, dump } from "./src/dump.mjs";`，
对外 API 与导入路径完全不变（`import { parse, stringify } from "./sml.mjs"` 照旧）。

## 后续切片（按风险从低到高）

1. `errors.mjs`：`throwCode` + 错误构造。它是 `tokenize` 与 `parse` 共用的最小依赖，
   抽出去之后 `tokenize` 才能独立；
2. `lex.mjs`：`tokenize`（~180 行，纯函数，入参 `text` + 特性集）；
3. `pattern.mjs`：Loom 模式 → `RegExp` 的编译器（~200 行，纯函数 + 上限常量）；
4. `contract.mjs`：`valueMatchesType` / `checkContract` / `applyDefaults`（顶层纯函数）；
5. `parser.mjs`：需要先把 `parse()` 的闭包状态（`i` / `toks` / `feats` / `contracts`…）
   提成一个 `Ctx` 对象，属**行为重构**，单独一轮做并靠 conformance 兜底。

## 验收

```bat
node -e "import('./js/sml.mjs').then(m=>console.log(m.stringify(m.parse('a: 1'))))"
node js/test_include.mjs
python tests/conformance/run_conformance.py --require-all
python tools/check/probe_matrix.py --suite values
```

## 已知事实（不要重复踩）

- JS 与 Lua 都只有一个数字类型 ⇒ `1.0` / `1` 不可区分，`probe_matrix` 的 `19` 号用例长期放行；
- 超过 2^53 的整数走字符串（`ambr` 的 `intValue` 同口径），不是缺陷，别为了「看起来对齐」
  改成 `BigInt` —— 那会让 `ambr` 的向量测试与解码器分叉。
