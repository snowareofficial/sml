---
title: "Chapter 12: smlconv multi-target translator"
translationKey: "book-ch12"
---

# Chapter 12: smlconv multi-target translator

`smlconv` is SML's command-line translator: it turns **one SML document** into Slint, LVGL, XML, SVG, LaTeX, Markdown or HTML in one shot, wires straight into Hugo/Zola doc sites, and can even generate arbitrary text via rule tables — **no glue code**.

> In one line: `swsml` is the library (parser + `sml::emit::*` backends); `smlconv` is the CLI front-end that drives them. This chapter shows how to use it.

> ⚠️ **Experimental**: the CLI surface and emit backends may still change between releases; for production-critical paths, watch the version number.

## 12.1 Install

```bash
# from crates.io (Rust toolchain required)
cargo install smlconv

# or build from this repository
cargo build --release -p smlconv
```

After installing, `smlconv --help` lists every option.

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
smlconv -i doc.sml --to md
```

Output (excerpt):

```markdown
# My Document

## Intro

Write SML once, translate everywhere.
```

Omit `-i` to read from stdin, omit `-o` to write to stdout, so piping works too:

```bash
cat doc.sml | smlconv --to md
```

## 12.3 Translate to more targets

`--to` accepts: `md`/`markdown`, `xml`, `svg`, `latex`, `slint`, `lvgl`, `html`, `custom`, `sml`.

| Target | Command | Typical use |
|--------|---------|-------------|
| Markdown | `smlconv -i d.sml --to md` | docs, README |
| XML | `smlconv -i d.sml --to xml` | data exchange, config export |
| SVG | `smlconv -i d.sml --to svg` | diagrams, visualization |
| LaTeX | `smlconv -i d.sml --to latex` | papers, typesetting |
| Slint | `smlconv -i d.sml --to slint -o ui.slint` | **describe UI in SML, generate Slint** |
| LVGL | `smlconv -i d.sml --to lvgl -o ui.xml` | embedded screens (LVGL v8.3+ native XML) |
| HTML | `smlconv -i d.sml --to html` | web fragments |

## 12.4 In practice: describe a UI in SML, emit Slint

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
smlconv -i panel.sml --to slint -o panel.slint
```

Open `panel.slint` in the Slint designer to preview. `smlconv` maps SML's block structure onto Slint components and controls, so you no longer hand-roll parsers and templates per UI framework.

> For the full SML→Slint field conventions, see `swsml`'s `sml::emit::to_slint` docs.

## 12.5 Doc-site automation: Hugo / Zola

Feed SML straight into a static-site generator; it emits front-matter `.md` files:

```bash
# Hugo: generate content/zh/docs/<name>.md
smlconv -i doc.sml --hugo ./site --hugo-lang zh --hugo-section docs

# Zola: generate .md with TOML front matter
smlconv -i doc.sml --zola ./content --zola-section docs
```

In `--hugo` / `--zola` mode, `-o` is ignored and files are written by input filename (or the name from `@feature base`), dropping one manual step from your publish pipeline.

## 12.6 Custom generators: rule tables for arbitrary text

`--to custom` plus `--custom-rules` points at an SML rule table describing "what to match, what to emit", letting you render Dockerfiles, scaffolds or any text without a heavyweight templating engine:

```bash
smlconv -i data.sml --to custom --custom-rules rules.sml -o out.txt
```

The rule table is SML too — you describe both the data and the generation logic in the same language.

## 12.7 Hands-on

Take any SML config you have and try translating it to different targets to feel "write once, use everywhere":

```bash
smlconv -i your.sml --to xml
smlconv -i your.sml --to svg
smlconv -i your.sml --to latex
```

→ [Appendix: SML vs JSON/YAML/TOML](/en/book/appendix)
