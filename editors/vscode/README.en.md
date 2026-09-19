# SML — VSCode Extension

Provides editing support for [SML](../README.md) (SNOWARE Markup Language).

## Features

| Feature | Description |
|---|---|
| **Syntax highlighting** | keys, strings, numbers, bool/null, comments, directives, fragments, contract keywords, types, modifiers |
| **Diagnostics** | real-time parse with errors located to exact line/column (red squiggles + Problems panel) |
| **Completion** | directives, contract keywords, types, modifiers, literals, contract names, fragment names, in-document keys |
| **Hover** | ① hover `@contract` / `@is` / `loose` / `include` for explanations and examples; ② **hover a contract name to see the instance after the contract is applied** — defaults really filled in by the parser, not a copy of the declaration |
| **Go to definition** | `@is Server` → `@contract Server`; `&base` → `@base { }` (F12 / Ctrl+click / right-click "Go to Definition" — all three use the same provider) |
| **Hover on block names** | put the cursor on a block name (`primary {` / `Server primary {`): shows the **path** (`database.primary`), the contract it applies, and the **structure after the contract filled in defaults**; blocks without a contract still show their structure |
| **Hover a field** | cursor on a field inside a contract declaration (`port: int default 5432 min 1 max 65535`), **or** on a key in the data area that belongs to a contract → type / enum / default / range / required-or-optional + the **trailing `#` description** + the current value (saying whether it was written explicitly or filled from the default) |
| **Go to definition** | `@is Server` → `@contract Server`; `&base` → `@base { }`; `Contract block {` → `@contract Contract`; **a data key → its field declaration in the contract** (F12 / Ctrl+click) |
| **Special colors** | select a word → right-click "应用特殊颜色" (apply special color): pick a color and it is written into the workspace **`HL-cfg.sml`** (travels with the repo, hand-editable). By default it applies **only to syntax units** (contract / fragment / type / key / directive) so comments and strings with the same text are not recolored; choose "按普通词着色" (`unit: text`) to color every occurrence |
| **Spotlight highlight** | select a word → right-click "SML: 特别高亮选中词（当前工作区）": lights up **every occurrence in the workspace** (status bar shows N matches / M files; click it or re-run on the same word to clear) |
| **Formatting** | reformat per SML spec (parse → serialize); no change if parse fails |

## Install (from source)

The extension is not on the Marketplace; build and install locally. Use **npx**
to call `vsce` without a global install:

```bash
cd editors/vscode

npm run package          # sync parser + package (recommended; equivalent to the two steps below)
# or manually:
#   python scripts/sync-parser.py
#   npx --yes @vscode/vsce package

code --install-extension sml-lang-0.4.2.vsix
```

One command to package and install (overwrites the old version):

```bash
npm run install-local
```

Or install manually: VSCode → `Extensions` → `...` → `Install from VSIX`, and
select the generated `.vsix`.

> **This `.vsix` is VS Code only**: VSIX is a VS Code-specific package format — **Zed
> cannot install it** (Zed uses `extension.toml` + a Tree-sitter grammar, i.e. a
> different stack, kept under `editors/zed/`). Zed users: see the
> [Zed extension README](../zed/README.md).

> **Why `npm run package` and not just `vsce package`**:
> Before packaging, `scripts/sync-parser.py` copies the repo's `js/sml.mjs`
> into `src/vendor/`. The VSIX only contains files inside the extension
> directory; if the bridge imported the out-of-tree `../../../js/sml.mjs`
> directly, that module **would not be bundled**, and the extension would fail
> on other machines because the module is missing.

For development without packaging: open `editors/vscode` in VSCode and press `F5`
to launch the extension host for debugging.

## Why not LSP

SML has a small grammar and a zero-dependency parser (`js/sml.mjs`) that can be
imported directly, so in-process calls are lighter — no install, no port
coordination. The cost is being limited to VSCode.

If other editors need support later, wrap `src/sml-parse.mjs` in an LSP server
and reuse it; the extension's core logic needs no rewrite (see
[TODO.md](../../TODO.md)).

## Spotlight highlight (temporary searchlight)

Put the cursor on a word (or select text **within one line**) → right-click →
"**SML: 特别高亮选中词（当前工作区）**":

- Every occurrence of that word **across the workspace** is highlighted; the scope comes from
  `sml.specialHighlight.include` (default `**/*.sml`, honouring `files.exclude`).
- The status bar shows `N matches / M files`; hitting a cap is reported as **truncated**.
  Clear it by **clicking the status bar**, **re-running the command on the same word**, or
  right-click → "SML: 清除特别高亮" (shown only while a highlight is active).
- Editing a highlighted file re-scans **that file only** (fast) — never the whole workspace.

⚠️ Deliberate: ① **literal** matching (selecting `(`, `*`, `[` searches for those characters,
not a regex); ② **no semantic filtering** — matches inside comments/strings light up too
(predictability is the point); ③ only **visible editors** can be decorated, so other files count
towards the totals and get painted from the cache when opened.

> Not to be confused with the `HL-cfg.sml` mechanism (`sml.reloadHighlight` /
> `sml.setHighlightMode`), which is **static** keyword colouring; the spotlight is **temporary**.

## Known limitations

- ~~**Contract validation does not run**~~: this note is **outdated** (corrected
  2026-09-18). The JS implementation **does support** contracts (`@contract` /
  `@is` / default fill-in / strict vs `loose` / `[T]` array types), so semantic
  errors such as type mismatch, enum out-of-range and undeclared fields are
  reported together with syntax errors. The historic gap was that the `[T]`
  shorthand was unimplemented (writing `tags: [str] optional` per the docs used to
  produce a false error); fixed — see [CHANGELOG](../../CHANGELOG.md).
- The diagnostic text is the parser's own message (no code suffix in the editor):
  look up the condition on [/errors/](https://sml.swebase.cn/errors/) by keyword, or
  use `parseSafe()` in your own tooling, which does return a stable `code`.
- On parse failure only the **first** error is reported (the parser stops at the
  first error); fix it and re-trigger to see later errors.
- Completion is based on text scanning (regex), not full semantic analysis.
- "Hover shows the expanded contract result" covers **top-level** annotated blocks only; when no
  instance can be found the hover says so explicitly ("no expandable instance found") and shows the
  declaration alone — it never guesses.
  ⚠️ **Precondition (the usual reason people think it is broken)**: the expansion half requires the
  **whole document** to validate (syntax **and** contracts — `contractInstance` calls
  `parseSafe(text)` internally). If the document has *any* error, the hover shows the declaration
  only, plus the "no expandable instance found" note. **Check the Problems panel first.**
  ⚠️ Also mind the syntax: put `@is` **inside** the block (on the line after `web {`).
  Writing `web @is Server { }` is **not valid SML** (Rust: `E-PARSE-012`; JS: "stray closing brace").
- "Go to definition" also only resolves **same-name definitions** (document-level names) — no scope
  analysis.

## File structure

```
editors/vscode/
├── package.json                      # extension manifest (language/grammar/config contributions)
├── language-configuration.json       # comments, brackets, indentation, folding
├── syntaxes/sml.tmLanguage.json      # TextMate grammar (highlighting, declarative)
└── src/
    ├── extension.js                  # diagnostics / completion / hover / formatting
    └── sml-parse.mjs                 # bridge layer: reuses ../../../js/sml.mjs
```

The bridge layer reuses the repo's JS implementation so the **extension and the
language implementation behave identically**: the position where the extension
reports an error is exactly where the parser really reports it.
