// SML 特别高亮：把选中的词在**整个工作区**里点亮（临时探照灯）
//
// 用法：选中一个词 → 右键「SML: 特别高亮选中词（当前工作区）」。
//   · 再触发同一个词 = 取消；状态栏显示命中规模，点一下也能清除；
//   · 右键菜单里另有「SML: 清除特别高亮」（只在有高亮时出现）。
//
// 实现取舍（每条都是取舍，别顺手「优化」掉）：
//  · **字面**子串匹配，不按正则解释 —— 用户选中 `(` / `*` / `[` 时也必须按字面找，
//    否则轻则少命中，重则抛异常。要的是探照灯，不是正则引擎。
//  · 只给**可见编辑器**上色（VSCode 的 decorations 只能作用于可见编辑器）；其余文件
//    照样计入统计，打开时按缓存补上。
//  · **不做语义判断**：注释 / 字符串里的同名文字照样点亮。文本级探照灯要**可预期**，
//    「聪明」在这里是负资产 —— 用户没法预测哪一处会亮。
//  · 规模有上限，超了**明说截断**，不假装搜全了。
const vscode = require("vscode");

const MAX_FILES = 500;                    // 参与搜索的文件数上限
const MAX_MATCHES = 20000;                // 命中处数上限
const MAX_FILE_BYTES = 4 * 1024 * 1024;   // 单文件超过就跳过（不把大文件读进内存）
// 与补全/跳转/语义高亮用同一套字符类：允许 `@`/`&` 前缀与中文名
const WORD_RE = /[@&]?[A-Za-z0-9_\u4e00-\u9fa5.\-]+/;

const state = {
  term: "",
  byUri: new Map(),   // uriString -> vscode.Range[]
  files: 0,           // 命中的文件数
  total: 0,           // 命中处数
  truncated: false,   // 是否触到上限（文件数或命中数）
  deco: null,
  status: null,
  sml: null,
};

let refreshTimer = null;

async function getSml() {
  if (!state.sml) state.sml = await import("./sml-parse.mjs");
  return state.sml;
}

function opts() {
  const cfg = vscode.workspace.getConfiguration("sml");
  return {
    caseSensitive: cfg.get("specialHighlight.caseSensitive", true) !== false,
    wholeWord: cfg.get("specialHighlight.wholeWord", false) === true,
  };
}

function decoType() {
  if (!state.deco) {
    state.deco = vscode.window.createTextEditorDecorationType({
      backgroundColor: new vscode.ThemeColor("editor.findMatchHighlightBackground"),
      border: "1px solid",
      borderColor: new vscode.ThemeColor("editor.findMatchBorder"),
      overviewRulerColor: new vscode.ThemeColor("editor.findMatchHighlightForeground"),
      overviewRulerLane: vscode.OverviewRulerLane.Center,
      rangeBehavior: vscode.DecorationRangeBehavior.ClosedClosed,
    });
  }
  return state.deco;
}

function paint(editor) {
  if (!state.deco) return;
  const ranges = state.byUri.get(editor.document.uri.toString());
  // 没有命中的编辑器也要**显式清空**一次：否则上一次的高亮会残留在屏幕上
  editor.setDecorations(state.deco, ranges || []);
}

function paintAll() {
  for (const ed of vscode.window.visibleTextEditors) paint(ed);
}

function updateStatus() {
  if (!state.status) return;
  if (!state.term) {
    state.status.hide();
    return;
  }
  state.status.text =
    `$(search) 特别高亮: ${state.term} · ${state.total} 处 / ${state.files} 文件` +
    (state.truncated ? "（已截断）" : "");
  state.status.tooltip = "点击清除特别高亮（或对同一个词再触发一次命令）";
  state.status.command = "sml.clearSpecialHighlight";
  state.status.show();
}

async function setActiveFlag(on) {
  try {
    await vscode.commands.executeCommand("setContext", "sml.specialHighlightActive", !!on);
  } catch {
    /* setContext 失败不影响功能本身 */
  }
}

async function clearAll(notify) {
  for (const ed of vscode.window.visibleTextEditors) {
    if (state.deco) ed.setDecorations(state.deco, []);
  }
  const had = state.term;
  state.term = "";
  state.byUri.clear();
  state.files = 0;
  state.total = 0;
  state.truncated = false;
  updateStatus();
  await setActiveFlag(false);
  if (notify && had) vscode.window.showInformationMessage(`已清除特别高亮（${had}）`);
}

/// 扫整个工作区，落进 state 并上色。返回是否命中（0 命中时调用方给提示）。
async function scan(term) {
  const include = vscode.workspace.getConfiguration("sml").get("specialHighlight.include", "**/*.sml");
  const { caseSensitive, wholeWord } = opts();
  const mod = await getSml();
  if (typeof mod.findOccurrences !== "function") {
    vscode.window.showErrorMessage("SML: 解析器版本过旧（缺 findOccurrences），请重装扩展");
    return 0;
  }

  const byUri = new Map();
  let files = 0;
  let total = 0;
  let truncated = false;

  let uris = [];
  try {
    // 多要一个，用来判断「是不是被上限截断了」
    uris = await vscode.workspace.findFiles(include || "**/*.sml", null, MAX_FILES + 1);
  } catch {
    uris = [];
  }
  if (uris.length > MAX_FILES) {
    truncated = true;
    uris = uris.slice(0, MAX_FILES);
  }

  const decoder = new TextDecoder("utf-8");
  for (const uri of uris) {
    if (total >= MAX_MATCHES) {
      truncated = true;
      break;
    }
    let bytes;
    try {
      bytes = await vscode.workspace.fs.readFile(uri);
    } catch {
      continue;   // 读不了的（权限/二进制）跳过，不打断整次搜索
    }
    if (bytes.length > MAX_FILE_BYTES) continue;
    const hits = mod.findOccurrences(decoder.decode(bytes), term, {
      caseSensitive,
      wholeWord,
      max: MAX_MATCHES - total,
    });
    if (!hits.length) continue;
    byUri.set(
      uri.toString(),
      hits.map((h) => new vscode.Range(h.line, h.col, h.line, h.col + h.length))
    );
    files++;
    total += hits.length;
  }

  state.term = term;
  state.byUri = byUri;
  state.files = files;
  state.total = total;
  state.truncated = truncated;
  paintAll();
  updateStatus();
  await setActiveFlag(true);
  return total;
}

/// 命令入口：拿当前选中（没选中就用光标下的词）→ 全工作区高亮
async function highlightSelection() {
  const ed = vscode.window.activeTextEditor;
  if (!ed) {
    vscode.window.showInformationMessage("SML: 请先打开文件并选中要特别高亮的词");
    return;
  }
  let term = "";
  if (!ed.selection.isEmpty) {
    term = ed.document.getText(ed.selection);
  } else {
    const r = ed.document.getWordRangeAtPosition(ed.selection.active, WORD_RE);
    if (r) term = ed.document.getText(r);
  }
  term = (term || "").trim();
  if (!term) {
    vscode.window.showInformationMessage("SML: 没有可高亮的内容 —— 请选中一段文字，或把光标放到词上");
    return;
  }
  if (term.includes("\n") || term.includes("\r")) {
    vscode.window.showWarningMessage("SML: 只支持同一行内的文本（跨行选中无法用单一词表示）");
    return;
  }
  if (term === state.term) {
    await clearAll(true);   // 同一个词再来一次 = 取消（比让人去找「清除」命令快）
    return;
  }
  await scan(term);
  if (state.total === 0) {
    vscode.window.showInformationMessage(`SML: 工作区内没有找到「${term}」`);
  }
}

/// 正在编辑的文档重扫（本地、快）——保持范围新鲜；**不**做整工作区重搜
async function refreshLocal(doc) {
  if (refreshTimer) clearTimeout(refreshTimer);
  refreshTimer = setTimeout(async () => {
    const key = doc.uri.toString();
    const before = (state.byUri.get(key) || []).length;
    let hits = [];
    try {
      const mod = await getSml();
      const { caseSensitive, wholeWord } = opts();
      hits = mod.findOccurrences(doc.getText(), state.term, {
        caseSensitive,
        wholeWord,
        max: MAX_MATCHES,
      });
    } catch {
      return;
    }
    if (hits.length) {
      state.byUri.set(key, hits.map((h) => new vscode.Range(h.line, h.col, h.line, h.col + h.length)));
    } else {
      state.byUri.delete(key);
    }
    state.total += hits.length - before;
    if (before === 0 && hits.length > 0) state.files++;
    if (before > 0 && hits.length === 0) state.files = Math.max(0, state.files - 1);
    if (state.total < 0) state.total = 0;
    paintAll();
    updateStatus();
  }, 200);
}

function initSpecialHighlight(context) {
  state.status = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 90);
  context.subscriptions.push(
    state.status,
    vscode.commands.registerCommand("sml.specialHighlight", () => highlightSelection()),
    vscode.commands.registerCommand("sml.clearSpecialHighlight", () => clearAll(true)),
    vscode.window.onDidChangeVisibleTextEditors(() => paintAll()),
    vscode.workspace.onDidChangeTextDocument((e) => {
      if (!state.term) return;
      if (state.byUri.has(e.document.uri.toString())) refreshLocal(e.document);
    })
  );
  decoType();
  void setActiveFlag(false);
}

module.exports = { initSpecialHighlight };
