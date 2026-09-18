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
const EXPECT_VENDOR = { size: 69404, shaPrefix: "70f1ee47" };

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
      const r = mod.parseSafe ? mod.parseSafe(text) : { ok: true };
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
                  : `  原因：文档能解析，但没有用 \`@is ${word}\` 标注的**顶层**块（嵌在别的块里不算）`);
              }
            }
            return new vscode.Hover(new vscode.MarkdownString(md), range);
          }
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

  // —— 自检：把「为什么没反应」摊开写进「输出 → SML」——
  initSelfCheck(context);
  initLanguageGuard(context);

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
