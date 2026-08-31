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
//   parseSafe(text, opts?)        -> { ok, value|error, position }
//   stringify(v) / dump(v)        -> string   （序列化回 SML，round-trip）
//   契约错误以 message 中 "contract:" 前缀标识，可被 playground 高亮。

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
      while (i < n) {
        if (text[i] === "*" && text[i + 1] === "/") { i += 2; break; }
        i++;
      }
    } else if (c === "_" && text[i + 1] === "*") {
      i += 2;
      while (i < n) {
        if (text[i] === "*" && text[i + 1] === "_") { i += 2; break; }
        i++;
      }
    } else if (c === '"') {
      flush();
      const qStart = i;
      let s = "";
      i++;
      while (i < n) {
        const cc = text[i];
        if (cc === '"') { i++; break; }
        if (cc === "\\" && i + 1 < n) {
          i++;
          const e = text[i];
          if (e === "u") {
            // \u{1F680} 或 \u1F680（4 位十六进制码点）
            if (text[i + 1] === "{") {
              let j = i + 2, hex = "";
              while (j < n && text[j] !== "}") { hex += text[j]; j++; }
              i = j + 1;
              s += String.fromCodePoint(parseInt(hex, 16));
            } else {
              const hex = text.slice(i + 1, i + 5);
              i += 4;
              s += String.fromCodePoint(parseInt(hex, 16));
            }
          } else {
            s += ({ n: "\n", t: "\t", r: "\r", "0": "\0", '"': '"', "\\": "\\" }[e] ?? e);
            i++;
          }
        } else { s += cc; i++; }
      }
      toks.push({ t: "str", v: s, pos: qStart });
    } else if ("(){}[]:,".includes(c)) {
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
  if (/^-?\d+\.?\d*$/.test(w) || /^-?\.\d+$/.test(w) || /^-?\d+\.?\d*[eE][+-]?\d+$/.test(w)) {
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
    default: return true;
  }
}

function applyDefaults(contract, obj) {
  for (const [k, sp] of Object.entries(contract.fields)) {
    if (!(k in obj) && sp.def !== undefined) obj[k] = sp.def;
  }
}

function checkContract(contracts, contract, obj, path) {
  const errs = [];
  for (const [k, sp] of Object.entries(contract.fields)) {
    const full = path ? `${path}.${k}` : k;
    if (!(k in obj)) {
      if (sp.required && sp.def === undefined) errs.push(`契约字段缺失: ${full}`);
      continue;
    }
    const v = obj[k];
    if (sp.type === "contract") {
      const sub = contracts[sp.refName];
      if (!sub) { errs.push(`契约 ${sp.refName} 未定义（字段 ${full}）`); continue; }
      if (v && typeof v === "object" && !Array.isArray(v)) {
        const subErr = checkContract(contracts, sub, v, full);
        if (subErr) errs.push(...subErr);
      } else {
        errs.push(`字段 ${full} 应为主对象（组合契约 ${sp.refName}）`);
      }
      continue;
    }
    if (!valueMatchesType(v, sp)) {
      errs.push(`字段 ${full} 类型错误：期望 ${typeName(sp)}，实得 ${Array.isArray(v) ? "array" : typeof v}`);
      continue;
    }
    if (sp.min !== undefined || sp.max !== undefined) {
      let lo = sp.min, hi = sp.max;
      if (lo !== undefined && v < lo) errs.push(`字段 ${full} 小于最小值 ${lo}`);
      if (hi !== undefined && v > hi) errs.push(`字段 ${full} 大于最大值 ${hi}`);
    }
  }
  if (!contract.loose) {
    for (const k of Object.keys(obj)) {
      if (k === "__type" || k === "__name") continue;
      if (!(k in contract.fields)) {
        errs.push(`契约未声明字段：${path ? path + "." + k : k}`);
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

// 把 "a.b.c" 这样的点分路径在 obj 上建成嵌套块，返回最内层对象
function ensureNsPath(obj, path) {
  let cur = obj;
  for (const part of path.split(".").filter(Boolean)) {
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
      if (keys.length === 0) fail("sml: 键列表不能为空（至少指定一个键）");
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

  const toks = tokenize(text);
  const fragments = new Map();
  const contracts = {};
  const nsMap = {};
  let i = 0;
  const peek = () => toks[i];
  const fail = (msg, pos) => {
    const t = toks[i];
    const p = (typeof pos === "number") ? pos : (t && typeof t.pos === "number" ? t.pos : (toks[toks.length - 1]?.pos ?? 0));
    const e = new Error(msg);
    e.pos = p;
    throw e;
  };

  const feats = collectFeatures(text, baseFeatures);

  function literal() {
    const t = peek();
    if (!t) fail("sml: 期望字面量");
    if (t.t === "str") { i++; return coerceStr(t.v, fragments); }
    if (t.t === "word") { i++; return coerceWord(t.v, fragments, nsMap); }
    fail("sml: 期望字面量, 得 " + t.t);
  }

  function parseFieldSpec() {
    const t = peek();
    if (!t || t.t !== "word") fail("sml: 字段类型期望标识符");
    const typeWord = t.v;
    i++;
    let sp = { type: "any", required: true, def: undefined, min: undefined, max: undefined, enumVals: null, arrInner: null, refName: null };
    if (typeWord === "str") sp.type = "str";
    else if (typeWord === "int") sp.type = "int";
    else if (typeWord === "num") sp.type = "num";
    else if (typeWord === "bool") sp.type = "bool";
    else if (typeWord === "any") sp.type = "any";
    else if (typeWord === "enum") {
      sp.type = "enum";
      sp.enumVals = [];
      if (peek() && peek().t === "(") {
        i++;
        while (peek() && peek().t !== ")") {
          if (peek().t === "word" || peek().t === "str") { sp.enumVals.push(peek().v); i++; }
          else if (peek().t === ",") i++;
          else break;
        }
        if (peek() && peek().t === ")") i++;
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
    } else {
      sp.type = "contract";
      sp.refName = typeWord;
    }
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

  function parseContractBody() {
    const fields = {};
    if (peek() && peek().t === "{") i++; else fail("sml: @contract 后须契约体 { }");
    while (peek() && peek().t !== "}") {
      if (peek().t === "," || peek().t === ";") { i++; continue; }
      if (peek().t !== "word") fail("sml: 契约字段期望名称, 得 " + peek().t);
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
    if (!feats.has("include")) fail("sml: include 未启用（需要 feature 'include'）");
    const targets = parseIncludeTargets(line, feats);
    const results = [];
    for (const tg of targets) {
      let text = files[tg.path];
      if (text === undefined) {
        // 也允许直接用 key（不带扩展名）
        text = files[tg.path.replace(/\.sml$/, "")];
      }
      if (text === undefined) fail("sml: include 目标未找到: " + tg.path);
      const childPrefix = tg.ns ? nsPrefix + tg.ns + "." : nsPrefix;
      const v = parse(text, { files, features: feats, nsPrefix: childPrefix });
      // 部分引用：仅保留指定顶层键（命名空间挂在 ns 下时同样只挑这些）
      let filtered = v;
      if (tg.keys && Array.isArray(tg.keys)) {
        filtered = {};
        for (const k of tg.keys) {
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
      if (node[k] === undefined) node[k] = v;
      else if (Array.isArray(node[k])) node[k].push(v);
      else node[k] = [node[k], v];
    };
    let appliedContract = null;
    while (i < toks.length) {
      const tok = peek();
      if (tok.t === "}" || tok.t === "]") {
        if (closing === tok.t) { i++; break; }
        break;
      }
      if (tok.t === ",") { i++; continue; }
      if (tok.t === "@") {
        i++;
        if (!peek()) fail("sml: @ 后需名称");
        const fname = peek().v;
        if (fname === "version") {
          i++;
          const lit = (peek() && peek().v) ?? "";
          if (lit !== "v1" && lit !== "1") {
            fail("sml: @version 须写作 `@version v1`；`version` 不可作为片段名");
          }
          i++;
          continue;
        }
        if (fname === "feature") {
          // 已在 collectFeatures 处理，这里只需消费掉这一条 @feature 指令的 token：
          // 直到行尾（下一个 @ 指令、块边界、或 , ; 作为语句分隔）
          while (i < toks.length) {
            const tk = toks[i];
            if (tk.t === "@" || tk.t === "}" || tk.t === "]") break;
            if (tk.t === "," || tk.t === ";") { i++; break; }
            i++;
          }
          continue;
        }
        if (fname === "contract") {
          i++;
          const cname = peek() && peek().v;
          if (!cname) fail("sml: @contract 后须契约名");
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
          const cname = peek() && peek().v;
          if (!cname) fail("sml: @is 后须契约名");
          i++;
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
          Object.assign(node, structuredClone(fragments.get(fName)));
          continue;
        }
      }
      if (key === undefined) fail("sml: 期望键");
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
      if (!c) fail("sml: 应用未定义契约 " + appliedContract);
      applyDefaults(c, node);
      const errs = checkContract(contracts, c, node, "");
      if (errs) fail("contract: " + appliedContract + " — " + errs.join("; "));
    }
    return node;
  }

  function parseArray() {
    const arr = [];
    while (i < toks.length) {
      const tok = peek();
      if (tok.t === "]") { i++; break; }
      if (tok.t === ",") { i++; continue; }
      if (tok.t === "{") {
        i++;
        arr.push(parseBlock("}"));
      } else if (tok.t === "word" || tok.t === "str") {
        arr.push(tok.t === "str" ? coerceStr(tok.v, fragments) : coerceWord(tok.v, fragments, nsMap));
        i++;
      } else break;
    }
    return arr;
  }

  const first = peek();
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
