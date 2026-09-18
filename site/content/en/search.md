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
- **Not** the Gitee activity feed or the downloads page — their content is injected at runtime or is
  just links, so hits would be meaningless

Can't find it? Browse the [textbook index](/en/book/) or the
[error code reference](/en/errors/).
