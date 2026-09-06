---
title: "Government Data Demo"
---

# SML × Government Data Exchange { ❄ }

Government data has three chronic pain points — SML addresses each one:

|Pain point | Consequence | SML answer |
|---|---|---|
|18-digit ID / credit codes parsed as numbers | JSON floats eat precision: `330106201503071234` → `...071200` | Identifiers are strings, preserved verbatim |
|Misspelled fields pass silently | `prot` and `port` coexist in one config | `strict` contract rejects on the spot |
|Copy-pasted per-department configs | Fix one place, miss five | `@base` fragment: define once, expand everywhere |

Below is a district-level "government service item" exchange file — two window
blocks share one `strict` contract and one locale fragment. **Run it, or break it
as the task suggests**, and watch the contract intercept:

{{< sml-playground gov >}}

Both engines (JS preview / Rust wasm faithful) run the **same contract
validation** with identical verdicts.
