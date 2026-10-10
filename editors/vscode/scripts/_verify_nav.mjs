// include / import 的**编辑器导航**闸门：真 activate 扩展、真调 provider、用真文件断言。
//
// 为什么单开一个（而不是只往 _verify_ext.mjs 里加字符串断言）：那个文件的注释自己就警告过
// 「判据落在哪儿决定你能不能看见问题」—— 断言"源码里有 provideDefinition 这段字符串"
// 既拦不住"跳错文件"，也拦不住"悬停说反了"。这里直接调用 provider 拿返回值来判。
//
// mock 对齐 VS Code 1.138（**不提供** InsertTextFormat）——与 _verify_activate.mjs 同一口径。
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import Module from "node:module";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
let failed = 0;
const check = (ok, msg, extra = "") => {
  if (!ok) failed++;
  console.log(`${ok ? "✓" : "✗"}   ${msg}${ok ? "" : "   ← " + extra}`);
};

// ---------- 夹具：真目录 + 真文件 ----------
const DIR = path.join(tmpdir(), `sml_nav_${process.pid}`);
rmSync(DIR, { recursive: true, force: true });
mkdirSync(path.join(DIR, "conf.d"), { recursive: true });
const MAIN = path.join(DIR, "main.sml");
const DB = path.join(DIR, "conf.d", "db.sml");
const A = path.join(DIR, "a.sml");
const MAIN_TEXT = [
  "@version v1",
  'include "conf.d/db.sml"',
  'import "a.sml"',
  'include "nope.sml"',
  // 命名空间别名（`as db`）+ 一处 `ns.键` 引用：这两行的悬停与跳转见下面 ⑤–⑧。
  'include "conf.d/db.sml" as db',
  "use: db.host",
  // 通配 / 正则目标：悬停要给出**命中数**（0 命中时解析器静默，这句话是唯一线索）→ ⑪⑫
  'include "conf.d/*.sml"',
  'include "conf.d/nope_*.sml"',
  "x: 1",
  "",
].join("\n");
writeFileSync(MAIN, MAIN_TEXT, "utf8");
writeFileSync(DB, "host: db1\nport: 5432\n", "utf8");
writeFileSync(A, "y: 2\n", "utf8");
const FS = { [MAIN]: MAIN_TEXT, [DB]: "host: db1\nport: 5432\n", [A]: "y: 2\n" };

// ---------- mock vscode ----------
const norm = (p) => String(p).replace(/\\/g, "/");
class Position {
  constructor(line, character) { this.line = line; this.character = character; }
  translate(a, b) {
    if (a && typeof a === "object")
      return new Position(this.line + (a.lineDelta || 0), this.character + (a.characterDelta || 0));
    return new Position(this.line + (a || 0), this.character + (b || 0));
  }
}
class Range {
  constructor(a, b, c, d) {
    if (typeof a === "number") { this.start = new Position(a, b); this.end = new Position(c, d); }
    else { this.start = a; this.end = b; }
  }
}
class Uri {
  constructor(p) { this.fsPath = p; this.scheme = "file"; this.path = "/" + norm(p); }
  static file(p) { return new Uri(p); }
  static joinPath(base, ...segs) { return new Uri(path.join(base.fsPath, ...segs)); }
  toString() { return "file:///" + norm(this.fsPath); }
}
class MarkdownString { constructor(v) { this.value = v; } }
const Disposable = () => ({ dispose() {} });
const providers = [];
const commands = [];
const vscode = {
  Position, Range, Uri, MarkdownString, Disposable,
  Hover: class { constructor(c, r) { this.contents = c; this.range = r; } },
  Location: class { constructor(u, r) { this.uri = u; this.range = r; } },
  Diagnostic: class { constructor(r, m, s) { this.range = r; this.message = m; this.severity = s; } },
  CompletionItem: class { constructor(l) { this.label = l; } },
  SnippetString: class { constructor(v) { this.value = v; } },
  ThemeColor: class { constructor(id) { this.id = id; } },
  TextEdit: { replace: (r, t) => ({ range: r, newText: t }) },
  CompletionItemKind: new Proxy({}, { get: (t, k) => String(k) }),
  DiagnosticSeverity: new Proxy({}, { get: (t, k) => String(k) }),
  StatusBarAlignment: new Proxy({}, { get: (t, k) => String(k) }),
  OverviewRulerLane: new Proxy({}, { get: (t, k) => String(k) }),
  DecorationRangeBehavior: new Proxy({}, { get: (t, k) => String(k) }),
  ConfigurationTarget: new Proxy({}, { get: (t, k) => String(k) }),
  EndOfLine: new Proxy({}, { get: (t, k) => String(k) }),
  version: "1.138.0",
  workspace: {
    workspaceFolders: [{ uri: Uri.file(DIR), name: "ws", index: 0 }],
    textDocuments: [],
    getConfiguration: () => ({ get: (k, d) => d, update: async () => {} }),
    onDidOpenTextDocument: Disposable, onDidChangeTextDocument: Disposable,
    onDidSaveTextDocument: Disposable, onDidCloseTextDocument: Disposable,
    onDidChangeConfiguration: Disposable, onDidChangeWorkspaceFolders: Disposable,
    findFiles: async () => [MAIN, DB, A].map((p) => Uri.file(p)),
    fs: { readFile: async (u) => Buffer.from(FS[u.fsPath] || "", "utf8") },
    asRelativePath: (u) => path.relative(DIR, u.fsPath).split(path.sep).join("/"),
  },
  window: {
    activeTextEditor: null, visibleTextEditors: [],
    createTextEditorDecorationType: () => ({ dispose() {}, key: "" }),
    createStatusBarItem: () => ({ text: "", tooltip: "", command: "", show() {}, hide() {}, dispose() {} }),
    createOutputChannel: () => ({ appendLine() {}, append() {}, show() {}, hide() {}, clear() {}, dispose() {} }),
    onDidChangeActiveTextEditor: Disposable, onDidChangeVisibleTextEditors: Disposable,
    showInformationMessage: async () => undefined, showWarningMessage: async () => undefined,
    showErrorMessage: async () => undefined, showQuickPick: async () => undefined,
    showInputBox: async () => undefined, withProgress: async (o, fn) => fn({ report() {} }),
  },
  languages: {
    createDiagnosticCollection: () => ({ set() {}, delete() {}, clear() {}, dispose() {} }),
    registerCompletionItemProvider: (sel, p) => { providers.push({ kind: "completion", p }); return Disposable(); },
    registerHoverProvider: (sel, p) => { providers.push({ kind: "hover", p }); return Disposable(); },
    registerDefinitionProvider: (sel, p) => { providers.push({ kind: "definition", p }); return Disposable(); },
    registerDocumentFormattingEditProvider: () => Disposable(),
    getLanguages: async () => ["sml"],
    setTextDocumentLanguage: async (d, l) => { d.languageId = l; return d; },
  },
  commands: {
    registerCommand: (id) => { commands.push(id); return Disposable(); },
    executeCommand: async () => undefined,
    getCommands: async () => commands,
  },
};

// ---------- 假 document（只实现 provider 真正用到的成员）----------
const lines = MAIN_TEXT.split("\n");
const doc = {
  uri: Uri.file(MAIN), fileName: MAIN, languageId: "sml", version: 1,
  // ⚠️ `getText()` 必须支持**带 range** 的调用：`provideHover` 用
  // `document.getText(range)` 取光标下的词。此前这个 mock 忽略 range 直接返回全文，
  // 于是「悬停 `ns.键`」这类按词分流的逻辑在测试里永远拿不到词（真实 VSCode 里正常）——
  // 这种 mock 与宿主的偏差会把「功能没生效」误判成「代码写错」。
  getText: (r) =>
    r
      ? MAIN_TEXT.slice(doc.offsetAt(r.start), doc.offsetAt(r.end))
      : MAIN_TEXT,
  lineAt: (pos) => ({ text: lines[pos.line] ?? "" }),
  offsetAt: (pos) => lines.slice(0, pos.line).reduce((n, l) => n + l.length + 1, 0) + pos.character,
  positionAt: (offset) => {
    let n = offset, i = 0;
    while (i < lines.length && n > lines[i].length) { n -= lines[i].length + 1; i++; }
    return new Position(i, n);
  },
  getWordRangeAtPosition: (pos) => {
    const t = lines[pos.line] ?? "";
    const re = /[@&]?[A-Za-z0-9_\u4e00-\u9fa5.\-]+/g;
    let m;
    while ((m = re.exec(t)) !== null) {
      if (pos.character >= m.index && pos.character <= m.index + m[0].length)
        return new Range(pos.line, m.index, pos.line, m.index + m[0].length);
    }
    return null;
  },
};

// ---------- 载入扩展并 activate ----------
const origLoad = Module._load;
Module._load = function (request) {
  if (request === "vscode") return vscode;
  return origLoad.apply(this, arguments);
};
const context = {
  subscriptions: { push() {} },
  extensionPath: ROOT,
  extension: { packageJSON: { version: "0.0.0-test" } },
  globalState: { get: () => undefined, update: async () => {} },
  workspaceState: { get: () => undefined, update: async () => {} },
};
let err = null;
try {
  const ext = (await import(pathToFileURL(path.join(ROOT, "src", "extension.js")).href)).default;
  ext.activate(context);
} catch (e) {
  err = e;
}
console.log("=== include / import 编辑器导航闸门 ===");
check(err === null, "activate() 不抛异常", err && (err.stack || String(err)));

const get = (kind) => providers.find((x) => x.kind === kind)?.p;
const completion = get("completion"), hover = get("hover"), definition = get("definition");
check(!!completion && !!hover && !!definition, "三个 provider 都已注册");

// —— ① 跳转：光标落在 `include "conf.d/db.sml"` 的路径上 ⇒ 跳到该文件 ——
if (definition) {
  const line = lines.findIndex((l) => l.includes("conf.d/db.sml"));
  const col = lines[line].indexOf("db.sml") + 2;
  const loc = await definition.provideDefinition(doc, new Position(line, col));
  check(!!loc, "① 跳转：返回了 Location");
  check(!!loc && norm(loc.uri.fsPath) === norm(DB), "① 跳转：落点是 conf.d/db.sml", loc && norm(loc.uri.fsPath));
}

// —— ② 悬停：同一位置 ⇒ 说明文件路径 + 目标顶层键 ——
if (hover) {
  const line = lines.findIndex((l) => l.includes("conf.d/db.sml"));
  const col = lines[line].indexOf("db.sml") + 2;
  const h = await hover.provideHover(doc, new Position(line, col));
  const md = h && String(h.contents && h.contents.value);
  check(!!md && md.includes("db.sml"), "② 悬停：markdown 含目标文件", md && md.slice(0, 90));
  check(!!md && md.includes("host"), "② 悬停：列出了目标的顶层键（host）", md && md.slice(0, 120));
}

// —— ③ 悬停：目标不存在时如实说"未找到"（不许假装成功）——
if (hover) {
  const line = lines.findIndex((l) => l.includes("nope.sml"));
  const col = lines[line].indexOf("nope.sml") + 2;
  const h = await hover.provideHover(doc, new Position(line, col));
  const md = h && String(h.contents && h.contents.value);
  check(!!md && md.includes("未找到"), "③ 悬停：不存在的目标要明说「未找到」", md && md.slice(0, 90));
}

// —— ④ 补全：在 include 路径位置 ⇒ 列出工作区其它 .sml（相对当前文档目录）——
if (completion) {
  const line = lines.findIndex((l) => l === 'include "conf.d/db.sml"');
  const items = await completion.provideCompletionItems(doc, new Position(line, lines[line].length));
  const labels = items.map((i) => i.label);
  check(labels.includes("a.sml"), "④ 补全：列出 a.sml", JSON.stringify(labels.slice(0, 6)));
  check(labels.includes("conf.d/db.sml"), "④ 补全：列出 conf.d/db.sml（相对文档目录）", JSON.stringify(labels.slice(0, 6)));
  check(!labels.includes("main.sml"), "④ 补全：不列当前文档自身");
}

// —— ⑤ 悬停 `as db` 别名 ⇒ 说清「这个命名空间里有什么键」——
// 此前别名上什么都不显示（路径分支的区间只覆盖引号里的路径），而「ns 里有什么」
// 正是写完 `include … as ns` 之后最常问的问题。
const aliasLine = lines.findIndex((l) => l.includes(" as db"));
const aliasCol = lines[aliasLine].indexOf(" as ") + 4;
if (hover) {
  const h = await hover.provideHover(doc, new Position(aliasLine, aliasCol));
  const md = h && String(h.contents && h.contents.value);
  check(!!md && md.includes("命名空间"), "⑤ 悬停别名：说明这是命名空间", md && md.slice(0, 80));
  check(!!md && md.includes("host"), "⑤ 悬停别名：列出该命名空间下的键（host）", md && md.slice(0, 140));
}

// —— ⑥ 悬停 `db.host` 引用 ⇒ 指出它来自哪个被包含文件的什么定义 ——
const refLine = lines.findIndex((l) => l.includes("db.host"));
const refCol = lines[refLine].indexOf("db.host") + 1;
if (hover) {
  const h = await hover.provideHover(doc, new Position(refLine, refCol));
  const md = h && String(h.contents && h.contents.value);
  check(!!md && md.includes("命名空间引用"), "⑥ 悬停 ns.键：标注为命名空间引用", md && md.slice(0, 80));
  check(!!md && md.includes("db.sml"), "⑥ 悬停 ns.键：给出源文件", md && md.slice(0, 120));
  check(!!md && md.includes("第 1 行"), "⑥ 悬停 ns.键：给出定义所在行（host 在 db.sml 第 1 行）", md && md.slice(0, 160));
}

// —— ⑦ 跳转 `db.host` ⇒ 落到被包含文件里的 `host` 定义 ——
if (definition) {
  const loc = await definition.provideDefinition(doc, new Position(refLine, refCol));
  check(!!loc, "⑦ 跳转 ns.键：返回了 Location");
  check(!!loc && norm(loc.uri.fsPath) === norm(DB), "⑦ 跳转 ns.键：落点是 conf.d/db.sml", loc && norm(loc.uri.fsPath));
  check(!!loc && loc.range.start.line === 0, "⑦ 跳转 ns.键：落在 host 的定义行",
    loc && JSON.stringify(loc.range.start));
}

// —— ⑧ 跳转 `as db` 别名 ⇒ 跳到被包含文件 ——
if (definition) {
  const loc = await definition.provideDefinition(doc, new Position(aliasLine, aliasCol));
  check(!!loc && norm(loc.uri.fsPath) === norm(DB), "⑧ 跳转别名：落点是 conf.d/db.sml",
    loc && norm(loc.uri.fsPath));
}

// —— ⑪⑫ 通配 / 正则目标的悬停：给出命中数；0 命中要明确告警 ——
{
  const globLine = lines.findIndex((l) => l.includes('conf.d/*.sml'));
  const globCol = lines[globLine].indexOf("conf.d/") + 2;
  const h = await hover.provideHover(doc, new Position(globLine, globCol));
  const md = h && String(h.contents && h.contents.value);
  check(!!md && md.includes("命中") && md.includes("db.sml"), "⑪ 通配悬停：给出命中数与被命中的文件", md && md.slice(0, 140));
  const noneLine = lines.findIndex((l) => l.includes("nope_*.sml"));
  const noneCol = lines[noneLine].indexOf("conf.d/") + 2;
  const h2 = await hover.provideHover(doc, new Position(noneLine, noneCol));
  const md2 = h2 && String(h2.contents && h2.contents.value);
  check(!!md2 && md2.includes("一个都没命中"), "⑫ 通配 0 命中：明确告警（解析器会静默展开为空）", md2 && md2.slice(0, 140));
}

// —— ⑨ 单元着色（HL-cfg 的 unit）要认命名空间前缀 ——
{
  const mod = await import(pathToFileURL(path.join(ROOT, "src", "sml-parse.mjs")).href);
  const text = ['include "x.sml" as ui', "@is ui.base.Service", "ui.base.net.region: 1", "&ui.base.base"].join("\n");
  const c = mod.findUnitOccurrences(text, "Service", "contract");
  check(c.length === 1, "⑨ unit=contract 认 `@is ui.base.Service`（带前缀的引用也着色）", JSON.stringify(c));
  const k = mod.findUnitOccurrences(text, "region", "key");
  check(k.length === 1, "⑨ unit=key 认 `ui.base.net.region:`", JSON.stringify(k));
  const f = mod.findUnitOccurrences(text, "base", "fragment");
  check(f.length === 1, "⑨ unit=fragment 认 `&ui.base.base`", JSON.stringify(f));
  // 负对照：不带前缀的老写法必须照旧命中，别把行首锚定改坏
  const c2 = mod.findUnitOccurrences("@is Service\n", "Service", "contract");
  check(c2.length === 1, "⑨ 负对照：`@is Service` 仍命中", JSON.stringify(c2));

  // —— ⑩ 命名空间悬停的两条路径：能解析 ⇒ 真实键；解析失败 ⇒ 标明是近似 ——
  const okText = 'include "n.sml" as n\n';
  const md1 = mod.namespaceHoverMarkdown(okText, "n", { "n.sml": "alpha: 1\nbeta: 2\n" });
  check(md1.includes("alpha") && md1.includes("beta"),
    "⑩ 文档能解析：列出**真实**的命名空间键", md1.slice(0, 120));
  check(!md1.includes("近似"), "⑩ 文档能解析：不该出现「近似」字样", md1.slice(0, 120));
  const md2 = mod.namespaceHoverMarkdown(
    'include "missing.sml" as n\n', "n", {}, ["host", "port"]);
  check(md2.includes("取不到") && md2.includes("近似") && md2.includes("host"),
    "⑩ 解析失败：标明是近似键而不是只说「取不到」", md2.slice(0, 160));
}

rmSync(DIR, { recursive: true, force: true });
console.log(failed === 0 ? "\nNAV VERIFY ALL PASS" : `\n${failed} 项失败`);
process.exit(failed ? 1 : 0);
