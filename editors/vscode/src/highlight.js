// SML 自定义高亮
//
// 目标：让用户用一份 HL-cfg.sml 自定义关键词高亮，并可整体调低/关闭高亮，
// 且**不与语言官方关键字冲突**。
//
// 实现取舍：不改 TextMate grammar（grammar 是静态 JSON，改动须重载窗口），
// 改用 VSCode 的 decorations API 叠加渲染 —— 配置改动即时生效。
//
// 配置文件本身用 SML 书写，由本项目的解析器解析（自举）：
//
//   密级词 {
//       words: [ 公开 内部 秘密 机密 绝密 ]
//       color: "#ff9f43"
//       bold: true
//   }

const vscode = require("vscode");
const path = require("path");
const fs = require("fs");

// 官方关键字：自定义组不得覆盖（否则会让语言结构失去辨识度）
const RESERVED = new Set([
  "@contract", "@is", "@version", "@feature", "@when", "@for",
  "include", "import", "as", "in", "loose", "strict",
  "str", "int", "num", "bool", "any", "array", "enum",
  "default", "min", "max", "required", "optional",
  "true", "false", "null",
]);

const state = {
  sml: null,
  groups: [],
  decoTypes: [],      // 自定义组的 decoration type
  baseType: null,     // off / minimal 的覆盖层
  minimalTypes: [],   // minimal 下重新点亮的注释/字符串/数字
  warned: new Set(),
};

async function getSml() {
  if (!state.sml) state.sml = await import("./sml-parse.mjs");
  return state.sml;
}

function configPath() {
  const rel = vscode.workspace
    .getConfiguration("sml")
    .get("highlight.configFile", "HL-cfg.sml");
  const folder = vscode.workspace.workspaceFolders?.[0];
  if (!folder) return null;
  return path.join(folder.uri.fsPath, rel);
}

function mode() {
  return vscode.workspace.getConfiguration("sml").get("highlight.mode", "full");
}

// 词 -> 正则片段：ASCII 词加 \b 边避免子串误伤；中文无词边界，直接匹配
function wordPattern(w) {
  const esc = w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return /^[\x20-\x7e]+$/.test(w) ? `\\b${esc}\\b` : esc;
}

function disposeTypes() {
  for (const t of state.decoTypes) t.dispose();
  for (const t of state.minimalTypes) t.dispose();
  if (state.baseType) state.baseType.dispose();
  state.decoTypes = [];
  state.minimalTypes = [];
  state.baseType = null;
}

async function loadGroups() {
  const p = configPath();
  if (!p || !fs.existsSync(p)) return { groups: [], skipped: [], file: null };

  const { parseSafe } = await getSml();
  const r = parseSafe(fs.readFileSync(p, "utf-8"));
  if (!r.ok) throw new Error(`${path.basename(p)} 解析失败：${r.error}`);

  const groups = [];
  const skipped = [];
  for (const [name, val] of Object.entries(r.value || {})) {
    if (!val || typeof val !== "object" || Array.isArray(val)) continue;
    if (name === "__type" || name === "__name") continue;
    const words = val.words;
    if (!Array.isArray(words) || words.length === 0) continue;

    const kept = [];
    for (const w of words) {
      const s = String(w);
      if (RESERVED.has(s.toLowerCase())) {
        skipped.push(`${name}.${s}`);
        continue;
      }
      kept.push(s);
    }
    if (!kept.length) continue;

    groups.push({
      name,
      words: kept,
      color: typeof val.color === "string" ? val.color : undefined,
      background: typeof val.background === "string" ? val.background : undefined,
      bold: val.bold === true,
      italic: val.italic === true,
      underline: val.underline === true,
      matchCase: val.matchCase === true,
    });
  }
  return { groups, skipped, file: p };
}

function buildTypes(groups) {
  for (const g of groups) {
    const opts = {};
    if (g.color) opts.color = g.color;
    if (g.background) opts.backgroundColor = g.background;
    if (g.bold) opts.fontWeight = "bold";
    if (g.italic) opts.fontStyle = "italic";
    if (g.underline) opts.textDecoration = "underline";
    const pattern = new RegExp(
      g.words.map(wordPattern).join("|"),
      g.matchCase ? "g" : "gi"
    );
    state.decoTypes.push({ type: vscode.window.createTextEditorDecorationType(opts), pattern, group: g });
  }
}

// minimal 模式：先整体压成单色，再把注释/字符串/数字重新点亮
function buildMinimalTypes() {
  state.baseType = vscode.window.createTextEditorDecorationType({
    color: new vscode.ThemeColor("editor.foreground"),
    backgroundColor: new vscode.ThemeColor("editor.background"),
  });
  const revive = [
    { pattern: /(?:#|--|\/\/)[^\n]*/g, color: new vscode.ThemeColor("editor.foreground") },
    { pattern: /"[^"\n]*"/g, color: "#ce9178" },
    { pattern: /(?<!\w)-?\d+(\.\d+)?/g, color: "#b5cea8" },
  ];
  for (const r of revive) {
    state.minimalTypes.push({
      type: vscode.window.createTextEditorDecorationType({
        color: r.color,
        backgroundColor: new vscode.ThemeColor("editor.background"),
      }),
      pattern: r.pattern,
    });
  }
}

function rangesOf(document, pattern) {
  const text = document.getText();
  const out = [];
  let m;
  pattern.lastIndex = 0;
  while ((m = pattern.exec(text)) !== null) {
    if (m[0].length === 0) { pattern.lastIndex++; continue; }
    out.push(new vscode.Range(document.positionAt(m.index), document.positionAt(m.index + m[0].length)));
    if (out.length > 20000) break; // 硬上限，避免超大文档卡死
  }
  return out;
}

function clearDecorations(editor) {
  for (const d of state.decoTypes) editor.setDecorations(d.type, []);
  for (const d of state.minimalTypes) editor.setDecorations(d.type, []);
  if (state.baseType) editor.setDecorations(state.baseType, []);
}

async function applyTo(editor) {
  if (!editor || editor.document.languageId !== "sml") return;
  clearDecorations(editor);

  const m = mode();
  if (m === "off" || m === "minimal") {
    if (!state.baseType) buildMinimalTypes();
    const all = new vscode.Range(
      editor.document.positionAt(0),
      editor.document.positionAt(editor.document.getText().length)
    );
    editor.setDecorations(state.baseType, [all]);
    if (m === "minimal") {
      for (const d of state.minimalTypes) {
        editor.setDecorations(d.type, rangesOf(editor.document, d.pattern));
      }
      return; // minimal 下不叠加自定义组，保持克制
    }
  }

  for (const d of state.decoTypes) {
    editor.setDecorations(d.type, rangesOf(editor.document, d.pattern));
  }
}

async function reload(notify = false) {
  disposeTypes();
  try {
    const { groups, skipped } = await loadGroups();
    state.groups = groups;
    buildTypes(groups);
    if (skipped.length && !state.warned.has(skipped.join())) {
      state.warned.add(skipped.join());
      vscode.window.showWarningMessage(
        `HL-cfg：${skipped.length} 个词与官方关键字冲突已忽略（${skipped.slice(0, 3).join("、")}${skipped.length > 3 ? "…" : ""}）`
      );
    }
    if (notify) {
      vscode.window.showInformationMessage(
        groups.length
          ? `HL-cfg 已载入 ${groups.length} 组：${groups.map((g) => g.name).join("、")}`
          : "HL-cfg 未找到或没有有效分组（需顶层块含 words 数组）"
      );
    }
  } catch (e) {
    vscode.window.showErrorMessage(`SML 高亮配置加载失败：${e.message}`);
  }
  const ed = vscode.window.activeTextEditor;
  if (ed) await applyTo(ed);
}

function initHighlight(context) {
  let timer = null;
  const schedule = () => {
    if (timer) clearTimeout(timer);
    timer = setTimeout(() => {
      const ed = vscode.window.activeTextEditor;
      if (ed) applyTo(ed);
    }, 150);
  };

  context.subscriptions.push(
    vscode.commands.registerCommand("sml.reloadHighlight", () => reload(true)),
    vscode.commands.registerCommand("sml.setHighlightMode", async () => {
      const pick = await vscode.window.showQuickPick(
        [
          { label: "full", description: "全部高亮（默认）" },
          { label: "minimal", description: "只保留注释 / 字符串 / 数字" },
          { label: "off", description: "全部关闭" },
        ],
        { placeHolder: "选择 SML 高亮强度" }
      );
      if (!pick) return;
      await vscode.workspace
        .getConfiguration("sml")
        .update("highlight.mode", pick.label, vscode.ConfigurationTarget.Workspace);
      await reload(false);
    }),
    vscode.commands.registerCommand("sml.createHighlightConfig", async () => {
      const p = configPath();
      if (!p) return vscode.window.showErrorMessage("请先打开一个工作区文件夹");
      if (fs.existsSync(p)) return vscode.window.showInformationMessage(`已存在：${p}`);
      const tpl = `# SML 自定义高亮配置（本文件自身也是 SML）
#
# 每个顶层块 = 一组。可用字段：
#   words      关键词列表（必填）
#   color      前景色，如 "#ff9f43"
#   background 背景色
#   bold / italic / underline   布尔
#   matchCase  区分大小写（默认 false）
#
# 与官方关键字（str/int/default/@contract/…）重名的词会被忽略并提示。

密级词 {
    words: [ 公开 内部 秘密 机密 绝密 ]
    color: "#ff9f43"
    bold: true
}

部门名 {
    words: [ 公安分局 人社局 城管局 ]
    color: "#4ec9b0"
}
`;
      fs.writeFileSync(p, tpl, "utf-8");
      const doc = await vscode.workspace.openTextDocument(p);
      await vscode.window.showTextDocument(doc);
      await reload(false);
    }),

    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("sml.highlight")) reload(false);
    }),
    vscode.window.onDidChangeActiveTextEditor((ed) => applyTo(ed)),
    vscode.workspace.onDidChangeTextDocument(schedule),
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (configPath() && doc.uri.fsPath === configPath()) reload(false);
    }),
    { dispose: () => disposeTypes() }
  );

  reload(false);
}

module.exports = { initHighlight, applyTo, reload };
