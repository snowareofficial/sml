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
  // 用 `pos` 反推每个 token 的行号（0 起）。
  //
  // 为什么需要：`include` / `import` 的参数收集必须知道「这条指令到哪结束」，
  // 而 tokenize **丢掉了换行** —— 只靠 token 类型猜边界是不行的（实测：`import "a.sml" as wa`
  // 之后每一行的裸词都被吞进同一条指令，直到撞上下一个 `:`；后果是 `regions` 这类键名
  // 凭空消失、命名空间被拼成 `waimportcommon`，而且**不报错**）。
  //
  // 为什么不边扫描边 ++line：跨行字符串 / 跨行块注释 / \r\n 等分支太多，漏一处就会让
  // 行边界判断错位 —— 而它正是本次要修的东西。这里一次性建「每行起始偏移」表再二分，
  // 与扫描逻辑完全解耦。
  if (toks.length) {
    const starts = [0];
    for (let k = 0; k < n; k++) if (text[k] === "\n") starts.push(k + 1);
    for (const tk of toks) {
      let lo = 0, hi = starts.length - 1;
      while (lo < hi) {
        const mid = (lo + hi + 1) >> 1;
        if (starts[mid] <= tk.pos) lo = mid;
        else hi = mid - 1;
      }
      tk.line = lo;
    }
  }
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
function throwCode(code, msg, pos) {
  const e = new Error(msg);
  e.code = code;
  // `pos` 可选：带上它，`parseSafe` 才能把错误落到**正确的那一行**
  // （不带 pos 的诊断会落在第 1 行 —— 见 `collectFeatures` 的未知名报错）
  if (typeof pos === "number") e.pos = pos;
  throw e;
}

function coerceWord(w, fragments, nsMap, feats) {
  if (w === "true") return true;
  if (w === "false") return false;
  if (w === "null") return null;
  const ev = w.match(/^\$env\.(.+)$/);
  if (ev) {
    // W16：`env` 特性关闭时**不得静默内插**。此前 JS 无条件内插 —— 用 `parse(text,
    // {features})` 当沙箱的调用方（如编辑器/服务端）会以为已经关掉了环境读取。
    // 码与 Rust 同：coerce_word 里 `$env` 未启用报 E-FEATURE-002（不是 001）。
    if (feats && !feats.has("env")) {
      throw throwCode("E-FEATURE-002",
        "sml: 当前特性集禁用了 `$env`（env），裸词 `" + w + "` 无法解析");
    }
    return envLookup(ev[1]);
  }
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
    // W16：`fragment` 特性关闭 ⇒ E-FEATURE-001（与 Rust 的 coerce_word 同码）；
    // 开着但名字未定义 ⇒ **E-INCLUDE-006**（改前 `return w` 静默退化成字符串，
    // 拼错的片段名于是变成普通数据，下游取值取不到还查不出原因）。
    if (feats && !feats.has("fragment")) {
      throw throwCode("E-FEATURE-001",
        "sml: 当前特性集禁用了片段引用（fragment），`" + w + "` 无法解析");
    }
    const name = w.slice(1);
    if (fragments.has(name)) return structuredClone(fragments.get(name));
    throw throwCode("E-INCLUDE-006", "sml: 未定义的片段引用 `" + w + "`");
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
  // 严格模式（v2/v3/v4）裸词字符串被禁用：与 Rust 同码 E-FEATURE-005（此前 JS 无该检查）。
  // 仅**值位置**的裸词受此约束 —— 键名、指令参数、片段名等裸词不受影响。
  if (!feats || (!feats.has("bareword-str") && !feats.has("bareword-string"))) {
    throwCode("E-FEATURE-005", "sml: 字符串必须加引号：当前特性集禁用了裸词字符串");
  }
  return w;
}

function coerceStr(s, fragments, feats) {
  const ev = s.match(/^\$env\.(.+)$/);
  if (ev) {
    // 与 coerceWord 同一道闸（引号串里的 `$env.X` 同样要受 `env` 特性管辖）
    if (feats && !feats.has("env")) {
      throw throwCode("E-FEATURE-002",
        "sml: 当前特性集禁用了 `$env`（env），字符串 `" + s + "` 无法解析");
    }
    return envLookup(ev[1]);
  }
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

// `@feature enable/disable` 的**合法名字表**（用于"未知名"校验）。
//
// 为什么需要它：此前 `collectFeatures` 把任何名字直接 `feats.add(w)` ⇒
// `@feature enable no-such-thing` **静默通过**（`feats.has("no-such-thing")` 永远为 false，
// 用户以为开了某项能力、其实什么都没开，且没有任何提示）。Rust 侧报 `E-FEATURE-003`
// （`rust/sml-parse/src/scan.rs` 的 `apply_feature_directive`），这里对齐同码。
//
// ⚠️ 合法名 = Rust 注册表（`rust/sml-feature/src/lib.rs` 的 `FEATURES`）**并集** JS 自身别名：
//   · `top-array` / `bareword-str` 是 JS 的写法（Rust 叫 `top-level-array` / `bareword-string`）；
//   · `escape` 是 JS 独有。
// 两种写法都算合法 —— 否则会把既有合法文档（如 `@feature enable top-array`）误报成未知特性。
const KNOWN_FEATURES = new Set([
  // 与 Rust 注册表同名（15 个）
  "bareword-string", "include", "env", "contract", "fragment", "top-level-array",
  "namespace", "implicit-ns", "multi-include", "glob-include", "regex-include",
  "ext-rewrite", "when", "for", "typed-block",
  // JS 别名 / 独有
  "top-array", "bareword-str", "escape",
]);

// 特性名别名映射：JS 的写法（`bareword-str` / `top-array`）与 Rust 注册表
// （`bareword-string` / `top-level-array`）同义。判定「某特性是否开启 / 是否被允许」时
// 必须按**规范名**统一，否则 `enable bareword-string` 与 `bareword-str` 会被当成两回事。
const FEAT_ALIAS = {
  "bareword-str": "bareword-string", "bareword-string": "bareword-str",
  "top-array": "top-level-array", "top-level-array": "top-array",
};
// 集合里是否包含某特性（含别名）
function featAllowed(set, f) {
  return set.has(f) || set.has(FEAT_ALIAS[f] || f);
}

// 调用方允许集归一化：接受 Set 或 Array（探针与既有调用方用数组），null = 不限制（全开）。
function normalizeAllowed(x) {
  if (x instanceof Set) return x;
  if (Array.isArray(x)) return new Set(x);
  return null;
}

// —— 版本检测：从**原文**扫出 `@version vN` 或 `@feature base vN` ——
//
// 必须在词法之前、在 `collectFeatures` 之前跑：版本决定特性基集（见 scanVersionBase）。
// 与 Rust 的 `Version::from_word` 同口径：接受 `v1`/`1`…`v4`/`4`；其余一律 E-FEATURE-004。
// 优先级：`@version` 显式声明 > `@feature base` > 默认 V1（与 Rust 的
// `declared.or(base).unwrap_or(V1)` 一致）。
function detectVersion(text) {
  // ⚠️ 必须**行首**（允许缩进）才算指令；注释里的 `@version` 不是声明。
  const re = /^[ \t]*@(version|feature\s+base)[ \t]+([^\s#/]+)/gm;
  let m;
  let declared = null, base = null;
  while ((m = re.exec(text)) !== null) {
    if (m[1] === "version") declared = m[2];
    else base = m[2];
  }
  const lit = declared !== null ? declared : base;
  if (lit === null) return { v: null, declaredLit: null };
  // 非法版本（含 v0 / v5 / 字母）⇒ E-FEATURE-004，文案点名「当前支持 v1..v4」
  if (!/^v?[1-4]$/.test(lit)) {
    throwCode("E-FEATURE-004",
      "sml: 版本声明非法，当前支持 v1..v4（@version vN / @feature base vN，N=1..4）");
  }
  return { v: lit, declaredLit: lit };
}

// 版本 → 特性基集（与 Rust 的 `FeatureSet::for_version` 同构）：
// 基集 = `DEFAULT_FEATURES`，仅**裸词字符串**这一项按版本切换 —— v1 开、v2/v3/v4 关
// （严格模式：自由文本必须引号，数字 / bool / null / 片段引用 / `$env.X` 仍为裸词）。
// v4 与 v3 在特性基集上**完全相同**；v4 额外把片段定义改为显式 `type:`/`name:` 关键字
// 形式（位置参数自 v4 废弃），那一层属解析器语义、不在此基集内。
function scanVersionBase(vLit) {
  const base = new Set(DEFAULT_FEATURES);
  const n = vLit ? parseInt(vLit.replace(/^v/, ""), 10) : 1;
  if (n >= 2) { base.delete("bareword-str"); base.delete("bareword-string"); }
  return base;
}

// 从文本里扫出 @feature enable/disable 声明，返回生效的 feature Set
//
// `allowed` 是调用方给出的**允许集上限**（Set；null = 不限制）。文档内部 @feature 只能在这
// 之上**收窄**（disable），不得**越权开启**（enable 一个不在 allowed 里的特性 ⇒ E-FEATURE-009）。
function collectFeatures(text, base, allowed) {
  const feats = new Set(base || DEFAULT_FEATURES);
  // ⚠️ 必须**行首**（允许缩进）才算指令：`#   · @feature enable 显式开启…` 这种
  // **注释里提到**的写法不是指令。旧写法 `/@feature…/g` 连注释里的也收 —— 以前
  // "任意名字都静默入集"所以看不出来，一加未知名校验就冒出一堆**假错误**
  // （`examples/advanced.sml:5` 就是这么中的：报"未知特性 `显式开启高级能力（glob-include`"）。
  const re = /^[ \t]*@feature[ \t]+(enable|disable)[ \t]+([^\n]*)/gm;
  let m;
  while ((m = re.exec(text)) !== null) {
    const mode = m[1];
    // 截掉行尾注释：`@feature enable for  # 备注` 不能把 `#` / `备注` 当特性名
    let list = m[2];
    const cut = list.search(/#|\/\/|--/);
    if (cut >= 0) list = list.slice(0, cut);
    // 逗号与空白都算分隔（与 Rust 的 `scan.rs::names()` 一致：`@feature enable a, b` 合法）
    const words = list.split(/[\s,]+/).filter(Boolean);
    for (const w of words) {
      // 未知名**不得静默**：与 Rust 同码 E-FEATURE-003（见 KNOWN_FEATURES 的说明）
      if (!KNOWN_FEATURES.has(w)) {
        throwCode("E-FEATURE-003",
          "sml: 未知特性 `" + w + "`，可用：" + [...KNOWN_FEATURES].join(", "),
          m.index);   // 带位置 ⇒ 诊断落在**该行**而不是第 1 行
      }
      if (mode === "enable") {
        // 调用方允许集是**上限**：文档不得越权开启一个不在允许集里的特性（见 E-FEATURE-009）。
        // allowed 未提供（null）表示不限制，任何 enable 都放行（与"全开"语义一致）。
        if (allowed && !featAllowed(allowed, w)) {
          throwCode("E-FEATURE-009",
            "sml: 文档请求启用特性 `" + w + "`，不在调用方允许集内（不得越权开启）",
            m.index);
        }
        feats.add(w);
      } else feats.delete(w);
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
  // ⚠️ 把整行替换成**等长空格**，而不是删掉整行（含它的换行）。
  //
  // 为什么：token 的 `pos` 是「剥掉指令之后那份文本」里的偏移，而 `parseSafe` 把它
  // 交给 `offsetToPosition(原文, pos)` 换算行列 —— 删行会让其后所有偏移比原文少
  // N 个字符（N = 被剥掉的指令行数），于是**每多一条 `@feature` 行，错误位置就往前
  // 漂一行**。换成等长空格后行列与原文一一对应，指令行也照样不会被词法看到。
  return String(text).replace(/^[ \t]*@feature[^\n]*/gm, (m) => " ".repeat(m.length));
}

// —— 模式 / 正则的三处上限，集中定义 ——
// 此前 `PATTERN_MAX_LEN` 定义在 parse() 内部而校验处却写死 4096，常量成了死代码；
// 现在统一提到模块级，两处都能引用。
const PATTERN_MAX_LEN = 4096; // 被校验值的长度上限：超长输入直接拒收，不交给 RegExp 硬算
const REGEX_SRC_MAX = 200;    // 用户内联正则的源长度上限
const QUANT_MAX = 1000;       // 量词上界：挡住 `次: 999999999` 这类展开
// 模式最坏展开的**编译期**预算（E-LIMIT-002，理由见 compilePatternToRe 的注释：
// JS 是原生 RegExp、运行时插不进计数器，只能编译期估上界）。取值 1e5 的经验依据：
// 正常业务模式（几十个原子 × 有界量词 ≤ 1000）远低于它，而嵌套量词
// `(a*)*`（1e6）、`(a{1000}){1000}`（1e6）这类灾难性回溯形状会被挡住。
const PATTERN_STEP_BUDGET = 100000;
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
    if (Array.isArray(cur[part])) {
      // 同一命名空间被引了多次：SML 的重复键规则是**聚合为数组**（Rust 侧
      // `ns { … } ns { … }` 就是两个块），所以这里追加一个新对象而不是覆盖，
      // 否则先前那份会被静默丢掉。
      const next = {};
      cur[part].push(next);
      cur = next;
      continue;
    }
    if (cur[part] === undefined || typeof cur[part] !== "object") {
      cur[part] = {};
    }
    cur = cur[part];
  }
  return cur;
}

// 把 src 合并进 target。
//
// ⚠️ 同键规则必须与**文档内的键**完全一致：SML 的重复键一律**聚合为数组**，
// 不是「后者覆盖」也不是「对象深合并」。
//
// 为什么：Rust 侧的 include 是**文本内联**（子文件 token 直接拼进 token 流），
// 于是子文件与主文件的同名键到了解析器眼里就是「重复键」，按聚合处理；JS 侧若在这里
// 覆盖 / 深合并，同一份文档两端就会得到不同的树。实测 `examples/advanced.sml`：
// `widget_login` 在 Rust 是 2 元素数组，在 JS 只有 1 个对象（挑键引入的那份被
// 后面 glob 引入的那份深合并掉了）—— 而**不报错**，只是树不一样。
//
// 唯一不参与聚合的是内部标记键 `__type` / `__name`（脚手架，不是数据）。
function mergeInto(target, src) {
  for (const k of Object.keys(src)) {
    if (k === "__type" || k === "__name") continue;
    // 同上：JSON.parse 之来源可能产生 own 的 `__proto__` 键，合并前必须拦掉
    if (DANGEROUS_KEYS.has(k)) continue;
    const v = src[k];
    if (target[k] === undefined) target[k] = v;
    else if (Array.isArray(target[k])) target[k].push(v);
    else target[k] = [target[k], v];
  }
}

/// 最小 glob 匹配（只支持 `*`，匹配整个文件名）。
/// 与 Rust 的 `sml-include::parse::glob_matches` 逐条对齐：首/末段是字面段时
/// 必须前缀 / 后缀整体匹配，中段用 `indexOf` 顺序推进。
function globMatchName(pattern, text) {
  const segs = pattern.split("*");
  if (segs.length === 1) return pattern === text;
  let pos = 0;
  if (!pattern.startsWith("*")) {
    if (!text.startsWith(segs[0])) return false;
    pos = segs[0].length;
  }
  for (const seg of segs.slice(pattern.startsWith("*") ? 0 : 1)) {
    if (seg === "") continue;
    const idx = text.indexOf(seg, pos);
    if (idx < 0) return false;
    pos = idx + seg.length;
  }
  return pattern.endsWith("*") ? true : pos === text.length;
}

/// 受限正则的模式长度上限（与 Rust `sml-regex::MAX_REGEX_LEN` 同一个数）。
const MAX_RESTRICTED_RE_LEN = 256;

/// SML 字符串里的**合法转义**（与 `sml-lex` 的集合一致）：`\n \t \r \0 \" \\ \uXXXX`。
const LEGAL_ESCAPES = new Set(["n", "t", "r", "0", '"', "\\"]);

/// 预扫 include / import 行里的 `re:"…"` 目标，返回 `{ text, patterns }`：
///   · `patterns`：**开引号的偏移 → 模式的原文**（不经字符串转义）；
///   · `text`：把模式里**非法转义**的反斜杠换成空格后的副本（等长 ⇒ 偏移不变）。
///
/// 为什么必须有这一手：正则模式里写 `\.` 是常态（转义点号），而 SML 的字符串转义集里
/// 没有 `\.` ⇒ 经 token 走的实现会先撞上 `E-LEX-004 未知转义符`。
/// **Rust 侧没这个问题**，因为 `include re:"…"` 是在**词法之前**按行取原文交给正则引擎的
/// （`sml-include::parse_include_line`）。这里等价地把原文记下来：解析时用 `patterns`
/// 里的原文去匹配，token 只负责提供位置。于是**照文档写 `re:"widget_.*\.sml"` 即可**
/// （不必像别的 JS 实现那样被迫写 `\\.`），两端同一份拼写、同一条模式。
///
/// ⚠️ 只扫 include / import 行：别处的 `"…\.…"` 仍应按 SML 规则报 `E-LEX-004`，
/// 不能因为这里而变成静默的"空格替换"（那才是真的改数据）。
function prescanRegexPatterns(text) {
  const patterns = new Map();
  if (!/^[ \t]*@?(include|import)[ \t]/m.test(text)) return { text, patterns };
  const edits = [];
  const LINE = /^[ \t]*@?(?:include|import)[ \t][^\n]*/gm;
  const RE = /re[ \t]*:[ \t]*"((?:[^"\\]|\\.)*)"/g;
  let lm;
  while ((lm = LINE.exec(text)) !== null) {
    RE.lastIndex = 0;
    let m;
    while ((m = RE.exec(lm[0])) !== null) {
      const raw = m[1];
      // 开引号的偏移：`m[0]` 的末位是闭引号 ⇒ 闭引号 - (模式长度 + 1) = 开引号。
      // ⚠️ 这里必须 `- 2`（不是 `- 1`）：曾写错一位，于是"被换成空格的"是模式里
      // 某一个正常字符，反斜杠原样留给词法 ⇒ 仍然 `E-LEX-004`。
      const quote = lm.index + m.index + m[0].length - 2 - raw.length;
      patterns.set(quote, raw);
      for (let k = 0; k < raw.length; k++) {
        if (raw[k] !== "\\") continue;
        const nx = raw[k + 1];
        const ok =
          nx !== undefined &&
          (LEGAL_ESCAPES.has(nx) || (nx === "u" && /^[0-9a-fA-F]{4}/.test(raw.slice(k + 2))));
        if (!ok) edits.push(quote + 1 + k);
        if (nx !== undefined) k++;
      }
    }
  }
  if (edits.length === 0) return { text, patterns };
  const chars = text.split(""); // 按 UTF-16 码元切，`pos` 也是码元偏移 ⇒ 一一对应
  for (const at of edits) chars[at] = " ";
  return { text: chars.join(""), patterns };
}

/// 把 **受限正则**（Rust `sml-regex` 的子集）编译成等价的 JS `RegExp`。
///
/// 为什么不能直接 `new RegExp(pat)` —— 两端的**元字符集合不同**，而这里匹配的是文件名，
/// 一旦不同就直接表现为「同一份文档在一端命中、另一端不命中」，且**不报错**：
///   · Rust 子集里**没有**分组 `()`、交替 `|`、`{n,m}`、`\d`/`\w` 这类转义类 ——
///     它们在那边的语义是**字面字符**；抛给 JS 却会变成元字符；
///   · `^` / `$` 只有**首尾**才是锚，中间是字面字符（JS 里中间的 `^` 是「行首断言」，
///     会让模式永远不匹配）。
/// 所以这里**逐原子转写**：只保留 `.` / `[...]` / `* + ?` / 首 `^` / 尾 `$`，
/// 其余一律转义成字面量。语法非法（量词前无原子、字符类未闭合、以 `\` 结尾）与
/// 超长都**报码**（`E-PARSE-025` / `E-LIMIT-007`），不静默给一个「永不匹配」的正则。
///
/// ⚠️ 与 Rust 的已知差异：那边有回溯**步数预算**（`MAX_REGEX_STEPS`，超限报 `E-LIMIT-002`）；
/// JS 是原生 RegExp，运行时插不进计数器。缓解手段沿用本文件对 `@type` 正则的既有口径：
/// 模式长度上限 + 文件名本身很短（匹配对象是单个文件名，不是整份文档）。
function compileRestrictedRegex(pat) {
  const src = String(pat);
  const inner = src.replace(/^\^/, "").replace(/\$$/, "");
  if ([...inner].length > MAX_RESTRICTED_RE_LEN) {
    throw throwCode("E-LIMIT-007", "sml: include 的受限正则过长（上限 " + MAX_RESTRICTED_RE_LEN + " 字符）");
  }
  const esc = (c) => (/[.*+?^${}()|[\]\\]/.test(c) ? "\\" + c : c);
  const escInClass = (c) => (/[\]\\^-]/.test(c) ? "\\" + c : c);
  let out = "";
  let i = 0;
  while (i < inner.length) {
    const c = inner[i];
    let atom;
    if (c === "\\") {
      if (i + 1 >= inner.length) {
        throw throwCode("E-PARSE-025", "sml: include 的受限正则以反斜杠结尾");
      }
      atom = esc(inner[i + 1]); // Rust 侧 `\x` 一律是字面 x，这里转义成同一个字面
      i += 2;
    } else if (c === ".") {
      atom = ".";
      i += 1;
    } else if (c === "[") {
      let j = i + 1;
      let neg = false;
      if (inner[j] === "^") { neg = true; j += 1; }
      const items = [];
      while (j < inner.length && inner[j] !== "]") {
        if (inner[j + 1] === "-" && inner[j + 2] !== undefined && inner[j + 2] !== "]") {
          items.push(escInClass(inner[j]) + "-" + escInClass(inner[j + 2]));
          j += 3;
        } else {
          items.push(escInClass(inner[j]));
          j += 1;
        }
      }
      if (j >= inner.length) {
        throw throwCode("E-PARSE-025", "sml: include 的受限正则字符类未闭合（缺少 ]）");
      }
      atom = "[" + (neg ? "^" : "") + items.join("") + "]";
      i = j + 1;
    } else if (c === "*" || c === "+" || c === "?") {
      throw throwCode("E-PARSE-025", "sml: include 的受限正则在量词 " + c + " 之前没有可重复的原子");
    } else {
      atom = esc(c);
      i += 1;
    }
    const q = inner[i];
    if (q === "*" || q === "+" || q === "?") { out += atom + q; i += 1; }
    else out += atom;
  }
  // 锚只可能来自原模式的首/尾（中间的 `^`/`$` 已被上面按字面转义）
  return new RegExp((src.startsWith("^") ? "^" : "") + out + (src.endsWith("$") ? "$" : ""));
}

/// 把 `dir/模式` 拆成 (目录, 文件名模式)，再在**宿主给的 files 表**里「列目录」。
///
/// JS 没有文件系统（浏览器 playground 与扩展宿主都只给一张扁平的「路径 → 内容」表），
/// 所以列目录只能在这张表上做。判据与 Rust 的 `glob_or_regex_dir` 一致：
///   · 只匹配**同一目录**下的条目（不递归，模式也不跨 `/`）；
///   · 结果按文件名排序（跨端顺序稳定，聚合出的数组顺序才不会飘）；
///   · 同一文件可能有多把键（工作区相对 / 相对文档目录 / 裸名），按文件名去重。
function expandDirKeys(fileTable, rawPath, matchName) {
  // ⚠️ 模式只按 **`/`** 切，绝不能先 `replace(/\\/g, "/")`：正则模式里的 `\` 是**转义符**
  // （`re:"widget_.*\.sml"` 的 `\.`），一旦被当成分隔符就变成 `widget_.*/sml` ⇒ 永不命中，
  // 而两端这时都"成功解析出空树"，静默不一致。这与 Rust 侧 Windows 上
  // `split_dir`（按平台分隔符切）的缺陷同源，那里也已改成按 `/` 切。
  // 文件**键**那一侧仍然把 `\` 归一到 `/`（宿主可能给 Windows 风格路径）。
  const raw = String(rawPath);
  const slash = raw.lastIndexOf("/");
  const dir = slash < 0 ? "" : raw.slice(0, slash);
  const pat = slash < 0 ? raw : raw.slice(slash + 1);
  const hit = new Map();
  for (const k of Object.keys(fileTable)) {
    const kk = String(k).replace(/\\/g, "/");
    const s = kk.lastIndexOf("/");
    const kdir = s < 0 ? "" : kk.slice(0, s);
    const kbase = s < 0 ? kk : kk.slice(s + 1);
    if (!(kdir === dir || (dir !== "" && kdir.endsWith("/" + dir)))) continue;
    if (!matchName(pat, kbase)) continue;
    if (!hit.has(kbase)) hit.set(kbase, k);
  }
  return [...hit.keys()].sort().map((b) => hit.get(b));
}

/// glob 目标（`dir/*.sml`）⇒ 实际文件键。
function expandGlobKeys(fileTable, rawPath) {
  return expandDirKeys(fileTable, rawPath, globMatchName);
}

/// `re:"…"` 正则目标 ⇒ 实际文件键（模式匹配的是**文件名**，与 Rust 同口径）。
function expandRegexKeys(fileTable, rawPath) {
  const pat = String(rawPath).replace(/^re:/, "");
  // ⚠️ 先切出**目录部分**，只把「文件名模式」交给正则引擎：整串（含 `sub/`）拿去编译的话，
  // 正则就会要求文件名里含 `sub/`，永不命中；而 `sub` 已在 `expandDirKeys` 里当目录用掉了。
  // （与 Rust 的 `split_dir_slash` + `glob_or_regex_dir(base.join(dir), pat)` 同构。）
  const slash = pat.lastIndexOf("/");
  const patOnly = slash < 0 ? pat : pat.slice(slash + 1);
  const re = compileRestrictedRegex(patOnly); // 过长 / 非法在这里报码（E-LIMIT-007 / E-PARSE-025）
  return expandDirKeys(fileTable, pat, (_p, name) => re.test(name));
}

/// 把一个**通配 / 正则目标**在 `files` 表里展开成命中的文件键；普通路径返回 `null`。
///
/// 给编辑器用：`include "dir/*.sml"` / `include re:"…"` 的路径不是一个真实文件，
/// 悬停没法像普通路径那样"找到/未找到"。而**命中 0 个时解析器是静默的**
/// （与 Rust 的 `glob_or_regex_dir` 同语义），所以「命中了几个」只能由编辑器说清楚。
export function expandIncludePaths(fileTable, rawPath) {
  const p = String(rawPath);
  if (p.startsWith("re:")) return expandRegexKeys(fileTable, p);
  if (p.includes("*")) return expandGlobKeys(fileTable, p);
  return null;
}

/// `import` 的**点分模块名**（`ui.buttons` / `admin.panel`）。
///
/// 与 Rust 的 `sml-include::parse::is_dotted_module_name` **同一条判据**：经 `import` 引入、
/// 既无路径分隔符、又不显式带 `.sml` 扩展名。这种写法里点号是**目录层级**
/// （`ui.buttons` ⇒ `ui/buttons.sml`），且默认挂成命名空间 `ui.buttons`；
/// 带引号显式写了 `.sml` 的则一律按字面路径内联（`import "common.sml"` ≡ `include "common.sml"`）。
function isDottedModuleName(raw, viaImport) {
  const p = String(raw);
  return !!viaImport && !p.includes("/") && !p.includes("\\") && !p.endsWith(".sml");
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
    let isRe = false;
    if (!path) {
      const qM = raw.match(/"([^"]+)"/);
      if (qM) {
        // `re:"…"`：受限正则目标。`re:` 前缀要**留在 path 里** —— 下游靠它区分
        // 「正则目标」与「字面路径」（与 Rust 的 `raw.starts_with("re:")` 同口径）。
        isRe = /re\s*:\s*$/.test(raw.slice(0, qM.index));
        path = qM[1];
      } else {
        // 裸词形式：`include re:pattern`（Rust 同样接受，且豁免「路径必须加引号」）
        const bare = raw.trim();
        if (/^re\s*:\s*\S/.test(bare)) {
          isRe = true;
          path = bare.replace(/^re\s*:\s*/, "").trim();
        } else {
          const wM = raw.match(/([A-Za-z0-9_.\-]+)/);
          path = wM ? wM[1] : null;
        }
      }
      if (path && isRe) path = "re:" + path;
    }
    if (!path) continue;

    // 部分引用不触发 implicit-ns 自动命名空间（否则挑出的键会被塞进文件名命名空间）
    if (keys && !ns) {
      // 平铺到当前作用域，ns 保持 null
    } else if (
      feats.has("implicit-ns") && !ns && !isRe &&
      !path.includes("*") && !path.includes(".")
    ) {
      // ⚠️ 通配 / 正则目标**不参与** implicit-ns：`re:"conf"` 的 raw 里没有点，
      // 老判据会给它挂一个名叫 `re:"conf"` 的命名空间（Rust 侧同样有这个坑，已一并修）。
      ns = path;
    }
    // 补 .sml（点分模块名 / 通配 / 正则目标都不补：它们不是字面文件名）
    if (!isRe && !path.includes("*") && !path.includes(".") && !path.includes("/")) path += ".sml";
    targets.push({ path, ns, viaImport, keys, regex: isRe });
  }
  return targets;
}

// ---------------------------------------------------------------------------
// —— 契约映射（D5 / 任务 F，JS 侧对齐 rust/sml-parse/src/contract_map.rs）——
//
// 三条铁规矩照 Rust：① opt-in（不开 traceContracts 时 parse 返回值与旧版逐字一致）；
// ② 交换表示是一份 SML（contractMapToSml，与 Rust ContractMap::to_sml 逐字节一致）；
// ③ 输出确定（按路径排序、无时间戳，同输入逐字节相同）。
//
// 路径段用 { key } / { index } 两种对象（**不造点分字符串**）：靠 SML 类型自识别区分
// （0 是下标、"0" 是键），键名含 `.` / `[` 时点分写法有歧义。
function pathEq(a, b) {
  if (a.length !== b.length) return false;
  for (let k = 0; k < a.length; k++) {
    const x = a[k], y = b[k];
    const xi = x.index !== undefined, yi = y.index !== undefined;
    if (xi !== yi) return false;
    if (xi) { if (x.index !== y.index) return false; }
    else { if (x.key !== y.key) return false; }
  }
  return true;
}

// 按码位逐字比较（Rust String/char Ord 语义；JS 默认按 UTF-16 码元，对辅助平面
// 字符会与 Rust 不一致，故显式展开为码点）。
function cmpCodePoints(a, b) {
  const A = Array.from(a), B = Array.from(b);
  const n = Math.min(A.length, B.length);
  for (let k = 0; k < n; k++) {
    if (A[k] !== B[k]) return A[k] < B[k] ? -1 : 1;
  }
  if (A.length === B.length) return 0;
  return A.length < B.length ? -1 : 1;
}

// 按路径排序的比较器：逐段比；同位置 Key 在 Index 前（对应 Rust PathSeg 枚举声明序）；
// Key 段按码位字典序；Index 段按数值；前缀更短者更小。
function cmpPath(a, b) {
  const n = Math.min(a.length, b.length);
  for (let k = 0; k < n; k++) {
    const x = a[k], y = b[k];
    const xi = x.index !== undefined, yi = y.index !== undefined;
    if (xi !== yi) return xi ? 1 : -1;   // Key(x) < Index(y) ⇒ Key 在前
    if (xi) { if (x.index !== y.index) return x.index < y.index ? -1 : 1; }
    else { const c = cmpCodePoints(x.key, y.key); if (c !== 0) return c; }
  }
  return a.length === b.length ? 0 : (a.length < b.length ? -1 : 1);
}

// 裸词安全判据：照 Rust contract_map::word_or_quoted —— 全由词字符（字母/数字/
// `_` `@` `.` `-`，含 CJK）组成、不以 ASCII 数字开头、且非三个保留字 ⇒ 裸写，
// 否则加引号并转义（\ " 换行 回车 制表）。
function wordOrQuoted(s) {
  const safe = s.length > 0
    && !/^[0-9]/.test(s)
    && s !== "true" && s !== "false" && s !== "null"
    && /^[\p{L}\p{N}_@.\-]+$/u.test(s);
  if (safe) return s;
  let out = '"';
  for (const ch of s) {
    if (ch === "\\") out += "\\\\";
    else if (ch === '"') out += '\\"';
    else if (ch === "\n") out += "\\n";
    else if (ch === "\r") out += "\\r";
    else if (ch === "\t") out += "\\t";
    else out += ch;
  }
  return out + '"';
}

// 契约映射 → 一份 SML 文档（顶层数组）。与 Rust ContractMap::to_sml 逐字节一致：
// 空表输出 "[]\n"；每条 `  { path: [ … ] contracts: [ … ] }`；顶层实体 path: []；
// 无时间戳。本函数对入参按路径稳定排序（幂等），故调用方是否预先 freeze 均可。
export function contractMapToSml(contractMap) {
  const entries = (contractMap || []).slice().sort((a, b) => cmpPath(a.path, b.path));
  if (entries.length === 0) return "[]\n";
  let out = "[\n";
  for (const e of entries) {
    out += "  { path: ";
    if (e.path.length === 0) {
      out += "[]";
    } else {
      out += "[ ";
      out += e.path.map((s) => (s.index !== undefined ? String(s.index) : wordOrQuoted(s.key))).join(" ");
      out += " ]";
    }
    out += " contracts: [ ";
    out += e.names.map(wordOrQuoted).join(" ");
    out += " ] }\n";
  }
  out += "]\n";
  return out;
}

// 解析（递归下降）
// ---------------------------------------------------------------------------

export function parse(text, opts, includeStackArg, effectiveFeatsArg, callerAllowedArg) {
  // —— 文件级规范化：去掉开头的 **UTF-8 BOM**（U+FEFF）——
  //
  // 为什么必须去：本词法器按显式分隔符切词（不是用 `\s`），所以 BOM **不会**被当空白，
  // 而是被吞进第一个单词 ⇒ `\ufeffx: 1` 的键名会变成 `\ufeffx`（实测：`{"\ufeffx":1}`）。
  // 来源极常见：Windows 记事本「另存为 UTF-8」、Excel 导出的文本。
  // 对齐实现：Rust 侧在 `sml-parse::scan::strip_version` 收口（文本入口唯一漏斗），
  // 子文件在 `sml-include::expand_file_tokens` 收口；这里覆盖文本入口 + 每个被 include 的子文件
  // （子文件也是走本函数递归解析的）。
  if (typeof text === "string" && text.charCodeAt(0) === 0xfeff) text = text.slice(1);
  opts = opts || {};
  const files = opts.files || {};
  const nsPrefix = opts.nsPrefix || "";
  // 正在展开的文件链（含根文档），用于 include 环检测（见 resolveIncludes）。
  const includeStack = (includeStackArg instanceof Array) ? includeStackArg : [];
  // 调用方允许集（**上限**）：顶层由 opts.features 给出（兼容 Set / Array）；
  // 递归子文件沿用父层的 callerAllowedArg，保证"调用方控制"贯穿整棵 include 树。
  const callerAllowed = (effectiveFeatsArg === undefined || effectiveFeatsArg === null)
    ? normalizeAllowed(opts.features)
    : callerAllowedArg;
  // —— 外置扩展（与 Rust 侧 `sml::ext` / `sml::contract_ext` 对齐）——
  //   opts.directives: { "form": { positional?: bool, call(arg, body) -> {discard:true} | {emit: 对象} } }
  //   opts.types:      { "image": (v) => true | false | "错误信息" }
  //   opts.warnings:   []   传入数组时，弃用提示会 push 进去（与 Rust 的 Diagnostic 对应）
  // 不带这些选项时，行为与未扩展时**完全一致**。
  const extDirs = opts.directives || null;
  const extTypes = opts.types || null;

  // —— 契约映射（D5 / 任务 F）：opt-in，不开时零开销、返回值与旧版逐字一致 ——
  //
  // 压栈点照 Rust 的 5 处（对象值块 / 裸块 / 值数组 / 数组对象元素 / 嵌套数组元素），
  // 记录点 2 处（`@is` 应用、块级 typed-block 应用）。`@contract` / `@type` / 片段体
  // 是**定义**、不进结果树，故不压栈、不记录。
  const traceContracts = !!opts.traceContracts;
  const pathStack = [];
  const contractMapEntries = [];
  const pushKey = (k) => { if (traceContracts) pathStack.push({ key: k }); };
  const pushIndex = (n) => { if (traceContracts) pathStack.push({ index: n }); };
  const popPath = () => { if (traceContracts) pathStack.pop(); };
  const recordContract = (name) => {
    if (!traceContracts) return;
    const path = pathStack.map((s) => (s.index !== undefined ? { index: s.index } : { key: s.key }));
    let e = null;
    for (const en of contractMapEntries) { if (pathEq(en.path, path)) { e = en; break; } }
    if (e) { if (!e.names.includes(name)) e.names.push(name); }
    else contractMapEntries.push({ path: path, names: [name] });
  };

  // `@feature` 整行在**词法之前**剥掉（见 stripFeatureLines 的说明）——
  // 特性集仍由下面的 collectFeatures 从**原文**读，两者互不影响。
  //
  // `re:"…"` 的**模式原文**也在词法之前扫出来（见 prescanRegexPatterns）：正则里的 `\.`
  // 不是合法的 SML 转义，只有先取出原文才能像 Rust 那样按原文匹配。两次预处理都是
  // **等长替换**，所以 token 的 `pos` 始终与原文对应（诊断行列才不会漂）。
  const pre = prescanRegexPatterns(text);
  const toks = tokenize(stripFeatureLines(pre.text));
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

  // —— 特性基集与调用方允许集 ——
  // 顶层：先按版本推导基集（DEFAULT_FEATURES 仅切换 bareword-string），再与调用方允许集取交；
  // 递归子文件：直接继承父层已生效的特性集（effectiveFeatsArg），不重算版本（与"暂缓的
  // 每文件独立解析-合并"保持距离：当前仍是根文档版本贯穿整棵 include 树）。
  let feats;
  if (effectiveFeatsArg !== undefined && effectiveFeatsArg !== null) {
    feats = collectFeatures(text, effectiveFeatsArg, callerAllowed);
  } else {
    const vinfo = detectVersion(text);            // 非法版本 ⇒ E-FEATURE-004
    const versionBase = scanVersionBase(vinfo.v);
    const base = callerAllowed
      ? new Set([...versionBase].filter((f) => featAllowed(callerAllowed, f)))
      : versionBase;
    feats = collectFeatures(text, base, callerAllowed);
    // 调用方允许集与文档所需特性**无交集** ⇒ 直接失败（不静默降级），与 Rust 同码 E-FEATURE-009。
    // （effectiveFeatsArg !== null 的子文件路径不跑此判定：子文件继承父层已生效集，空集只可能
    //  由顶层无交集导致，那里已经报过。）
    if (callerAllowed && feats.size === 0) {
      throwCode("E-FEATURE-009", "sml: 文档请求的特性与调用方允许的特性无交集");
    }
  }

  function literal() {
    const t = peek();
    if (!t) fail("E-PARSE-021", "sml: 期望字面量");
    if (t.t === "str") { i++; return coerceStr(t.v, fragments, feats); }
    if (t.t === "word") { i++; return coerceWord(t.v, fragments, nsMap, feats); }
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
    // W16：**编译期**「最坏展开」预算（E-LIMIT-002）。
    //
    // JS 把模式编译成**原生 RegExp**，运行时插不进步数计数器 —— Rust 的 `sml-regex`
    // 是自研回溯引擎，能在每次匹配决策处计步（`MAX_REGEX_STEPS`）；JS 没有这个位置。
    // 于是预算落在**编译期**：估算最坏情况的原子重复次数 ——
    //   顺序 = 各元素**求和**、量词 = 与之**相乘**（`次: {m,n}` 取 n；`*` / `+`
    //   无上界时取 `QUANT_MAX` 作保守代理；内联正则按 `REGEX_SRC_MAX` 计入）。
    // 它挡的是 `(a*)*` / `(a{1000}){1000}` 这类**嵌套量词**造成的灾难性回溯 ——
    // 与 Rust 的步数预算是**同一个因**（防病态模式挂死主线程）、**同码**，
    // 但条件粒度不同（Rust 在运行时限步、JS 在编译期估上界），已写进码表的 note。
    const build = (node) => {
      if (Array.isArray(node)) {
        let src = "", cost = 0;
        for (const x of node) { const r = build(x); src += r.src; cost += r.cost; }
        return { src: src, cost: cost };
      }
      if (typeof node === "string") {
        const s = escapeRe(node);
        return { src: s, cost: Math.max(1, node.length) };
      }
      if (node == null || typeof node !== "object")
        throw throwCode("E-CONTRACT-013", "sml: 模式元素类型不支持");
      const g = (...keys) => {
        for (const k of keys) if (node[k] !== undefined) return node[k];
        return undefined;
      };
      if (g("直到", "until") !== undefined)
        throw throwCode("E-CONTRACT-013", "sml: JS 引擎暂不支持 直到/until（懒惰量词在高危结构下有回溯风险；该场景请用 Rust 引擎）");
      let base, cost;
      if (g("字面", "lit", "literal") !== undefined) {
        const lit = String(g("字面", "lit", "literal"));
        base = escapeRe(lit);
        cost = Math.max(1, lit.length);
      } else if (g("类", "class") !== undefined) {
        base = classToRe(String(g("类", "class")));
        cost = 1;
      } else if (g("任一", "alt", "any-of") !== undefined) {
        // 分支取**最大**代价（任一分支都可能在回溯中被尝试）
        const parts = g("任一", "alt", "any-of").map((x) => build(x));
        cost = 0;
        for (const p of parts) cost = Math.max(cost, p.cost);
        base = "(?:" + parts.map((p) => p.src).join("|") + ")";
      } else if (g("组", "group") !== undefined) {
        const p = build(g("组", "group"));
        base = "(?:" + p.src + ")";
        cost = p.cost;
      } else if (g("序列", "seq") !== undefined) {
        const p = build(g("序列", "seq"));
        base = p.src;
        cost = p.cost;
      } else if (g("用", "use") !== undefined) {
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
        // 内联正则内部**可能本身就带量词**（`(a+)+`），无法从外面看穿，故按
        // 「最坏 = 允许的最大源长度」计入代价：这样「内联正则 + 无界量词」
        // （经典 ReDoS 形状）会直接超预算被拒，而单独一个内联正则照常可用。
        cost = REGEX_SRC_MAX;
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
        // 量词把代价**乘上去**：`(a*)*` ⇒ 1000 × 1000 = 1e6，超预算即拒
        const mult = qMax === undefined ? QUANT_MAX : Math.max(qMax, mn);
        cost *= Math.max(1, mult);
      }
      return { src: base, cost: cost };
    };
    const r = build(v);
    if (r.cost > PATTERN_STEP_BUDGET) {
      throw throwCode("E-LIMIT-002",
        "sml: 模式匹配超出步数预算（编译期估算最坏展开 " + r.cost + " > "
        + PATTERN_STEP_BUDGET + "），疑似病态规则");
    }
    return new RegExp("^(?:" + r.src + ")$", "u");
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
      // —— 契约体里的 `@requires`：本端暂未实现，必须**响亮拒绝** ——
      // 参照 E-CONTRACT-017（契约叠加未实现）的既有先例：绝不静默忽略、
      // 绝不静默产出一棵树。此前 `@` 只会被 1392 行的「期望字段名」顺带
      // 撞成 E-PARSE-006（码不对、也没说清真实原因），现改为明确的本码。
      if (peek().t === "@") {
        i++; /* 消费 @ */
        const dt = peek();
        const dname = dt && (dt.t === "word" || dt.t === "str") ? dt.v : "";
        if (dname === "requires") {
          fail("E-CONTRACT-018",
            "sml: 本实现暂不支持契约的 `@requires` 指令（它要求先开启所列特性）；" +
            "请移除该指令，或改用已实现它的 Rust 端");
        }
        fail("E-PARSE-005", "sml: 契约体内不支持指令 `@" + dname + "`（契约体仅支持 `@requires`）");
      }
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
  //
  // `dirPos` = **这条指令本身**在文档里的偏移。凡是「这条指令出问题」的失败
  // （目标找不到 / 特性未开 / 子文件解析失败）都归位到它，否则位置会落成
  // 「收集参数时循环停在哪个 token」—— 实测能飘到几十行外的注释上，
  // 编辑器就在那行画红线，看起来完全是另一个错误。
  function resolveIncludes(line, dirPos, viaImport) {
    if (!feats.has("include")) fail("E-FEATURE-001", "sml: include 未启用（需要 feature 'include'）", dirPos);
    let targets;
    try {
      targets = parseIncludeTargets(line, feats);
    } catch (e) {
      // `parseIncludeTargets` 在 parse() 作用域之外，只能用模块级 throwCode 抛码，
      // 因此没有位置（会落成第 1 行）。这里补上指令位置。
      if (e && typeof e.pos !== "number") e.pos = dirPos;
      throw e;
    }
    const results = [];
    for (const tg of targets) {
      // 点分模块名（`import ui.buttons`）：路径按目录层级展开，并**默认挂命名空间**。
      // 与 Rust 的 `finalize_target` 同口径（那里也是「只有点分模块名才自动挂 ns」）——
      // 两边都改过之后，`import` 与 `include` 的差异只剩这一条，且判据同一份。
      const dotted = isDottedModuleName(tg.path, viaImport);
      const ns = tg.ns || (dotted && !(tg.keys && tg.keys.length) ? tg.path : null);
      // —— 通配 / 受限正则目标（需 `glob-include` / `regex-include`）——
      // 展开成 0..N 个实际文件，逐个按同一条指令的 ns / keys 处理（与 Rust 同构）。
      //
      // ⚠️ **先判 `re:` 再判 `*`**：正则模式里常常也带 `*`（`re:"widget_.*\.sml"`），
      //    顺序反了会被当成 glob 模式，匹配语义完全不同。
      const isRegex = !!tg.regex || String(tg.path).startsWith("re:");
      let paths = [tg.path];
      if (isRegex) {
        if (!feats.has("regex-include")) {
          fail("E-FEATURE-001", "sml: 正则 include 需要特性 `regex-include`（请 @feature enable regex-include）", dirPos);
        }
        // 与 Rust 同码：部分引用只作用于单文件目标
        if (tg.keys && Array.isArray(tg.keys)) {
          fail("E-INCLUDE-009", "sml: 部分引用 `{ keys }` 不能配合 glob/regex 通配（请指定单个文件）", dirPos);
        }
        try {
          paths = expandRegexKeys(files, tg.path);
        } catch (e) {
          // 模式过长 / 非法：`compileRestrictedRegex` 只能抛模块级带码错误（无位置），
          // 这里补上指令位置，别让它落成第 1 行。
          if (e && typeof e.pos !== "number") e.pos = dirPos;
          throw e;
        }
        // ⚠️ 「命中 0 个」与 Rust 一致：**静默展开为空**（`glob_or_regex_dir` 返回空表，
        // 外层 for 自然什么都不做）。不在这里报错 —— 两端同码同语义优先，
        // 而"模式写错导致什么都没引入"这件事由**编辑器**负责说清楚（悬停会显示命中数）。
      } else if (String(tg.path).includes("*")) {
        if (!feats.has("glob-include")) {
          fail("E-FEATURE-001", "sml: 通配 include 需要特性 `glob-include`（请 @feature enable glob-include）", dirPos);
        }
        if (tg.keys && Array.isArray(tg.keys)) {
          fail("E-INCLUDE-009", "sml: 部分引用 `{ keys }` 不能配合 glob/regex 通配（请指定单个文件）", dirPos);
        }
        // 同上：命中 0 个 = 静默展开为空（与 Rust 一致）
        paths = expandGlobKeys(files, tg.path);
      }
      for (const onePath of paths) {
        let text = files[onePath];
        if (text === undefined) {
          // 也允许直接用 key（不带扩展名）
          text = files[String(onePath).replace(/\.sml$/, "")];
        }
        if (text === undefined && dotted) {
          // 点分模块名 ⇒ 点当目录层级：`ui.buttons` → `ui/buttons.sml`
          // （与 Rust 的 `resolve_target_paths` 同口径；此前 JS 根本没走到这一步，
          //   整条指令被上面那个吞词 bug 吃掉了）
          text = files[String(onePath).split(".").join("/") + ".sml"];
        }
        if (text === undefined) fail("E-INCLUDE-001", "sml: include 目标未找到: " + onePath, dirPos);
        // include 环检测（与 Rust sml-include / c/sml.c / lua/lib/sml.soup 同码）：
        // 栈 = 当前**正在展开**的文件链；命中即环 —— 菱形包含（b、c 各被引用一次）不算环。
        // 此前 JS 无环检测，互相 include 会一路递归到宿主 RangeError；现在改成响亮的 E-INCLUDE-002。
        if (includeStack.includes(onePath)) {
          fail("E-INCLUDE-002", "sml: include 循环引用: " + onePath, dirPos);
        }
        const childPrefix = tg.ns ? nsPrefix + tg.ns + "." : nsPrefix;
        // 外置扩展随 include 递归传递：被包含的文件里同样可以用方言指令与外置类型
        let v;
        try {
          v = parse(text, {
            files,
            nsPrefix: childPrefix,
            directives: extDirs || undefined,
            types: extTypes || undefined,
            warnings: opts.warnings,
          }, includeStack.concat(onePath), feats, callerAllowed);
        } catch (e) {
          // 子文件里的失败：`e.pos` 属于**子文件**，直接抛出去会被画在主文件的同名行上
          // （实测：子文件第 2 行报错 → 主文件第 2 行标红，毫无关联）。
          // 归位到 include 指令本身，并把目标文件名写进文案（否则看不出错在哪个文件）。
          throwCode(
            (e && e.code) || "E-INCLUDE-001",
            "include " + onePath + ": " + ((e && e.message) || e),
            dirPos
          );
        }
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
        results.push({ value: filtered, ns });
      }
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
    let closed = false;
    while (i < toks.length) {
      const tok = peek();
      if (tok.t === "}" || tok.t === "]") {
        if (closing === tok.t) { i++; closed = true; break; }
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
          // `@version` 的合法性（v1..v4 / 1..4）已在解析入口的 detectVersion 校验；
          // 非法版本会在 collectFeatures 之前就抛 E-FEATURE-004。这里只负责消费指令，
          // 让 `@version` 不被误当成片段名（`version` 不可作为片段名）。
          i++;
          if (peek()) i++;
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
          // W16：契约未启用时不得静默生效。这条不只是「行为差异」——用它当沙箱的
          // 调用方（parseSafe / 编辑器）会以为契约校验已被关掉，实际照样执行。
          if (!feats.has("contract")) fail("E-FEATURE-001", "sml: @contract 需要特性 `contract`，但当前特性集已禁用");
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
          // W16：`@is` 与 `@contract` 同属 contract 特性（Rust 侧同样两道都门控）
          if (!feats.has("contract")) fail("E-FEATURE-001", "sml: @is 需要特性 `contract`，但当前特性集已禁用");
          i++;
          const raw = peek() && peek().v;
          if (!raw) fail("E-PARSE-019", "sml: @is 后须契约名");
          const firstLine = peek().line;  // 首个契约名所在源行（D 同 line 判据）
          i++;
          // D（任务 D）：多契约「响亮拒绝」—— `@is` 后出现第 2 个裸词（同一行、
          // 且该词不是「后跟 `:` 的键」）即报 E-CONTRACT-017，绝不静默给树。
          // 与 Rust D1 同口径：遇 `:` / `{` / `]` / 换行 / 逗号（可省）即停。
          {
            let j = i;
            while (j < toks.length) {
              const tk = toks[j];
              if (tk.line !== firstLine) break;            // 换行即停
              if (tk.t === ",") { j++; continue; }          // 逗号可省，跳过
              if (tk.t === "word" || tk.t === "str") {
                if (toks[j + 1] && toks[j + 1].t === ":") break;  // 后跟 `:` 是键，非契约名
                fail("E-CONTRACT-017", "sml: 该实现暂不支持一个块应用多份契约（@is 多契约写法）");
              }
              break;  // `{` / `]` / 其它 token → 停
            }
          }
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
        // —— 片段定义：`@name { ... }`；参数只认**显式**写法 `type:` / `name:` ——
        //
        // W16：此前这里对**任何** `@xxx` 都静默宽容 —— `@foo bar { x: 1 }`（位置参数）
        // 被当片段收下（拼错的指令名于是变成数据）、`@foo bar`（无片段体）整行丢掉。
        // 现在按 Rust 的判据报错，⚠️ 但**不一刀切**：`@foo { ... }`（无参数带体）
        // 仍是合法片段定义。
        i++; // 消费片段名（与其它分支一致：fname 由自己消费）
        let ftype = null, farg = null;
        // 显式参数：仅当 `type`/`name` **紧跟冒号**时才算参数
        // （故名为 `type` 的片段 `@type { .. }` 仍可定义 —— 与 Rust 的 is_param 一致）
        while (peek() && peek().t === "word"
               && (peek().v === "type" || peek().v === "name")
               && toks[i + 1] && toks[i + 1].t === ":") {
          const kw = peek().v;
          i += 2; // 消费 type/name 与 :
          const vt = peek();
          if (!vt || (vt.t !== "word" && vt.t !== "str")) {
            fail("E-PARSE-020", "sml: 片段 `@" + fname + "` 的参数 `" + kw + ":` 后须值");
          }
          if (kw === "type") {
            if (ftype !== null) fail("E-PARSE-020", "sml: 片段 `@" + fname + "` 的 `type:` 参数重复");
            ftype = vt.v;
          } else {
            if (farg !== null) fail("E-PARSE-020", "sml: 片段 `@" + fname + "` 的 `name:` 参数重复");
            farg = vt.v;
          }
          i++;
        }
        if (!peek() || peek().t !== "{") {
          fail("E-PARSE-005", "sml: `@" + fname + "` 不是合法指令且缺少片段体 { ... }；"
            + "若本意是「片段定义」，参数须显式写作 `type: X` 与 `name: Y`"
            + "（位置参数形式自 v4 起已废弃；不带参数时写作 `@" + fname + " { ... }`）；"
            + "若本意是「指令」，请检查拼写（合法指令：contract / is / version / type）");
        }
        i++;
        {
          const sub = parseBlock("}");
          if (ftype) { sub.__type = ftype; if (farg) sub.__name = farg; }
          if (!feats.has("fragment")) {
            fail("E-FEATURE-001", "sml: 片段定义 `@" + fname + "` 需要特性 `fragment`，但当前特性集已禁用");
          }
          fragments.set(nsPrefix + fname, sub);
        }
        continue;
      }
      // include 处理：键为 include / import
      const key = (peek() && peek().v);
      if (key === "include" || key === "import") {
        // 指令自身的**位置与行号**：位置用于报错归位（见 resolveIncludes），
        // 行号用于「只在同一条源行内收集参数」。
        const dirPos = peek().pos;
        const dirLine = peek().line;
        i++;
        if (peek() && peek().t === ":") i++;
        // 收集 include 参数：引号路径 / as / 命名空间词 / 逗号。
        //
        // ⚠️ **必须在同一条源行内收集**（`tk.line === dirLine`）。曾经只靠
        // `:` / `@` / `}` / `]` 猜边界，而 tokenize 丢了换行 ⇒ `import "a.sml" as wa`
        // 后面**每一行**的裸词都被吞进同一条指令，直到撞上下一个 `:`（下一次出现
        // `${…}:` 的位置）。实测后果（examples/advanced.sml）：
        //   · 中间的 8 条 include/import 全部失效；
        //   · `regions` 这个键名被当成参数吞掉 ⇒ 顶层少一个键；
        //   · 命名空间被拼成 `waimportcommon`（`as wa` + 下一行的 `import "common`）；
        //   · 而且 **`ok = true`、零诊断** —— 静默给出错误的树。
        // 行级收集与 Rust 的 `sml-include::parse_include_line`（逐行解析）同口径。
        const importMode = (key === "import");
        let line = "";
        let lastWasAs = false;
        while (peek() && peek().line === dirLine) {
          const tk = peek();
          // 路径带引号回填：`parseIncludeTargets` 的引号分支会按引号切分，
          // 含空白 / `#` / 逗号的路径不会被误拆（此前丢引号会让它们被裸词正则截断）。
          if (tk.t === "str") { line += '"' + tk.v + '" '; i++; lastWasAs = false; continue; }
          if (tk.t === "word" && tk.v === "as") { line += " as "; i++; lastWasAs = true; continue; }
          if (tk.t === "word" && tk.v === "in") { line += " in "; i++; lastWasAs = false; continue; }
          if (tk.t === ",") { line += ","; i++; lastWasAs = false; continue; }
          if (tk.t === "{") { line += "{"; i++; lastWasAs = false; continue; }
          if (tk.t === "}") { line += "} "; i++; lastWasAs = false; continue; }
          // `re:"…"` 受限正则目标：tokenize 把 `re` 与 `:` 拆成两个 token（`:` 是分隔符），
          // 所以这里要**成组**吃掉三个 token —— 否则 `:` 会先命中下面的 break 分支，
          // 这条指令变成空集合，而 `re` / `:` / 路径三个 token 会被当**数据**解析出垃圾键。
          if (
            tk.t === "word" && tk.v === "re" &&
            toks[i + 1] && toks[i + 1].t === ":" &&
            toks[i + 2] && (toks[i + 2].t === "str" || toks[i + 2].t === "word") &&
            toks[i + 2].line === dirLine
          ) {
            const tk2 = toks[i + 2];
            if (tk2.t === "str") {
              // 用预扫记下的**原文模式**（Rust 同样是按原文取模式）；取不到才退回 token 值
              // ——例如用户自己写了 `\\.`，那 token 值本身就是对的。
              const rawPat = pre.patterns.has(tk2.pos) ? pre.patterns.get(tk2.pos) : tk2.v;
              line += 're:"' + rawPat + '" ';
            } else {
              // 裸词形式 `re:pattern`（Rust 接受；`re:` 豁免「路径必须加引号」）
              line += "re:" + tk2.v + " ";
            }
            i += 3;
            lastWasAs = false;
            continue;
          }
          if (tk.t === ":") break;                       // 同一行里的下一个键（块内 include）
          if (tk.t === "@" || tk.t === "}" || tk.t === "]") break;
          if (tk.t === "word" && lastWasAs) { line += tk.v; i++; lastWasAs = false; continue; }
          if (tk.t === "word" && importMode) { line += tk.v; i++; lastWasAs = false; continue; }
          if (tk.t === "word" && !lastWasAs) break;       // 其它裸词（键名）停下
          break;
        }
        const resolved = resolveIncludes(line, dirPos, importMode);
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
      // 键位置上的裸 `:`（P1 欠账，与 Rust / C 对齐）：`::` 被词法切成两个 `:`，
      // 第二个 `:` 落到**键位置**。此前把它当键名 ⇒ `X { a: B::C }` 静默得到
      // {"a":"B",":":"C"}（键被改、值被拆、**还不报错** —— 典型静默数据损坏）。
      // Rust 对同一输入报 E-PARSE-006，此处补齐同码；文案点明 `::` 与「加引号」。
      if (peek() && peek().t === ":") {
        fail("E-PARSE-006", "sml: 键位置出现裸冒号 `:`（`::` 会被切成两个 `:`）；含 `:` 的标量请加引号，如 a: \"B::C\"");
      }
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
            args.push(peek().t === "str" ? coerceStr(peek().v, fragments, feats) : coerceWord(peek().v, fragments, nsMap, feats));
            i++;
          }
          if (peek() && peek().t === "{") {
            i++;
            pushKey(key);
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
                // D（任务 D）：块级多契约「响亮拒绝」—— args 里出现第 2 个已注册契约词
                // 即报 E-CONTRACT-017（单契约 `server web prod` 的 web/prod 不是契约，不受影响）。
                for (const a of args) {
                  const an = a in contracts ? a : nsPrefix + a in contracts ? nsPrefix + a : null;
                  if (an) fail("E-CONTRACT-017", "sml: 该实现暂不支持一个块应用多份契约（块级多契约写法）");
                }
                const c = contracts[cname];
                applyDefaults(c, sub);
                const errs = checkContract(contracts, c, sub, "");
                if (errs) fail(errs[0].code, "contract: " + cname + " — " + errs.map((x) => x.msg).join("; "));
                recordContract(cname);
              }
            }
            popPath();
            setField(key, sub);
            continue;
          }
        }
      }
      const nxt = peek();
      if (nxt && nxt.t === "{") {
        i++;
        pushKey(key);
        const subObj = parseBlock("}");
        popPath();
        setField(key, subObj);
      } else if (nxt && nxt.t === "[") {
        i++;
        pushKey(key);
        const subArr = parseArray();
        popPath();
        setField(key, subArr);
      } else if (nxt && (nxt.t === "word" || nxt.t === "str")) {
        setField(key, nxt.t === "str" ? coerceStr(nxt.v, fragments, feats) : coerceWord(nxt.v, fragments, nsMap, feats));
        i++;
      } else if (colon) {
        setField(key, null);
      } else {
        setField(key, coerceWord(key, fragments, nsMap, feats));
      }
    }
    // 块**未闭合**（遇到文件结尾）必须报错。
    //
    // 此前循环因 token 流耗尽而退出后直接 `return node` ⇒ `basic { a: 1`（缺 `}`）被
    // **静默接受**，而 Rust / C / C++ / Lua 全部报 `E-PARSE-001`。后果最重的一环是
    // **编辑器诊断**（它走 `parseSafe`）因此看不见这类错误 —— 用户只会觉得"不报错但也不对"。
    // 数组路径早就有这个检查（见 `parseArray` 的 `closed` 标志），块路径漏了。
    //
    // ⚠️ 判据必须是 `closed` 而**不是** `i >= toks.length`：正常闭合的块消费掉 `}` 之后
    // `i` 也恰好等于长度，用后者会把**合法文档**误判成未闭合。
    if (closing != null && !closed) {
      fail("E-PARSE-001", "sml: 未闭合的块（遇到文件结尾，缺少结束符号 " + closing + "）");
    }
    if (appliedContract) {
      const c = contracts[appliedContract];
      if (!c) fail("E-CONTRACT-001", "sml: 应用未定义契约 " + appliedContract);
      applyDefaults(c, node);
      const errs = checkContract(contracts, c, node, "");
      if (errs) fail(errs[0].code, "contract: " + appliedContract + " — " + errs.map((x) => x.msg).join("; "));
      recordContract(appliedContract);
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
        pushIndex(arr.length);
        const elemObj = parseBlock("}");
        popPath();
        arr.push(elemObj);
      } else if (tok.t === "[") {
        // 嵌套数组（`m: [ [ a ] [ b ] ]`）必须递归。
        //
        // 此前缺少这一支，落到 `else break`：数组被**静默截断**，剩下的 `[` 被外层
        // 块解析当成键名 —— `m: [ [ a ] ]` 会得到 `{"m":[],"[":"a"}`：
        // 不报错，但数据是错的。Rust 侧同一输入得到 `{"m":[["a"]]}`。
        // 嵌套深度由入口处的 MAX_PARSE_DEPTH 闸门统一封顶。
        i++;
        pushIndex(arr.length);
        const elemArr = parseArray();
        popPath();
        arr.push(elemArr);
      } else if (tok.t === "word" || tok.t === "str") {
        arr.push(tok.t === "str" ? coerceStr(tok.v, fragments, feats) : coerceWord(tok.v, fragments, nsMap, feats));
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
  let __result;
  if (first && first.t === "[") { i++; __result = parseArray(); }
  else if (first && first.t === "{") { i++; __result = parseBlock("}"); }
  else { __result = parseBlock(null); }
  // opt-in：开启 traceContracts 时返回 { value, contractMap }（contractMap 已按路径
  // 排序、无时间戳）；关闭时**逐字返回旧值**（向后兼容的钉子）。
  if (traceContracts) {
    contractMapEntries.sort((a, b) => cmpPath(a.path, b.path));
    return { value: __result, contractMap: contractMapEntries };
  }
  return __result;
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
