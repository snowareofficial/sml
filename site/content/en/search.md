---
title: "Search"
translationKey: "search"
---

# Search the textbook

The index is generated **at build time**
(`site/tools/gen_search_index.py` → `static/search-index.json`); the page only filters it locally:
no network calls, no tracking, no third-party search library.

<div id="search-app">
  <p>Loading the index… (if this never fills in, run <code>python site/tools/gen_search_index.py</code> first)</p>
</div>

## What is indexed

- Every textbook chapter (both languages: intro, chapters 1–13, appendix)
- Reading-oriented pages such as the home page, Playground and examples
- **Error codes**: when a query looks like a code (`E-CONTRACT-002`, `E-CONTRACT-*`), the page
  **lazily loads** `errors.json` and lists "code hits" above the prose hits; clicking one deep-links
  to [`/en/errors/#<code>`](/en/errors/). If `errors.json` cannot be fetched it is skipped
  silently and prose hits work as usual
- **Not** the Gitee activity feed or the downloads page — their content is injected at runtime or is
  just links, so hits would be meaningless

Can't find it? Browse the [textbook index](/en/book/) or the
[error code reference](/en/errors/).
