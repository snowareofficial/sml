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

// SML 标识符可含中文（如契约名 `受理人`），故收集函数一律用 Unicode 感知正则：
// `[\p{L}_]` 匹配任意语言的字母或下划线，`[\p{L}\p{N}_.\-]` 含字母/数字/点/连字符。
// 仅用 `[A-Za-z_][\w.-]*` 会把中文契约名整个漏掉，导致补全/高亮/跳转全部失效。

/// 收集文档中出现过的契约名（供补全）
export function collectContractNames(text) {
  const names = new Set();
  const re = /@contract\s+([\p{L}_][\p{L}\p{N}_.\-]*)/gu;
  let m;
  while ((m = re.exec(text)) !== null) names.add(m[1]);
  return [...names];
}

/// 收集文档中出现过的片段名（供补全）
export function collectFragmentNames(text) {
  const names = new Set();
  const re = /@([\p{L}_][\p{L}\p{N}_.\-]*)\s*\{/gu;
  let m;
  while ((m = re.exec(text)) !== null) {
    if (m[1] !== "contract" && m[1] !== "is" && m[1] !== "version" && m[1] !== "include") {
      names.add(m[1]);
    }
  }
  return [...names];
}

/// 收集 @type 声明的自定义类型名（供契约字段的类型位补全）
///
/// `@type name: 手机号 { ... }` -> "手机号"
export function collectTypeNames(text) {
  const names = new Set();
  const re = /@type\s+name:\s*([^\s{]+)/g;
  let m;
  while ((m = re.exec(text)) !== null) {
    const n = m[1];
    // 排除 `name` 关键字本身被误当作类型名（如 `@type name: { }` 的残缺写法）
    if (n && n !== "name") names.add(n);
  }
  return [...names];
}

/// 定位块级类型标注 `<契约名> <块名> { .. }` 中契约名的位置（供语义高亮）。
///
/// grammar 无法知道哪些名字是契约名（那是语义），故由扩展按已定义的契约名
/// 反查并用 decorations 上色。
///
/// 判定：行首第一个词命中契约表，且后面**至少还有一个词**再接 `{`
/// （`contact { }` 这类无名块不是类型标注，须排除）。
///
/// 返回 [{ line, col, length, name }]，line/col 从 0 起。
export function findAnnotatedBlocks(text, contractNames) {
  const set = new Set(contractNames || []);
  if (set.size === 0) return [];
  const out = [];
  const lines = text.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const m = /^(\s*)([^\s:{}]+)(?:\s+[^\s{}]+)+\s*\{/.exec(line);
    if (!m) continue;
    const head = m[2];
    if (head.startsWith("@")) continue; // 指令 / 片段定义不是类型标注
    if (!set.has(head)) continue;
    out.push({ line: i, col: m[1].length, length: head.length, name: head });
  }
  return out;
}

/// 收集文档中出现过的键名（供同文档内补全）
export function collectKeys(text) {
  const keys = new Set();
  const re = /^\s*([\p{L}_][\p{L}\p{N}_.\-]*)\s*:/gmu;
  let m;
  while ((m = re.exec(text)) !== null) keys.add(m[1]);
  return [...keys];
}
