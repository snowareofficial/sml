# SML — VSCode Extension

Provides editing support for [SML](../README.md) (SNOWARE Markup Language).

## Features

| Feature | Description |
|---|---|
| **Syntax highlighting** | keys, strings, numbers, bool/null, comments, directives, fragments, contract keywords, types, modifiers |
| **Diagnostics** | real-time parse with errors located to exact line/column (red squiggles + Problems panel) |
| **Completion** | directives, contract keywords, types, modifiers, literals, contract names, fragment names, in-document keys |
| **Hover** | hover `@contract` / `@is` / `loose` / `include` to see explanations and examples |
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

code --install-extension sml-lang-0.4.1.vsix
```

One command to package and install (overwrites the old version):

```bash
npm run install-local
```

Or install manually: VSCode → `Extensions` → `...` → `Install from VSIX`, and
select the generated `.vsix`.

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

## Known limitations

- **Contract validation does not run**: contracts are currently only supported
  in the Rust implementation (see [TODO.md](../../TODO.md)); the JS
  implementation only does syntax parsing. So semantic errors like type
  mismatches or enum out-of-range **will not be reported** in the editor; syntax
  errors are reported normally.
- On parse failure only the **first** error is reported (the parser stops at the
  first error); fix it and re-trigger to see later errors.
- Completion is based on text scanning (regex), not full semantic analysis.

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
