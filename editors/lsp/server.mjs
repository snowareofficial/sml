// SML 语言服务器（LSP over stdio）
//
// 纯 Node、零运行时依赖，复用仓库的 JS 实现（js/sml.mjs 经 vendor 桥接），
// 保证「编辑器里报的错 = 解析器真正报的错」。
//
// 能力：
//   - textDocument.publishDiagnostics（parseSafe 的语法错误）
//   - textDocument.completion（指令 / 契约名 / 类型名 / 片段 / 键名 / 模式关键字）
//   - textDocument.definition（@contract 定义 <-> 用法跳转）
//   - textDocument.hover（指令 / 模式关键字说明）
//
// 运行：
//   node editors/lsp/server.mjs
// 任意支持 LSP 的客户端（Neovim / Helix / 任意编辑器）连 stdio 即可。
// VSCode 用户已内置更丰富的扩展（editors/vscode），此服务器面向其它编辑器。

import { parseSafe } from "../vscode/src/sml-parse.mjs";
import {
  collectContractNames, collectFragmentNames, collectKeys, collectTypeNames,
} from "../vscode/src/sml-parse.mjs";

const KW = {
  "@contract": "契约定义：为块定义字段类型、枚举、默认值与区间约束。定义本身不进解析结果。",
  "@is": "应用契约：校验当前块并填充缺失字段默认值。契约须先于 @is 定义。",
  "@type": "定义值的格式模式（loom）：用 序列/类/次/名/字面/任一 等描述一个字符串应长什么样。",
  "@feature": "开启/关闭特性，如 @feature enable typed-block。",
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
    reply(msg.id, {
      capabilities: {
        textDocumentSync: 1,
        completionProvider: { triggerCharacters: ["@", ":", "&", " "] },
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
    const doc = KW[word];
    if (doc) reply(msg.id, { contents: { kind: "markdown", value: `**${word}**\n\n${doc}` } });
    else reply(msg.id, null);
  } else if (msg.id !== undefined) {
    reply(msg.id, null);
  }
}

process.stderr.write("sml-lsp ready\n");
