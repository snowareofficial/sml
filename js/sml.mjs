// SPDX-License-Identifier: MulanPSL-2.0
// SML — SNOWARE Markup Language (JavaScript 实现, ESM)
//
// 纯 JS、零依赖，Node >=14 与浏览器均可用。语法与 Soup 生态的
// lib/sml.soup (Lua) 及 sml-rs (Rust) 对齐：
//   裸词字符串 / 引号串（转义 + $env 内联）/ true/false/null / 数字 /
//   块 key { } / 裸块 type name { } / 数组 [ ] / 逗号可选 /
//   注释：单行 `#` / `--` / `//`，多行 `/* */` 与 `_* *_` /
//   @name { } 片段定义 & 引用 /
//   @contract Name [loose] { ... } 契约定义与 @is Name 契约应用 /
//   include "x" [as ns] 多文件包含与命名空间隔离 /
//   @feature enable/disable 功能裁剪。
//
// API:
//   parse(text, opts?)            -> value | throws
//       opts.files: { "ui.sml": "...", ... }  虚拟文件表（用于 include）
//       opts.features: Set<string> | null      开启的 feature（null = 全部默认开）
//   parseSafe(text, opts?)        -> { ok, value|error, code, position }
//   stringify(v) / dump(v)        -> string   （序列化回 SML，round-trip）
//   契约错误以 message 中 "contract:" 前缀标识，可被 playground 高亮。
//
// ===========================================================================
// ⚠️ 与 Rust 实现（swsml）的已知差异 —— 改动本文件前请先读
// ===========================================================================
//
// 本实现与 Rust 侧在**语义安全**上已对齐（前导零、超范围整数、inf/nan），
// 但以下差异由 JavaScript 语言能力决定，**无法在本实现内消除**。
// 若发现新差异，请先判断属于哪一类：
//
// 【A 类 · 能力限制，不可修】—— 需要 Rust 引擎才能得到 Rust 的行为
//   A1. 无法区分 1.0 与 1
//       JS 的 Number("1.0") === 1，String(1.0) === "1"。
//       Rust 侧有独立的 Value::Float，且保存原始字面量（raw），
//       能输出 1.0 / 1.10 / 1e10。本实现做不到 —— number 是原始值，
//       无法携带 raw；装箱成 new Number(x) 会让 typeof 变成 "object"，
//       破坏契约系统的 isNum/isInt 与 JSON.stringify。
//   A2. 大整数阈值不同（2^53 vs 2^63）
//       Rust i64 上界 9223372036854775807（19 位）；
//       JS  MAX_SAFE_INTEGER 9007199254740991（16 位）。
//       因此 330106201503071234 在 Rust 侧是精确的 Int，
//       在本实现必须保为字符串（否则 Number() 会静默损坏）。
//       差异方向是**更保守** —— 宁可保字符串，也不损坏。
//       连带后果：str 契约在长号上，Rust 侧会拦截、本实现会放行（见 5.8）。
//
// 【B 类 · 已修，不得回退】
//   B1. 裸词 inf / nan 曾被 Number() 解析成 NaN（挪威问题同类）。
//       修法见 coerceWord 的 numericHead 闸门。
//   B2. 前导零（0571 / -007）曾被吃掉。修法见 LEADING_ZERO_INT。
//   B3. enum 语法：本实现曾只认 enum(...) / enum a b c，
//       而官网文档与 Rust 都用 enum [ ... ]，两端完全相反。
//       现已三种都支持，以 enum [ ... ] 为准。
//
// 【同步提醒】
//   仓库源文件是 **js/sml.mjs**，但 Playground 加载的是
//   **site/static/sml.mjs**（shortcode 里写死 "/sml.mjs"）。
//   两者曾是两个手工副本并发生漂移 —— 改完本文件后，
//   务必运行 `python _sync_playground.py` 同步，否则网页上不生效。
//
// ---------------------------------------------------------------------------
// 词法
// ---------------------------------------------------------------------------

function tokenize(text) {
  const toks = [];
  const n = text.length;
  let i = 0;
  let buf = "";
  let bufStart = 0;
  const flush = () => {
    if (buf !== "") { toks.push({ t: "word", v: buf, pos: bufStart }); buf = ""; }
  };
  while (i < n) {
    const c = text[i];
    if (c === "#") {
      while (i < n && text[i] !== "\n") i++;
    } else if (c === "-" && text[i + 1] === "-") {
      while (i < n && text[i] !== "\n") i++;
    } else if (c === "/" && text[i + 1] === "/") {
      while (i < n && text[i] !== "\n") i++;
    } else if (c === "/" && text[i + 1] === "*") {
      i += 2;
      let closed = false;
      while (i < n) {
        if (text[i] === "*" && text[i + 1] === "/") { i += 2; closed = true; break; }
        i++;
      }
      // EOF 未闭合的块注释必须报错（与 Rust 的 sml-lex 同码）：
      // 此前静默吞掉文件剩余部分，后面的键会凭空消失。
      if (!closed) throwCode("E-LEX-002", "sml: 未闭合的块注释 /* ... */（遇到文件结尾）");
    } else if (c === "_" && text[i + 1] === "*") {
      i += 2;
      let closed = false;
      while (i < n) {
        if (text[i] === "*" && text[i + 1] === "_") { i += 2; closed = true; break; }
        i++;
      }
      if (!closed) throwCode("E-LEX-003", "sml: 未闭合的块注释 _* ... *_（遇到文件结尾）");
    } else if (c === '"') {
      flush();
      const qStart = i;
      let s = "";
      i++;
      let closed = false;
      while (i < n) {
        const cc = text[i];
        if (cc === '"') { i++; closed = true; break; }
        if (cc === "\\" && i + 1 < n) {
          i++;
          const e = text[i];
          if (e === "u") {
            // \u{1F680} 或 \u1F680（4 位十六进制码点）
            if (text[i + 1] === "{") {
              let j = i + 2, hex = "";
              while (j < n && text[j] !== "}") { hex += text[j]; j++; }
              // W16：码点非法必须报码 —— 原先直接 `String.fromCodePoint(parseInt(...))`，
              // 非法输入会抛**宿主 RangeError**（没有码，等于逃出错误码体系）。
              if (j >= n || !/^[0-9a-fA-F]+$/.test(hex)) {
                throwCode("E-LEX-005", "sml: Unicode 转义非法（\\u{...} 缺失或非十六进制）");
              }
              i = j + 1;
              const cp = parseInt(hex, 16);
              if (!Number.isFinite(cp) || cp > 0x10ffff || (cp >= 0xd800 && cp <= 0xdfff)) {
                throwCode("E-LEX-005", "sml: Unicode 转义非法（码点越界或落在代理区）");
              }
              s += String.fromCodePoint(cp);
            } else {
              const hex = text.slice(i + 1, i + 5);
              if (!/^[0-9a-fA-F]{4}$/.test(hex)) {
                throwCode("E-LEX-005", "sml: Unicode 转义非法（\\u 后须 4 位十六进制）");
              }
              i += 4;
              const cp = parseInt(hex, 16);
              if (cp >= 0xd800 && cp <= 0xdfff) {
                throwCode("E-LEX-005", "sml: Unicode 转义非法（代理区码点）");
              }
              s += String.fromCodePoint(cp);
            }
          } else {
            // W16：接受集收窄到 Rust 的严格集（`\n \t \r \0 \" \\ \uXXXX`）。
            // 原先未知转义**原样保留**（`\q` 静默变成 `q`）—— 属静默改数据。
            const m = { n: "\n", t: "\t", r: "\r", "0": "\0", '"': '"', "\\": "\\" }[e];
            if (m === undefined) {
              throwCode("E-LEX-004", "sml: 字符串含未知转义符 \\" + e);
            }
            s += m;
            i++;
          }
        } else { s += cc; i++; }
      }
      // W16：未闭合字符串必须报码。原先静默把文件剩余部分吃进字符串 ——
      // 后面的键会凭空消失（数据被悄悄截断）。
      if (!closed) {
        throwCode("E-LEX-001", "sml: 字符串未闭合（遇到文件结尾，缺少结束引号）");
      }
      toks.push({ t: "str", v: s, pos: qStart });
    } else if ("{}[]:,".includes(c)) {
      // ⚠️ 括号 `(` `)` **不是**分隔符，必须与 Rust 侧（sml-lex 的 Tok 只有
      // { } [ ] , : @）一致：裸词值 `备注: (重要)` / `公式: f(x)` 里的括号是
      // 普通字符。此前把它们列进分隔符，会把 `(重要)` 切成独立 token，
      // 导致值静默损坏成 null 并产生 `(`/`)` 垃圾键 —— 与 Rust 解析结果相反。
      // enum(...) 的兼容由 parseFieldSpec 在解析层重组（见 parseFieldSpec）。
      flush();
      toks.push({ t: c, v: c, pos: i });
      i++;
    } else if (c === "?") {
      flush();
      toks.push({ t: "?", v: "?", pos: i });
      i++;
    } else if (c === "@") {
      if (buf === "") { toks.push({ t: "@", v: "@", pos: i }); }
      else { buf += c; }
      i++;
    } else if (" \t\n\r".includes(c)) {
      flush();
      i++;
    } else {
      if (buf === "") bufStart = i;
      buf += c; i++;
    }
  }
  flush();
  return toks;
}

/// 把字符偏移换算为 { line, col }（均从 0 起），供编辑器定位诊断。
export function offsetToPosition(text, offset) {
  const clamped = Math.max(0, Math.min(offset, text.length));
  const before = text.slice(0, clamped);
  const line = (before.match(/\n/g) || []).length;
  const lastNl = before.lastIndexOf("\n");
  return { line, col: clamped - (lastNl + 1) };
}

// ---------------------------------------------------------------------------
// 值转换
// ---------------------------------------------------------------------------

function envLookup(name) {
  if (typeof process !== "undefined" && process.env && name in process.env) {
    return process.env[name];
  }
  if (typeof globalThis !== "undefined" && globalThis.__SML_ENV__ && name in globalThis.__SML_ENV__) {
    return globalThis.__SML_ENV__[name];
  }
  return "";
}

// 数字字面量形态（十进制 / 定点 / 科学计数，含正负号）。
// 刻意不接受 0x / 0o / 0b 等进制前缀与 `1_000` 分隔符 —— 它们一律按字符串处理。
const NUMERIC_LITERAL = /^[+-]?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?$/;
// 纯整数（无小数点、无指数）
const INT_LITERAL = /^[+-]?\d+$/;
// 带前导零的纯整数（0 之后还有数字），如 007 / 0755 / -007
const LEADING_ZERO_INT = /^[+-]?0\d+$/;

/**
 * 抛带**错误码**的错误（模块级版本；`parse()` 内部的对应物是 `fail`）。
 * 码见 errors/codes.sml —— 那是唯一事实来源。
 * @param {string} code 形如 "E-CONTRACT-013"
 * @param {string} msg  人读文案（各端可不同）
 */
function throwCode(code, msg) {
  const e = new Error(msg);
  e.code = code;
  throw e;
}

function coerceWord(w, fragments, nsMap) {
  if (w === "true") return true;
  if (w === "false") return false;
  if (w === "null") return null;
  const ev = w.match(/^\$env\.(.+)$/);
  if (ev) return envLookup(ev[1]);
  // 命名空间解引用：ns.field(.sub) 取值（如 include "ui" as ui 后 ui.title）
  if (nsMap && w.includes(".")) {
    const dot = w.indexOf(".");
    const head = w.slice(0, dot);
    if (Object.prototype.hasOwnProperty.call(nsMap, head)) {
      let cur = nsMap[head];
      for (const k of w.slice(dot + 1).split(".")) {
        if (cur != null && typeof cur === "object" && !Array.isArray(cur)) cur = cur[k];
        else return w;
      }
      if (cur !== undefined) return cur;
    }
  }
  if (w.startsWith("&")) {
    const name = w.slice(1);
    if (fragments.has(name)) return structuredClone(fragments.get(name));
    return w;
  }
  // 只有首字符为数字或小数点的词才承认是数字字面量（剥离正负号后判断）。
  //
  // Rust 的 f64 解析器接受 `inf` / `infinity` / `nan` 且大小写不敏感，
  // 若不设此闸，`status: inf`、`ratio: nan` 这类裸词会被静默解析成 NaN ——
  // 与 YAML 1.1 把 `NO` 识别成 false 是同一类问题（挪威问题）。
  const digits = w.replace(/^[+-]/, "");
  const head = digits.charAt(0);
  const numericHead = head === "." || (head >= "0" && head <= "9");
  if (numericHead && NUMERIC_LITERAL.test(w)) {
    // 前导零保留为字符串：mode: 0755 不应变成 755
    if (LEADING_ZERO_INT.test(w)) return w;
    // 超出安全整数范围的纯整数保留为字符串。
    //
    // 这里用 2^53 而非 Rust 的 i64 上界：JS 只有 Number（IEEE 754 双精度），
    // 超出 2^53 的整数无法精确表示，转 Number 即静默损坏（如 18 位身份证号）。
    // 与 Rust 侧「i64 内即精确」的差异源于两种语言数值类型的能力上限，
    // 差异方向是更保守 —— 宁可保为字符串，也不损坏。
    if (INT_LITERAL.test(w) && !Number.isSafeInteger(Number(w))) return w;
    return Number(w);
  }
  return w;
}

function coerceStr(s, fragments) {
  const ev = s.match(/^\$env\.(.+)$/);
  if (ev) return envLookup(ev[1]);
  return s;
}

function isNum(v) { return typeof v === "number" && !Number.isNaN(v); }
function isInt(v) { return typeof v === "number" && Number.isInteger(v); }

// ---------------------------------------------------------------------------
// 契约系统
// ---------------------------------------------------------------------------
// 契约字段规格:
//   { type, required, def, min, max, enumVals, arrInner }
// type ∈ "str"|"int"|"num"|"bool"|"any"|"enum"|"array"|"contract"

function typeName(sp) {
  if (sp.type === "enum") return "enum(" + sp.enumVals.join("|") + ")";
  if (sp.type === "array") return "array[" + (sp.arrInner ? typeName(sp.arrInner) : "?") + "]";
  if (sp.type === "contract") return sp.refName;
  if (sp.type === "pattern") return sp.refName;
  if (sp.type === "ext") return sp.refName;
  return sp.type;
}

function valueMatchesType(v, sp) {
  switch (sp.type) {
    case "any": return true;
    case "str": return typeof v === "string";
    case "bool": return typeof v === "boolean";
    case "int": return isInt(v);
    case "num": return isNum(v);
    case "enum": return typeof v === "string" && sp.enumVals.includes(v);
    case "array": {
      if (!Array.isArray(v)) return false;
      if (!sp.arrInner) return true;
      return v.every((e) => valueMatchesType(e, sp.arrInner));
    }
    case "contract": return true; // 组合契约在应用阶段递归校验
    case "pattern": return typeof v === "string"; // 格式校验在应用阶段（含引号提示）
    default: return true;
  }
}

function applyDefaults(contract, obj) {
  for (const [k, sp] of Object.entries(contract.fields)) {
    // 契约字段名来自文档，同样不可为危险键（纵深防御；
    // `in` 判定已挡住 __proto__ 之类，这里兜住 prototype 这类非原型链键）
    if (DANGEROUS_KEYS.has(k)) continue;
    if (!(k in obj) && sp.def !== undefined) obj[k] = sp.def;
  }
}

function checkContract(contracts, contract, obj, path) {
  const errs = [];
  for (const [k, sp] of Object.entries(contract.fields)) {
    const full = path ? `${path}.${k}` : k;
    if (!(k in obj)) {
      if (sp.required && sp.def === undefined) errs.push({ code: "E-CONTRACT-003", msg: `契约字段缺失: ${full}` });
      continue;
    }
    const v = obj[k];
    // 自定义模式类型：非字符串必须显式报错并提示加引号——
    // 像 `证件号: 221099 1988 0987 1211` 这种裸写会被词法切成数字，
    // 只剩末段还不报错，是本项目明确要消灭的「静默数据损坏」。
    if (sp.type === "pattern") {
      if (typeof v !== "string") {
        errs.push({ code: "E-CONTRACT-009", msg: `字段 ${full} 类型 ${sp.refName} 要求字符串（号码 / 编号 / 身份证请用引号包裹），实得 ${Array.isArray(v) ? "array" : typeof v}` });
        continue;
      }
      // 防御：JS RegExp 有回溯，超长输入直接拒绝而非硬算
      if (v.length > PATTERN_MAX_LEN) {
        errs.push({ code: "E-CONTRACT-009", msg: `字段 ${full} 的值过长（${v.length} > ${PATTERN_MAX_LEN}），拒绝校验` });
        continue;
      }
      let ok = false;
      try { ok = sp.patternRe.test(v); } catch { ok = false; }
      if (!ok) errs.push({ code: "E-CONTRACT-009", msg: `字段 ${full} 的值 \`${v}\` 不符合类型 ${sp.refName} 的格式要求` });
      continue;
    }
    if (sp.type === "ext") {
      // 外置类型：由下游的判定函数回答合不合法。
      // 约定返回值：true 通过 / false 失败 / 字符串 = 失败原因；抛错也按失败处理。
      let r;
      try { r = sp.extCheck(v); } catch (e) { r = String((e && e.message) || e); }
      if (r === false) errs.push({ code: "E-CONTRACT-007", msg: `字段 ${full} 不符合扩展类型 ${sp.refName}` });
      else if (typeof r === "string") errs.push({ code: "E-CONTRACT-007", msg: `字段 ${full} 不符合扩展类型 ${sp.refName}：${r}` });
      continue;
    }
    if (sp.type === "contract") {
      const sub = contracts[sp.refName];
      if (!sub) { errs.push({ code: "E-CONTRACT-001", msg: `契约 ${sp.refName} 未定义（字段 ${full}）` }); continue; }
      if (v && typeof v === "object" && !Array.isArray(v)) {
        const subErr = checkContract(contracts, sub, v, full);
        if (subErr) errs.push(...subErr);
      } else {
        errs.push({ code: "E-CONTRACT-008", msg: `字段 ${full} 应为主对象（组合契约 ${sp.refName}）` });
      }
      continue;
    }
    if (!valueMatchesType(v, sp)) {
      errs.push({ code: "E-CONTRACT-002", msg: `字段 ${full} 类型错误：期望 ${typeName(sp)}，实得 ${Array.isArray(v) ? "array" : typeof v}` });
      continue;
    }
    if (sp.min !== undefined || sp.max !== undefined) {
      let lo = sp.min, hi = sp.max;
      if (lo !== undefined && v < lo) errs.push({ code: "E-CONTRACT-005", msg: `字段 ${full} 小于最小值 ${lo}` });
      if (hi !== undefined && v > hi) errs.push({ code: "E-CONTRACT-005", msg: `字段 ${full} 大于最大值 ${hi}` });
    }
  }
  if (!contract.loose) {
    for (const k of Object.keys(obj)) {
      if (k === "__type" || k === "__name") continue;
      if (!(k in contract.fields)) {
        errs.push({ code: "E-CONTRACT-004", msg: `契约未声明字段：${path ? path + "." + k : k}` });
      }
    }
  }
  return errs.length ? errs : null;
}

// ---------------------------------------------------------------------------
// include / feature 解析辅助
// ---------------------------------------------------------------------------

const DEFAULT_FEATURES = new Set([
  "include", "namespace", "implicit-ns", "contract", "env", "escape",
  "fragment", "top-array", "bareword-str",
]);

// 从文本里扫出 @feature enable/disable 声明，返回生效的 feature Set
function collectFeatures(text, base) {
  const feats = new Set(base || DEFAULT_FEATURES);
  const re = /@feature\s+(enable|disable)\s+([^\n@]+)/g;
  let m;
  while ((m = re.exec(text)) !== null) {
    const mode = m[1];
    const words = m[2].trim().split(/\s+/).filter(Boolean);
    for (const w of words) {
      if (mode === "enable") feats.add(w);
      else feats.delete(w);
    }
  }
  return feats;
}

// 把 `@feature ...` **整行**从正文里剥掉（行级，与 Rust 的 `strip_features` 同口径）。
//
// ⚠️ 为什么必须在**词法之前**做：`tokenize` 会丢掉换行，解析器再也分不清「这条指令到哪
// 结束」。原先解析器那版靠「遇到 @ / } / ] / , / ; 就停」猜边界 —— 而特性名后面通常直接
// 跟着下一个块（`@feature enable for` + `svg { … }`），于是它把 `svg { …` 一起吞掉，
// **整份文档静默变成 `{}`**。实测 `_probe2.sml`：旧实现得 `{}`，Rust 得完整树（W16 修）。
function stripFeatureLines(text) {
  return String(text).replace(/^[ \t]*@feature[^\n]*\n?/gm, "");
}

// —— 模式 / 正则的三处上限，集中定义 ——
// 此前 `PATTERN_MAX_LEN` 定义在 parse() 内部而校验处却写死 4096，常量成了死代码；
// 现在统一提到模块级，两处都能引用。
const PATTERN_MAX_LEN = 4096; // 被校验值的长度上限：超长输入直接拒收，不交给 RegExp 硬算
const REGEX_SRC_MAX = 200;    // 用户内联正则的源长度上限
const QUANT_MAX = 1000;       // 量词上界：挡住 `次: 999999999` 这类展开
// 块/数组嵌套上限：与 Rust 侧 MAX_VALUE_DEPTH 同口径。
// 深嵌套在 JS 侧会抛 RangeError（严重时栈溢出），故在入口统一闸住。
const MAX_PARSE_DEPTH = 128;

// 原型污染防护：这三个键一旦被当普通键写入，就会改写 Object.prototype 本身。
const DANGEROUS_KEYS = new Set(["__proto__", "constructor", "prototype"]);

// 把 "a.b.c" 这样的点分路径在 obj 上建成嵌套块，返回最内层对象
function ensureNsPath(obj, path) {
  let cur = obj;
  for (const part of path.split(".").filter(Boolean)) {
    // 命名空间段含危险键时**直接报错**：否则 `include "x.sml" as __proto__.p`
    // 会顺着原型链把被包含文件的字段合并进全局 Object.prototype。
    if (DANGEROUS_KEYS.has(part)) {
      throw throwCode("E-PARSE-009", "sml: 命名空间段不可使用 `" + part + "`");
    }
    if (cur[part] === undefined || typeof cur[part] !== "object" || Array.isArray(cur[part])) {
      cur[part] = {};
    }
    cur = cur[part];
  }
  return cur;
}

// 把 src 合并进 target（对象合并；同键：src 覆盖，若都为对象则深合并）
function mergeInto(target, src) {
  for (const k of Object.keys(src)) {
    if (k === "__type" || k === "__name") continue;
    // 同上：JSON.parse 之来源可能产生 own 的 `__proto__` 键，合并前必须拦掉
    if (DANGEROUS_KEYS.has(k)) continue;
    if (src[k] && typeof src[k] === "object" && !Array.isArray(src[k]) &&
        target[k] && typeof target[k] === "object" && !Array.isArray(target[k])) {
      mergeInto(target[k], src[k]);
    } else {
      target[k] = src[k];
    }
  }
}

// 解析 include 目标列表文本（如 `"a.sml" as x, "b" import y` 或 `import a.b.c, d`）
// 部分引用（挑键）两种写法：
//   ① "x.sml" as w { a, b }       —— 路径在前，{ keys } 在后
//   ② { a, b } as w in "x.sml"     —— 键列表在前，in "file" 指定目标
// 省略 as 时挑出的键平铺；as ns 时挂到命名空间。
function parseIncludeTargets(line, feats) {
  // 拆逗号（仅在引号外，且不在 {} 括号内——部分引用的键列表逗号不应拆分目标）
  const parts = [];
  let buf = "", inStr = false, depth = 0;
  for (const ch of line) {
    if (ch === '"') { inStr = !inStr; buf += ch; }
    else if (ch === "{" && !inStr) { depth++; buf += ch; }
    else if (ch === "}" && !inStr) { depth = Math.max(0, depth - 1); buf += ch; }
    else if (ch === "," && !inStr && depth === 0) { parts.push(buf.trim()); buf = ""; }
    else buf += ch;
  }
  if (buf.trim()) parts.push(buf.trim());

  const targets = [];
  for (let raw of parts) {
    let ns = null;
    let viaImport = false;
    let keys = null;

    // 提取 `in "file"` 或 `in file`：作为目标路径（语法②）
    const inM = raw.match(/\bin\s+"?([^"\s]+)"?/);
    let inPath = null;
    if (inM) {
      inPath = inM[1];
      raw = raw.slice(0, inM.index) + raw.slice(inM.index + inM[0].length);
    }

    // 提取 `{ k1, k2, ... }` 部分引用键列表（语法① / ② 都可能出现）
    const braceM = raw.match(/\{\s*([^}]*)\s*\}/);
    if (braceM) {
      keys = braceM[1].split(",").map((s) => s.trim().replace(/^"|"$/g, "")).filter(Boolean);
      // 本函数在 parse() 作用域之外，拿不到其中的 fail()（那是个 const 闭包，
      // 直接调用会抛宿主 ReferenceError: fail is not defined）。必须用模块级的
      // throwCode()，才能把稳定的 e.code 交给调用方。
      if (keys.length === 0) throw throwCode("E-INCLUDE-005", "sml: 键列表不能为空（至少指定一个键）");
      raw = raw.slice(0, braceM.index) + raw.slice(braceM.index + braceM[0].length);
    }

    // as ns
    const asM = raw.match(/\bas\s+([A-Za-z0-9_.\-]+)/);
    if (asM) { ns = asM[1]; raw = raw.slice(0, asM.index) + raw.slice(asM.index + asM[0].length); }
    if (/\bimport\b/.test(raw)) viaImport = true;

    // 取路径：in "file" 优先，否则引号内，否则裸词（import 形式）
    let path = inPath;
    if (!path) {
      const qM = raw.match(/"([^"]+)"/);
      const wM = raw.match(/([A-Za-z0-9_.\-]+)/);
      path = qM ? qM[1] : (wM ? wM[1] : null);
    }
    if (!path) continue;

    // 部分引用不触发 implicit-ns 自动命名空间（否则挑出的键会被塞进文件名命名空间）
    if (keys && !ns) {
      // 平铺到当前作用域，ns 保持 null
    } else if (feats.has("implicit-ns") && !path.includes(".") && !ns) {
      ns = path;
    }
    // 补 .sml（点分模块名 implicit-ns 已由 ns 处理，这里仅补裸扩展名）
    if (!path.includes(".") && !path.includes("/")) path += ".sml";
    targets.push({ path, ns, viaImport, keys });
  }
  return targets;
}

// ---------------------------------------------------------------------------
// 解析（递归下降）
// ---------------------------------------------------------------------------

export function parse(text, opts) {
  opts = opts || {};
  const files = opts.files || {};
  const baseFeatures = opts.features || null;
  const nsPrefix = opts.nsPrefix || "";
  // —— 外置扩展（与 Rust 侧 `sml::ext` / `sml::contract_ext` 对齐）——
  //   opts.directives: { "form": { positional?: bool, call(arg, body) -> {discard:true} | {emit: 对象} } }
  //   opts.types:      { "image": (v) => true | false | "错误信息" }
  //   opts.warnings:   []   传入数组时，弃用提示会 push 进去（与 Rust 的 Diagnostic 对应）
  // 不带这些选项时，行为与未扩展时**完全一致**。
  const extDirs = opts.directives || null;
  const extTypes = opts.types || null;

  // `@feature` 整行在**词法之前**剥掉（见 stripFeatureLines 的说明）——
  // 特性集仍由下面的 collectFeatures 从**原文**读，两者互不影响。
  const toks = tokenize(stripFeatureLines(text));
  // 嵌套深度闸门：在入口对 token 流做一次 O(n) 线性扫描。
  // 比在每个递归点插桩更简单，也更难绕过（词法已定，括号/方括号即成对出现）。
  {
    let depth = 0;
    for (const tk of toks) {
      if (tk.t === "{" || tk.t === "[") {
        depth++;
        if (depth > MAX_PARSE_DEPTH) {
          throw throwCode("E-LIMIT-001", "sml: 嵌套过深（超过 " + MAX_PARSE_DEPTH + " 层），疑似递归或恶意输入");
        }
      } else if (tk.t === "}" || tk.t === "]") {
        if (depth > 0) depth--;
      }
    }
  }
  const fragments = new Map();
  const contracts = {};
  // @type 自定义类型：名 -> 模式数据（Loom-in-SML：规则即 SML 数据）
  const types = new Map();
  const nsMap = {};
  let i = 0;
  const peek = () => toks[i];
  // 抛**带错误码**的错误：码是稳定契约（见 errors/codes.sml），文案不是 ——
  // 各端措辞可以不同，码相同就是同一件事。
  const fail = (code, msg, pos) => {
    const t = toks[i];
    const p = (typeof pos === "number") ? pos : (t && typeof t.pos === "number" ? t.pos : (toks[toks.length - 1]?.pos ?? 0));
    const e = new Error(msg);
    e.code = code;
    e.pos = p;
    throw e;
  };

  const feats = collectFeatures(text, baseFeatures);

  function literal() {
    const t = peek();
    if (!t) fail("E-PARSE-021", "sml: 期望字面量");
    if (t.t === "str") { i++; return coerceStr(t.v, fragments); }
    if (t.t === "word") { i++; return coerceWord(t.v, fragments, nsMap); }
    fail("E-PARSE-021", "sml: 期望字面量, 得 " + t.t);
  }

  // —— 模式语言（Loom-in-SML）：编译为 JS RegExp ——
  //
  // 与 Rust 侧的差异必须知道：JS RegExp 存在回溯，不是 NFA 引擎。
  // 缓解措施：
  //   1) 不支持 直到/until（避免懒惰量词的高危结构，明确报错而非静默）
  //   2) 校验值长度上限 4096
  //   3) 结构化模式生成的量词大多有界，风险可控
  //   4) regex 逃生舱内嵌用户正则时同样受上述约束（长度上限 REGEX_SRC_MAX）
  //   5) 量词上界 QUANT_MAX
  // 三个常量定义在模块级（见文件上方），此处不再重复声明。
  function escapeRe(s) {
    return String(s).replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  }
  function classToRe(c) {
    // Unicode 语义，与 Rust 侧对齐：数字含全角、字母含汉字
    const table = {
      "数字": "\\p{Nd}", digit: "\\p{Nd}",
      "字母": "\\p{L}", alpha: "\\p{L}",
      "空白": "\\s", space: "\\s",
      "字": "[\\p{L}\\p{Nd}_]", word: "[\\p{L}\\p{Nd}_]",
      "任意": "[\\s\\S]", any: "[\\s\\S]",
    };
    if (table[c]) return table[c];
    if ([...c].length === 1) return escapeRe(c);
    throw throwCode("E-CONTRACT-013", "sml: 未知字符类 `" + c + "`");
  }
  // 量词描述 → [min, max]（max===undefined 表示无上界）。
  // 支持：数字 / "+" / "*" / "?" / "a-b" 字符串。
  function quantToMinMax(t) {
    if (typeof t === "number") return [t, t];
    if (t === "+") return [1, undefined];
    if (t === "*") return [0, undefined];
    if (t === "?") return [0, 1];
    if (/^\d+$/.test(t)) return [Number(t), Number(t)];
    const m = /^(\d+)-(\d+)$/.exec(t);
    if (m) return [Number(m[1]), Number(m[2])];
    throw throwCode("E-CONTRACT-013", "sml: 未知量词 `" + t + "`");
  }
  // [min, max] → 正则量词片段（无上界写作 {min,}）。
  function minMaxToRe(min, max) {
    if (max === undefined) return "{" + min + ",}";
    return "{" + min + "," + max + "}";
  }
  function compilePatternToRe(v, stack) {
    stack = stack || [];
    const build = (node) => {
      if (Array.isArray(node)) return node.map(build).join("");
      if (typeof node === "string") return escapeRe(node);
      if (node == null || typeof node !== "object")
        throw throwCode("E-CONTRACT-013", "sml: 模式元素类型不支持");
      const g = (...keys) => {
        for (const k of keys) if (node[k] !== undefined) return node[k];
        return undefined;
      };
      if (g("直到", "until") !== undefined)
        throw throwCode("E-CONTRACT-013", "sml: JS 引擎暂不支持 直到/until（懒惰量词在高危结构下有回溯风险；该场景请用 Rust 引擎）");
      let base;
      if (g("字面", "lit", "literal") !== undefined)
        base = escapeRe(String(g("字面", "lit", "literal")));
      else if (g("类", "class") !== undefined)
        base = classToRe(String(g("类", "class")));
      else if (g("任一", "alt", "any-of") !== undefined)
        base = "(?:" + g("任一", "alt", "any-of").map((x) => build(x)).join("|") + ")";
      else if (g("组", "group") !== undefined)
        base = "(?:" + build(g("组", "group")) + ")";
      else if (g("序列", "seq") !== undefined)
        base = build(g("序列", "seq"));
      else if (g("用", "use") !== undefined) {
        const name = String(g("用", "use"));
        if (stack.includes(name)) throw throwCode("E-CONTRACT-014", "sml: 规则 `" + name + "` 循环引用");
        if (!types.has(name)) throw throwCode("E-CONTRACT-014", "sml: 未定义的规则 `" + name + "`");
        stack.push(name);
        const p = build(types.get(name));
        stack.pop();
        return p; // 引用处不重复应用量词/命名
      }
      else if (g("正则", "regex", "re") !== undefined) {
        // 用户内联正则：JS RegExp 会回溯，`(a+)+$` 这类能指数级挂死主线程。
        // 源长度闸门是最省事也最难绕的第一道防线（与值长上限 PATTERN_MAX_LEN 配合）。
        const reSrc = String(g("正则", "regex", "re"));
        if (reSrc.length > REGEX_SRC_MAX) {
          throw throwCode("E-LIMIT-007", "sml: 内联正则过长（" + reSrc.length + " > " + REGEX_SRC_MAX + "），拒绝编译");
        }
        base = "(?:" + reSrc.replace(/^\^/, "").replace(/\$$/, "") + ")";
      }
      else throw throwCode("E-CONTRACT-013", "sml: 无法识别的模式元素");
      // —— 量词：支持 次: 数字/符号/a-b、次: {最小,最大}（中英等价）、
      //     或直接 最小/最大 平铺在元素上（机翻等价的直觉写法）——
      let qMin = undefined, qMax = undefined;
      const times = g("次", "times", "repeat");
      if (times !== undefined) {
        if (typeof times === "object" && times !== null) {
          // 对象形式：{ 最小: n, 最大: m }（键名走 i18n，中英等价）
          const lo = times["最小"] ?? times["min"];
          const hi = times["最大"] ?? times["max"];
          if (lo !== undefined) qMin = Number(lo);
          if (hi !== undefined) qMax = Number(hi);
        } else {
          const [mn, mx] = quantToMinMax(times);
          qMin = mn; qMax = mx;
        }
      }
      // 平铺形式：直接把 最小/最大 写在模式元素上
      const flatMin = g("最小", "min");
      const flatMax = g("最大", "max");
      if (flatMin !== undefined) qMin = Number(flatMin);
      if (flatMax !== undefined) qMax = Number(flatMax);
      // 可选：等价 {0,1}，仅当未显式给出量词时生效
      const isOptional = node["可选"] === true || node["optional"] === true;
      if (isOptional && qMin === undefined && qMax === undefined) { qMin = 0; qMax = 1; }
      // 校验并应用：非法输入一律抛出明确错误，绝不静默忽略
      if (qMin !== undefined || qMax !== undefined) {
        if ((qMin !== undefined && !Number.isInteger(qMin)) ||
            (qMax !== undefined && !Number.isInteger(qMax)))
          throw throwCode("E-CONTRACT-015", "sml: 量词最小/最大必须为整数");
        if (qMin !== undefined && qMin < 0) throw throwCode("E-CONTRACT-015", "sml: 量词最小不能为负（得 " + qMin + "）");
        // 上界：`次: 999999999` 会被展开成十亿条指令，编译期先于步数预算就把内存吃光
        if (qMin !== undefined && qMin > QUANT_MAX)
          throw throwCode("E-LIMIT-009", "sml: 量词最小过大（" + qMin + " > " + QUANT_MAX + "）");
        if (qMax !== undefined && qMax > QUANT_MAX)
          throw throwCode("E-LIMIT-009", "sml: 量词最大过大（" + qMax + " > " + QUANT_MAX + "）");
        if (qMax !== undefined && qMin !== undefined && qMax < qMin)
          throw throwCode("E-CONTRACT-015", "sml: 量词最大(" + qMax + ")不能小于最小(" + qMin + ")");
        const mn = qMin === undefined ? 0 : qMin;
        base = "(?:" + base + ")" + minMaxToRe(mn, qMax);
      }
      return base;
    };
    const body = build(v);
    return new RegExp("^(?:" + body + ")$", "u");
  }

  /// 新建字段规格（默认值集中在一处，避免两条解析路径手写两份而漂移）。
  function newFieldSpec() {
    return { type: "any", required: true, def: undefined, min: undefined, max: undefined, enumVals: null, arrInner: null, refName: null };
  }

  /// 解析字段修饰符（`?` / `optional` / `required` / `default v` / `min n` / `max n`）。
  ///
  /// 抽成函数是因为 `[T]` 简写与「类型名开头」两条路径都要用同一套规则。
  function parseFieldModifiers(sp) {
    let defaultSet = false;
    while (true) {
      const m = peek();
      if (!m) break;
      if (m.t === "?") { sp.required = false; i++; continue; }
      if (m.t === "word") {
        if (m.v === "optional") { sp.required = false; i++; continue; }
        if (m.v === "required") { sp.required = true; i++; continue; }
        if (m.v === "default") { i++; sp.def = literal(); defaultSet = true; continue; }
        if (m.v === "min") { i++; sp.min = Number(literal()); continue; }
        if (m.v === "max") { i++; sp.max = Number(literal()); continue; }
      }
      break;
    }
    if (defaultSet) sp.required = false;
    return sp;
  }

  function parseFieldSpec() {
    const t = peek();
    // `[T]` 数组类型简写 —— Rust 侧与 README / 教程一直用的写法。
    //
    // 此前 JS 只认 `array [T]` 关键字形式，于是文档里照抄的 `tags: [str] optional`
    // 会在这里抛「字段类型期望标识符」：**对合法 SML 的假报错**，
    // 而 README 的契约示例恰好就是 `[str]`。VSCode 扩展直接复用了本解析器，
    // 因此这一处假报错会一路显示到编辑器里。
    if (t && t.t === "[") {
      i++;
      let inner = null;
      if (peek() && peek().t !== "]") inner = parseFieldSpec();
      if (peek() && peek().t === "]") i++;
      else fail("E-PARSE-001", "sml: 数组类型 `[T]` 缺少 `]`");
      const sp = newFieldSpec();
      sp.type = "array";
      sp.arrInner = inner;
      return parseFieldModifiers(sp);
    }
    if (!t || t.t !== "word") fail("E-PARSE-006", "sml: 字段类型期望标识符");
    const typeWord = t.v;
    i++;
    let sp = newFieldSpec();
    if (typeWord === "str") sp.type = "str";
    else if (typeWord === "int") sp.type = "int";
    else if (typeWord === "num") sp.type = "num";
    else if (typeWord === "bool") sp.type = "bool";
    else if (typeWord === "any") sp.type = "any";
    else if (typeWord.startsWith("enum(")) {
      // `enum(公开, 内部, 机密)` 的兼容重组。
      //
      // 括号不再是分隔符后（与 Rust 词法对齐），这类写法会被 `,` 切成
      // Word("enum(公开") , Word("内部") , Word("机密)") —— 此处按
      // 「起于 enum( 、止于以 ) 结尾的词」重组回枚举成员列表。
      // 官网与教程大量使用 `enum(a, b, c)`，而 Rust 侧只认 `enum [ ... ]`，
      // 故这一层兼容是两端行为统一的必要代价。
      sp.type = "enum";
      sp.enumVals = [];
      let rest = typeWord.slice(5); // 去掉前缀 "enum("
      for (;;) {
        if (rest.endsWith(")")) {
          const last = rest.slice(0, -1);
          if (last !== "") sp.enumVals.push(last);
          break;
        }
        sp.enumVals.push(rest);
        if (peek() && peek().t === ",") i++;
        const nx = peek();
        if (!nx || (nx.t !== "word" && nx.t !== "str")) break;
        rest = nx.v;
        i++;
      }
    } else if (typeWord === "enum") {
      sp.type = "enum";
      sp.enumVals = [];
      // 三种写法都接受，但以 `enum [ ... ]` 为准 —— 它是官网文档与 Rust 实现
      // 使用的形式。此前本实现只认 `enum(...)` / `enum a b c`，导致
      // 「按官网文档写的契约在 Playground 上报错」，与 Rust 侧完全相反。
      if (peek() && peek().t === "(") {
        i++;
        while (peek() && peek().t !== ")") {
          if (peek().t === "word" || peek().t === "str") { sp.enumVals.push(peek().v); i++; }
          else if (peek().t === ",") i++;
          else break;
        }
        if (peek() && peek().t === ")") i++;
      } else if (peek() && peek().t === "[") {
        i++;
        while (peek() && peek().t !== "]") {
          if (peek().t === "word" || peek().t === "str") { sp.enumVals.push(peek().v); i++; }
          else if (peek().t === ",") i++;
          else break;
        }
        if (peek() && peek().t === "]") i++;
      } else {
        while (peek() && (peek().t === "word" || peek().t === "str")) { sp.enumVals.push(peek().v); i++; }
      }
    } else if (typeWord === "array") {
      sp.type = "array";
      if (peek() && peek().t === "[") {
        i++;
        if (peek() && peek().t !== "]") {
          const inner = parseFieldSpec();
          sp.arrInner = inner;
        }
        if (peek() && peek().t === "]") i++;
      }
    } else if (types.has(typeWord)) {
      // @type 声明的自定义类型：解析期即编译为 RegExp（缓存），校验期直接用
      sp.type = "pattern";
      sp.refName = typeWord;
      sp.patValue = types.get(typeWord);
      sp.patternRe = compilePatternToRe(sp.patValue);
    } else if (extTypes && extTypes[typeWord]) {
      // 外置类型（下游注册，如 image / link / time）：
      // 名字与规则都留在下游，本文件不需要知道「image 是什么」。
      // 判定次序与 Rust 侧一致：@type > 外置 > 契约引用。
      sp.type = "ext";
      sp.refName = typeWord;
      sp.extCheck = extTypes[typeWord];
    } else {
      sp.type = "contract";
      sp.refName = typeWord;
    }
    return parseFieldModifiers(sp);
  }

  function parseContractBody() {
    const fields = {};
    if (peek() && peek().t === "{") i++; else fail("E-PARSE-019", "sml: @contract 后须契约体 { }");
    while (peek() && peek().t !== "}") {
      if (peek().t === "," || peek().t === ";") { i++; continue; }
      if (peek().t !== "word") fail("E-PARSE-006", "sml: 契约字段期望名称, 得 " + peek().t);
      const fkey = peek().v; i++;
      if (peek() && peek().t === ":") i++;
      const sp = parseFieldSpec();
      fields[fkey] = sp;
      if (peek() && (peek().t === "," || peek().t === ";")) i++;
    }
    if (peek() && peek().t === "}") i++;
    return fields;
  }

  // 解析 include：返回若干 { text, ns } 目标并递归 parse
  function resolveIncludes(line) {
    if (!feats.has("include")) fail("E-FEATURE-001", "sml: include 未启用（需要 feature 'include'）");
    const targets = parseIncludeTargets(line, feats);
    const results = [];
    for (const tg of targets) {
      let text = files[tg.path];
      if (text === undefined) {
        // 也允许直接用 key（不带扩展名）
        text = files[tg.path.replace(/\.sml$/, "")];
      }
      if (text === undefined) fail("E-INCLUDE-001", "sml: include 目标未找到: " + tg.path);
      const childPrefix = tg.ns ? nsPrefix + tg.ns + "." : nsPrefix;
      // 外置扩展随 include 递归传递：被包含的文件里同样可以用方言指令与外置类型
      const v = parse(text, {
        files,
        features: feats,
        nsPrefix: childPrefix,
        directives: extDirs || undefined,
        types: extTypes || undefined,
        warnings: opts.warnings,
      });
      // 部分引用：仅保留指定顶层键（命名空间挂在 ns 下时同样只挑这些）
      let filtered = v;
      if (tg.keys && Array.isArray(tg.keys)) {
        filtered = {};
        for (const k of tg.keys) {
          // 键名来自 include 行文本（用户可控），同样要挡危险键
          if (DANGEROUS_KEYS.has(k)) continue;
          if (k in v) filtered[k] = v[k];
        }
      }
      results.push({ value: filtered, ns: tg.ns });
    }
    return results;
  }

  function parseBlock(closing) {
    const node = {};
    const setField = (k, v) => {
      // 原型污染防护：文档里的**键名**同样不可为危险键。
      // 只堵 include 的命名空间路径不够 —— `__proto__: x` 直接写在文档里，
      // 赋值时会顺着原型链改写该对象的原型。
      if (DANGEROUS_KEYS.has(k)) fail("E-PARSE-010", "sml: 键名不可使用 `" + k + "`");
      if (node[k] === undefined) node[k] = v;
      else if (Array.isArray(node[k])) node[k].push(v);
      else node[k] = [node[k], v];
    };
    let appliedContract = null;
    while (i < toks.length) {
      const tok = peek();
      if (tok.t === "}" || tok.t === "]") {
        if (closing === tok.t) { i++; break; }
        // W16：闭合符**不匹配**必须报错，不再静默结束本块。
        // 原先这里直接 `break`（且**不消费**该 token）：`a { ] }` 静默得到 `{"a":{}}`，
        // `]` 被吞掉、数据形状被悄悄改掉。码按 Rust 分两种：
        //   - 块内遇到 `]` ⇒ E-PARSE-002（闭合符错配）；
        //   - 顶层（closing === null）遇到多余的 `}` / `]` ⇒ E-PARSE-003（多余的结束符号）。
        if (tok.t === "]" && closing === "}") {
          fail("E-PARSE-002", "sml: 闭合符错配（块应以 } 闭合，却遇到 ]）");
        }
        fail("E-PARSE-003", "sml: 多余的结束符号 " + tok.t + "（此处没有需要关闭的容器）");
      }
      if (tok.t === ",") { i++; continue; }
      if (tok.t === "@") {
        i++;
        if (!peek()) fail("E-PARSE-011", "sml: @ 后需名称");
        const fname = peek().v;
        if (fname === "version") {
          i++;
          const lit = (peek() && peek().v) ?? "";
          if (lit !== "v1" && lit !== "1") {
            fail("E-FEATURE-004", "sml: @version 须写作 `@version v1`；`version` 不可作为片段名");
          }
          i++;
          continue;
        }
        if (fname === "feature") {
          // 正常情况下**到不了这里**：`@feature` 整行已在词法前被 stripFeatureLines 剥掉。
          // 这里只兜住病理情况（`@feature` 写在行中间）。⚠️ 别改回「遇到 } / ] 才停」的老写法：
          // tokenize 已丢换行，那样会把后面第一个块的**开头与内容**一起吞掉（整份文档 -> `{}`）。
          i++; // 消费 feature
          while (i < toks.length) {
            const tk = toks[i];
            // 下一个字段（`word :`）或任何容器边界即停 —— 宁可少吃不误吃
            if (tk.t === "word" && toks[i + 1] && toks[i + 1].t === ":") break;
            if (tk.t === "@" || tk.t === "{" || tk.t === "}" || tk.t === "[" || tk.t === "]") break;
            if (tk.t === "," || tk.t === ";") { i++; break; }
            i++;
          }
          continue;
        }
        if (fname === "type") {
          i++; // 消费指令名 type（本分支与其它分支一致：fname 由自己消费）
          // 自定义类型：`@type name: X { 模式 }`。
          // 仅显式 name: 形式是指令；`@type { }` 仍是「名为 type 的片段」
          // （与 Rust 实现的边界一致，回归测试守着该语义）。
          const isDecl = peek() && peek().t === "word" && peek().v === "name"
            && toks[i + 1] && toks[i + 1].t === ":";
          if (isDecl) {
            i += 2; // 消费 name 与 :
            if (!peek() || (peek().t !== "word" && peek().t !== "str"))
              fail("E-PARSE-019", "sml: @type name: 后须类型名");
            const tname = peek().v; i++;
            if (!peek() || peek().t !== "{") fail("E-PARSE-019", "sml: @type " + tname + " 后须 { } 模式体");
            i++;
            const body = parseBlock("}");
            types.set(nsPrefix + tname, body);
            continue;
          }
          // 非指令形式：下落到片段定义逻辑（片段名 = "type"）
        }
        // —— 外置扩展指令（下游注册，无需改动本文件）——
        //
        // 与 Rust 侧 `sml_parse::ext::Directive` 对齐。内置指令名由调用方自行回避
        // （与内置同名会改写核心语义，Rust 侧在注册时就拒绝）。
        //
        // 未注册时**不要**在这里兜底：未知名指令在 JS 侧会落进下面的「片段定义」
        // 分支（而 Rust 侧是报错）—— 这是既有差异，另行收敛，不在扩展点里改。
        if (extDirs && extDirs[fname]) {
          i++; // 消费指令名
          if (peek() && peek().t === ":") i++;
          let arg = null;
          // 显式形式（推荐）：`@xxx name: X { }` —— 与片段参数同形，无歧义
          if (peek() && peek().t === "word" && peek().v === "name"
              && toks[i + 1] && toks[i + 1].t === ":") {
            i += 2;
            if (!peek() || (peek().t !== "word" && peek().t !== "str"))
              fail("E-EXT-003", "sml: 扩展指令 @" + fname + " 的 name: 后须值");
            arg = peek().v; i++;
          } else if (extDirs[fname].positional && peek() && peek().t === "word"
                     && toks[i + 1] && toks[i + 1].t === "{") {
            // 位置参数（v4 起废弃）：仅指令显式开启时接受，并产出弃用警告
            arg = peek().v; i++;
            if (Array.isArray(opts.warnings)) {
              opts.warnings.push({
                kind: "deprecated",
                message: "sml: 扩展指令 `@" + fname + " " + arg
                  + " { .. }` 使用了位置参数形式；自 v4 起推荐显式写作 `@" + fname
                  + " name: " + arg + " { .. }`",
              });
            }
          }
          let body = null;
          if (peek() && peek().t === "{") { i++; body = parseBlock("}"); }
          const out = extDirs[fname].call(arg, body);
          // discard（或不返回）= 元数据块，不进主树；emit = 展开合并进当前块
          if (out && out.emit !== undefined) {
            const em = out.emit;
            if (em && typeof em === "object" && !Array.isArray(em)) Object.assign(node, em);
            else fail("E-EXT-004", "sml: 扩展指令 @" + fname + " 的 emit 须返回对象（用于合并进所在块）");
          }
          continue;
        }
        if (fname === "contract") {
          i++;
          const cname = peek() && peek().v;
          if (!cname) fail("E-PARSE-019", "sml: @contract 后须契约名");
          i++;
          let loose = false;
          if (peek() && peek().t === "word" && peek().v === "loose") { loose = true; i++; }
          else if (peek() && peek().t === "word" && peek().v === "strict") { loose = false; i++; }
          const fields = parseContractBody();
          contracts[nsPrefix + cname] = { fields, loose };
          continue;
        }
        if (fname === "is") {
          i++;
          const raw = peek() && peek().v;
          if (!raw) fail("E-PARSE-019", "sml: @is 后须契约名");
          i++;
          // `@is type(契约名)` 与 `@is 契约名` 等价 —— 括号形式让「类型标注」
          // 的意图更显眼，且与块级标注 `type(契约名) 块名 { .. }` 同形。
          // 括号在词法中不是分隔符，故 `type(办事人)` 整体是一个 word，
          // 字符串层解包即可。仅当契约表里**确实存在**名为 `type(x)` 的契约时
          // 才按原名解析（极端但合法的老文档）—— 向后兼容优先，与 Rust 一致。
          const inner = /^type\((.+)\)$/.exec(raw);
          const nameExists = raw in contracts || nsPrefix + raw in contracts;
          const cname = inner && !nameExists ? inner[1] : raw;
          appliedContract = nsPrefix + cname;
          if (!(appliedContract in contracts)) appliedContract = cname; // 回退裸名
          continue;
        }
        // @name [type [name]] { ... } 片段定义
        i++;
        if (peek() && peek().t === ":") i++;
        let ftype = null, farg = null;
        if (peek() && peek().t === "word") {
          ftype = peek().v; i++;
          if (peek() && peek().t === "word") { farg = peek().v; i++; }
        }
        if (peek() && peek().t === "{") {
          i++;
          const sub = parseBlock("}");
          if (ftype) { sub.__type = ftype; if (farg) sub.__name = farg; }
          fragments.set(nsPrefix + fname, sub);
        }
        continue;
      }
      // include 处理：键为 include / import
      const key = (peek() && peek().v);
      if (key === "include" || key === "import") {
        i++;
        if (peek() && peek().t === ":") i++;
        // 收集 include 参数：引号路径 / as / 命名空间词 / 逗号；遇到 `:`（下个键）
        // 或 @ / } / ] 即停止。tokenize 已去换行，故用这些边界切分语句。
        const importMode = (key === "import");
        let line = "";
        let lastWasAs = false;
        while (peek()) {
          const tk = peek();
          if (tk.t === "str") { line += tk.v + " "; i++; lastWasAs = false; continue; }
          if (tk.t === "word" && tk.v === "as") { line += " as "; i++; lastWasAs = true; continue; }
          if (tk.t === "word" && tk.v === "in") { line += " in "; i++; lastWasAs = false; continue; }
          if (tk.t === ",") { line += ","; i++; lastWasAs = false; continue; }
          if (tk.t === "{") { line += "{"; i++; lastWasAs = false; continue; }
          if (tk.t === "}") { line += "} "; i++; lastWasAs = false; continue; }
          if (tk.t === ":") break;                       // 下一行键开始
          if (tk.t === "@" || tk.t === "}" || tk.t === "]") break;
          if (tk.t === "word" && lastWasAs) { line += tk.v; i++; lastWasAs = false; continue; }
          if (tk.t === "word" && importMode) { line += tk.v; i++; lastWasAs = false; continue; }
          if (tk.t === "word" && !lastWasAs) break;       // 其它裸词（下一行的键）停下
          break;
        }
        const resolved = resolveIncludes(line);
        for (const r of resolved) {
          if (r.ns) {
            const target = ensureNsPath(node, r.ns);
            mergeInto(target, r.value);
            nsMap[r.ns] = r.value;
          } else {
            mergeInto(node, r.value);
          }
        }
        continue;
      }
      // 片段展开：块首字段为 &name 且其后是其它键值（非 `&name: v` 退化形式）时，
      // 将已定义片段的字段合并进当前块，再继续解析后续字段。
      if (typeof key === "string" && key.startsWith("&") && !(peek() && peek().t === ":")) {
        const fName = key.slice(1);
        if (fragments.has(fName)) {
          i++; // 消费掉 &name token，避免重复处理
          // 片段合并同样跳过危险键：片段体可能来自外部构造（JSON 等），
          // 直接 Object.assign 会把危险键原样写进当前块。
          const frag = structuredClone(fragments.get(fName));
          for (const fk of Object.keys(frag)) {
            if (DANGEROUS_KEYS.has(fk)) continue;
            node[fk] = frag[fk];
          }
          continue;
        }
      }
      if (key === undefined) fail("E-PARSE-006", "sml: 期望键");
      i++;
      let colon = false;
      if (peek() && peek().t === ":") { colon = true; i++; }
      if (!colon && peek() && peek().t === "word") {
        let probe = i, found = false;
        while (probe < toks.length) {
          const p = toks[probe];
          if (p.t === "word" || p.t === "str") probe++;
          else if (p.t === "{") { found = true; break; }
          else break;
        }
        if (found) {
          const args = [];
          while (peek() && (peek().t === "word" || peek().t === "str")) {
            args.push(peek().t === "str" ? coerceStr(peek().v, fragments) : coerceWord(peek().v, fragments, nsMap));
            i++;
          }
          if (peek() && peek().t === "{") {
            i++;
            const sub = parseBlock("}");
            sub.__type = key;
            if (args.length === 1) sub.__name = args[0];
            // —— 块级类型标注：`<契约名> <块名> { .. }` ——
            //
            // 裸块首词若命中契约表，自动把该契约应用到本块，等价于块内首行
            // `@is 契约名`。与既有裸块 `type [name...] { }` 完全同形，故零新语法；
            // 由 opt-in 特性 `typed-block` 门控，未开启时既有文档行为不变。
            if (feats.has("typed-block") && feats.has("contract")) {
              const cname = key in contracts ? key : nsPrefix + key in contracts ? nsPrefix + key : null;
              if (cname) {
                const c = contracts[cname];
                applyDefaults(c, sub);
                const errs = checkContract(contracts, c, sub, "");
                if (errs) fail(errs[0].code, "contract: " + cname + " — " + errs.map((x) => x.msg).join("; "));
              }
            }
            setField(key, sub);
            continue;
          }
        }
      }
      const nxt = peek();
      if (nxt && nxt.t === "{") {
        i++;
        setField(key, parseBlock("}"));
      } else if (nxt && nxt.t === "[") {
        i++;
        setField(key, parseArray());
      } else if (nxt && (nxt.t === "word" || nxt.t === "str")) {
        setField(key, nxt.t === "str" ? coerceStr(nxt.v, fragments) : coerceWord(nxt.v, fragments, nsMap));
        i++;
      } else if (colon) {
        setField(key, null);
      } else {
        setField(key, coerceWord(key, fragments, nsMap));
      }
    }
    if (appliedContract) {
      const c = contracts[appliedContract];
      if (!c) fail("E-CONTRACT-001", "sml: 应用未定义契约 " + appliedContract);
      applyDefaults(c, node);
      const errs = checkContract(contracts, c, node, "");
      if (errs) fail(errs[0].code, "contract: " + appliedContract + " — " + errs.map((x) => x.msg).join("; "));
    }
    return node;
  }

  function parseArray() {
    const arr = [];
    let closed = false;
    while (i < toks.length) {
      const tok = peek();
      if (tok.t === "]") { i++; closed = true; break; }
      if (tok.t === ",") { i++; continue; }
      if (tok.t === "{") {
        i++;
        arr.push(parseBlock("}"));
      } else if (tok.t === "[") {
        // 嵌套数组（`m: [ [ a ] [ b ] ]`）必须递归。
        //
        // 此前缺少这一支，落到 `else break`：数组被**静默截断**，剩下的 `[` 被外层
        // 块解析当成键名 —— `m: [ [ a ] ]` 会得到 `{"m":[],"[":"a"}`：
        // 不报错，但数据是错的。Rust 侧同一输入得到 `{"m":[["a"]]}`。
        // 嵌套深度由入口处的 MAX_PARSE_DEPTH 闸门统一封顶。
        i++;
        arr.push(parseArray());
      } else if (tok.t === "word" || tok.t === "str") {
        arr.push(tok.t === "str" ? coerceStr(tok.v, fragments) : coerceWord(tok.v, fragments, nsMap));
        i++;
      } else if (tok.t === "}") {
        // W16：数组里多余的 `}` ⇒ E-PARSE-003（与 Rust 同码）。
        // 原先落到下面的 `else break`，`m: [ } ]` 静默得到 `{"m":[]}`。
        fail("E-PARSE-003", "sml: 多余的结束符号 }（数组应以 ] 闭合）");
      } else break;
    }
    // W16：顶层数组未闭合（缺少 `]`）必须报错，而非按 EOF 静默收尾。
    if (!closed && i >= toks.length) {
      fail("E-PARSE-001", "sml: 未闭合的数组（遇到文件结尾，缺少结束符号 ]）");
    }
    return arr;
  }

  const first = peek();
  // W16：顶层标量不可往返 ⇒ `E-PARSE-008`（判据：顶层**恰好一个标量 token**）。
  // 此前 `42` 会被 `parseBlock(null)` 当成「键即值」的裸键，解析成 `{"42": 42}`
  // —— **凭空造键**，重新序列化得到 `"42": 42` ≠ `42`（与 Rust/C 实测同病，四端一致）。
  // 判据边界：`hello world`（两 token）得到 `{"hello":"world"}`、值能往返 ⇒ 不算；
  // 带指令的顶层标量（token 数 > 1）**不报** —— 有意保守，宁漏不误伤。
  if (toks.length === 1 && (toks[0].t === "word" || toks[0].t === "str")) {
    fail("E-PARSE-008", "sml: 顶层须为容器（键值块、对象块或数组），单独的标量无法往返");
  }
  if (first && first.t === "[") { i++; return parseArray(); }
  if (first && first.t === "{") { i++; return parseBlock("}"); }
  return parseBlock(null);
}

/// 安全解析，不抛异常。
export function parseSafe(text, opts) {
  try {
    return { ok: true, value: parse(text, opts) };
  } catch (e) {
    const msg = String((e && e.message) || e);
    const pos = typeof e?.pos === "number" ? e.pos : null;
    return {
      ok: false,
      error: msg,
      // 错误码（见 errors/codes.sml）。取不到就是 null —— 不是所有失败都有码：
      // 宿主自己抛的异常（如 RangeError）以及尚未带码的少数分支就没有。
      code: (e && e.code) || null,
      pos,
      position: pos == null ? null : offsetToPosition(text, pos),
    };
  }
}

// ---------------------------------------------------------------------------
// 序列化
// ---------------------------------------------------------------------------

function quoteIfNeeded(s) {
  if (s === "" || /[ \t\n\r:#{}]/.test(s)) {
    return '"' + String(s).replace(/\\/g, "\\\\").replace(/"/g, '\\"') + '"';
  }
  return String(s);
}

function dumpValue(v, indent, out) {
  const pad = "  ".repeat(indent);
  if (v === null) out.push("null");
  else if (typeof v === "boolean") out.push(v ? "true" : "false");
  else if (typeof v === "number") out.push(String(v));
  else if (typeof v === "string") out.push(quoteIfNeeded(v));
  else if (Array.isArray(v)) {
    if (v.length === 0) out.push("[]");
    else {
      out.push("[");
      for (const e of v) out.push("\n" + "  ".repeat(indent + 1) + dumpInline(e));
      out.push("\n" + pad + "]");
    }
  } else if (typeof v === "object") {
    const keys = Object.keys(v).filter((k) => k !== "__type" && k !== "__name");
    if (keys.length === 0) { out.push("{}"); return; }
    out.push("\n" + pad + "{");
    for (const k of keys) {
      out.push("\n" + "  ".repeat(indent + 1) + k + ": ");
      dumpValue(v[k], indent + 1, out);
    }
    out.push("\n" + pad + "}");
  }
}

function dumpInline(v) {
  if (v === null) return "null";
  if (typeof v === "boolean" || typeof v === "number") return String(v);
  if (typeof v === "string") return quoteIfNeeded(v);
  if (Array.isArray(v)) return "[ " + v.map(dumpInline).join(", ") + " ]";
  if (typeof v === "object") {
    const keys = Object.keys(v).filter((k) => k !== "__type" && k !== "__name");
    return "{ " + keys.map((k) => {
      let vs;
      if (typeof v[k] === "string") vs = quoteIfNeeded(v[k]);
      else if (v[k] === null) vs = "null";
      else if (typeof v[k] === "object") vs = Array.isArray(v[k]) ? "[..]" : "{..}";
      else vs = String(v[k]);
      return k + ": " + vs;
    }).join(", ") + " }";
  }
  return "";
}

export function stringify(v) {
  const out = [];
  if (v && typeof v === "object" && !Array.isArray(v)) {
    for (const k of Object.keys(v)) {
      if (k === "__type" || k === "__name") continue;
      out.push(k + ": ");
      dumpValue(v[k], 0, out);
      out.push("\n");
    }
  } else {
    out.push(dumpInline(v));
  }
  return out.join("");
}

export const dump = stringify;

// Node 直接运行自检
if (typeof process !== "undefined" && typeof import.meta !== "undefined" &&
    process.argv[1] && import.meta.url.endsWith(process.argv[1].split(/[\\/]/).pop())) {
  const t = "name: John\nage: 27\naddress: { city: NY }\ntags: [ dev tools ]\n";
  console.log("parsed:", JSON.stringify(parse(t)));
  console.log("stringify:\n" + stringify(parse(t)));

  const c = `@contract Cfg loose {
  api_key: str
  port: int default 8080 min 1 max 65535
  debug: bool default false
}
@is Cfg
api_key: re_xxx
port: 99999
`;
  const r = parseSafe(c);
  console.log("contract (bad):", JSON.stringify(r));

  // include 自检
  const main = `include "ui" as ui\ntitle: ui.title`;
  const files = { "ui.sml": `title: Hello` };
  console.log("include:", JSON.stringify(parse(main, { files })));
}
