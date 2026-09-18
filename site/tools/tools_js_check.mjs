// tools_js_check.mjs — 官网两个小工具（site/static/site-tools.js）的无浏览器验收。
//
//     node site/tools/tools_js_check.mjs
//
// 为什么要有它：这个文件改的是**检索语义与深链**（W12），而站点没有前端测试框架。
// 靠「打开页面看一眼」既不可复现，也验不到通配、hash 深链、高亮这些分支。这里用一个
// 极小的 DOM 桩把脚本真跑起来（`vm` 里编译 = 顺带做了语法检查），再对**渲染结果**
// 断言 —— 与仓库其它地方「不接受只看代码推断的结论」的做法一致。
//
// 覆盖：① 不过滤时卡片数 = 码表条数；② `E-CONTRACT-*` 通配；③ 完整码（含大小写）；
// ④ `#E-PARSE-008` 深链（填搜索框 + 高亮 + 滚进视野）；⑤ `#E-*-008` 通配深链；
// ⑥ 无命中时的空态与计数；⑦ 领域 chip 过滤；⑧ 教科书搜索里也能按码检索并深链回 /errors。
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import vm from "node:vm";

const HERE = import.meta.dirname;
const STATIC = join(HERE, "..", "static");
// 允许指定被测文件：判别实验要拿 HEAD 版（`git show HEAD:site/static/site-tools.js`）
// 跑**同一份**断言，必须红给你看 —— 这条路与 C/JS 两批的做法一致。
const SRC = readFileSync(process.argv[2] || join(STATIC, "site-tools.js"), "utf8");
const ERRORS = JSON.parse(readFileSync(join(STATIC, "errors.json"), "utf8"));
const INDEX = JSON.parse(readFileSync(join(STATIC, "search-index.json"), "utf8"));

let failed = 0;
const check = (name, cond, extra) => {
  if (!cond) failed++;
  console.log(`${cond ? "ok  " : "FAIL"}  ${name}${extra !== undefined && !cond ? "  <- " + extra : ""}`);
};

// ───────────────────────── 极简 DOM 桩 ─────────────────────────
function makeEl(tag) {
  const e = {
    tagName: tag, children: [], dataset: {}, style: {},
    href: "", title: "", type: "", placeholder: "", value: "", id: "",
    _text: "", _l: {}, _scrolled: false, _classes: new Set(),
    get className() { return [...this._classes].join(" "); },
    set className(v) { this._classes = new Set(String(v).split(/\s+/).filter(Boolean)); },
    get textContent() { return this._text; },
    set textContent(v) { this._text = String(v); this.children.length = 0; },
    appendChild(c) { this.children.push(c); if (c.id) REG[c.id] = c; return c; },
    addEventListener(t, fn) { this._l[t] = fn; },
    setAttribute() {},
    scrollIntoView() { this._scrolled = true; },
  };
  e.classList = {
    add: (c) => e._classes.add(c),
    remove: (c) => e._classes.delete(c),
    contains: (c) => e._classes.has(c),
  };
  return e;
}
const REG = {};
const CONTAINERS = {};
let winListeners = {};
const docListeners = {};

function makeSandbox() {
  winListeners = {};
  const location = { hash: "" };
  const document = {
    readyState: "complete",
    body: { dataset: { root: "", errorsUrl: "errors.json", searchUrl: "search-index.json" } },
    documentElement: { lang: "zh" },
    head: makeEl("head"),
    createElement: (t) => makeEl(t),
    getElementById: (id) => REG[id] || CONTAINERS[id] || null,
    addEventListener: (t, fn) => { docListeners[t] = fn; },
  };
  const window = {
    location,
    addEventListener: (t, fn) => { winListeners[t] = fn; },
  };
  const sandbox = {
    document, window, location, console,
    setTimeout, clearTimeout,
    fetch: (url) => {
      const key = String(url).replace(/^.*\//, "");
      const data = key === "errors.json" ? ERRORS : INDEX;
      return Promise.resolve({ ok: true, json: () => Promise.resolve(data) });
    },
  };
  sandbox.globalThis = sandbox;
  return sandbox;
}

const flush = async (n = 6) => { for (let i = 0; i < n; i++) await new Promise((r) => setTimeout(r, 0)); };
const fire = (el, type) => { if (el && el._l[type]) el._l[type](); };
const allCards = (app) => {
  const out = [];
  const walk = (n) => {
    for (const c of n.children) { if (c._classes.has("code-card")) out.push(c); walk(c); }
  };
  walk(app);
  return out;
};
const findChip = (app, prefix) => {
  let hit = null;
  const walk = (n) => {
    for (const c of n.children) {
      if (c._classes.has("tool-chip") && c.textContent.startsWith(prefix)) hit = c;
      walk(c);
    }
  };
  walk(app);
  return hit;
};
// 输入框在 tool-bar 里（不是 app 的直接子节点），故一律递归找
const findByClass = (app, cls) => {
  let hit = null;
  const walk = (n) => {
    for (const c of n.children) { if (!hit && c._classes.has(cls)) hit = c; walk(c); }
  };
  walk(app);
  return hit;
};
const setInput = (app, v) => {
  const input = findByClass(app, "tool-input");
  input.value = v;
  fire(input, "input");
  return input;
};

// ───────────────────────── 场景一：错误码页 ─────────────────────────
{
  const app = makeEl("div");
  CONTAINERS["errors-app"] = app;
  const sb = makeSandbox();
  vm.runInNewContext(SRC, sb, { filename: "site-tools.js" });   // 语法错误会在这里抛
  await flush();

  check("① 不过滤：卡片数 = 码表条数", allCards(app).length === ERRORS.codes.length,
    `${allCards(app).length} vs ${ERRORS.codes.length}`);

  setInput(app, "E-CONTRACT-*");
  let cards = allCards(app);
  const wantContract = ERRORS.codes.filter((c) => c.id.startsWith("E-CONTRACT-")).length;
  check("② 通配 E-CONTRACT-*", cards.length === wantContract && cards.every((c) => c.id.startsWith("E-CONTRACT-")),
    `${cards.length} vs ${wantContract}`);

  setInput(app, "e-contract-002");   // 小写也认
  cards = allCards(app);
  check("③ 完整码（小写）", cards.length === 1 && cards[0].id === "E-CONTRACT-002", cards.map((c) => c.id).join(","));

  // 深链：#E-PARSE-008 → 填搜索框 + 只留一条 + 高亮 + 滚进视野
  sb.window.location.hash = "#E-PARSE-008";
  fire({ _l: winListeners }, "hashchange");
  cards = allCards(app);
  const input = findByClass(app, "tool-input");
  check("④ 深链 #E-PARSE-008 填进搜索框", input.value === "E-PARSE-008", input.value);
  check("④ 深链只留一条并高亮 + 滚进视野",
    cards.length === 1 && cards[0].id === "E-PARSE-008" &&
    cards[0]._classes.has("target") && cards[0]._scrolled === true,
    `n=${cards.length} target=${cards[0] && cards[0]._classes.has("target")} scroll=${cards[0] && cards[0]._scrolled}`);
  check("④ 卡片 id = 码本身（浏览器原生锚点可用）", !!REG["E-PARSE-008"]);

  // 通配深链
  sb.window.location.hash = "#E-*-008";
  fire({ _l: winListeners }, "hashchange");
  cards = allCards(app);
  check("⑤ 通配深链 #E-*-008", cards.length > 0 && cards.every((c) => c.id.endsWith("-008")),
    cards.map((c) => c.id).join(","));

  // 空态
  setInput(app, "zzz-不存在");
  cards = allCards(app);
  const none = !!findByClass(app, "tool-none");
  check("⑥ 无命中：空态 + 计数为 0", cards.length === 0 && none, `cards=${cards.length} none=${none}`);

  // 领域 chip
  setInput(app, "");
  const chip = findChip(app, "PARSE");
  fire(chip, "click");
  cards = allCards(app);
  const wantParse = ERRORS.codes.filter((c) => c.domain === "PARSE").length;
  check("⑦ 领域 chip 过滤 PARSE", chip && cards.length === wantParse, `${cards.length} vs ${wantParse}`);
  fire(chip, "click");   // 清掉过滤，免得影响后续
}

// ───────────────────────── 场景二：教科书搜索里按码检索 ─────────────────────────
{
  const app = makeEl("div");
  CONTAINERS["search-app"] = app;
  const sb = makeSandbox();
  vm.runInNewContext(SRC, sb, { filename: "site-tools.js" });
  await flush();

  const input = findByClass(app, "tool-input");
  input.value = "E-CONTRACT-*";
  fire(input, "input");
  // ⚠️ 搜索框有 **120ms 防抖**（`setTimeout(paint, 120)`），故必须真等过一个防抖窗口；
  // 码表又是懒加载的（第一拍 allCodes 还是 null，拿到后再 paint 一次）。
  await new Promise((r) => setTimeout(r, 250));
  await flush(10);
  const hits = [];
  const walk = (n) => { for (const c of n.children) { if (c._classes.has("hit")) hits.push(c); walk(c); } };
  walk(app);
  const codeHits = hits.filter((h) => String(h.href).indexOf("errors/#E-CONTRACT-") >= 0);
  check("⑧ 教科书搜索按码检索并深链回 /errors",
    codeHits.length > 0 && codeHits.length === ERRORS.codes.filter((c) => c.id.startsWith("E-CONTRACT-")).length,
    `codeHits=${codeHits.length}`);
}

console.log(failed === 0 ? "\nALL PASS" : `\n${failed} FAILED`);
process.exit(failed ? 1 : 0);
