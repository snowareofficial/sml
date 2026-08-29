// SML 解析桥接层
//
// 直接复用仓库的 JS 实现（js/sml.mjs），保证插件与语言实现**行为一致**：
// 同一份文本，插件报错的地方就是解析器真正报错的地方。
// 该实现零依赖、纯 ESM，可被 VSCode 扩展宿主（Node）直接 import。
//
// 注：契约校验目前仅 Rust 实现支持（见 ../../TODO.md），JS 侧只做语法解析。
// 因此契约相关的语义错误在插件中不会报出——这是已知限制，已在 README 说明。

// 注：引用的是 `./vendor/sml.mjs`（由 scripts/sync-parser.py 从
// `js/sml.mjs` 复制而来），**不是** `../../../js/sml.mjs`：
// VSIX 只包含扩展目录内的文件，跨目录 import 的模块不会被打进包，
// 装到别的机器上会找不到模块。打包前请运行 sync-parser.py。
import { parseSafe, parse, stringify, offsetToPosition } from "./vendor/sml.mjs";

export { parseSafe, parse, stringify, offsetToPosition };

/// 解析文本，产出编辑器可用的诊断列表。
/// 返回 [{ line, col, message, severity }]，line/col 从 0 起。
export function diagnose(text) {
  const r = parseSafe(text);
  if (r.ok) return [];
  const pos = r.position ?? (r.pos != null ? offsetToPosition(text, r.pos) : { line: 0, col: 0 });
  // 解析一旦失败即中止，因此同时只有一条错误；把错误范围标到该行行尾
  const lineText = (text.split("\n")[pos.line] ?? "");
  return [
    {
      line: pos.line,
      col: pos.col,
      endCol: lineText.length,
      message: r.error,
      severity: "error",
    },
  ];
}

/// 收集文档中出现过的契约名（供补全）
export function collectContractNames(text) {
  const names = new Set();
  const re = /@contract\s+([A-Za-z_][\w.-]*)/g;
  let m;
  while ((m = re.exec(text)) !== null) names.add(m[1]);
  return [...names];
}

/// 收集文档中出现过的片段名（供补全）
export function collectFragmentNames(text) {
  const names = new Set();
  const re = /@([A-Za-z_][\w.-]*)\s*\{/g;
  let m;
  while ((m = re.exec(text)) !== null) {
    if (m[1] !== "contract" && m[1] !== "is" && m[1] !== "version" && m[1] !== "include") {
      names.add(m[1]);
    }
  }
  return [...names];
}

/// 收集文档中出现过的键名（供同文档内补全）
export function collectKeys(text) {
  const keys = new Set();
  const re = /^\s*([A-Za-z_][\w.-]*)\s*:/gm;
  let m;
  while ((m = re.exec(text)) !== null) keys.add(m[1]);
  return [...keys];
}
