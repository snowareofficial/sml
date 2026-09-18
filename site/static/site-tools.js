/* 官网的两个小工具（零依赖，与主题一致）：
 *   1) 错误码查询    —— 数据来自 errors.json（由 errors/gen_json.py 生成）
 *   2) 教科书搜索    —— 数据来自 search-index.json（由 site/tools/gen_search_index.py 生成）
 *
 * 为什么不用 Algolia / lunr / Fuse 之类：本站是零依赖自包含主题，两件工具的检索需求
 * 都很小（135 条码 / 几十页书），**子串检索对中文本来就好用**，引一个检索库不划算。
 * 索引在构建期生成，运行时只做一次 fetch + 内存过滤，不联网、不追踪。
 *
 * 两个 widget 各自按「容器是否存在」自动初始化，所以同一个脚本可以在所有页面无脑引入。
 */
(function () {
  "use strict";

  var body = document.body;
  var root = (body && body.dataset && body.dataset.root) || "";

  function el(tag, cls, text) {
    var n = document.createElement(tag);
    if (cls) n.className = cls;
    if (text != null) n.textContent = text;
    return n;
  }

  function getJSON(url) {
    return fetch(url, { credentials: "omit" }).then(function (r) {
      if (!r.ok) throw new Error(url + " -> HTTP " + r.status);
      return r.json();
    });
  }

  /* ------------------------------------------------------------------ *
   * 1) 错误码查询
   * ------------------------------------------------------------------ */

  /* W12：码查询的通配支持。
   *
   * 三种写法各有用途：
   *   ① 关键词（默认）：在码 / 标题 / 规范文案 / 领域 / 备注 / 实现列表里做**子串**匹配
   *      —— 中文天然适用，不需要分词；
   *   ② 完整码或前缀（`E-PARSE-008` / `E-CONTRACT-`）：就是子串的特例，无需特判；
   *   ③ **通配**（`E-CONTRACT-*`、`E-*-008`）：只对**码本身**匹配（`^…$` 全匹配），
   *      避免把备注里恰好出现的词也扫进来。
   * 返回 null 表示「按普通关键词走」。 */
  function codeQueryPattern(q) {
    if (q.indexOf("*") < 0) return null;
    var esc = q.replace(/[.+?^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*");
    return new RegExp("^" + esc + "$");
  }

  /* W12：深链的解析。`/errors/#E-PARSE-008` 与 `/errors/#E-CONTRACT-*` 都认：
   * 前者定位到唯一一条并高亮，后者直接把表格过滤成该族。 */
  function hashQuery() {
    var h = (window.location.hash || "").replace(/^#/, "");
    if (!h) return "";
    try { return decodeURIComponent(h); } catch (e) { return h; }
  }

  /* 查询串是否「像错误码」（用于教科书搜索里顺带列出码命中，见 initTextbookSearch） */
  function looksLikeCode(q) {
    return /\*/.test(q) || /^[ewi]-[a-z]+(-[0-9]*)?$/i.test(q);
  }

  function initErrorLookup() {
    var app = document.getElementById("errors-app");
    if (!app) return;
    var lang = document.documentElement.lang === "en" ? "en" : "zh";

    getJSON(body.dataset.errorsUrl || root + "errors.json")
      .then(function (data) {
        renderErrorLookup(app, data, lang);
      })
      .catch(function (e) {
        app.appendChild(el("p", "tool-error", (lang === "en" ? "Failed to load the code table: " : "错误码表加载失败：") + e.message));
      });
  }

  function renderErrorLookup(app, data, lang) {
    var T = lang === "en"
      ? {
          ph: "Search code / keyword (e.g. E-CONTRACT-002, contract, depth)",
          all: "All domains",
          count: function (n, total) { return n + " / " + total + " codes"; },
          none: "Nothing matched.",
          id: "Code", title: "Title", msg: "Canonical message", dom: "Domain",
          sev: "Level", impls: "Implemented in", st: "Coverage",
          note: "Notes", coverage: "Coverage",
          permalink: "Link to this code (deep link)",
          tip: "Search by code or keyword; wildcards match the code itself (E-CONTRACT-*, E-*-008). Every card is deep-linkable: /errors/#E-PARSE-008",
          status: { done: "in all listed impls", partial: "partial across impls" },
          sevName: { error: "error", warning: "warning", info: "info" }
        }
      : {
          ph: "搜索错误码或关键词（如 E-CONTRACT-002、契约、嵌套）",
          all: "全部领域",
          count: function (n, total) { return n + " / " + total + " 条"; },
          none: "没有匹配的条目。",
          id: "错误码", title: "标题", msg: "规范文案", dom: "领域",
          sev: "级别", impls: "已实现", st: "覆盖度",
          note: "备注", coverage: "覆盖度",
          permalink: "本条深链（可直接分享）",
          tip: "按码、前缀（E-CONTRACT-）或通配（E-CONTRACT-*、E-*-008）检索；每张卡片都能深链：/errors/#E-PARSE-008",
          status: { done: "所列实现均已实现", partial: "各实现状态不一" },
          sevName: { error: "错误", warning: "告警", info: "提示" }
        };

    var bar = el("div", "tool-bar");
    var input = el("input", "tool-input");
    input.type = "search";
    input.placeholder = T.ph;
    input.setAttribute("aria-label", T.ph);
    var count = el("span", "tool-count");
    bar.appendChild(input);
    bar.appendChild(count);

    // 领域筛选：按钮由数据里的 domains 生成，加一个「全部」
    var domBar = el("div", "tool-chips");
    var activeDom = "";
    var doms = Object.keys(data.domains || {});
    var chips = [{ k: "", label: T.all }].concat(
      doms.map(function (d) { return { k: d, label: d + " · " + data.domains[d] }; })
    );
    chips.forEach(function (c) {
      var b = el("button", "tool-chip" + (c.k === "" ? " active" : ""), c.label);
      b.type = "button";
      b.addEventListener("click", function () {
        activeDom = c.k;
        Array.prototype.forEach.call(domBar.children, function (x) { x.classList.remove("active"); });
        b.classList.add("active");
        paint();
      });
      domBar.appendChild(b);
    });

    var host = el("div");
    app.appendChild(bar);
    app.appendChild(domBar);
    app.appendChild(host);

    var note = el("p", "tool-note",
      T.coverage + "：" + data.coverage + "（" + data.count + "）");
    app.appendChild(note);
    app.appendChild(el("p", "tool-note", T.tip));

    function paint() {
      var q = input.value.trim().toLowerCase();
      var pat = codeQueryPattern(q);   // W12：含 `*` 时只匹配码本身
      // W12：查询本身「像码」（`E-CONTRACT-002` / `E-CONTRACT-`）时也**只匹配码** ——
      // 否则备注里引用了这个码的其它条目会一起冒出来（实测 `e-contract-002` 会带出
      // `E-CONTRACT-006`，因为它的 note 里写着「不能与 E-CONTRACT-002 共用」）。
      // 关键词（如「契约」「嵌套」）仍走全字段子串 —— 中文按子串本来就好用。
      var codeOnly = !!pat || looksLikeCode(q);
      var target = hashQuery().toLowerCase();
      var rows = data.codes.filter(function (c) {
        if (activeDom && c.domain !== activeDom) return false;
        if (!q) return true;
        if (codeOnly) {
          return pat ? pat.test(c.id.toLowerCase()) : c.id.toLowerCase().indexOf(q) >= 0;
        }
        return (c.id + " " + c.title + " " + c.msg + " " + c.domain + " " + (c.note || "") + " " + c.impls.join(" "))
          .toLowerCase().indexOf(q) >= 0;
      });
      count.textContent = T.count(rows.length, data.codes.length);
      host.textContent = "";
      if (!rows.length) {
        host.appendChild(el("p", "tool-none", T.none));
        return;
      }
      rows.forEach(function (c) {
        var card = el("div", "code-card sev-" + c.severity);
        // W12 深链：卡片 id = 码本身，于是 /errors/#E-PARSE-008 能直接被浏览器锚定；
        // 命中 hash 的那条额外高亮（paint 末尾再 scrollIntoView）。
        card.id = c.id;
        if (target && c.id.toLowerCase() === target) card.classList.add("target");
        var head = el("div", "code-head");
        var idLink = el("a", "code-id", c.id);
        idLink.href = "#" + c.id;
        idLink.title = T.permalink;
        head.appendChild(idLink);
        head.appendChild(el("span", "code-title", c.title));
        head.appendChild(el("span", "code-badge", T.sevName[c.severity] || c.severity));
        card.appendChild(head);

        var dl = el("dl", "code-meta");
        function row(k, v) {
          dl.appendChild(el("dt", null, k));
          dl.appendChild(el("dd", null, v));
        }
        row(T.msg, c.msg);
        row(T.impls, c.impls.join(" / "));
        row(T.st, T.status[c.status] || c.status);
        if (c.note) row(T.note, c.note);
        card.appendChild(dl);
        host.appendChild(card);
      });
      // hash 命中的那条滚进视野（表格是异步渲染的，浏览器自己的锚定来不及生效）
      if (target) {
        var hit = document.getElementById(hashQuery());
        if (hit && hit.scrollIntoView) hit.scrollIntoView({ block: "center" });
      }
    }

    input.addEventListener("input", paint);
    // W12 深链：`/errors/#E-PARSE-008`（或通配 `#E-CONTRACT-*`）进来时先填进搜索框再过滤；
    // 之后点卡片上的码链接也会改 hash，靠 hashchange 复用同一条路径。
    function applyHash() {
      var h = hashQuery();
      if (!h) return;
      input.value = h;
      paint();
    }
    if (window.addEventListener) window.addEventListener("hashchange", applyHash);
    if (window.location.hash) applyHash(); else paint();
  }

  /* ------------------------------------------------------------------ *
   * 2) 教科书搜索
   * ------------------------------------------------------------------ */

  function initTextbookSearch() {
    var app = document.getElementById("search-app");
    if (!app) return;
    var lang = document.documentElement.lang === "en" ? "en" : "zh";
    var T = lang === "en"
      ? { ph: "Search the textbook (e.g. contract, include, smltools)", hint: "Matches titles, headings and body text. Chinese queries match by substring. Error codes are looked up too (E-CONTRACT-*, E-PARSE-008).", none: "Nothing found.", n: function (n) { return n + " hit(s)"; }, codeHits: function (n) { return "Matching error codes: " + n; } }
      : { ph: "搜索教科书（如 契约、include、smltools）", hint: "标题、小标题与正文都会匹配；中文按子串匹配。错误码也一并检索（E-CONTRACT-*、E-PARSE-008）。", none: "没有找到。", n: function (n) { return n + " 条结果"; }, codeHits: function (n) { return "错误码命中：" + n + " 条"; } };

    getJSON(body.dataset.searchUrl || root + "search-index.json")
      .then(function (data) {
        var entries = data.entries || [];
        var bar = el("div", "tool-bar");
        var input = el("input", "tool-input");
        input.type = "search";
        input.placeholder = T.ph;
        input.setAttribute("aria-label", T.ph);
        var count = el("span", "tool-count");
        bar.appendChild(input);
        bar.appendChild(count);
        app.appendChild(bar);
        app.appendChild(el("p", "tool-note", T.hint));
        var host = el("div");
        app.appendChild(host);

        // 子串匹配 + 加权打分。中文天然适用（无需分词），英文按空白切词做 AND。
        function search(q) {
          var terms = q.toLowerCase().split(/\s+/).filter(Boolean);
          if (!terms.length) return [];
          var out = [];
          for (var i = 0; i < entries.length; i++) {
            var e = entries[i];
            var title = (e.title || "").toLowerCase();
            var heads = (e.headings || []).join(" \u0001 ").toLowerCase();
            var bodyText = (e.text || "").toLowerCase();
            var score = 0, ok = true;
            for (var t = 0; t < terms.length; t++) {
              var term = terms[t], hit = 0;
              if (title.indexOf(term) >= 0) hit += 60;
              if (heads.indexOf(term) >= 0) hit += 25;
              var idx = bodyText.indexOf(term);
              if (idx >= 0) {
                hit += 8;
                // 出现次数封顶，避免长文靠「堆关键词」压过精确命中
                var n = 0, p = idx;
                while (p >= 0 && n < 5) { n++; p = bodyText.indexOf(term, p + term.length); }
                hit += n * 2;
                e.__idx = idx;
              }
              if (!hit) { ok = false; break; }
              score += hit;
            }
            if (ok) out.push({ e: e, score: score });
          }
          out.sort(function (a, b) { return b.score - a.score; });
          return out;
        }

        function snippet(e, q) {
          var text = e.text || "";
          var i = text.toLowerCase().indexOf(q.toLowerCase().split(/\s+/)[0] || "");
          if (i < 0) i = 0;
          var s = Math.max(0, i - 40);
          return (s > 0 ? "…" : "") + text.slice(s, s + 160) + (s + 160 < text.length ? "…" : "");
        }

        // W12：教科书搜索里也能按码与前缀检索。查询「看起来像码」时才去取错误码表
        // （懒加载一次；取不到就静默跳过 —— 正文命中照常可用，不能因为码表挂了就整页空）。
        var allCodes = null, codesTried = false;
        function ensureCodes() {
          if (codesTried) return;
          codesTried = true;
          getJSON(body.dataset.errorsUrl || root + "errors.json")
            .then(function (d) { allCodes = d.codes || []; paint(); })
            .catch(function () { allCodes = []; });
        }
        function matchCodes(q) {
          if (!looksLikeCode(q) || !allCodes) return [];
          var pat = codeQueryPattern(q.toLowerCase());
          var qq = q.toLowerCase();
          return allCodes.filter(function (c) {
            return pat ? pat.test(c.id.toLowerCase()) : c.id.toLowerCase().indexOf(qq) >= 0;
          });
        }

        function paint() {
          var q = input.value.trim();
          host.textContent = "";
          if (!q) { count.textContent = ""; return; }
          if (looksLikeCode(q)) ensureCodes();
          var hits = search(q);
          var codeHits = matchCodes(q);
          count.textContent = T.n(hits.length + codeHits.length);
          if (codeHits.length) {
            host.appendChild(el("p", "tool-note", T.codeHits(codeHits.length)));
            codeHits.slice(0, 20).forEach(function (c) {
              var a = el("a", "hit");
              a.href = root + "errors/#" + c.id;   // 深链回错误码页并定位该条
              a.appendChild(el("div", "hit-title", c.id + " · " + c.title));
              a.appendChild(el("div", "hit-body", c.msg));
              host.appendChild(a);
            });
          }
          if (!hits.length && !codeHits.length) { host.appendChild(el("p", "tool-none", T.none)); return; }
          hits.slice(0, 30).forEach(function (h) {
            var a = el("a", "hit");
            a.href = root + h.e.url;
            a.appendChild(el("div", "hit-title", h.e.title + (h.e.lang === "en" ? "" : "") + " · " + h.e.url));
            a.appendChild(el("div", "hit-body", snippet(h.e, q)));
            host.appendChild(a);
          });
        }
        var timer = null;
        input.addEventListener("input", function () {
          if (timer) clearTimeout(timer);
          timer = setTimeout(paint, 120);
        });
        if (location.hash) { input.value = decodeURIComponent(location.hash.slice(1)); paint(); }
      })
      .catch(function (e) {
        app.appendChild(el("p", "tool-error", (lang === "en" ? "Failed to load the search index: " : "搜索索引加载失败：") + e.message));
      });
  }

  /* 样式从 JS 注入：站点主题的 CSS 是内联在 baseof 里的一整块，
     这里不去改主题（免得动到所有页面），把两个 widget 需要的样式挂在自己的作用域下。 */
  function injectCSS() {
    if (document.getElementById("site-tools-css")) return;
    var css = [
      ".tool-bar{display:flex;gap:10px;align-items:center;margin:14px 0 8px}",
      ".tool-input{flex:1;padding:9px 12px;border-radius:8px;border:1px solid var(--border);background:var(--bg);color:var(--fg);font-size:14px;font-family:inherit}",
      ".tool-input:focus{outline:none;border-color:var(--accent)}",
      ".tool-count{color:var(--fg-dim);font-size:13px;white-space:nowrap}",
      ".tool-chips{display:flex;flex-wrap:wrap;gap:6px;margin-bottom:10px}",
      ".tool-chip{border:1px solid var(--border);background:var(--bg-elev);color:var(--fg-dim);border-radius:999px;padding:3px 10px;font-size:12px;cursor:pointer;font-family:inherit}",
      ".tool-chip:hover{border-color:var(--accent);color:var(--fg)}",
      ".tool-chip.active{background:var(--accent-soft);border-color:var(--accent-soft);color:#fff}",
      ".tool-note{color:var(--fg-dim);font-size:12px;margin:6px 0 14px}",
      ".tool-none{color:var(--fg-dim)}",
      ".tool-error{color:#f85149}",
      ".code-card{border:1px solid var(--border);border-left-width:3px;border-radius:8px;padding:10px 14px;margin-bottom:10px;background:var(--bg)}",
      ".code-card.sev-error{border-left-color:#f85149}",
      ".code-card.sev-warning{border-left-color:#d29922}",
      ".code-card.sev-info{border-left-color:var(--accent)}",
      ".code-head{display:flex;align-items:baseline;gap:10px;flex-wrap:wrap}",
      ".code-id{font-weight:600;color:inherit;text-decoration:none}",
      ".code-id:hover{color:var(--accent);text-decoration:underline}",
      ".code-card.target{border-color:var(--accent);box-shadow:0 0 0 1px var(--accent) inset}",
      ".code-title{color:var(--fg)}",
      ".code-badge{margin-left:auto;font-size:11px;color:var(--fg-dim);border:1px solid var(--border);border-radius:999px;padding:1px 8px}",
      ".code-meta{display:grid;grid-template-columns:max-content 1fr;gap:2px 12px;margin:8px 0 0;font-size:13px}",
      ".code-meta dt{color:var(--fg-dim)}",
      ".code-meta dd{margin:0}",
      ".hit{display:block;border:1px solid var(--border);border-radius:8px;padding:10px 14px;margin-bottom:8px;text-decoration:none;color:inherit}",
      ".hit:hover{border-color:var(--accent)}",
      ".hit-title{color:var(--accent);font-size:14px;margin-bottom:4px}",
      ".hit-body{color:var(--fg-dim);font-size:13px;line-height:1.5}"
    ].join("");
    var st = document.createElement("style");
    st.id = "site-tools-css";
    st.textContent = css;
    document.head.appendChild(st);
  }

  function boot() {
    injectCSS();
    initErrorLookup();
    initTextbookSearch();
  }
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", boot);
  } else {
    boot();
  }
})();
