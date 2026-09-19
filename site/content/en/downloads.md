---
title: "Download"
---

# Download SML { ❄ }

## VS Code Extension (0.4.2)

Adds SML syntax highlighting, diagnostics, completion, hover, go-to-definition and
formatting to VSCode — plus a few things that only make sense for SML:

- **Hover**: on a contract name (`@is Server`) it shows the contract declaration
  *and* the **structure after the parser filled in defaults**; on a block name
  (`primary {`) it shows the **path** (`database.primary`), the contract it applies
  and the filled structure
- **Go to definition**: `@is Server` → `@contract Server`, `&base` → `@base { }`
- **Special colors**: select a word → right-click "应用特殊颜色"; the color is
  written into the workspace `HL-cfg.sml` (travels with the repo, hand-editable)
  and by default applies **only to syntax units** (contract / fragment / type /
  key / directive) so comments and strings with the same text are left alone
- **Spotlight**: lights up a word across the **whole workspace** (status bar shows
  N matches / M files)
- **Self-check**: right-click "SML: 自检" dumps *why nothing happens* into the
  "Output → SML" panel (extension version, **bundled parser fingerprint**,
  registered commands, language mode, document validation, navigability)

- Download: [sml-lang-0.4.2.vsix](/dl/sml-lang-0.4.2.vsix)
- Install: VSCode → Extensions → `...` → Install from VSIX, then pick the
  downloaded file
- Docs: the extension folder ships `README.md` (bilingual) and `README.en.md`

> **This build was repackaged**: it fixes a fatal issue where the extension
> **could not activate at all** on VS Code 1.13x (module-level code read
> `vscode.InsertTextFormat`, an enum removed from newer hosts). If you installed
> an earlier 0.4.2, **install this file again** — the old package shows up as
> "hover / context menu do nothing at all". After installing, right-click
> "SML: 自检" to confirm the bundled parser fingerprint
> (`73683 B / 0ea28eec…`).

> The extension is not on the Marketplace; only a manual VSIX install is
> provided (VSIX is a VS Code-only format — Zed cannot install it; Zed users
> should read `editors/zed/README.md` in the repository).
> Contracts (`@contract` / `@is`) are supported by Rust, JS, C, C++ and Lua, so
> the editor **does report semantic errors** too (the earlier "contracts are
> Rust-only" note is outdated — see "Known limitations" in the extension README).

Reference implementations for each language (Rust `swsml`, etc.) ship with the
source repository — see the [home page](/en/).
