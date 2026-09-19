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
  getText: () => MAIN_TEXT,
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

rmSync(DIR, { recursive: true, force: true });
console.log(failed === 0 ? "\nNAV VERIFY ALL PASS" : `\n${failed} 项失败`);
process.exit(failed ? 1 : 0);
