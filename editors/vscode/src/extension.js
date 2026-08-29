// SML 语言支持（VSCode 扩展）
//
// 提供三项能力：
//   1. 语法高亮     —— syntaxes/sml.tmLanguage.json（TextMate grammar，声明式）
//   2. 错误提示     —— 复用 js/sml.mjs 解析，把错误定位到精确行列并显示为诊断
//   3. 补全         —— 指令 / 契约关键字 / 类型 / 修饰符 / 常量 / 键名 / 片段名
//
// 设计取舍：不引入独立语言服务器（LSP），而是用 VSCode API 直接实现。
// 理由：SML 语法小、解析器零依赖且可直接 import，进程内调用更轻、
// 免安装/免端口协调；代价是能力局限在 VSCode。若将来需支持其他编辑器，
// 可把 src/sml-parse.mjs 包一层 LSP server 复用（见 ../../TODO.md）。

const vscode = require("vscode");
const path = require("path");

// 桥接层是 ESM，扩展宿主为 CJS，故用动态 import 载入
let sml = null;
async function ensureSml() {
  if (!sml) {
    sml = await import("./sml-parse.mjs");
  }
  return sml;
}

// ---------------------------------------------------------------------------
// 补全候选
// ---------------------------------------------------------------------------

const DIRECTIVES = [
  {
    label: "@version v1",
    kind: vscode.CompletionItemKind.Keyword,
    detail: "版本声明",
    documentation: "声明文档遵循的 SML 语法版本，须写在文档开头。",
    insertText: "@version v1",
  },
  {
    label: "@contract",
    kind: vscode.CompletionItemKind.Keyword,
    detail: "契约定义",
    documentation: new vscode.MarkdownString(
      "定义契约（schema），为块提供字段类型、枚举、默认值、区间约束。\n\n" +
        "```sml\n@contract Server {\n    host: str\n    port: int default 5432\n}\n```\n\n" +
        "契约定义本身不进解析结果。需要 `loose` 才允许未声明字段。"
    ),
    insertText: "@contract ${1:Name} {\n\t$0\n}",
    insertTextFormat: vscode.InsertTextFormat.Snippet,
  },
  {
    label: "@is",
    kind: vscode.CompletionItemKind.Keyword,
    detail: "应用契约",
    documentation: new vscode.MarkdownString(
      "在当前块应用契约：校验字段类型/枚举/区间，并填充缺失字段的默认值。\n\n" +
        "契约须在 `@is` 之前定义。\n\n```sml\ndb {\n    @is Server\n    host: db1.internal\n}\n```"
    ),
    insertText: "@is ${1:Name}",
    insertTextFormat: vscode.InsertTextFormat.Snippet,
  },
  {
    label: "include",
    kind: vscode.CompletionItemKind.Keyword,
    detail: "引入外部文件",
    documentation: "把外部 .sml 文件内联进来。相对路径按**被包含文件自身所在目录**解析。",
    insertText: 'include "${1:path}"',
    insertTextFormat: vscode.InsertTextFormat.Snippet,
  },
];

const CONTRACT_TYPES = [
  { label: "str", detail: "字符串" },
  { label: "int", detail: "整数" },
  { label: "num", detail: "数值（整数或浮点）" },
  { label: "bool", detail: "布尔（true / false）" },
  { label: "any", detail: "任意类型" },
  { label: "[str]", detail: "字符串数组" },
  { label: "[int]", detail: "整数数组" },
  { label: "enum [ ]", detail: "枚举：取值须来自给定列表" },
].map((t) => ({
  label: t.label,
  kind: vscode.CompletionItemKind.TypeParameter,
  detail: `类型：${t.detail}`,
  insertText: t.label === "enum [ ]" ? "enum [ ${1:a} ${2:b} ]" : t.label,
  insertTextFormat:
    t.label === "enum [ ]" ? vscode.InsertTextFormat.Snippet : vscode.InsertTextFormat.PlainText,
}));

const MODIFIERS = [
  { label: "required", detail: "必填（默认行为，可省略）" },
  { label: "optional", detail: "可选：缺失时不报错" },
  { label: "default", detail: "默认值：字段缺失时填充" },
  { label: "min", detail: "数值下界（含）" },
  { label: "max", detail: "数值上界（含）" },
  { label: "loose", detail: "允许契约未声明的字段（写在契约名后）" },
].map((m) => ({
  label: m.label,
  kind: vscode.CompletionItemKind.Keyword,
  detail: `修饰符：${m.detail}`,
}));

const CONSTANTS = ["true", "false", "null"].map((c) => ({
  label: c,
  kind: vscode.CompletionItemKind.Constant,
  detail: "字面量",
}));

// ---------------------------------------------------------------------------
// 诊断
// ---------------------------------------------------------------------------

async function updateDiagnostics(doc, collection) {
  if (doc.languageId !== "sml") {
    collection.delete(doc.uri);
    return;
  }
  const { diagnose } = await ensureSml();
  const items = diagnose(doc.getText());
  const diags = items.map((it) => {
    const range = new vscode.Range(
      new vscode.Position(it.line, it.col),
      new vscode.Position(it.line, Math.max(it.col + 1, it.endCol ?? it.col + 1))
    );
    const d = new vscode.Diagnostic(
      range,
      it.message,
      it.severity === "warning"
        ? vscode.DiagnosticSeverity.Warning
        : vscode.DiagnosticSeverity.Error
    );
    d.source = "sml";
    return d;
  });
  collection.set(doc.uri, diags);
}

// ---------------------------------------------------------------------------
// 激活
// ---------------------------------------------------------------------------

function activate(context) {
  const collection = vscode.languages.createDiagnosticCollection("sml");
  context.subscriptions.push(collection);

  // 变更即校验（防抖，避免大文件频繁解析）
  let timer = null;
  const schedule = (doc) => {
    if (doc.languageId !== "sml") return;
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => updateDiagnostics(doc, collection), 200);
  };

  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument((doc) => updateDiagnostics(doc, collection)),
    vscode.workspace.onDidChangeTextDocument((e) => schedule(e.document)),
    vscode.workspace.onDidSaveTextDocument((doc) => updateDiagnostics(doc, collection)),
    vscode.workspace.onDidCloseTextDocument((doc) => collection.delete(doc.uri))
  );

  // 已打开的文档立即校验一次
  vscode.workspace.textDocuments.forEach((doc) => updateDiagnostics(doc, collection));

  // —— 补全 ——
  const provider = {
    async provideCompletionItems(document, position) {
      const linePrefix = document
        .lineAt(position)
        .text.slice(0, position.character);
      const fullText = document.getText();
      const { collectContractNames, collectFragmentNames, collectKeys } = await ensureSml();
      const items = [];

      // 1) 行首或 @ 触发：指令
      if (/(^|\s)@\w*$/.test(linePrefix) || /^\s*$/.test(linePrefix)) {
        items.push(...DIRECTIVES);
      }

      // 2) 契约体内（键后跟冒号）：类型 + 修饰符
      const isContractBody = /@contract[^{]*\{[^}]*$/s.test(
        fullText.slice(0, document.offsetAt(position))
      );
      if (isContractBody) {
        if (/:\s*\w*$/.test(linePrefix)) items.push(...CONTRACT_TYPES);
        else items.push(...MODIFIERS, ...CONTRACT_TYPES);
      }

      // 3) 值位置（冒号后）：常量 / 片段引用 / 契约名（组合）
      if (/:\s*\w*$/.test(linePrefix) && !isContractBody) {
        items.push(...CONSTANTS);
        for (const n of collectContractNames(fullText)) {
          items.push({
            label: n,
            kind: vscode.CompletionItemKind.Struct,
            detail: "契约（组合：字段值须符合该契约）",
          });
        }
        for (const n of collectFragmentNames(fullText)) {
          items.push({
            label: "&" + n,
            kind: vscode.CompletionItemKind.Reference,
            detail: "片段引用（展开为片段内容）",
            insertText: "&" + n,
          });
        }
      }

      // 4) @is 之后：已定义的契约名
      if (/@is\s+\w*$/.test(linePrefix)) {
        for (const n of collectContractNames(fullText)) {
          items.push({
            label: n,
            kind: vscode.CompletionItemKind.Struct,
            detail: "契约名",
          });
        }
      }

      // 5) 行首键名补全（同文档出现过的键）
      if (/^\s*\w*$/.test(linePrefix) && !isContractBody && !linePrefix.includes("@")) {
        for (const k of collectKeys(fullText)) {
          items.push({
            label: k,
            kind: vscode.CompletionItemKind.Property,
            detail: "本文档中出现过的键",
            insertText: `${k}: `,
          });
        }
      }

      return items;
    },
  };
  context.subscriptions.push(
    vscode.languages.registerCompletionItemProvider(
      { language: "sml", scheme: "file" },
      provider,
      "@",
      ":",
      "&",
      " "
    )
  );

  // —— 悬浮说明：契约/指令关键字 ——
  const hoverProvider = {
    provideHover(document, position) {
      const range = document.getWordRangeAtPosition(position, /[@&]?[\w.-]+/);
      if (!range) return null;
      const word = document.getText(range);
      const map = {
        "@contract": "**契约定义**：为块定义字段类型、枚举、默认值与区间约束。定义本身不进解析结果。",
        "@is": "**应用契约**：校验当前块并填充缺失字段的默认值。契约须在 `@is` 之前定义。",
        "loose": "**放宽严格性**：允许契约未声明的字段。默认严格（未声明字段会报错）。",
        "@version": "**版本声明**：声明文档遵循的 SML 语法版本，须写在文档开头。",
        "include": "**文件引入**：把外部 .sml 内联进来（文本内联，可出现在块内）。相对路径按被包含文件所在目录解析。",
        "optional": "字段可选：缺失时不报错。",
        "required": "字段必填（默认行为）。",
        "default": "字段缺失时填充的默认值。",
        "min": "数值下界（含）。",
        "max": "数值上界（含）。",
      };
      const doc = map[word];
      if (!doc) return null;
      return new vscode.Hover(new vscode.MarkdownString(doc), range);
    },
  };
  context.subscriptions.push(
    vscode.languages.registerHoverProvider({ language: "sml", scheme: "file" }, hoverProvider)
  );

  // —— 格式化：把当前文档按 SML 规范重排（解析 -> stringify）——
  context.subscriptions.push(
    vscode.languages.registerDocumentFormattingEditProvider(
      { language: "sml", scheme: "file" },
      {
        async provideDocumentFormattingEdits(document) {
          const { parseSafe, stringify } = await ensureSml();
          const text = document.getText();
          const r = parseSafe(text);
          if (!r.ok) {
            vscode.window.showErrorMessage(`无法格式化：${r.error}`);
            return [];
          }
          const out = stringify(r.value);
          return [
            vscode.TextEdit.replace(
              new vscode.Range(
                document.positionAt(0),
                document.positionAt(text.length)
              ),
              out
            ),
          ];
        },
      }
    )
  );
}

function deactivate() {}

module.exports = { activate, deactivate };
