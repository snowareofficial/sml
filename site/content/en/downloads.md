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
- **include / namespace navigation**: on a path F12 jumps to the included file and
  hover shows that file's top-level keys; on a namespace alias (`as db`) hover shows
  **which keys that namespace actually holds** and F12 jumps to the included file; on a
  reference (`db.host`) hover tells you which file it comes from and what kind of
  definition it is, and F12 jumps straight there
- **Special colors**: select a word → right-click "应用特殊颜色"; the color is
  written into the workspace `HL-cfg.sml` (travels with the repo, hand-editable)
  and by default applies **only to syntax units** (contract / fragment / type /
  key / directive) so comments and strings with the same text are left alone.
  **Namespace prefixes count too** (`@is ui.base.Service` and friends are coloured).
  For **what this is good for when writing DSLs / doing analysis**, see the section of
  the same name in the extension README
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
> (`96110 B / c7a5054d…`).

> The extension is not on the Marketplace; only a manual VSIX install is
> provided (VSIX is a VS Code-only format — Zed cannot install it; Zed users
> should read `editors/zed/README.md` in the repository).
> Contracts (`@contract` / `@is`) are supported by Rust, JS, C, C++ and Lua, so
> the editor **does report semantic errors** too (the earlier "contracts are
> Rust-only" note is outdated — see "Known limitations" in the extension README).

**New / fixed in this build (parser `96110 B / c7a5054d`)**:

- **Namespace navigation**: hover on an `as db` alias (which keys that namespace
  *actually* holds) and F12; hover / F12 on a reference such as `db.host` (which file it
  comes from, what kind of definition it is).
- **Unit-scoped colouring now recognises namespace prefixes**: references through
  `as ns` (`@is ui.base.Service`, `ui.base.net.region:`, `&ui.base.base`) are coloured
  too (previously only definitions inside the included file were).
- **Five implementations agree**: `import "x.sml"` ≡ `include "x.sml"` (Rust used to add
  an extra dotted namespace layer and produced a different tree than JS); the JS side
  gained `glob-include` (`include "dir/*.sml"`) and dotted module names
  (`import ui.buttons`).
- **Diagnostics land in the right place**: a missing include target no longer paints a red
  squiggle onto a comment dozens of lines away; errors inside an included file are
  attributed to the `include` line that pulled it in, with the target file named in the
  message.

Reference implementations for each language (Rust `swsml`, etc.) ship with the
source repository — see the [home page](/en/).
