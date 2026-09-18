---
title: "搜索"
translationKey: "search"
---

# 搜索教科书

索引在**构建期**生成（`site/tools/gen_search_index.py` → `static/search-index.json`），
页面里只做一次本地过滤：不联网、不追踪、不引第三方检索库。

<div id="search-app">
  <p>正在加载索引……（若长时间无内容，先跑 <code>python site/tools/gen_search_index.py</code>）</p>
</div>

## 索引覆盖范围

- 全部教科书章节（中英各一套：序章、1–13 章、附录）
- 首页、Playground、示例页等阅读型页面
- **不含** Gitee 动态与下载页 —— 它们的正文是运行时注入或纯链接，搜出来没有意义

搜不到想要的？直接看[教科书目录](/book/)或[错误码总表](/errors/)；还可以在
[QQ 群](/gitee/)里问。
