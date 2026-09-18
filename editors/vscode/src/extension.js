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
let smlLoadError = null;
async function ensureSml() {
  if (!sml && !smlLoadError) {
    try {
      sml = await import("./sml-parse.mjs");
    } catch (e) {
      // 加载失败必须留下痕迹：VSCode 会静默吞掉 provider 抛出的异常，
      // 若此处不记录，用户只会觉得「补全不存在」而无从排查。
      smlLoadError = e;
      console.error("[SML] 解析器加载失败，补全与诊断不可用：", e);
    }
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
  {
    label: "import (部分引用)",
    kind: vscode.CompletionItemKind.Keyword,
    detail: "只挑指定顶层键并入，避免整文件 copy",
    documentation: new vscode.MarkdownString(
      "部分引用：只从目标文件挑出指定顶层键并入当前作用域（不引入其余键）。\n\n" +
        "```sml\nimport \"widgets.sml\" { login, search }\n```\n" +
        "配合 `as ns` 挂到命名空间隔离：\n" +
        "```sml\nimport { login } as w in \"widgets.sml\"\n```"
    ),
    insertText: 'import "${1:path}" { ${2:key1}, ${3:key2} }',
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

// 模式（@type）关键字：中英等价，二者可混写（sml-pattern 的 BILINGUAL 表）
const PATTERN_KEYWORDS = [
  { zh: "序列", en: "seq", detail: "顺序匹配其后各项（值为数组）" },
  { zh: "类", en: "class", detail: "字符类：数字/字母/空白/字/任意" },
  { zh: "次", en: "times", detail: "量词：4 或 \"+\" / \"*\"；也可写 次: { 最小, 最大 }" },
  { zh: "最小", en: "min", detail: "量词下界：与 次 / 最大 配合，或直接平铺在元素上" },
  { zh: "最大", en: "max", detail: "量词上界：省略则无上界（min..*）" },
  { zh: "名", en: "name", detail: "命名捕获" },
  { zh: "字面", en: "lit", detail: "字面量（精确匹配）" },
  { zh: "任一", en: "alt", detail: "多选一（分支）" },
  { zh: "组", en: "group", detail: "内联分组（值为数组）" },
  { zh: "用", en: "use", detail: "引用另一条规则" },
  { zh: "可选", en: "optional", detail: "该项可省略" },
  { zh: "直到", en: "until", detail: "推进直到其后条件成立（JS 引擎暂不支持，请用 Rust 引擎）" },
  { zh: "正则", en: "regex", detail: "regex 逃生舱：直接写正则表达式" },
].flatMap((k) => [
  {
    label: k.zh,
    kind: vscode.CompletionItemKind.Keyword,
    detail: `模式关键字：${k.detail}`,
    insertText: `${k.zh}: `,
  },
  {
    label: k.en,
    kind: vscode.CompletionItemKind.Keyword,
    detail: `模式关键字（英文）：${k.detail}`,
    documentation: `等价于中文关键字「${k.zh}」`,
    insertText: `${k.en}: `,
  },
]);

// 字符类取值（中英等价）
const PATTERN_CLASSES = [
  { zh: "数字", en: "digit" },
  { zh: "字母", en: "alpha" },
  { zh: "空白", en: "space" },
  { zh: "字", en: "word" },
  { zh: "任意", en: "any" },
].flatMap((c) => [
  {
    label: c.zh,
    kind: vscode.CompletionItemKind.TypeParameter,
    detail: `字符类：${c.zh}`,
  },
  {
    label: c.en,
    kind: vscode.CompletionItemKind.TypeParameter,
    detail: `字符类（英文）：${c.zh}`,
  },
]);

// 特性名（供 @feature enable/disable 补全）。带说明区分默认开启与 opt-in。
const FEATURE_NAMES = [
  { n: "contract", d: "契约系统 @contract / @is（默认开启）" },
  { n: "fragment", d: "片段复用 @name / &name（默认开启）" },
  { n: "include", d: "文件包含 include（默认开启）" },
  { n: "env", d: "$env.VAR 环境变量内插（默认开启）" },
  { n: "namespace", d: "include ... as ns 命名空间（默认开启）" },
  { n: "typed-block", d: "块级类型标注 `<契约名> <块名> { }`（opt-in）" },
  { n: "when", d: "@when 条件裁剪（opt-in）" },
  { n: "for", d: "@for 有界循环展开（opt-in）" },
].map((f) => ({
  label: f.n,
  kind: vscode.CompletionItemKind.Property,
  detail: `特性：${f.d}`,
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

// ---------------------------------------------------------------------------
// 语义高亮：块级类型标注 `<契约名> <块名> { .. }`
// ---------------------------------------------------------------------------
//
// grammar（TextMate 正则）无法知道哪些名字是契约 —— 那是语义，只有解析过文档
// 才知道。故这里按「文档中已定义的契约名」反查块首词，用 decorations 给它上
// 类型色，让「这块数据受哪个契约约束」一眼可见。
function initSemanticHighlight(context) {
  const deco = vscode.window.createTextEditorDecorationType({
    color: new vscode.ThemeColor("symbolIcon.classForeground"),
  });

  const refresh = async (editor) => {
    if (!editor || editor.document.languageId !== "sml") return;
    const mod = await ensureSml();
    if (!mod || !mod.findAnnotatedBlocks) return;
    const text = editor.document.getText();
    const blocks = mod.findAnnotatedBlocks(text, mod.collectContractNames(text));
    editor.setDecorations(
      deco,
      blocks.map(
        (b) => new vscode.Range(b.line, b.col, b.line, b.col + b.length)
      )
    );
  };

  if (vscode.window.activeTextEditor) refresh(vscode.window.activeTextEditor);
  context.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor((e) => refresh(e))
  );

  // 文档变更防抖：契约定义可能刚写完，稍后再反查
  let timer = null;
  context.subscriptions.push(
    vscode.workspace.onDidChangeTextDocument((e) => {
      if (e.document.languageId !== "sml") return;
      const editor = vscode.window.activeTextEditor;
      if (!editor || editor.document !== e.document) return;
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => refresh(editor), 200);
    })
  );
}

function activate(context) {
  const collection = vscode.languages.createDiagnosticCollection("sml");
  context.subscriptions.push(collection);

  // 启动自检：解析器不可用时明确告知，避免「补全静默失效」无从排查
  ensureSml().then((m) => {
    if (!m) {
      vscode.window.showWarningMessage(
        "SML 扩展：解析器加载失败，补全与错误提示不可用（详见「扩展主机」输出日志）。"
      );
    }
  });

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
      const mod = await ensureSml();
      if (!mod) return [];
      const { collectContractNames, collectFragmentNames, collectKeys, collectTypeNames } = mod;
      const items = [];
      const contractNames = collectContractNames(fullText);
      const typeNames = collectTypeNames(fullText);
      const before = fullText.slice(0, document.offsetAt(position));

      // SML 标识符可含中文（如契约名 `受理人`），故所有位置匹配统一用 CJK 感知的字符类
      // `[A-Za-z0-9_\u4e00-\u9fa5]`，避免 \w 把中文名整个漏掉导致补全/标注失效。
      const ID = "A-Za-z0-9_\\u4e00-\\u9fa5";

      // 1) 行首或 @ 触发：指令
      if (new RegExp(`(^|\\s)@[${ID}]*$`).test(linePrefix) || /^\s*$/.test(linePrefix)) {
        items.push(...DIRECTIVES);
      }

      // 1b) 行首：契约名直接作块级类型标注 `<契约名> <块名> { .. }`
      //     （与既有裸块同形，零新语法；需 @feature enable typed-block 才校验）
      if (new RegExp(`^\\s*[${ID}]*$`).test(linePrefix) && !linePrefix.includes("@")) {
        for (const n of contractNames) {
          items.push({
            label: n,
            kind: vscode.CompletionItemKind.Struct,
            detail: "契约名 · 块级类型标注 `<契约名> <块名> { }`",
            documentation: new vscode.MarkdownString(
              "以契约名作为块前缀，声明该块即校验：\n\n```sml\n" +
                `${n} 实例名 {\n  ...\n}\n` +
                "```\n\n等价于在块内首行写 `@is " +
                n +
                "`。需 `@feature enable typed-block`。"
            ),
          });
        }
      }

      // 1c) @feature enable/disable 之后：特性名
      if (new RegExp(`@feature\\s+(enable|disable)\\s+[${ID}]*$`).test(linePrefix)) {
        items.push(...FEATURE_NAMES);
      }

      // 2) 契约体内（键后跟冒号）：内置类型 + **@type 自定义类型** + 修饰符
      const isContractBody = /@contract[^{]*\{[^}]*$/s.test(before);
      if (isContractBody) {
        const typeItems = [
          ...CONTRACT_TYPES,
          ...typeNames.map((n) => ({
            label: n,
            kind: vscode.CompletionItemKind.TypeParameter,
            detail: "自定义类型（@type 声明的模式）",
          })),
        ];
        if (new RegExp(`:\\s*[${ID}]*$`).test(linePrefix)) items.push(...typeItems);
        else items.push(...MODIFIERS, ...typeItems);
      }

      // 2b) @type 模式体内：模式关键字（中英）与字符类
      const isTypeBody = /@type[^{]*\{[^}]*$/s.test(before);
      if (isTypeBody) {
        if (new RegExp(`(类|class)\\s*:\\s*[${ID}]*$`).test(linePrefix)) items.push(...PATTERN_CLASSES);
        else items.push(...PATTERN_KEYWORDS, ...PATTERN_CLASSES);
      }

      // 3) 值位置（冒号后）：常量 / 片段引用 / 契约名（组合）
      if (new RegExp(`:\\s*[${ID}]*$`).test(linePrefix) && !isContractBody) {
        items.push(...CONSTANTS);
        for (const n of contractNames) {
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

      // 4) @is 之后：契约名；@is type( 之后：契约名并自动补右括号
      if (new RegExp(`@is\\s+type\\([${ID}]*$`).test(linePrefix)) {
        for (const n of contractNames) {
          items.push({
            label: n,
            kind: vscode.CompletionItemKind.Struct,
            detail: "契约名（类型标注形式 @is type(契约名)）",
            insertText: n + ")",
          });
        }
      } else if (new RegExp(`@is\\s+[${ID}]*$`).test(linePrefix)) {
        for (const n of contractNames) {
          items.push({
            label: n,
            kind: vscode.CompletionItemKind.Struct,
            detail: "契约名",
          });
        }
      }

      // 5) 行首键名补全（同文档出现过的键）
      if (new RegExp(`^\\s*[${ID}]*$`).test(linePrefix) && !isContractBody && !linePrefix.includes("@")) {
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

  // —— 悬浮说明：契约展开结果 + 指令关键字 ——
  const hoverProvider = {
    async provideHover(document, position) {
      const range = document.getWordRangeAtPosition(position, /[@&]?[A-Za-z0-9_\u4e00-\u9fa5.\-]+/);
      if (!range) return null;
      const word = document.getText(range);

      // 契约名（`@is Server` / `@contract Server` 里的 Server）→ 显示契约展开结果。
      // 内容由桥接层组装（纯文本，可脱离 VSCode 单测），这里只做包装。
      const mod = await ensureSml();
      if (mod && mod.contractHoverMarkdown && mod.collectContractNames) {
        const text = document.getText();
        if (mod.collectContractNames(text).includes(word)) {
          const md = mod.contractHoverMarkdown(text, word);
          if (md) return new vscode.Hover(new vscode.MarkdownString(md), range);
        }
      }

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

  // —— 跳转到定义：`@is Server` -> `@contract Server`；`&base` -> `@base { }` ——
  //
  // 只做「同名定义」的跳转，不做作用域分析（SML 的片段/契约是文档级名字）。
  // 找不到定义时返回 null，交给 VSCode 显示「未找到定义」——不要自己弹窗报错，
  // 否则用户只是把光标放到一个普通裸词上也会被打断。
  context.subscriptions.push(
    vscode.languages.registerDefinitionProvider(
      { language: "sml", scheme: "file" },
      {
        async provideDefinition(document, position) {
          const mod = await ensureSml();
          if (!mod || !mod.findDefinition) return null;
          // 允许 `&`/`@` 前缀与中文名（与补全、语义高亮用同一套字符类）
          const range = document.getWordRangeAtPosition(
            position,
            /[@&]?[A-Za-z0-9_\u4e00-\u9fa5.\-]+/
          );
          if (!range) return null;
          const word = document.getText(range);
          const isRef = word.startsWith("&");
          const name = isRef ? word.slice(1) : word;
          if (!name) return null;
          const text = document.getText();
          // `&frag` 只可能是片段；裸名（含 `@is Server` 的 Server）先按契约找，再退回片段
          for (const kind of isRef ? ["fragment"] : ["contract", "fragment"]) {
            const loc = mod.findDefinition(text, name, kind);
            if (!loc) continue;
            const start = new vscode.Position(loc.line, loc.col);
            return new vscode.Location(
              document.uri,
              new vscode.Range(start, start.translate({ characterDelta: loc.length }))
            );
          }
          return null;
        },
      }
    )
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

  // —— 自定义高亮：HL-cfg.sml + 强度开关（见 src/highlight.js）——
  require("./highlight.js").initHighlight(context);

  // —— 特别高亮：把选中词在整个工作区点亮（见 src/special-highlight.js）——
  // 与上面的「自定义高亮」不是一回事：那个按 HL-cfg.sml 静态配置点亮**关键词**，
  // 这个是**临时探照灯** —— 选中什么就点亮什么，再触发一次即取消。
  require("./special-highlight.js").initSpecialHighlight(context);

  // —— 语义高亮：块级类型标注 `<契约名> <块名> { .. }` 的契约名 ——
  initSemanticHighlight(context);

  suggestIconTheme(context);
}

// 文件图标主题需用户选择才生效，故首次激活时询问一次（可永久关闭提示）
async function suggestIconTheme(context) {
  const KEY = "sml.iconThemePrompted";
  if (context.globalState.get(KEY)) return;
  const cfg = vscode.workspace.getConfiguration("workbench");
  const current = cfg.get("iconTheme", "");
  if (current === "sml-icons") return;

  const pick = await vscode.window.showInformationMessage(
    "SML：是否为 .sml 文件启用青色 {*} 图标？（继承现有图标集，只影响 .sml）",
    "启用",
    "不再提示"
  );
  if (pick === "启用") {
    await cfg.update("iconTheme", "sml-icons", vscode.ConfigurationTarget.Global);
  } else if (pick === "不再提示") {
    await context.globalState.update(KEY, true);
  }
}

function deactivate() {}

module.exports = { activate, deactivate };
