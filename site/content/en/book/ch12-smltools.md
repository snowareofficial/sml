---
title: "Chapter 12: smltools multi-target translator"
translationKey: "book-ch12"
# 本章原名 ch12-smlconv（crate 改名 smlconv → smltools），保留旧地址跳转
aliases:
  - "/en/book/ch12-smlconv/"
---

# Chapter 12: smltools multi-target translator

`smltools` is SML's command-line translator: it turns **one SML document** into Slint, LVGL, XML, SVG, LaTeX, Markdown or HTML in one shot, wires straight into Hugo/Zola doc sites, and can even generate arbitrary text via rule tables — **no glue code**.

> In one line: `swsml` is the library (parser + `sml::emit::*` backends); `smltools` is the CLI front-end that drives them. This chapter shows how to use it.

> ⚠️ **Experimental**: the CLI surface and emit backends may still change between releases; for production-critical paths, watch the version number.

## 12.1 Install

```bash
# from crates.io (Rust toolchain required)
cargo install smltools

# or build from this repository
cargo build --release -p smltools
```

After installing, `smltools --help` lists every option.

## 12.2 The simplest case: SML → Markdown

Create `doc.sml`:

```sml
title: My Document
section {
  name: Intro
  body: Write SML once, translate everywhere.
}
```

Translate to Markdown:

```bash
smltools -i doc.sml --to md
```

Output (excerpt):

```markdown
# My Document

## Intro

Write SML once, translate everywhere.
```

Omit `-i` to read from stdin, omit `-o` to write to stdout, so piping works too:

```bash
cat doc.sml | smltools --to md
```

## 12.3 Translate to more targets

`--to` accepts: `md`/`markdown` (default), `json`, `toml`, `xml`, `svg`, `latex`, `slint`,
`lvgl`, `html`, `custom`, `sml`, plus `tmlanguage` / `highlight` for editors.

| Target | Command | Typical use |
|--------|---------|-------------|
| Markdown | `smltools -i d.sml --to md` | docs, README |
| JSON | `smltools -i d.sml --to json` | feed jq / JSON-only toolchains |
| TOML | `smltools -i d.sml --to toml` | Cargo / pyproject ecosystems |
| XML | `smltools -i d.sml --to xml` | data exchange, config export |
| SVG | `smltools -i d.sml --to svg` | diagrams, visualization |
| LaTeX | `smltools -i d.sml --to latex` | papers, typesetting |
| Slint | `smltools -i d.sml --to slint -o ui.slint` | **describe UI in SML, generate Slint** |
| LVGL | `smltools -i d.sml --to lvgl -o ui.xml` | embedded screens (LVGL v8.3+ native XML) |
| HTML | `smltools -i d.sml --to html` | web fragments |
| SML | `smltools -i d.sml --to sml` | normalise / re-layout (idempotent) |

## 12.4 Migration: bring existing configs into SML

`--from` accepts `sml` (default) / `json` / `toml` / `yaml` / `xml`, and **infers it from the
file extension** when omitted: `.json` → json, `.toml` → toml, `.yaml`/`.yml` → yaml,
`.xml`/`.svd` → xml, anything else → sml.

```bash
smltools -i app.json  --to sml > app.sml     # JSON  -> SML
smltools -i conf.yaml --to sml > conf.sml    # YAML  -> SML
smltools -i Cargo.toml --to sml > c.sml      # TOML  -> SML
smltools -i chip.svd   --to sml > chip.sml   # XML / CMSIS-SVD -> SML (--from optional)
```

The reverse works too (`--from sml --to json`, and so on). On the JSON side keys are emitted in
**lexicographic order** (`Value::Object` is a `BTreeMap`), so the original writing order is not
preserved — this is a **language convention**, not a defect: object key order is **not guaranteed**
(the Rust reference implementation sorts; C / C++ / JS / Lua keep source order). Use **arrays** when
order matters. See "Cross-implementation divergences and conventions" in the README.

To get a feel for the scale, take a real file: a 432 KB CMSIS-SVD (CH32V103xx) becomes roughly
277 KB / 6500 lines of SML with every register and bitfield intact and diffable. And
`xml→json` is **byte-for-byte identical** to `xml→sml→json`, which is how you know the
conversion lost nothing.

### XML / SVD mapping rules

- Root element → a single top-level key; child elements → keys, **same-name siblings merged into
  an array** (document order preserved)
- **Text-only elements collapse to plain strings**: `<name>PWR</name>` → `name: PWR`
- Attributes → `_attrs`; text goes to `_text` only when the element **also** has attributes or
  children
- Empty elements → `{}`; **every leaf stays a string** (XML has no types — no guessing)
- Namespace prefixes preserved (`xs:name`, `xmlns:xs`); CDATA verbatim; line endings normalised
  per the XML spec (`\r\n` → `\n`)

> Why there is no "automatically fold repeated structures into fragments": SML fragments are
> **parameterless value copies**. Seven DMA channels whose offsets and descriptions all differ
> share no parameter to factor out, so merging them would only reshape the data without shrinking
> it. The size reduction comes from the collapsing and layout rules above, not from magic.

## 12.5 The rest of the toolbox

```bash
# Directory batch: migrate a whole directory (flat into the -o dir, sub-dirs not recreated)
smltools -i conf.d -o out/ --from json --to sml

# Static checks: no translation output; exit code 1 when error-level findings exist
smltools --lint -i doc.sml

# Strip SML-only traces so JSON and friends can consume the output losslessly
smltools -i doc.sml --to json --strip
```

`--lint` only checks SML documents (combining it with `--from json` is an error). `--strip`
removes the internal marker keys `__name`/`__type` and float raw literals — fragments,
contracts, `include`, `$env` and `@when` are all resolved **at parse time**, so the parsed value
is plain data already.

**Defining editor highlighting in SML** is the most unusual item in this chapter: the input is not
data but a highlighting declaration.

```bash
smltools -i my-dialect.sml --to tmlanguage         # upgraded TextMate grammar (stdout)
smltools -i my-dialect.sml --to highlight -o out/  # a bundle of 5 artifacts (directory)
```

`--to highlight` emits `syntaxes/sml.tmLanguage.json`, `vscode/settings.fragment.json` (takes
effect inside the project), `themes/`, `zed/highlights.scm` and `zed/themes/sml.json`. So even
"colour my own dialect" is expressed as SML — the same input format as every other backend.

## 12.6 In practice: describe a UI in SML, emit Slint

SML unifies "config, data, UI structure" into one source of truth. A settings panel, for example:

```sml
Window {
  title: Settings
  width: 480
  height: 320
  controls {
    name: Username
    type: text
    placeholder: Type here
  }
  controls {
    name: Enable notifications
    type: toggle
  }
}
```

```bash
smltools -i panel.sml --to slint -o panel.slint
```

Open `panel.slint` in the Slint designer to preview. `smltools` maps SML's block structure onto Slint components and controls, so you no longer hand-roll parsers and templates per UI framework.

> For the full SML→Slint field conventions, see `swsml`'s `sml::emit::to_slint` docs.

## 12.7 Doc-site automation: Hugo / Zola

Feed SML straight into a static-site generator; it emits front-matter `.md` files:

```bash
# Hugo: generate content/zh/docs/<name>.md
smltools -i doc.sml --hugo ./site --hugo-lang zh --hugo-section docs

# Zola: generate .md with TOML front matter
smltools -i doc.sml --zola ./content --zola-section docs
```

In `--hugo` / `--zola` mode, `-o` is ignored and files are written by input filename (or the name from `@feature base`), dropping one manual step from your publish pipeline.

## 12.8 Custom generators: rule tables for arbitrary text

`--to custom` plus `--custom-rules` points at an SML rule table describing "what to match, what to emit", letting you render Dockerfiles, scaffolds or any text without a heavyweight templating engine:

```bash
smltools -i data.sml --to custom --custom-rules rules.sml -o out.txt
```

The rule table is SML too — you describe both the data and the generation logic in the same language.

## 12.9 Editor: what the VS Code extension gives you

`smltools` handles "file → another format"; the **editor extension** handles "while you are
writing SML". Beyond syntax highlighting, diagnostics, completion and formatting, the 0.4.2
VS Code extension (manual VSIX install, not on the Marketplace) adds a few things that only
make sense for SML:

| Capability | How | Why it matters |
|---|---|---|
| Hover a contract | cursor on a contract name (`@is Server`) | shows the declaration **plus the structure the parser actually produced after filling defaults** — the real result, not a restatement of the declaration |
| Hover a block | cursor on a block name (`primary {`) | shows the block **path** (`database.primary`), the contract it applies and the filled structure |
| Go to definition | `@is Server` → `@contract Server`; `&base` → `@base { }` | fragments/contracts are **document-level names**: same name, jump — no scope analysis |
| Special colors | select a word → right-click "应用特殊颜色" | the color is written into the workspace `HL-cfg.sml` (**itself an SML file**, hand-editable, travels with the repo). By default it applies **only to syntax units** (contract / fragment / type / key / directive), so comments and strings with the same text stay as they are |
| Spotlight | select a word → right-click | lights it up across the whole workspace (check the blast radius before renaming a field) |
| Self-check | right-click "SML: 自检" | when hover / jump / highlighting "does nothing", it dumps every link in the chain (extension version, **bundled parser fingerprint**, registered commands, language mode, document validation, navigability) into the "Output → SML" panel |

> Why these are worth building: SML's **contract layer is an optional overlay** — the same data
> looks different with and without it. Hovering to show the *post-contract* result puts "what
> the parser did" in front of you, which a plain text editor simply cannot do.
>
> Conversely, if it "does nothing", run **SML: 自检** first: the two most painful real causes have
> been "the extension never activated" and "the file's *language mode* is not SML" — both look
> exactly like a broken extension.

## 12.10 Hands-on

Take any SML config you have and try translating it to different targets to feel "write once, use everywhere":

```bash
smltools -i your.sml --to xml
smltools -i your.sml --to svg
smltools -i your.sml --to latex
```

Then go the other way — take an existing JSON / YAML / TOML / XML and migrate it in:

```bash
smltools -i your.json --to sml | head -40     # look at the first 40 lines before committing
smltools -i your.json --to sml -o your.sml
smltools -i your.sml --to json | diff - <(smltools -i your.json --to json)   # round-trip self-check
```

That last line is the important one: you do not have to take "migration loses nothing" on faith —
diff it yourself.

→ [Appendix: SML vs JSON/YAML/TOML](/en/book/appendix)
