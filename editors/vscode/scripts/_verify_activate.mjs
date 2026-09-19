// 激活冒烟测试：在 node 里用**最小 mock** 真跑一遍 `activate()`。
//
// 为什么必须有这一步：`_verify_ext.mjs` 只加载桥接层（sml-parse.mjs），**从不加载
// extension.js**。于是「顶层读了宿主里已不存在的 API」这类错误它完全看不见 ——
// 而那类错误的后果是**整个扩展模块加载失败、activate 从不执行**（provider / 命令 /
// 输出面板全都没注册），界面上表现为「悬停 / 右键菜单全都没反应」，报错只在扩展主机日志里。
// 本仓库真栽过：VS Code 1.138 的宿主里没有 `vscode.InsertTextFormat`（HANDOFF §22.12）。
//
// 纪律：**mock 的 API 面必须对齐目标宿主，而不是对齐 @types/vscode**。所以这里故意
// **不提供** `InsertTextFormat` —— 目标宿主没有的东西，mock 也不该有。
//
// 用法：node scripts/_verify_activate.mjs   （失败即退出码 1）
import Module from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { pathToFileURL } from "node:url";

const ROOT = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
let failed = 0;
const check = (ok, label, extra = "") => {
  if (!ok) failed++;
  console.log(`  ${ok ? "✓" : "✗ 失败"} ${label}${extra ? "  " + extra : ""}`);
};

// ---------------- 最小 vscode mock（对齐 VS Code 1.138 的可用 API） ----------------
const Disposable = () => ({ dispose() {} });
class Position { constructor(line, character) { this.line = line; this.character = character; }
  translate(a, b) { return a && typeof a === "object"
    ? new Position(this.line + (a.lineDelta || 0), this.character + (a.characterDelta || 0))
    : new Position(this.line + (a || 0), this.character + (b || 0)); } }
class Range { constructor(a, b, c, d) { if (a instanceof Position) { this.start = a; this.end = b; } else { this.start = new Position(a, b); this.end = new Position(c, d); } } }
class Uri { constructor(p) { this.fsPath = p; this.scheme = "file"; } static file(p) { return new Uri(p); } toString() { return "file:///" + String(this.fsPath).replace(/\\/g, "/"); } }
class MarkdownString { constructor(v) { this.value = v; } }

const registered = { providers: 0, commands: [] };
const updates = [];   // 记录对设置的写入（用来断言「图标主题不会被切成仅 .sml 那个」）
const context = {
  subscriptions: { push: (...x) => x.length },
  extensionPath: ROOT,
  extension: { packageJSON: { version: "test" } },
  // ⚠️ 必须给 globalState/workspaceState：真实宿主一定提供，早先这份 mock 没给，
  // 而进程又恰好在异步提示跑起来之前就 exit 了 ⇒ 洞里藏了个假绿灯（这次补上等待才暴露）。
  globalState: {
    _m: new Map(),
    get(k) { return this._m.get(k); },
    update(k, v) { this._m.set(k, v); return Promise.resolve(); },
  },
  workspaceState: {
    _m: new Map(),
    get(k) { return this._m.get(k); },
    update(k, v) { this._m.set(k, v); return Promise.resolve(); },
  },
};
const vscode = {
  Position, Range, Uri, MarkdownString,
  Hover: class { constructor(c, r) { this.contents = c; this.range = r; } },
  Location: class { constructor(u, r) { this.uri = u; this.range = r; } },
  Diagnostic: class { constructor(r, m, s) { this.range = r; this.message = m; this.severity = s; } },
  CompletionItem: class { constructor(l) { this.label = l; } },
  SnippetString: class { constructor(v) { this.value = v; } },
  ThemeColor: class { constructor(id) { this.id = id; } },
  TextEdit: { replace: (r, t) => ({ range: r, newText: t }) },
  // 枚举：按 1.138 的实际可用面给（⚠️ 故意不给 InsertTextFormat）
  CompletionItemKind: new Proxy({}, { get: (t, k) => String(k) }),
  DiagnosticSeverity: { Error: 0, Warning: 1, Information: 2, Hint: 3 },
  StatusBarAlignment: { Left: 1, Right: 2 },
  OverviewRulerLane: { Left: 1, Center: 2, Right: 4, Full: 7 },
  DecorationRangeBehavior: { ClosedClosed: 1 },
  ConfigurationTarget: { Global: 1, Workspace: 2 },
  EndOfLine: { LF: 1, CRLF: 2 },
  workspace: {
    workspaceFolders: [{ uri: Uri.file(ROOT), name: "ws", index: 0 }],
    textDocuments: [],
    // 记录 update 调用：图标主题那条断言全靠它（见下）
    getConfiguration: (section) => ({
      get: (k, d) => (section === "workbench" && k === "iconTheme" ? "" : d),
      update: async (k, v) => { updates.push([`${section}.${k}`, v]); },
    }),
    onDidOpenTextDocument: Disposable, onDidChangeTextDocument: Disposable,
    onDidSaveTextDocument: Disposable, onDidCloseTextDocument: Disposable,
    onDidChangeConfiguration: Disposable, onDidChangeWorkspaceFolders: Disposable,
    findFiles: async () => [],
    fs: { readFile: async () => Buffer.from("") },
    asRelativePath: (u) => (u && u.fsPath) || String(u),
  },
  window: {
    activeTextEditor: null,
    visibleTextEditors: [],
    createTextEditorDecorationType: () => Disposable(),
    createStatusBarItem: () => ({ show() {}, hide() {}, dispose() {} }),
    createOutputChannel: () => ({ appendLine() {}, append() {}, show() {}, hide() {}, clear() {}, dispose() {} }),
    onDidChangeActiveTextEditor: Disposable,
    onDidChangeVisibleTextEditors: Disposable,
    // 图标主题提示会问「是否启用」——这里回「启用」，好把它的**实际动作**断言出来
    showInformationMessage: async () => "启用",
    showWarningMessage: async () => undefined,
    showErrorMessage: async () => undefined,
    showQuickPick: async () => undefined,
    showInputBox: async () => undefined,
    withProgress: async (o, fn) => fn({ report() {} }),
  },
  languages: {
    createDiagnosticCollection: () => ({ set() {}, delete() {}, clear() {}, dispose() {} }),
    registerCompletionItemProvider: () => { registered.providers++; return Disposable(); },
    registerHoverProvider: () => { registered.providers++; return Disposable(); },
    registerDefinitionProvider: () => { registered.providers++; return Disposable(); },
    registerDocumentFormattingEditProvider: () => { registered.providers++; return Disposable(); },
    getLanguages: async () => ["sml", "plaintext"],
    setTextDocumentLanguage: async (d, l) => { d.languageId = l; return d; },
  },
  commands: {
    registerCommand: (id) => { registered.commands.push(id); return Disposable(); },
    executeCommand: async () => undefined,
    getCommands: async () => registered.commands,
  },
  version: "1.138.0",
};

// ---------------- 用 mock 载入并 activate ----------------
const origLoad = Module._load;
Module._load = function (request) {
  if (request === "vscode") return vscode;
  return origLoad.apply(this, arguments);
};

console.log("=== 激活冒烟（mock 对齐 VS Code 1.138：**不提供** InsertTextFormat）===");
let err = null;
try {
  const ext = (await import(pathToFileURL(path.join(ROOT, "src", "extension.js")).href)).default;
  ext.activate(context);
} catch (e) {
  err = e;
}
check(err === null, "activate() 不抛异常", err ? String(err && err.message) : "");
check(registered.providers === 4, "注册了 4 个 provider（补全/悬浮/跳转/格式化）", `实际 ${registered.providers}`);
for (const id of ["sml.selfCheck", "sml.specialHighlight", "sml.applySpecialColor"]) {
  check(registered.commands.includes(id), `注册了命令 ${id}`);
}
check(!("InsertTextFormat" in vscode), "mock 确实没有 InsertTextFormat（对齐 1.138）");

// —— 图标主题：提示语与行为必须一致 ——
// 真踩过：提示写「继承现有图标集，只影响 .sml」，代码却设了 `sml-icons`（**仅 .sml**）
// ⇒ 用户一按「启用」，整个工作区的文件图标全没了。文案是对的、代码是错的，极难发现。
await new Promise((r) => setTimeout(r, 60));   // 让 activate 里那个异步提示跑完
const themeUpdates = updates.filter(([k]) => k === "workbench.iconTheme");
check(
  themeUpdates.length === 1 && themeUpdates[0][1] === "sml-icons-seti",
  "「启用」后切到「SML Icons + Seti」（合并主题）",
  JSON.stringify(themeUpdates)
);
check(!updates.some(([, v]) => v === "sml-icons"),
  "绝不自动切到「仅 .sml」（那会让其他文件没有图标）");
check(registered.commands.includes("sml.selectIconTheme"), "注册了「文件图标主题」命令（可切回）");

console.log(failed === 0 ? "\nACTIVATE SMOKE ALL PASS" : `\n${failed} 项失败`);
process.exit(failed ? 1 : 0);
