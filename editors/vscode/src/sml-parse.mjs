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

/// 找出「用 `@is <契约名>` 标注的块」在**解析结果**里的实例（默认值已由解析器填充）。
///
/// 做法：先在文本里找到 `@is <name>`，再向上找最近的 `名字 {`（或 `契约名 块名 {`）
/// 取出块名，最后从解析结果里取该键 —— 拿到的就是**应用契约之后**的结构。
///
/// 只认**顶层**块：取不到就返回 `null`，由调用方退化为「只显示契约声明」。
/// 宁可少显示，也不要显示错的内容。
///
/// 返回 `{ key, line, value }` 或 `null`。
export function contractInstance(text, name) {
  if (!name) return null;
  const r = parseSafe(text);
  if (!r.ok) return null;
  const lines = text.split("\n");
  const esc = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const top = (key) => (r.value && typeof r.value === "object" ? r.value[key] : undefined);
  // `@is Name` 所在行往上找最近的 `名字 {`，取块名
  const pickUp = (startLine) => {
    for (let j = startLine; j >= 0; j--) {
      const bm = /^\s*([^\s:{}]+)(?:\s+([^\s{}]+))?\s*\{\s*$/.exec(lines[j]);
      if (!bm) continue;
      const key = bm[2] || bm[1];
      if (key.startsWith("@")) continue;
      const value = top(key);
      if (value === undefined) return null;
      return { key, line: j, value };
    }
    return null;
  };
  // 两种标注写法都认（文档主推 `@is`，类型标注形式在语义高亮里也在用）
  const isRe = new RegExp("^\\s*@is\\s+(" + esc + ")(?![\\p{L}\\p{N}_.\\-])", "u");
  const annoRe = new RegExp("^\\s*" + esc + "\\s+([^\\s{}]+)\\s*\\{\\s*$", "u");
  for (let i = 0; i < lines.length; i++) {
    if (isRe.test(lines[i])) {
      const hit = pickUp(i - 1);
      if (hit) return hit;
    }
    const a = annoRe.exec(lines[i]);
    if (a && !a[1].startsWith("@")) {
      const value = top(a[1]);
      if (value !== undefined) return { key: a[1], line: i, value };
    }
  }
  return null;
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
