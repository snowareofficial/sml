// SML 解析桥接层
//
// 直接复用仓库的 JS 实现（js/sml.mjs），保证插件与语言实现**行为一致**：
// 同一份文本，插件报错的地方就是解析器真正报错的地方。
// 该实现零依赖、纯 ESM，可被 VSCode 扩展宿主（Node）直接 import。
//
// 注：JS 实现的契约（`@contract` / `@is` / 默认值填充 / 严格与 loose）**已经支持**
// （早前注释写「契约仅 Rust 支持」，2026-09-18 更正）。因此本桥接层额外提供三件事：
//   - `findDefinition` / `contractHoverMarkdown`：跳转到契约定义、悬浮显示契约展开结果
//   - 语义错误（字段类型/枚举/区间/未声明字段）随解析一起报出，编辑器已能显示
// 唯一已知缺口：**嵌套数组值**（`m: [ [ a ] ]`）此处未验证，见 ../../TODO.md。

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

/// 官方指令名：写作 `@xxx` 但**不是**片段定义，收集与跳转都要排除。
///
/// 原实现只排了 4 个（contract / is / version / include），于是 `@when $env.X { }`、
/// `@for`、`@feature`、`@type name: X { }` 会被当成片段名收进补全列表 ——
/// 补全里冒出根本不存在的片段。这里列全，并让收集与跳转共用这一份名单。
const RESERVED_DIRECTIVES = new Set([
  "contract",
  "is",
  "type",
  "version",
  "feature",
  "when",
  "for",
  "include",
  "import",
]);

/// 收集文档中出现过的片段名（供补全）
export function collectFragmentNames(text) {
  const names = new Set();
  const re = /@([\p{L}_][\p{L}\p{N}_.\-]*)\s*\{/gu;
  let m;
  while ((m = re.exec(text)) !== null) {
    if (!RESERVED_DIRECTIVES.has(m[1])) names.add(m[1]);
  }
  return [...names];
}

/// 查找定义位置（供「跳转到定义」）。
///
/// - `kind === "contract"`：找 `@contract Name`（对应 `@is Name` 的跳转）
/// - `kind === "fragment"`：找 `@Name { }`（对应 `&Name` 的跳转）
///
/// 返回 `{ line, col, length }`（行列从 0 起，`length` 为名字长度）或 `null`。
/// 名字用 `\p{L}` 系列匹配，**中文契约名/片段名同样可跳**（与收集函数保持一致）。
export function findDefinition(text, name, kind = "contract") {
  if (!name) return null;
  if (kind === "fragment" && RESERVED_DIRECTIVES.has(name)) return null;
  const esc = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  // `@` 只在**词首**才是标记：词中的 `@` 是普通字符（如邮箱 `a@b.c`），
  // 故用后顾断言排除「前面还是词字符」的情况，避免把邮箱当指令。
  const NOT_WORD = "(?<![\\p{L}\\p{N}_.\\-])";
  const re =
    kind === "fragment"
      ? new RegExp(`${NOT_WORD}@${esc}(?![\\p{L}\\p{N}_.\\-])`, "u")
      : new RegExp(`${NOT_WORD}@contract\\s+(${esc})(?![\\p{L}\\p{N}_.\\-])`, "u");
  const lines = text.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const m = re.exec(lines[i]);
    if (!m) continue;
    const len = kind === "fragment" ? name.length : m[1].length;
    return { line: i, col: m.index + m[0].length - len, length: len };
  }
  return null;
}

/// 在文本里找 `term` 的**字面**出现位置（编辑器「特别高亮」用）。
///
/// 不按正则解释 —— 用户选中 `(`、`*`、`[` 这类字符时也必须按字面找，
/// 否则轻则少命中、重则抛异常。返回 `[{ line, col, length }]`（行列从 0 起）。
///
/// 选项：`caseSensitive`（默认 true）、`wholeWord`（默认 false）、`max`（默认 20000，超出即停）。
/// 放在本模块而非 `extension.js`，是为了**能脱离 VSCode 用 node 直接测**（与悬浮/跳转同一理由）。
export function findOccurrences(text, term, options = {}) {
  const { caseSensitive = true, wholeWord = false, max = 20000 } = options;
  const out = [];
  if (!text || !term) return out;
  const hay = caseSensitive ? text : text.toLowerCase();
  const needle = caseSensitive ? term : term.toLowerCase();

  // 行首表：一次扫完，之后二分定位 —— 比每个命中都重新数换行快得多（大文件下差一个量级）
  const starts = [0];
  for (let i = 0; i < text.length; i++) if (text[i] === "\n") starts.push(i + 1);
  const pos = (idx) => {
    let lo = 0, hi = starts.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (starts[mid] <= idx) lo = mid;
      else hi = mid - 1;
    }
    return { line: lo, col: idx - starts[lo] };
  };
  // 词边界用「SML 标识符字符集」判：字母（含中文）/数字/_/./-/@/&
  const isWordChar = (ch) => ch !== undefined && /[\p{L}\p{N}_.\-@&]/u.test(ch);

  let i = 0;
  while (out.length < max) {
    const k = hay.indexOf(needle, i);
    if (k < 0) break;
    i = k + Math.max(1, needle.length);   // 空串已在上面挡掉，这里保证前进
    if (wholeWord && (isWordChar(text[k - 1]) || isWordChar(text[k + term.length]))) continue;
    const { line, col } = pos(k);
    out.push({ line, col, length: term.length });
  }
  return out;
}

/// 可被「特殊颜色」识别的语法单元种类。
///
/// 为什么需要这个：给一个词上色，如果只按**字面**匹配，那么一个普通的 `active`
/// 会被染到程序里所有 `active` 上（包括注释、字符串、无关的键）。SML 的「单元」是
/// **语法位置**的概念 —— 同一个词出现在不同位置含义不同。所以特殊颜色要么按单元着色
/// （只染语法位置），要么用户明确选择「按普通词着色」（text）。
export const UNIT_KINDS = ["contract", "fragment", "type", "key", "directive", "text"];

/// 判定 `name` 在本文档里是哪种语法单元（按具体度排序，取最具体的那个）。
///
/// 顺序：契约 > 片段 > 类型 > 指令 > 键。都不是则返回 `null`
/// （由调用方决定是否退化为「普通词」）。中文名同样适用。
export function detectUnitKind(text, name) {
  if (!name) return null;
  const esc = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const B = "(?![\\p{L}\\p{N}_.\\-])";
  const u = (p) => new RegExp(p, "u");
  const lines = text.split("\n");
  const any = (re) => lines.some((l) => re.test(l));
  if (any(u("^\\s*@contract\\s+" + esc + B)) || any(u("^\\s*@is\\s+" + esc + B)) ||
      any(u("^\\s*" + esc + "\\s+[^\\s{}]+\\s*\\{"))) return "contract";
  if (any(u("^\\s*@" + esc + "\\s*\\{")) || any(u("&" + esc + B))) return "fragment";
  if (any(u("^\\s*@type\\s+name:\\s*" + esc + B))) return "type";
  if (any(u("^\\s*@" + esc + B)) || any(u("^\\s*@feature\\s+(?:enable|disable)\\s+" + esc + B))) return "directive";
  if (any(u("^\\s*" + esc + "\\s*:"))) return "key";
  return null;
}

/// 找出 `name` 作为**某种语法单元**出现的位置（供「特殊颜色」按单元着色）。
///
/// 与 `findOccurrences`（字面匹配）的区别：这里只认语法位置，例如
/// `contract` 只染 `@contract X` / `@is X` / `X 块名 {`，不会染注释或字符串里的同名文字。
/// `text` 退化为字面匹配（用户明确选择「按普通词着色」时用）。
///
/// 返回 `[{ line, col, length }]`（行列从 0 起），最多 `max` 条。
export function findUnitOccurrences(text, name, unit = "text", max = 20000) {
  if (!name) return [];
  if (!unit || unit === "text") return findOccurrences(text, name, { wholeWord: true, max });
  const esc = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const B = "(?![\\p{L}\\p{N}_.\\-])";
  const pats = {
    contract: [
      "^\\s*@contract\\s+(" + esc + ")" + B,     // 定义处
      "^\\s*@is\\s+(" + esc + ")" + B,           // 用法
      "^\\s*(" + esc + ")\\s+[^\\s{}]+\\s*\\{",  // 块级类型标注 `<契约名> 块名 {`
    ],
    fragment: [
      "^\\s*@(" + esc + ")\\s*\\{",              // 定义处 `@base {`
      "&(" + esc + ")" + B,                      // 引用 `&base`
    ],
    type: [
      "^\\s*@type\\s+name:\\s*(" + esc + ")" + B,  // 定义处
      "^\\s*[^\\s:{}]+\\s*:\\s*(" + esc + ")" + B, // 契约字段的类型位
    ],
    key: ["^(\\s*)(" + esc + ")\\s*:"],          // 数据区键名
    directive: ["^\\s*@(" + esc + ")" + B],
  }[unit];
  if (!pats) return findOccurrences(text, name, { wholeWord: true, max });
  const lines = text.split("\n");
  const out = [];
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    for (const p of pats) {
      const re = new RegExp(p, "gu");
      let m;
      // 一行里同一模式可能多次（如 `&a &a`），逐个取
      while ((m = re.exec(line)) !== null) {
        if (m[0].length === 0) { re.lastIndex++; continue; }
        // 抓取组：优先取含名字那一组（各组都试，取与 name 等长的那个）
        let col = -1;
        for (let g = m.length - 1; g >= 1; g--) {
          if (m[g] === name) { col = m.index + m[0].lastIndexOf(m[g]); break; }
        }
        if (col < 0) col = m.index + (m[0].length - name.length);
        out.push({ line: i, col, length: name.length });
        if (out.length >= max) return out;
      }
    }
  }
  // 去重（同一位置可能被两条模式同时命中）
  const seen = new Set();
  return out.filter((r) => {
    const k = r.line + ":" + r.col;
    if (seen.has(k)) return false;
    seen.add(k);
    return true;
  });
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

/// 提取 `@contract Name { ... }` 的**声明体**（纯文本括号配对，不做语义解析）。
///
/// 返回 `{ line, col, length, body: string[] }` 或 `null`：
/// `line/col/length` 指向契约名（供 hover 定位），`body` 是花括号内的原始行（已 trim）。
export function contractDeclaration(text, name) {
  if (!name) return null;
  const lines = text.split("\n");
  const esc = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const head = new RegExp(`^\\s*@contract\\s+(${esc})(?![\\p{L}\\p{N}_.\\-])`, "u");
  for (let i = 0; i < lines.length; i++) {
    const m = head.exec(lines[i]);
    if (!m) continue;
    // 从该行起做花括号配对，收集体内的原始行
    const col = m[0].length - m[1].length;
    const body = [];
    let depth = 0;
    let started = false;
    for (let j = i; j < lines.length; j++) {
      const line = lines[j];
      for (const ch of line) {
        if (ch === "{") {
          depth++;
          started = true;
        } else if (ch === "}") depth--;
      }
      if (j > i || started) {
        // 首行只取 `{` 之后的部分；其余行整行（去掉行尾注释不算，保持原始可读性）
        const text2 = j === i ? line.slice(line.indexOf("{") + 1) : line;
        const isLast = depth === 0;
        const piece = isLast ? text2.slice(0, text2.lastIndexOf("}")) : text2;
        if (piece.trim() !== "") body.push(piece.trim());
      }
      if (started && depth <= 0) break;
    }
    return { line: i, col, length: m[1].length, body };
  }
  return null;
}

/// 把一行里「结构性的花括号」数出来（先剥掉字符串与注释，避免 `{` 被误算）。
///
/// SML 里字符串可含 `{}`、注释也可含 `{}`，直接数字符会把层级算错 ⇒ 路径也就错了。
function bracesOf(rawLine) {
  const l = rawLine
    .replace(/"(?:[^"\\]|\\.)*"/g, '""')   // 字符串整体抹平（含转义）
    .replace(/#.*$/, "")                   // `#` 行尾注释
    .replace(/\/\/.*$/, "");               // `//` 行尾注释
  let open = 0;
  let close = 0;
  for (const ch of l) {
    if (ch === "{") open++;
    else if (ch === "}") close++;
  }
  return { open, close };
}

/// 解析一行是不是**块声明**：`键 {`、`键 名 {`、`契约名 块名 {`（行尾注释与空白可省）。
///
/// 返回 `{ head, name, col, headLen, nameCol }`：`head` 是首词（可能是键，也可能是契约名），
/// `name` 是第二个词（若有两个词）。都是 0 起列号，供「悬停块名」判断光标落在哪个词上。
function blockDeclOf(rawLine) {
  const m = /^(\s*)([^\s:{}]+)(?:\s+([^\s{}]+))?\s*\{\s*(?:[#/].*)?$/.exec(rawLine);
  if (!m) return null;
  const head = m[2];
  if (head.startsWith("@")) return null; // 指令 / 片段定义不是数据块
  const col = m[1].length;
  const name = m[3];
  return {
    head,
    name,
    col,
    headLen: head.length,
    nameCol: name ? col + head.length + (rawLine.slice(col + head.length).match(/^\s+/) || [""])[0].length : -1,
  };
}

/// 求**某个块**在解析结果里的位置：自身名、完整路径、以及（按路径取到的）值。
///
/// `line` 是块声明行（0 起）。做法：从头做一次花括号配对扫描，得到每一层的名字栈，
/// 第 `line` 行开的新块其路径就是「外层栈 + 自己」——这样**嵌套块**也能定位
/// （此前只认顶层，`database { primary { @is Server … } }` 里 primary 的实例取不到，
/// 于是悬停只剩契约声明，看起来就像「没生效」）。
///
/// 返回 `{ key, path, value, closeLine, contractFromHead }` 或 `null`。
export function blockPath(text, line) {
  const lines = text.split("\n");
  if (line < 0 || line >= lines.length) return null;
  const decl = blockDeclOf(lines[line]);
  if (!decl) return null;
  const key = decl.name || decl.head;

  // 一次前向扫描：维护「当前处于哪几层块内」的名字栈；遇到 target 行就把栈定格为它的外层路径。
  const stack = [];
  let outer = null;
  for (let i = 0; i < lines.length && outer === null; i++) {
    const d = blockDeclOf(lines[i]);
    if (i === line) outer = stack.slice();
    if (d) stack.push(d.name || d.head);
    const { open, close } = bracesOf(lines[i]);
    // 该行自身的 `{` 已计入 stack；`}` 弹栈（同一行 `{ }` 相抵）
    const selfOpen = d ? 1 : 0;
    for (let k = 0; k < open - selfOpen + close; k++) stack.pop();
  }
  if (outer === null) return null;
  const path = [...outer, key];

  const r = parseSafe(text);
  let value;
  if (r.ok && r.value && typeof r.value === "object") {
    value = r.value;
    for (const seg of path) {
      if (value === null || typeof value !== "object") { value = undefined; break; }
      value = value[seg];
    }
  }
  // 块的收尾行（供扫块体）：从声明行起按花括号配对找
  let depth = 0;
  let closeLine = -1;
  for (let i = line; i < lines.length; i++) {
    const { open, close } = bracesOf(lines[i]);
    depth += open - close;
    if (i > line || open > 0) {
      if (depth <= 0) { closeLine = i; break; }
    }
  }
  return { key, path, value, closeLine, contractFromHead: decl.name ? decl.head : null };
}

/// 找 `名字 {`（或 `契约名 块名 {`）在**解析结果**里的实例（默认值已由解析器填充）。
///
/// 现在支持**嵌套块**（走 `blockPath` 的路径查找）；取不到就返回 `null`，
/// 由调用方退化为「只显示契约声明」——宁可少显示，也不要显示错的内容。
///
/// 返回 `{ key, path, line, value, contract }` 或 `null`。
export function contractInstance(text, name) {
  if (!name) return null;
  const r = parseSafe(text);
  if (!r.ok) return null;
  const lines = text.split("\n");
  const esc = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  // 两种标注写法都认（文档主推 `@is`，类型标注形式在语义高亮里也在用）
  const isRe = new RegExp("^\\s*@is\\s+(" + esc + ")(?![\\p{L}\\p{N}_.\\-])", "u");
  const annoRe = new RegExp("^\\s*" + esc + "\\s+([^\\s{}]+)\\s*\\{\\s*$", "u");
  for (let i = 0; i < lines.length; i++) {
    let declLine = -1;
    if (isRe.test(lines[i])) {
      // 往上找最近的块声明行
      for (let j = i - 1; j >= 0; j--) {
        if (blockDeclOf(lines[j])) { declLine = j; break; }
      }
    } else {
      const a = annoRe.exec(lines[i]);
      if (a && !a[1].startsWith("@")) declLine = i;
    }
    if (declLine < 0) continue;
    const info = blockPath(text, declLine);
    if (info && info.value !== undefined) {
      return { key: info.key, path: info.path, line: declLine, value: info.value, contract: name };
    }
  }
  return null;
}

/// 组装「块」的悬浮内容（Markdown）：光标停在块声明行上时用。
///
/// 三件事：① 这个块叫什么、在哪条路径上；② 它应用了哪个契约（块内 `@is X`
/// 或 `契约名 块名 {` 形式）；③ **契约应用之后的实际结构**（解析器真跑出来的）。
/// 没有契约的块也给结构 —— 悬停块名「什么都不显示」是最容易被当成扩展坏了的情况。
export function blockHoverMarkdown(text, line, contractNames) {
  const info = blockPath(text, line);
  if (!info) return null;
  const lines = text.split("\n");
  const out = [];
  out.push("**块 `" + info.key + "`**" + (info.path.length > 1 ? "　（路径 `" + info.path.join(".") + "`）" : ""));
  // 契约：块内首个 `@is X`；或块声明行是 `契约名 块名 {`
  let contract = null;
  if (info.contractFromHead && (contractNames || []).includes(info.contractFromHead)) {
    contract = info.contractFromHead;
  } else if (info.closeLine > 0) {
    for (let i = line + 1; i < info.closeLine; i++) {
      const m = /^\s*@is\s+([^\s{]+)/.exec(lines[i]);
      if (m) { contract = m[1]; break; }
    }
  }
  if (contract) {
    out.push("");
    out.push("应用契约 **`" + contract + "`** —— 下面结构是**解析器应用契约后**的结果（缺失字段已按默认值填充）。");
  }
  if (info.value !== undefined) {
    out.push("");
    out.push("```sml");
    for (const l of stringify(info.value).split("\n")) if (l.trim() !== "") out.push(l);
    out.push("```");
  } else {
    out.push("");
    out.push("_当前文档无法解析，取不到这个块的结构_（先修掉校验错误，这里就会显示结果）。");
  }
  if (contract) {
    const decl = contractDeclaration(text, contract);
    if (decl) {
      out.push("");
      out.push("**契约 `" + contract + "` 的声明**");
      out.push("");
      out.push("```sml");
      for (const l of decl.body) out.push(l);
      out.push("```");
    }
  }
  return out.join("\n");
}

/// 组装契约的悬浮内容（Markdown）。契约名不存在时返回 `null`。
///
/// 放在本模块而非 `extension.js`，是为了**能脱离 VSCode 用 node 直接测** ——
/// 悬浮内容是纯文本逻辑，没必要和编辑器 API 绑在一起才能验。
///
/// 两段内容：① 契约声明（有哪些字段、什么修饰符）；② **填入默认值后的结构**
/// （由解析器真正应用契约得到，不是抄一遍声明）。取不到实例时明说「未找到」，
/// 而不是把声明伪装成结果 —— 悬浮里最容易骗人的就是这种「看起来像结果」的东西。
export function contractHoverMarkdown(text, name) {
  const decl = contractDeclaration(text, name);
  if (!decl) return null;
  const out = ["**契约 `" + name + "`**　（`@contract` 声明）", "", "```sml"];
  for (const line of decl.body) out.push(line);
  out.push("```");
  const inst = contractInstance(text, name);
  if (inst) {
    out.push("");
    out.push("**填入默认值后的结构** —— 来自块 `" + inst.key + "`（第 " + (inst.line + 1) + " 行）");
    out.push("");
    out.push("```sml");
    for (const line of stringify(inst.value).split("\n")) {
      if (line.trim() !== "") out.push(line);
    }
    out.push("```");
  } else {
    out.push("");
    out.push(
      "_未找到可展开的实例_：本文档里没有用 `@is " +
        name +
        "` 或 `" +
        name +
        " 块名 {` 标注的**顶层**块（或当前文档无法解析）。上面只是契约声明本身。"
    );
  }
  return out.join("\n");
}

/// 收集文档中出现过的键名（供同文档内补全）
export function collectKeys(text) {
  const keys = new Set();
  const re = /^\s*([\p{L}_][\p{L}\p{N}_.\-]*)\s*:/gmu;
  let m;
  while ((m = re.exec(text)) !== null) keys.add(m[1]);
  return [...keys];
}
