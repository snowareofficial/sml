// SML 语言服务器（LSP over stdio）
//
// 纯 Node、零运行时依赖，复用仓库的 JS 实现（js/sml.mjs 经 vendor 桥接），
// 保证「编辑器里报的错 = 解析器真正报的错」。
//
// 能力：
//   - textDocument.publishDiagnostics（parseSafe 的语法错误 + `@weakdoc` 未找到的信息级提示）
//   - textDocument.completion（指令 / 契约名 / 类型名 / 片段 / 键名 / 模式关键字）
//   - textDocument.definition（@contract 定义 <-> 用法跳转）
//   - textDocument.hover（指令 / 模式关键字说明）
//
// 运行：
//   node editors/lsp/server.mjs
// 任意支持 LSP 的客户端（Neovim / Helix / 任意编辑器）连 stdio 即可。
// VSCode 用户已内置更丰富的扩展（editors/vscode），此服务器面向其它编辑器。

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseSafe } from "../vscode/src/sml-parse.mjs";
import {
  collectContractNames, collectFragmentNames, collectKeys, collectTypeNames,
  collectFeatagNames, collectWeakDocs, missingProfileRefs, parseProfileFields,
} from "../vscode/src/sml-parse.mjs";

const KW = {
  "@contract": "契约定义：为块定义字段类型、枚举、默认值与区间约束。定义本身不进解析结果。",
  "@is": "应用契约：校验当前块并填充缺失字段默认值。契约须先于 @is 定义。",
  "@type": "定义值的格式模式（loom）：用 序列/类/次/名/字面/任一 等描述一个字符串应长什么样。",
  "@feature": "开启/关闭特性；新增特性名一律 <域>.<名> ｜ Enable/disable features; new names always use <domain>.<name>.",
  "@version": "版本集声明（版本号 = 功能集）｜ Edition declaration: a version number names a feature set. ⚠️ `50 \"camelia\" dev` 是规范的开发期写法，但 5.0 功能集尚未启用，当前所有端写入即报 E-FEATURE-004；正式文档请用 v4.",
  "@when": "条件裁剪，需 @feature enable when ｜ Conditional pruning; needs @feature enable when.",
  "@for": "有界循环展开，需 @feature enable for ｜ Bounded loop expansion; needs @feature enable for.",
  "@requires": "契约级依赖声明（Rust 单端；JS 端报 E-CONTRACT-018）｜ Contract-level dependency (Rust only).",
  "@weakbind": "弱绑定：本块按内置契约解释；载荷仍在树里，库不校验（需 type.weak，Rust 单端）｜ Weak binding: data stays in the tree, library does not validate.",
  "@weakdata": "旁路数据：载荷只进旁路表，绝不进主树（需 type.weak，Rust 单端）｜ Side data: payload never enters the main tree.",
  "@weakdoc": "引用 .smlex profile：只写名字，不接受载荷块（需 type.weak，Rust 单端）｜ Reference a .smlex profile by name; no payload block.",
  "from-featag": "字段只能由 featag 落点写入，文档直接写会报错（需 type.featag）｜ Only a featag target may write this key.",
  "__featags": "块头标签落成的保序数组，由解析器产出 ｜ Ordered array of block-head featags, produced by the parser.",
  "null": "外来 JSON 需要它；契约里请用 optional 表示键缺失，default null 在 v5 起禁用 ｜ Prefer optional for a missing key; default null is forbidden from v5.",
  "warn": "契约严格度：未声明字段放行并产出告警 ｜ Contract strictness: allow undeclared fields with a warning.",
  "strict": "契约严格度：未声明字段报错（默认档）｜ Contract strictness: undeclared fields error (default).",
  "dev": "开发期版本集必须带 dev，且只能在 @version 行末尾 ｜ A Dev edition requires a trailing dev marker.",
  "camelia": "5.0 的代号，与 50 永久双向映射 ｜ Codename of 5.0, permanently mapped to 50.",
  "type.featag": "块头 featag 标签（opt-in，Rust 单端）｜ Block-head featag tags (opt-in, Rust only).",
  "lex.ident": "标识符白名单与每契约单一语种（opt-in，Rust 单端）｜ Identifier allowlist (opt-in, Rust only).",
  "type.weak": "弱声明族共用开关（opt-in，Rust 单端）｜ Shared switch for the weak* family (opt-in, Rust only).",
  "loose": "放宽严格性：允许契约未声明的字段。",
  "optional": "字段可选：缺失时不报错。",
  "default": "字段缺失时填充的默认值。",
  "enum": "枚举：取值须来自给定列表，如 enum(公开, 内部, 机密)。",
  "str": "字符串类型。",
  "int": "整数类型。",
  "num": "数值类型（整数或浮点）。",
  "bool": "布尔（true / false）。",
  "序列": "顺序匹配其后各项（模式关键字，值收集为数组）。",
  "类": "字符类：数字/字母/空白/字/任意（模式关键字）。",
  "次": "量词：4 或 \"+\" / \"*\"（模式关键字）。",
  "名": "命名捕获（模式关键字）。",
  "字面": "字面量，精确匹配（模式关键字）。",
  "任一": "多选一分支（模式关键字）。",
  "组": "内联分组（模式关键字）。",
  "用": "引用另一条规则（模式关键字）。",
  "可选": "该项可省略（模式关键字）。",
  "seq": "序列（英文，等价于「序列」）。",
  "class": "字符类（英文，等价于「类」）。",
  "times": "量词（英文，等价于「次」）。",
  "name": "命名捕获（英文，等价于「名」）。",
  "lit": "字面量（英文，等价于「字面」）。",
  "alt": "多选一（英文，等价于「任一」）。",
  "group": "内联分组（英文，等价于「组」）。",
  "use": "引用规则（英文，等价于「用」）。",
  "optional": "可省略（英文，等价于「可选」）。",
};

// ——— 最新标准清单 ｜ current standard registry ———
// FEATURE_NAMES ← rust/sml-feature/src/lib.rs::FEATURES（20 位，只追加不重排）
// EDITIONS      ← 同文件 EDITIONS（10/20/30/40 已发布；50 "camelia" dev 开发中）
// 这里只给提示，不代表本端解析器支持：weak* / featag / 50 是 Rust 单端，
// C / C++ / JS / Lua 按 S9 策略不实现。
const FEATURE_NAMES = [
  "bareword-string", "include", "env", "contract", "fragment", "top-level-array",
  "namespace", "implicit-ns", "multi-include", "glob-include", "regex-include",
  "ext-rewrite", "when", "for", "typed-block", "case-insensitive", "block-name",
  "type.featag", "lex.ident", "type.weak",
];
const EDITIONS = [
  { lit: "v1", desc: "1.0（连写 10）：允许裸词字符串；已发布、永久兼容" },
  { lit: "v2", desc: "2.0（连写 20）：字符串须显式引号；已发布、永久兼容" },
  { lit: "v3", desc: "3.0（连写 30）：与 2.0 同一套特性位；已发布、永久兼容" },
  { lit: "v4", desc: "4.0（连写 40）：片段定义改用显式 type:/name: 参数；最新已发布版" },
  { lit: "50", desc: "5.0 连写（规范形式）：⚠️ 未启用，写入报 E-FEATURE-004（Rust 与 JS 均拒绝）" },
  { lit: '"camelia"', desc: "5.0 代号，与 50 永久双向映射：⚠️ 未启用，写入报 E-FEATURE-004" },
];
const PROFILE_FIELDS = [
  "name", "version", "description", "applies_to", "checks", "hints", "emit",
];

// `.smlex` 三命名空间（项目 / 用户 / 系统）：按名先命中者胜，编辑器只读 hints。
let rootPath = null;
function profileRoots() {
  const roots = [];
  if (rootPath) roots.push(path.join(rootPath, ".smlex", "profiles"));
  if (process.platform === "win32") {
    if (process.env.APPDATA) roots.push(path.join(process.env.APPDATA, "sml", "profiles"));
    if (process.env.ProgramData) roots.push(path.join(process.env.ProgramData, "sml", "profiles"));
  } else {
    const xdg = process.env.XDG_CONFIG_HOME || (process.env.HOME ? path.join(process.env.HOME, ".config") : "");
    if (xdg) roots.push(path.join(xdg, "sml", "profiles"));
    roots.push("/usr/share/sml/profiles");
  }
  return roots;
}
function discoverProfiles() {
  const out = new Map();
  for (const dir of profileRoots()) {
    let files;
    try { files = fs.readdirSync(dir).filter((f) => f.endsWith(".smlex")); } catch { continue; }
    for (const file of files) {
      const name = path.basename(file, ".smlex");
      if (out.has(name)) continue;
      try {
        out.set(name, parseProfileFields(fs.readFileSync(path.join(dir, file), "utf-8")));
      } catch { /* 读不出的 profile 不进补全 */ }
    }
  }
  return out;
}

// ——— LSP 常量 ———
const CompletionItemKind = {
  Text: 1, Method: 2, Function: 3, Constructor: 4, Field: 5, Variable: 6,
  Class: 7, Struct: 8, Module: 9, Property: 10, Keyword: 14, Constant: 21,
  TypeParameter: 25,
};
const DiagnosticSeverity = { Error: 1, Warning: 2, Information: 3, Hint: 4 };

const docs = new Map(); // uri -> text

// ——— 帧读写 ———
// 单条消息字节上限：Content-Length 可由对端随意声称，不设限会被撑爆内存
const MAX_MESSAGE_BYTES = 16 * 1024 * 1024;

let buffer = Buffer.alloc(0);
process.stdin.on("data", (chunk) => {
  buffer = Buffer.concat([buffer, chunk]);
  while (true) {
    const headerEnd = buffer.indexOf("\r\n\r\n");
    if (headerEnd === -1) break;
    const header = buffer.slice(0, headerEnd).toString();
    const m = /Content-Length: (\d+)/i.exec(header);
    if (!m) { buffer = Buffer.alloc(0); break; }
    const len = parseInt(m[1], 10);
    // 上限保护：协议头声称的长度可以被任意伪造，不设限会让本进程
    // 一路 Buffer.concat 直到内存耗尽。超限即丢弃当前缓冲并重新同步。
    if (!Number.isFinite(len) || len < 0 || len > MAX_MESSAGE_BYTES) {
      buffer = Buffer.alloc(0);
      break;
    }
    const start = headerEnd + 4;
    if (buffer.length < start + len) break;
    const body = buffer.slice(start, start + len).toString();
    buffer = buffer.slice(start + len);
    let msg;
    try { msg = JSON.parse(body); } catch { continue; }
    handle(msg);
  }
});

function send(obj) {
  const json = JSON.stringify(obj);
  const payload = `Content-Length: ${Buffer.byteLength(json)}\r\n\r\n${json}`;
  process.stdout.write(payload);
}

function reply(id, result) { send({ jsonrpc: "2.0", id, result }); }
function notify(method, params) { send({ jsonrpc: "2.0", method, params }); }

let msgId = 1;
function log(...a) { /* 调试时打开：process.stderr.write(a.join(' ')+'\n') */ }

// ——— 诊断 ———
function publishDiagnostics(uri, text) {
  const r = parseSafe(text);
  const diags = [];
  if (!r.ok) {
    const pos = r.position || { line: 0, col: 0 };
    const lineText = (text.split("\n")[pos.line] ?? "");
    diags.push({
      range: {
        start: { line: pos.line, character: pos.col },
        end: { line: pos.line, character: Math.max(pos.col + 1, lineText.length) },
      },
      severity: DiagnosticSeverity.Error,
      source: "sml",
      message: r.error,
    });
  }
  // `@weakdoc X` 找不到 ⇒ **Information**（`W-CLI-001`）：只提示，不阻断编辑
  // （与 `smltools` 同码同文案；发现只在编辑器 / 工具侧做，解析器不读文件系统）
  try {
    const known = new Set(discoverProfiles().keys());
    for (const ref of missingProfileRefs(text, known)) {
      diags.push({
        range: {
          start: { line: ref.line, character: ref.col },
          end: { line: ref.line, character: Math.max(ref.endCol, ref.col + 1) },
        },
        severity: DiagnosticSeverity.Information,
        code: "W-CLI-001",
        source: "sml",
        message: `未找到 profile \`${ref.name}\`（项目 / 用户 / 系统三处都没命中）`,
      });
    }
  } catch { /* 发现失败不该让语法诊断一起消失 */ }
  notify("textDocument/publishDiagnostics", { uri, diagnostics: diags });
}

// ——— 补全 ———
function buildCompletions(uri, line, character) {
  const text = docs.get(uri) || "";
  const lines = text.split("\n");
  const linePrefix = lines[line]?.slice(0, character) ?? "";
  const items = [];

  // 指令
  if (/(^|\s)@\w*$/.test(linePrefix) || /^\s*$/.test(linePrefix)) {
    for (const k of Object.keys(KW)) {
      if (k.startsWith("@")) {
        items.push({ label: k, kind: CompletionItemKind.Keyword, detail: "指令：" + KW[k] });
      }
    }
  }
  const contractNames = collectContractNames(text);
  const typeNames = collectTypeNames(text);
  // 块级类型标注：行首契约名
  if (/^\s*\w*$/.test(linePrefix) && !linePrefix.includes("@")) {
    for (const n of contractNames) {
      items.push({ label: n, kind: CompletionItemKind.Struct, detail: "契约名 · 块级类型标注 `<契约名> <块名> {}`" });
    }
  }
  // @is / @is type(
  if (/@is\s+type\(\w*$/.test(linePrefix)) {
    for (const n of contractNames) items.push({ label: n, kind: CompletionItemKind.Struct, detail: "契约名（@is type(契约名)）", insertText: n + ")" });
  } else if (/@is\s+\w*$/.test(linePrefix)) {
    for (const n of contractNames) items.push({ label: n, kind: CompletionItemKind.Struct, detail: "契约名" });
  }
  // 值位置
  if (/:\s*\w*$/.test(linePrefix)) {
    for (const n of contractNames) items.push({ label: n, kind: CompletionItemKind.Struct, detail: "契约（组合）" });
    for (const n of typeNames) items.push({ label: n, kind: CompletionItemKind.TypeParameter, detail: "自定义类型（@type）" });
    for (const n of collectFragmentNames(text)) items.push({ label: "&" + n, kind: CompletionItemKind.Reference, detail: "片段引用", insertText: "&" + n });
    for (const c of ["true", "false", "null"]) items.push({ label: c, kind: CompletionItemKind.Constant, detail: "字面量" });
  }
  // 模式关键字（@type 体内）
  if (/@type[^{]*\{[^}]*$/s.test(text.slice(0, text.split("\n").slice(0, line).join("\n").length + character))) {
    for (const [k, v] of Object.entries(KW)) {
      if (!k.startsWith("@") && (k === "序列" || k === "类" || k === "次" || k === "名" || k === "字面" || k === "任一" || k === "组" || k === "用" || k === "可选" || k === "seq" || k === "class" || k === "times" || k === "name" || k === "lit" || k === "alt" || k === "group" || k === "use")) {
        items.push({ label: k, kind: CompletionItemKind.Keyword, detail: "模式关键字：" + v });
      }
    }
  }
  // 最新标准：带域特性名、版本集、weak*、featag、`.smlex` profile hints
  const profiles = discoverProfiles();
  if (/@feature\s+(?:base|enable|disable|mode|whitelist|blacklist)\s+[\w.\-]*$/.test(linePrefix)) {
    for (const n of FEATURE_NAMES) {
      items.push({ label: n, kind: CompletionItemKind.Property, detail: "特性（新特性名带域）" });
    }
  }
  if (/@version\s+[\w."]*$/.test(linePrefix)) {
    for (const e of EDITIONS) {
      items.push({ label: e.lit, kind: CompletionItemKind.Constant, detail: "版本集：" + e.desc });
    }
  }
  if (/@weakdoc\s+[\w.\-]*$/.test(linePrefix)) {
    for (const [n, p] of profiles) {
      items.push({ label: n, kind: CompletionItemKind.Module, detail: "profile：" + (p.description || n) });
    }
  }
  if (/@weakbind\s+[\w.\-]*$/.test(linePrefix)) {
    for (const n of collectContractNames(text)) {
      items.push({ label: n, kind: CompletionItemKind.Struct, detail: "契约名（弱绑定）" });
    }
  }
  if (/![\w.\-]*$/.test(linePrefix)) {
    for (const n of collectFeatagNames(text)) {
      if (n === "any" || n === "strict-paths") continue;   // 契约级开关，不是块头标签
      items.push({ label: n, kind: CompletionItemKind.Keyword, detail: "featag：" + n });
    }
  }
  if (uri.endsWith(".smlex") && /^\s*\w*$/.test(linePrefix)) {
    for (const f of PROFILE_FIELDS) {
      items.push({ label: f, kind: CompletionItemKind.Property, detail: "profile 字段：" + f });
    }
  }
  // hints 只服务**显式挂载**的 profile；applies_to 命中也不自动挂（smlex.md §3.1）
  for (const ref of collectWeakDocs(text)) {
    const p = profiles.get(ref);
    if (!p) continue;
    for (const k of p.keys || []) {
      items.push({ label: k, kind: CompletionItemKind.Property, detail: "profile " + ref + " 的 hints.keys", insertText: k + ": " });
    }
    const valueKey = /^\s*([\w.\-]+)\s*:\s*$/.exec(linePrefix)?.[1];
    for (const v of p.values?.[valueKey] || []) {
      items.push({ label: v, kind: CompletionItemKind.Constant, detail: "profile " + ref + " 的 hints.values." + valueKey });
    }
  }
  // 键名（同文档）
  for (const k of collectKeys(text)) {
    items.push({ label: k, kind: CompletionItemKind.Property, detail: "本文档中出现过的键", insertText: k + ": " });
  }
  return items;
}

// ——— 跳转定义 ———
function findDefinition(uri, line, character) {
  const text = docs.get(uri) || "";
  const lines = text.split("\n");
  const lineText = lines[line] ?? "";
  // 取光标处单词
  const re = /[\p{L}\p{N}_.\-]+/gu;
  let m, word = null;
  while ((m = re.exec(lineText)) !== null) {
    if (m.index <= character && character <= m.index + m[0].length) { word = m[0]; break; }
  }
  if (!word) return null;
  // 仅当该词是契约名时跳转
  const names = collectContractNames(text);
  if (!names.includes(word)) return null;
  // 找 @contract word 定义行（用负向断言而非 \b：中文标识符后无 ASCII 词边界）
  const defRe = new RegExp("@contract\\s+" + escapeRe(word) + "(?![\\p{L}\\p{N}_.\\-])", "u");
  for (let i = 0; i < lines.length; i++) {
    if (defRe.test(lines[i])) {
      return { uri, range: { start: { line: i, character: 0 }, end: { line: i, character: lines[i].length } } };
    }
  }
  return null;
}

function escapeRe(s) { return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"); }

// ——— 主分发 ———
function handle(msg) {
  if (msg.method === "initialize") {
    const uri = msg.params?.rootUri || msg.params?.workspaceFolders?.[0]?.uri || "";
    if (uri) {
      try { rootPath = fileURLToPath(uri); } catch { rootPath = null; }
    }
    reply(msg.id, {
      capabilities: {
        textDocumentSync: 1,
        // `.` 让 `type.featag` 这类带域名打到一半仍有补全；`!` 触发块头 featag。
        completionProvider: { triggerCharacters: ["@", ":", "&", " ", ".", "!"] },
        definitionProvider: true,
        hoverProvider: true,
      },
      serverInfo: { name: "sml-lsp", version: "0.1.0" },
    });
  } else if (msg.method === "initialized") {
    // noop
  } else if (msg.method === "shutdown") {
    reply(msg.id, null);
  } else if (msg.method === "exit") {
    process.exit(0);
  } else if (msg.method === "textDocument/didOpen" || msg.method === "textDocument/didChange") {
    const doc = msg.params.textDocument;
    const text = msg.method === "textDocument/didOpen"
      ? msg.params.textDocument.text
      : (msg.params.contentChanges[0]?.text ?? docs.get(doc.uri) ?? "");
    docs.set(doc.uri, text);
    publishDiagnostics(doc.uri, text);
  } else if (msg.method === "textDocument/completion") {
    const p = msg.params;
    const items = buildCompletions(p.textDocument.uri, p.position.line, p.position.character);
    reply(msg.id, { isIncomplete: false, items });
  } else if (msg.method === "textDocument/definition") {
    const p = msg.params;
    const loc = findDefinition(p.textDocument.uri, p.position.line, p.position.character);
    reply(msg.id, loc ? [loc] : []);
  } else if (msg.method === "textDocument/hover") {
    const p = msg.params;
    const lines = (docs.get(p.textDocument.uri) || "").split("\n");
    const lineText = lines[p.position.line] ?? "";
    const re = /[@&]?[\p{L}\p{N}_.\-]+/gu;
    let m, word = null;
    while ((m = re.exec(lineText)) !== null) {
      if (m.index <= p.position.character && p.position.character <= m.index + m[0].length) { word = m[0]; break; }
    }
    // `@weakdoc <名>`：显示 profile 发现结果；找不到就明说，不静默
    const ref = /^\s*@weakdoc\s+([\w.\-]+)/.exec(lineText)?.[1];
    if (ref && word === ref) {
      const profile = discoverProfiles().get(ref);
      const value = profile
        ? `**profile \`${ref}\`**\n\n${profile.description || "（无描述）"}` +
          (profile.keys?.length ? `\n\nhints.keys：${profile.keys.join("、")}` : "")
        : `**profile \`${ref}\` 未找到**：项目 / 用户 / 系统三处命名空间都没有同名 .smlex。`;
      reply(msg.id, { contents: { kind: "markdown", value } });
      return;
    }
    const doc = KW[word];
    if (doc) reply(msg.id, { contents: { kind: "markdown", value: `**${word}**\n\n${doc}` } });
    else reply(msg.id, null);
  } else if (msg.id !== undefined) {
    reply(msg.id, null);
  }
}

process.stderr.write("sml-lsp ready\n");
