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
const fs = require("fs");

// ---------------------------------------------------------------------------
// ⚠️ 顶层访问 vscode API 的安全护栏 —— 别删这段注释，它标的是一个致命坑
// ---------------------------------------------------------------------------
// **顶层绝不能直接读枚举成员。** VS Code 会**移除**枚举：本机 VS Code **1.138.0** 的扩展宿主里
// `vscode.InsertTextFormat` 已经不存在（`extensionHostProcess.js` 里该名字出现 0 次，自带
// `out/vscode-dts/vscode.d.ts` 里也没有 `enum InsertTextFormat`）。而
//     insertTextFormat: INSERT_SNIPPET
// 写在模块顶层 ⇒ **加载模块时就抛** `TypeError: Cannot read properties of undefined (reading 'Snippet')`
// ⇒ 整个扩展的 `activate` 从不执行 ⇒ provider / 命令 / 输出面板全都不会注册。而报错只落在
// 「扩展主机」日志里，界面上表现就是「悬停 / 右键菜单 / 特别高亮全都没反应」——与「扩展坏了」
// 无法区分。本仓库从 0.4.1 起就一直是这样（HANDOFF §22.12）。
// 因此：枚举一律经 helper 取，取不到就用**协议数值**（这几个数值是线上协议的稳定常量）。
const INSERT_SNIPPET = 2; // InsertTextFormat.Snippet
const INSERT_PLAIN = 1;   // InsertTextFormat.PlainText

/// 取 `CompletionItemKind` 的成员；枚举被移除时退化为 fallback（0 = Text，仅图标不同，不影响功能）
function CK(name, fallback) {
  const e = vscode.CompletionItemKind;
  const v = e ? e[name] : undefined;
  return v === undefined ? fallback : v;
}

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
// 输出面板（「输出 → SML」）+ 自检
// ---------------------------------------------------------------------------
// 为什么需要它：provider 抛的异常 VSCode 只在「扩展主机」日志里留一行，用户看不到；
// 而「悬停为什么不显示展开」「命令为什么没反应」这类问题**必须能从编辑器里问出来** ——
// 否则只能靠猜（本项目已经为「猜」付过一次代价：把非法的 `web @is Server { }` 当成
// 扩展坏了去查）。
let outChannel = null;
function channel() {
  if (!outChannel) outChannel = vscode.window.createOutputChannel("SML");
  return outChannel;
}
function log(line) {
  channel().appendLine(line);
}
// 同一份文档只解释一次「为什么没有展开」，避免鼠标划过就刷屏
const explained = new Set();

// 本仓库当前 `src/vendor/sml.mjs` 的指纹（每次重打 VSIX 后同步这里；
// 与 sync-parser.py 打印的值同源）。自检会拿它对**已安装的包**做一次核对 ——
// 「包是不是新的」这件事以前只靠人肉解包比对（见 HANDOFF §22.2 第 1 条）。
const EXPECT_VENDOR = { size: 73683, shaPrefix: "0ea28eec0253a864" };

// ---------------------------------------------------------------------------
// 补全候选
// ---------------------------------------------------------------------------

const DIRECTIVES = [
  {
    label: "@version v1",
    kind: CK("Keyword", 0),
    detail: "版本声明",
    documentation: "声明文档遵循的 SML 语法版本，须写在文档开头。",
    insertText: "@version v1",
  },
  {
    label: "@contract",
    kind: CK("Keyword", 0),
    detail: "契约定义",
    documentation: new vscode.MarkdownString(
      "定义契约（schema），为块提供字段类型、枚举、默认值、区间约束。\n\n" +
        "```sml\n@contract Server {\n    host: str\n    port: int default 5432\n}\n```\n\n" +
        "契约定义本身不进解析结果。需要 `loose` 才允许未声明字段。"
    ),
    insertText: "@contract ${1:Name} {\n\t$0\n}",
    insertTextFormat: INSERT_SNIPPET,
  },
  {
    label: "@is",
    kind: CK("Keyword", 0),
    detail: "应用契约",
    documentation: new vscode.MarkdownString(
      "在当前块应用契约：校验字段类型/枚举/区间，并填充缺失字段的默认值。\n\n" +
        "契约须在 `@is` 之前定义。\n\n```sml\ndb {\n    @is Server\n    host: db1.internal\n}\n```"
    ),
    insertText: "@is ${1:Name}",
    insertTextFormat: INSERT_SNIPPET,
  },
  {
    label: "include",
    kind: CK("Keyword", 0),
    detail: "引入外部文件",
    documentation: "把外部 .sml 文件内联进来。相对路径按**被包含文件自身所在目录**解析。",
    insertText: 'include "${1:path}"',
    insertTextFormat: INSERT_SNIPPET,
  },
  {
    label: "import (部分引用)",
    kind: CK("Keyword", 0),
    detail: "只挑指定顶层键并入，避免整文件 copy",
    documentation: new vscode.MarkdownString(
      "部分引用：只从目标文件挑出指定顶层键并入当前作用域（不引入其余键）。\n\n" +
        "```sml\nimport \"widgets.sml\" { login, search }\n```\n" +
        "配合 `as ns` 挂到命名空间隔离：\n" +
        "```sml\nimport { login } as w in \"widgets.sml\"\n```"
    ),
    insertText: 'import "${1:path}" { ${2:key1}, ${3:key2} }',
    insertTextFormat: INSERT_SNIPPET,
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
  kind: CK("TypeParameter", 0),
  detail: `类型：${t.detail}`,
  insertText: t.label === "enum [ ]" ? "enum [ ${1:a} ${2:b} ]" : t.label,
  insertTextFormat:
    t.label === "enum [ ]" ? INSERT_SNIPPET : INSERT_PLAIN,
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
  kind: CK("Keyword", 0),
  detail: `修饰符：${m.detail}`,
}));

const CONSTANTS = ["true", "false", "null"].map((c) => ({
  label: c,
  kind: CK("Constant", 0),
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
    kind: CK("Keyword", 0),
    detail: `模式关键字：${k.detail}`,
    insertText: `${k.zh}: `,
  },
  {
    label: k.en,
    kind: CK("Keyword", 0),
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
    kind: CK("TypeParameter", 0),
    detail: `字符类：${c.zh}`,
  },
  {
    label: c.en,
    kind: CK("TypeParameter", 0),
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
  kind: CK("Property", 0),
  detail: `特性：${f.d}`,
}));

// ---------------------------------------------------------------------------
// 诊断
// ---------------------------------------------------------------------------

/// 为 `parseSafe(text, { files })` 准备**被包含文件的内容表**。
///
/// 为什么必需：JS 解析器把 include 目标查这张表（`files[路径]`），查不到就报
/// `E-INCLUDE-001「include 目标未找到」`。扩展此前只传 `doc.getText()` ⇒
/// **凡是用 include 的文档在编辑器里整行标红**（用户实测：`examples/advanced.sml`
/// 的 include / import 行全红，而同一文件在命令行下完全正常）。
///
/// 键给三种形态以尽量命中"文档里的相对写法"：
///   · 工作区相对路径（`examples/common.sml`）
///   · 相对**当前文档目录**的路径（`common.sml` —— include 里通常这么写）
///   · 裸文件名（兜底）
async function buildFilesMap(doc) {
  const map = Object.create(null);
  let uris = [];
  try {
    uris = await vscode.workspace.findFiles("**/*.sml", "**/node_modules/**", 400);
  } catch {
    return map; // 拿不到列表就不传文件表（退回旧行为，不因此报错）
  }
  const docDir = doc.uri.path.replace(/\/[^/]*$/, "").replace(/^\//, "");
  for (const u of uris) {
    if (u.toString() === doc.uri.toString()) continue; // 自身由 text 提供
    try {
      const buf = await vscode.workspace.fs.readFile(u);
      const text = Buffer.from(buf).toString("utf8");
      const rel = String(vscode.workspace.asRelativePath(u, false)).replace(/\\/g, "/");
      map[rel] = text;
      const base = rel.split("/").pop();
      if (base) map[base] = text;
      if (docDir && rel.startsWith(docDir + "/")) map[rel.slice(docDir.length + 1)] = text;
    } catch {
      /* 单个文件读失败不影响其它 */
    }
  }
  return map;
}

/// 解析某一行上的 `include` / `import` 目标，并把它们**解析成工作区文件 Uri**。
///
/// 返回 `[{ path, col, end, viaImport, regex, uri }]`；`uri === null` 表示没找到目标
/// （悬停要如实说"未找到"，跳转则返回 null）。
///
/// 解析规则在桥接层 `parseIncludeTargets`（照 JS 解析器的 `parseIncludeTargets` 重写 ——
/// 那个函数**未导出**）；这里只负责"路径 → Uri"：按 `include` 的语义**先相对当前文档目录**，
/// 再退到工作区相对路径 / 裸文件名（与 `buildFilesMap` 的键策略一致）。
async function resolveIncludeTargets(doc, line) {
  const { parseIncludeTargets } = await ensureSml();
  if (!parseIncludeTargets) return [];
  const targets = parseIncludeTargets(line).map((t) => ({ ...t, uri: null }));
  if (!targets.length) return targets;
  let uris = [];
  try {
    uris = await vscode.workspace.findFiles("**/*.sml", "**/node_modules/**", 400);
  } catch {
    return targets;
  }
  const docDir = doc.uri.path.replace(/\/[^/]*$/, "");
  for (const t of targets) {
    if (t.regex) continue; // `re:"…"` 是正则匹配，不是文件路径
    const want = t.path.replace(/\\/g, "/");
    const cands = want.endsWith(".sml") ? [want] : [want, want + ".sml"];
    for (const u of uris) {
      if (u.toString() === doc.uri.toString()) continue;
      const p = String(u.path || u.fsPath || "").replace(/\\/g, "/");
      const rel = String(vscode.workspace.asRelativePath(u, false)).replace(/\\/g, "/");
      const hit = cands.some(
        (c) =>
          p === docDir + "/" + c || // ① 相对当前文档目录（include 的语义）
          p.endsWith("/" + c) || // ② 工作区内的相对路径
          rel === c ||
          rel.split("/").pop() === c.split("/").pop() // ③ 裸文件名兜底
      );
      if (hit) {
        t.uri = u;
        break;
      }
    }
  }
  return targets;
}

async function updateDiagnostics(doc, collection) {
  if (doc.languageId !== "sml") {
    collection.delete(doc.uri);
    return;
  }
  const { diagnose } = await ensureSml();
  const files = await buildFilesMap(doc); // ← 没有它，每个 include/import 行都会被标红
  const items = diagnose(doc.getText(), { files });
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

/// 报告**包内**解析器的指纹：与仓库当前版本对不上 ⇒ 装的是旧包（或源码又改过）。
/// 这条以前只能靠人肉解包比对（HANDOFF §22.2 第 1 条），现在编辑器里就能问到。
function reportVendor() {
  try {
    const fs = require("fs");
    const crypto = require("crypto");
    const b = fs.readFileSync(path.join(__dirname, "vendor", "sml.mjs"));
    const sha = crypto.createHash("sha256").update(b).digest("hex").slice(0, 16);
    log(`包内解析器 vendor/sml.mjs：${b.length} B / sha256 ${sha}`);
    if (b.length === EXPECT_VENDOR.size && sha.startsWith(EXPECT_VENDOR.shaPrefix)) {
      log("  ⇒ 与仓库当前版本一致 ✓（含 W16 之后的解析器）");
    } else {
      log(`  ⚠️ 与仓库当前版本**不一致**（期望 ${EXPECT_VENDOR.size} B / ${EXPECT_VENDOR.shaPrefix}…）`);
      log("  ⇒ 装的是旧包：重打并重装 VSIX（见 editors/vscode/README 的安装一节）");
    }
  } catch (e) {
    log("包内解析器读取失败：" + (e && e.message));
  }
}

/// 「.sml 文件没被当成 SML 打开」是「悬浮 / 诊断 / 补全 / 右键菜单**全都没反应**」的
/// 头号原因，而且它**不留任何痕迹**：
///   · provider 是按语言选择器（`{ language: "sml" }`）注册的 ⇒ 语言不符就不会被调用；
///   · 右键菜单项带 `when: editorLangId == sml` ⇒ 直接不显示；
///   · TextMate 语法也绑在 `sml` 这个语言上 ⇒ 连高亮都不生效。
/// 于是「扩展坏了」和「文件没被识别」看起来一模一样。这里把它变成一句话 + 一个按钮。
/// 注意：`workspaceContains:**/*.sml` 让扩展在**工作区里有 .sml 时**也会激活 ——
/// 否则语言模式不对时扩展压根不激活，这段检查根本跑不到（鸡生蛋问题）。
function initLanguageGuard(context) {
  const warned = new Set();
  const check = async (doc) => {
    if (!doc || !/\.sml$/i.test(doc.fileName || "")) return;
    if (doc.languageId === "sml") return;
    const key = doc.uri.toString() + "@" + doc.languageId;
    if (warned.has(key)) return;
    warned.add(key);
    log(`⚠️ ${doc.fileName} 当前语言模式是「${doc.languageId}」—— SML 的悬浮 / 诊断 / 补全 / 右键菜单都不会生效`);
    const pick = await vscode.window.showWarningMessage(
      `SML：这个 .sml 文件的语言模式是「${doc.languageId}」，所以悬浮 / 诊断 / 右键菜单都没有反应。`,
      "设为 SML",
      "打开自检"
    );
    if (pick === "设为 SML") {
      try {
        await vscode.languages.setTextDocumentLanguage(doc, "sml");
        log("已把该文件语言模式改为 sml ✓");
      } catch (e) {
        log("改语言模式失败：" + (e && e.message));
        log("  ⇒ 本窗口没有注册 `sml` 语言 ⇒ 扩展**没有被加载**（被禁用 / 受限模式 / 未安装），此时连右键菜单都不会出现。");
        vscode.window.showErrorMessage(
          "改不成功：本窗口没有注册 SML 语言 —— 扩展没被加载（检查是否被禁用、是否处于受限模式）。"
        );
      }
    } else if (pick === "打开自检") {
      vscode.commands.executeCommand("sml.selfCheck");
    }
  };
  context.subscriptions.push(vscode.workspace.onDidOpenTextDocument((d) => check(d)));
  vscode.workspace.textDocuments.forEach((d) => check(d));
  // 启动时先报一句「sml 语言是否已注册」—— 这是「扩展到底加载了没有」的判据
  if (vscode.languages.getLanguages) {
    vscode.languages.getLanguages().then(
      (langs) => {
        const has = langs.includes("sml");
        log("SML 语言已注册：" + (has ? "✓" : "✗ —— 扩展没被加载（被禁用 / 受限模式 / 未安装），此时连右键菜单都不会出现"));
        if (!has) log("  ⇒ 修复：扩展面板里确认 snoware.sml-lang 已启用、未被「限制模式」拦下，然后重启窗口。");
      },
      () => {}
    );
  }
}

/// 「应用特殊颜色」：把光标处 / 选中的词登记成 HL-cfg.sml 里的一组，并立即生效。
///
/// 为什么带「单元识别」这道闸门：特殊颜色若只按字面匹配，`active` 这类词会被染到
/// 注释、字符串、无关的键上 —— 一处着色、满屏变色。SML 里「单元」是**语法位置**的概念
/// （契约名 / 片段名 / 类型名 / 键 / 指令），所以默认只染语法位置；用户坚持按普通词染，
/// 也可以显式选「按普通词着色」（`unit: text`）。
///
/// 配置写在**文件**里（HL-cfg.sml，随仓库走），不是编辑器私有状态 —— 换机器、换人
/// 都能拿到同样的颜色，也能直接手写编辑（这正是「可以编写或选择」里的后半句）。
function initApplyColor(context) {
  const PRESETS = [
    { label: "$(circle-filled) 琥珀 #ff9f43", color: "#ff9f43" },
    { label: "$(circle-filled) 天青 #4ec9b0", color: "#4ec9b0" },
    { label: "$(circle-filled) 品红 #d16d9e", color: "#d16d9e" },
    { label: "$(circle-filled) 蓝紫 #9a7fd1", color: "#9a7fd1" },
    { label: "$(circle-filled) 橙红 #e06c75", color: "#e06c75" },
    { label: "$(circle-filled) 草绿 #98c379", color: "#98c379" },
    { label: "$(circle-filled) 金黄 #e5c07b", color: "#e5c07b" },
    { label: "$(circle-filled) 灰蓝 #7f8c9b", color: "#7f8c9b" },
    { label: "$(symbol-color) 自定义…（#RRGGBB 或主题色 id）", color: null },
  ];
  context.subscriptions.push(
    vscode.commands.registerCommand("sml.applySpecialColor", async () => {
      const ed = vscode.window.activeTextEditor;
      if (!ed) return vscode.window.showInformationMessage("先在 .sml 文件里选中一个词（或把光标放在词上）");
      const mod = await ensureSml();
      if (!mod || !mod.detectUnitKind || !mod.stringify) {
        return vscode.window.showErrorMessage("解析器不可用（详见「输出 → SML」）");
      }
      const sel = ed.selection;
      let word = "";
      if (!sel.isEmpty) {
        word = ed.document.getText(sel);
      } else {
        const r = ed.document.getWordRangeAtPosition(sel.active, /[@&]?[A-Za-z0-9_\u4e00-\u9fa5.\-]+/);
        if (r) word = ed.document.getText(r);
      }
      word = word.trim().replace(/^[@&]/, "");
      if (!word || /\s/.test(word)) {
        return vscode.window.showInformationMessage("请选中**一个词**（不含空白）再应用特殊颜色");
      }

      const text = ed.document.getText();
      let kind = mod.detectUnitKind(text, word);
      if (!kind) {
        const pick = await vscode.window.showWarningMessage(
          `「${word}」在本文档里不是可识别的语法单元（契约 / 片段 / 类型 / 键 / 指令）。` +
            "特殊颜色默认只对语法单元生效 —— 这样它不会染到注释和字符串里的同名文字。",
          "按普通词着色",
          "取消"
        );
        if (pick !== "按普通词着色") return;
        kind = "text";
      }

      const COLOR = await vscode.window.showQuickPick(PRESETS, {
        placeHolder: `给「${word}」（识别为 ${kind}${kind === "text" ? "：普通词" : ""}）选一个颜色`,
      });
      if (!COLOR) return;
      let color = COLOR.color;
      if (!color) {
        color = await vscode.window.showInputBox({
          prompt: "颜色：十六进制（#ff9f43）或主题色 id（editorError.foreground / charts.yellow）",
          value: "#ff9f43",
          validateInput: (v) =>
            /^#[0-9a-fA-F]{6}$/.test(v) || /^[a-zA-Z][\w.]*$/.test(v)
              ? null
              : "形如 #ff9f43 或 editorError.foreground",
        });
        if (!color) return;
      }

      const hl = require("./highlight.js");
      const p = hl.configPath ? hl.configPath() : null;
      if (!p) {
        return vscode.window.showErrorMessage("请先打开一个工作区文件夹 —— HL-cfg.sml 写在工作区根目录");
      }
      // 组名要能当 SML 键（不含空白；`-` 与 `.` 合法）
      const groupName = `${kind}_${word}`.replace(/[^\p{L}\p{N}_.\-]/gu, "-");
      const block = mod.stringify({ [groupName]: { words: [word], color, unit: kind } });
      const header = fs.existsSync(p)
        ? ""
        : "# SML 自定义高亮配置（由「SML: 应用特殊颜色」生成；本文件自身也是 SML）\n" +
          "#\n" +
          "# 每个顶层块 = 一组：words 必填；color / background / bold / italic / underline /\n" +
          "# matchCase 可选；unit 可选，限定只对某种**语法单元**生效\n" +
          "# （contract / fragment / type / key / directive；缺省或 text = 按普通词着色）。\n";
      try {
        fs.appendFileSync(p, header + (header ? "\n" : "\n") + block + "\n", "utf-8");
      } catch (e) {
        return vscode.window.showErrorMessage("写入 HL-cfg.sml 失败：" + (e && e.message));
      }
      await hl.reload(false);
      log(`应用特殊颜色：${word}（${kind}）→ ${color}，已写入 ${p}`);
      vscode.window.showInformationMessage(
        `已写入 ${path.basename(p)}：${groupName} → ${color}（立即生效；该文件可直接手写编辑）`
      );
    })
  );
}

/// 自检：把「为什么没反应」的每一环摊开写进「输出 → SML」。
/// 它只**报告**，不做任何修改 —— 排查工具不该顺手改状态。
function initSelfCheck(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand("sml.selfCheck", async () => {
      channel().show(true);
      log("");
      log("========== SML 扩展自检 ==========");
      log("时间：" + new Date().toLocaleString());
      try {
        const pkg = (context.extension && context.extension.packageJSON) || {};
        log(`扩展：snoware.sml-lang ${pkg.version || "?"}（目录 ${context.extensionPath}）`);
      } catch { /* 拿不到版本不影响自检 */ }

      const mod = await ensureSml();
      log("桥接层 sml-parse.mjs：" +
        (mod ? "已加载 ✓" : "**未加载 ✗**" + (smlLoadError ? " —— " + (smlLoadError.message || smlLoadError) : "")));
      reportVendor();
      if (!mod) {
        log("⇒ 解析器不可用：补全 / 悬浮 / 诊断 / 跳转**全部失效**。先修上面那条报错，或重装扩展。");
        log("========== 自检结束 ==========");
        return;
      }

      // 命令是否注册（能直接判出「装的是旧包」：旧包里没有 sml.specialHighlight）
      let cmds = [];
      try { cmds = await vscode.commands.getCommands(true); } catch { /* 忽略 */ }
      const need = ["sml.specialHighlight", "sml.clearSpecialHighlight", "sml.selfCheck"];
      log("命令注册：" + need.map((c) => c + (cmds.includes(c) ? " ✓" : " ✗")).join("  "));
      if (!cmds.includes("sml.specialHighlight")) log("  ⇒ 缺 `sml.specialHighlight` ⇒ **装的是旧包**，重装 VSIX 即可");

      // `sml` 语言是否已注册：判「扩展到底加载了没有」。没注册时，语言模式改不回 SML，
      // 右键菜单也不会出现 —— 这是「看起来像扩展坏了」的另一种真身。
      try {
        const langs = await vscode.languages.getLanguages();
        log("SML 语言已注册：" + (langs.includes("sml") ? "✓" : "✗ —— 扩展没被加载（被禁用 / 受限模式 / 未安装）"));
      } catch { /* 拿不到不影响后续 */ }

      const ed = vscode.window.activeTextEditor;
      if (!ed) {
        log("当前没有打开的编辑器（provider 只对已打开的 .sml 生效）。");
        log("========== 自检结束 ==========");
        return;
      }
      const doc = ed.document;
      log("当前文件：" + doc.uri.fsPath);
      log("语言模式：languageId = " + doc.languageId +
        (doc.languageId === "sml" ? " ✓" : " ⚠️ **不是 sml** ⇒ 所有 provider 都不生效（右下角点语言模式改成 SML）"));
      if (doc.languageId !== "sml") { log("========== 自检结束 =========="); return; }

      const text = doc.getText();
      // 自检也要带文件表：否则凡含 include 的文档都会被判"校验失败"（同诊断那条根因）
      const files = await buildFilesMap(doc);
      const r = mod.parseSafe ? mod.parseSafe(text, { files }) : { ok: true };
      log("文档校验：" + (r.ok ? "通过 ✓" : "**失败 ✗** → " + r.error));
      if (!r.ok) log("  ⇒ 两点后果：① 诊断面板有红字；② 悬浮的「展开」那半段不会出现（它还要求整份文档全绿）");

      const names = mod.collectContractNames ? mod.collectContractNames(text) : [];
      const frags = mod.collectFragmentNames ? mod.collectFragmentNames(text) : [];
      log("本文档声明的契约：" + (names.length ? names.join("、") : "（无）"));
      log("本文档声明的片段：" + (frags.length ? frags.join("、") : "（无）"));

      const sel = ed.selection;
      const wr = doc.getWordRangeAtPosition(sel.active, /[@&]?[A-Za-z0-9_\u4e00-\u9fa5.\-]+/);
      const word = wr ? doc.getText(wr) : "";
      const name = word.startsWith("&") ? word.slice(1) : word;
      log("光标下的词：" + JSON.stringify(word) + (wr ? `（第 ${wr.start.line + 1} 行第 ${wr.start.character + 1} 列）` : "（此处没有词）"));

      if (names.includes(name)) {
        let inst = null;
        try { inst = mod.contractInstance ? mod.contractInstance(text, name) : null; } catch { /* 忽略 */ }
        log(`契约 \`${name}\`：声明 ✓；实例 ` +
          (inst ? `✓（块 \`${inst.key}\`，第 ${inst.line + 1} 行）⇒ 悬浮会显示「填入默认值后的结构」`
                : "✗ ⇒ 悬浮**只显示声明**"));
        if (!inst) log(`  取不到实例的常见原因：文档没通过校验 / 没有**顶层** \`@is ${name}\` 块`);
      } else {
        log("⇒ 光标不在契约名上：悬浮的契约展开只对**契约名**生效（`@is X` / `@contract X` 里的 X）");
      }

      const def = mod.findDefinition
        ? mod.findDefinition(text, name, word.startsWith("&") ? "fragment" : "contract")
        : null;
      log("跳转到定义：" + (def ? `✓ 会跳到第 ${def.line + 1} 行第 ${def.col + 1} 列` : "✗ 找不到同名定义（只做文档级同名匹配）"));
      log("========== 自检结束 ==========");
      vscode.window.showInformationMessage("SML 自检完成：结果在「输出 → SML」面板");
    })
  );
}

function activate(context) {
  const collection = vscode.languages.createDiagnosticCollection("sml");
  context.subscriptions.push(collection, channel());
  log("扩展激活（" + new Date().toLocaleString() + "）—— 有「没反应」的地方，执行命令 `SML: 自检`");

  // 启动自检：解析器不可用时明确告知，避免「补全静默失效」无从排查
  ensureSml().then((m) => {
    if (m) {
      log("解析器加载成功 ✓（补全 / 悬浮 / 跳转 / 诊断可用）");
    } else {
      log("解析器加载失败 ✗" + (smlLoadError ? "：" + (smlLoadError.message || smlLoadError) : ""));
      vscode.window.showWarningMessage(
        "SML 扩展：解析器加载失败，补全与错误提示不可用（详见「输出 → SML」）。"
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
            kind: CK("Struct", 0),
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
            kind: CK("TypeParameter", 0),
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
            kind: CK("Struct", 0),
            detail: "契约（组合：字段值须符合该契约）",
          });
        }
        for (const n of collectFragmentNames(fullText)) {
          items.push({
            label: "&" + n,
            kind: CK("Reference", 0),
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
            kind: CK("Struct", 0),
            detail: "契约名（类型标注形式 @is type(契约名)）",
            insertText: n + ")",
          });
        }
      } else if (new RegExp(`@is\\s+[${ID}]*$`).test(linePrefix)) {
        for (const n of contractNames) {
          items.push({
            label: n,
            kind: CK("Struct", 0),
            detail: "契约名",
          });
        }
      }

      // 5) 行首键名补全（同文档出现过的键）
      if (new RegExp(`^\\s*[${ID}]*$`).test(linePrefix) && !isContractBody && !linePrefix.includes("@")) {
        for (const k of collectKeys(fullText)) {
          items.push({
            label: k,
            kind: CK("Property", 0),
            detail: "本文档中出现过的键",
            insertText: `${k}: `,
          });
        }
      }

      // 6) include / import 的**路径位置**：列出工作区里的 `.sml`（相对当前文档目录优先）。
      //    这是"模块化语法在编辑器里能用"的一半 —— 另一半是跳转（见 provideDefinition）。
      if (/^[\t ]*@?(include|import)[\t ]/.test(linePrefix)) {
        const opened = (linePrefix.match(/"/g) || []).length % 2 === 1; // 引号已开、未闭合
        // 不过滤"本行已用过的目标"：用户可能正想把这一处**改指到**别的文件，
        // 而 VS Code 本身会按已输入内容过滤，重复列出来的代价很小。
        let uris = [];
        try {
          uris = await vscode.workspace.findFiles("**/*.sml", "**/node_modules/**", 400);
        } catch {
          uris = [];
        }
        const docDir = document.uri.path.replace(/\/[^/]*$/, "");
        for (const u of uris) {
          if (u.toString() === document.uri.toString()) continue;
          const p = String(u.path || u.fsPath || "").replace(/\\/g, "/");
          const rel = String(vscode.workspace.asRelativePath(u, false)).replace(/\\/g, "/");
          // 展示与插入都用"相对当前文档目录"的写法（include 就是按那个解析的）
          const label = docDir && p.startsWith(docDir + "/") ? p.slice(docDir.length + 1) : rel;
          items.push({
            label,
            kind: CK("File", 0),
            detail: "被包含文件（工作区内）",
            insertText: opened ? label : '"' + label + '"',
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

      // include / import 的**路径**上：显示解析结果（找到 ✓ / 未找到 ✗ + 目标顶层键）。
      // 为什么单独做：`getWordRangeAtPosition` 的字符类不含 `/`，路径跨目录时会只拿到尾段。
      {
        const line = document.lineAt(position).text;
        if (/^[\t ]*@?(include|import)[\t ]/.test(line)) {
          const tgs = await resolveIncludeTargets(document, line);
          const hit = tgs.find((t) => position.character >= t.col - 1 && position.character <= t.end + 1);
          if (hit) {
            let md;
            if (hit.regex) {
              md = "**正则 include**：`" + hit.path + "`\n\n受限正则匹配由 Rust 侧的 `regex-include` 支持（需 `@feature enable regex-include`）。";
            } else if (hit.uri) {
              md = "**被包含文件**：`" + String(hit.uri.fsPath || hit.uri.path) + "` ✓\n\nF12 / Ctrl+Click 可跳过去。";
              try {
                const t = Buffer.from(await vscode.workspace.fs.readFile(hit.uri)).toString("utf8");
                const keys = mod && mod.collectKeys ? mod.collectKeys(t) : [];
                if (keys.length) {
                  md += "\n\n**该文件顶层键**：" + keys.slice(0, 12).join("、") + (keys.length > 12 ? " …" : "");
                }
              } catch {
                /* 读不到内容就只显示路径 */
              }
            } else {
              md = "**未找到目标**：`" + hit.path + "`\n\n相对路径按**被包含文件所在目录**解析；检查文件名与是否在工作区内。";
            }
            return new vscode.Hover(new vscode.MarkdownString(md));
          }
        }
      }

      if (mod && mod.contractHoverMarkdown && mod.collectContractNames) {
        const text = document.getText();
        const contractNames = mod.collectContractNames(text);
        if (contractNames.includes(word)) {
          const md = mod.contractHoverMarkdown(text, word);
          if (md) {
            // 「只显示声明、没有展开」是最容易被当成「功能坏了」的一种情况，
            // 故把**原因**写进输出面板（同一文档版本只解释一次，免得鼠标划过就刷屏）。
            const key = document.uri.toString() + "@" + document.version;
            if (!explained.has(key)) {
              if (explained.size > 200) explained.clear();
              explained.add(key);
              let inst = null;
              try { inst = mod.contractInstance ? mod.contractInstance(text, word) : null; } catch { /* 忽略 */ }
              if (!inst) {
                const r = mod.parseSafe ? mod.parseSafe(text) : { ok: true };
                log(`悬停「${word}」：只显示契约声明（取不到实例）`);
                log(!r.ok
                  ? `  原因：文档未通过校验 —— ${r.error}`
                  : `  原因：文档能解析，但没有用 \`@is ${word}\` 或 \`${word} 块名 {\` 标注的块`);
              }
            }
            return new vscode.Hover(new vscode.MarkdownString(md), range);
          }
        }

        // 块名（`primary {` / `Server primary {`）→ 显示这个块的结构 + 它应用的契约。
        // 为什么要做：块名上的悬停此前**什么都不显示**（只认契约名与关键字），
        // 而用户最常停的地方就是块名 —— 于是「悬停没用」的印象多半来自这里。
        if (mod.blockHoverMarkdown) {
          const lineText = document.lineAt(position.line).text;
          const bm = /^(\s*)([^\s:{}]+)(?:\s+([^\s{}]+))?\s*\{\s*(?:[#/].*)?$/.exec(lineText);
          if (bm) {
            const headCol = bm[1].length;
            const headEnd = headCol + bm[2].length;
            const nameCol = bm[3] ? lineText.indexOf(bm[3], headEnd) : -1;
            const c = position.character;
            const onHead = c >= headCol && c <= headEnd;
            const onName = nameCol >= 0 && c >= nameCol && c <= nameCol + bm[3].length;
            if (onHead || onName) {
              const md = mod.blockHoverMarkdown(text, position.line, contractNames);
              if (md) return new vscode.Hover(new vscode.MarkdownString(md), range);
            }
          }
        }

        // 字段级：契约声明里停在字段名上（`port: int default 5432`），或数据区里停在
        // 属于某契约的键上 → 给类型 / 枚举 / 默认值 / 区间 / 必填可选 + 行尾说明 + 当前值。
        if (mod.fieldHoverMarkdown) {
          const md = mod.fieldHoverMarkdown(text, word, position.line);
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
          // 字段优先：数据区的键 → 契约里的字段声明；契约里的字段 → 数据区第一处同名键。
          // 「键 → 字段」是这套契约机制里最省事的一跳：读到 `port: 9090` 想知道它是什么，
          // 直接跳过去看 `port: int default 5432 min 1 max 65535`。
          if (!isRef && mod.findFieldDefinition) {
            const f = mod.findFieldDefinition(text, name, position.line);
            if (f) {
              const start = new vscode.Position(f.line, f.col);
              return new vscode.Location(
                document.uri,
                new vscode.Range(start, start.translate({ characterDelta: f.length }))
              );
            }
          }
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

          // include / import 的**路径** ⇒ 跳到**被包含文件**（跨文件跳转）。
          // 注意：光标落在关键字或路径上都能触发（路径列区间 ±1 是给引号/空格留余量）。
          {
            const line = document.lineAt(position).text;
            if (/^[\t ]*@?(include|import)[\t ]/.test(line)) {
              const tgs = await resolveIncludeTargets(document, line);
              const hit = tgs.find(
                (t) => position.character >= t.col - 1 && position.character <= t.end + 1
              );
              if (hit && hit.uri) {
                return new vscode.Location(hit.uri, new vscode.Range(0, 0, 0, 0));
              }
            }
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
          // 带文件表：否则含 include 的文档会"无法格式化：include 目标未找到"
          const r = parseSafe(text, { files: await buildFilesMap(document) });
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

  // —— 自检：把「为什么没反应」摊开写进「输出 → SML」——
  initSelfCheck(context);
  initLanguageGuard(context);

  // —— 特殊颜色：选中词 → 右键应用（写入 HL-cfg.sml，随仓库走）——
  initApplyColor(context);

  // —— 自定义高亮：HL-cfg.sml + 强度开关（见 src/highlight.js）——
  require("./highlight.js").initHighlight(context);

  // —— 特别高亮：把选中词在整个工作区点亮（见 src/special-highlight.js）——
  // 与上面的「自定义高亮」不是一回事：那个按 HL-cfg.sml 静态配置点亮**关键词**，
  // 这个是**临时探照灯** —— 选中什么就点亮什么，再触发一次即取消。
  require("./special-highlight.js").initSpecialHighlight(context);

  // —— 语义高亮：块级类型标注 `<契约名> <块名> { .. }` 的契约名 ——
  initSemanticHighlight(context);

  suggestIconTheme(context);
  initIconThemeCommand(context);
}

// 文件图标主题需用户选择才生效，故首次激活时询问一次（可永久关闭提示）
//
// ⚠️ 这里踩过一个很贵的坑（用户报「启用完别的文件全没图标了」）：
// 我们**贡献了两个**图标主题 —— `sml-icons`（仅 .sml，其他文件没有图标）与
// `sml-icons-seti`（SML + Seti 兜底，其他文件沿用 Seti）。提示语写的是「继承现有图标集，
// 只影响 .sml」，但代码设的却是 **`sml-icons`** ⇒ 一按「启用」，用户整个工作区的图标全没了
// （只剩 .sml 有图标）。**提示语与行为不一致**，是最难被发现的那类 bug：文案是对的，代码是错的。
// 现在：默认一律用 `sml-icons-seti`；且**不覆盖**用户已有的第三方图标主题（改为引导他去选）；
// 另外给已经中招的人（当前就是 `sml-icons`）一次性修复提示。
const ICON_THEMES = {
  ours: ["sml-icons-seti", "sml-icons"],
  // 「继承其他文件图标」的那个（Seti 兜底）—— 一律优先用它
  combined: "sml-icons-seti",
  smlOnly: "sml-icons",
};

async function suggestIconTheme(context) {
  const promptedKey = "sml.iconThemePrompted";
  const repairedKey = "sml.iconThemeRepairPrompted";
  const cfg = vscode.workspace.getConfiguration("workbench");
  const current = cfg.get("iconTheme", "");

  // ① 修复路径：装过更早版本的用户被切到了「仅 .sml」主题 ⇒ 其他文件没有图标。
  //    给一次「换成 SML + Seti」的机会（不强制，也不重复烦他）。
  if (current === ICON_THEMES.smlOnly && !context.globalState.get(repairedKey)) {
    await context.globalState.update(repairedKey, true);
    const fix = await vscode.window.showInformationMessage(
      "SML：当前文件图标主题是「SML Icons（仅 .sml）」，其他文件会没有图标。" +
        "换成「SML Icons + Seti」可以保留其他文件的图标。",
      "换成 SML + Seti",
      "保持现状"
    );
    if (fix === "换成 SML + Seti") {
      await cfg.update("iconTheme", ICON_THEMES.combined, vscode.ConfigurationTarget.Global);
      vscode.window.showInformationMessage("已切换为「SML Icons + Seti」✓");
    }
    return;
  }

  if (context.globalState.get(promptedKey)) return;
  if (ICON_THEMES.ours.includes(current)) return;

  const pick = await vscode.window.showInformationMessage(
    "SML：是否为 .sml 文件启用青色图标？（用「SML Icons + Seti」——其他文件沿用你原来的 Seti 图标）",
    "启用",
    "不再提示"
  );
  if (pick === "启用") {
    // 用户已有**第三方**图标主题时不硬换（那会把他整套图标换掉，就是本次的教训）——
    // 改为把他送到「文件图标主题」选择器，由他自己挑。
    if (current && !ICON_THEMES.ours.includes(current)) {
      const go = await vscode.window.showInformationMessage(
        `你现在用的是「${current}」图标主题，直接切换会把它换掉。` +
          "建议在图标主题选择器里手动选「SML Icons + Seti」（它自带 Seti 兜底）。",
        "打开图标主题选择器",
        "仍然切换"
      );
      if (go === "打开图标主题选择器") {
        await vscode.commands.executeCommand("workbench.action.selectIconTheme");
        return;
      }
      if (go !== "仍然切换") return;
    }
    await cfg.update("iconTheme", ICON_THEMES.combined, vscode.ConfigurationTarget.Global);
  } else if (pick === "不再提示") {
    await context.globalState.update(promptedKey, true);
  }
}

/// 「SML: 文件图标主题」——把「切错了想切回来 / 想主动选」这件事变成一条命令。
///
/// 为什么必须给：图标主题是**全局设置**，一旦被切到「仅 .sml」，用户看到的是「整个工作区的
/// 文件图标都没了」，却未必知道那是本扩展干的（更不知道该改哪个设置）。给一条能一键切回的路。
function initIconThemeCommand(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand("sml.selectIconTheme", async () => {
      const cfg = vscode.workspace.getConfiguration("workbench");
      const cur = cfg.get("iconTheme", "");
      const pick = await vscode.window.showQuickPick(
        [
          { label: "SML Icons + Seti", description: "推荐：.sml 用 SML 图标，其他文件沿用 Seti", id: ICON_THEMES.combined },
          { label: "SML Icons（仅 .sml）", description: "⚠️ 其他文件**不会**有图标（只在你只想看 .sml 时选）", id: ICON_THEMES.smlOnly },
          { label: "打开 VS Code 的图标主题选择器…", description: `当前：${cur || "（未设置）"} —— 选回 Seti / Material 等你原来的主题`, id: "" },
        ],
        { placeHolder: "选择 .sml 文件图标方案" }
      );
      if (!pick) return;
      if (!pick.id) {
        await vscode.commands.executeCommand("workbench.action.selectIconTheme");
        return;
      }
      await cfg.update("iconTheme", pick.id, vscode.ConfigurationTarget.Global);
      vscode.window.showInformationMessage(`文件图标主题已切换为「${pick.label}」`);
    })
  );
}

function deactivate() {}

module.exports = { activate, deactivate };
