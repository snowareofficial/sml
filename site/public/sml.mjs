// SPDX-License-Identifier: MulanPSL-2.0
// SML — SNOWARE Markup Language (JavaScript 实现, ESM)
//
// 纯 JS、零依赖，Node ≥14 与浏览器均可用。语法与 Soup 生态的
// lib/sml.soup (Lua) 及 sml-rs (Rust) 对齐：
//   裸词字符串 / 引号串（转义 + $env 内联）/ true/false/null / 数字 /
//   块 key { } / 裸块 type name { } / 数组 [ ] / 逗号可选 /
//   注释：单行 `#` / `--` / `//`，多行 `/* */` 与 `_* *_` /
//   @name { } 片段定义 & 引用 /
//   @contract Name [loose] { ... } 契约定义与 @is Name 契约应用。
//
// API:
//   parse(text)   -> value | throws        （解析 SML 文本）
//   parseSafe(text) -> { ok, value|error }  （安全版，不抛异常）
//   stringify(v)  -> string                 （序列化回 SML，round-trip）
//   dump(v)       -> string                 （同 stringify）
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
          s += ({ n: "\n", t: "\t", r: "\r", "0": "\0", '"': '"', "\\": "\\" }[e] ?? e);
          i++;
        } else { s += cc; i++; }
      }
      toks.push({ t: "str", v: s, pos: qStart });
    } else if ("(){}[]:,".includes(c)) {
      flush();
      toks.push({ t: c, v: c, pos: i });
      i++;
    } else if (c === "?") {
      // 契约字段可选修饰符
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

function coerceWord(w, fragments) {
  if (w === "true") return true;
  if (w === "false") return false;
  if (w === "null") return null;
  const ev = w.match(/^\$env\.(.+)$/);
  if (ev) return (typeof process !== "undefined" && process.env && process.env[ev[1]]) ?? "";
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
  if (ev) return (typeof process !== "undefined" && process.env && process.env[ev[1]]) ?? "";
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

// 把契约中声明的默认值填进 obj（仅当字段缺失且声明了 default）
function applyDefaults(contract, obj) {
  for (const [k, sp] of Object.entries(contract.fields)) {
    if (!(k in obj) && sp.def !== undefined) {
      obj[k] = sp.def;
    }
  }
}

// 递归校验 obj 是否满足 contract；返回 null 表示通过，否则返回错误串
function checkContract(contracts, contract, obj, path) {
  const errs = [];
  for (const [k, sp] of Object.entries(contract.fields)) {
    const full = path ? `${path}.${k}` : k;
    if (!(k in obj)) {
      if (sp.required && sp.def === undefined) {
        errs.push(`契约字段缺失: ${full}`);
      }
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
  // loose 模式允许未声明字段；strict 模式禁止未声明字段
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
// 解析（递归下降）
// ---------------------------------------------------------------------------

export function parse(text) {
  const toks = tokenize(text);
  const fragments = new Map();   // 片段: name -> value
  const contracts = {};          // 契约: name -> { fields, loose }
  let i = 0;
  const peek = () => toks[i];
  const fail = (msg, pos) => {
    const t = toks[i];
    const p = (typeof pos === "number") ? pos : (t && typeof t.pos === "number" ? t.pos : (toks[toks.length - 1]?.pos ?? 0));
    const e = new Error(msg);
    e.pos = p;
    throw e;
  };

  // 读取一个裸词 / 引号串的字面量（不进 fragments &）
  function literal() {
    const t = peek();
    if (!t) fail("sml: 期望字面量");
    if (t.t === "str") { i++; return coerceStr(t.v, fragments); }
    if (t.t === "word") { i++; return coerceWord(t.v, fragments); }
    fail("sml: 期望字面量, 得 " + t.t);
  }

  // 解析一个契约字段规格（消费 `type ? optional default ...` 等），返回 FieldSpec
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
    else     if (typeWord === "enum") {
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
      // 引用其它契约名 -> 组合契约
      sp.type = "contract";
      sp.refName = typeWord;
    }
    // 修饰符: ? / optional / required / default / min / max
    let defaultSet = false;
    while (true) {
      const m = peek();
      if (!m) break;
      if (m.t === "?") { sp.required = false; i++; continue; }
      if (m.t === "word") {
        if (m.v === "optional") { sp.required = false; i++; continue; }
        if (m.v === "required") { sp.required = true; i++; continue; }
        if (m.v === "default") {
          i++;
          sp.def = literal();
          defaultSet = true;
          continue;
        }
        if (m.v === "min") { i++; sp.min = Number(literal()); continue; }
        if (m.v === "max") { i++; sp.max = Number(literal()); continue; }
      }
      break;
    }
    if (defaultSet) sp.required = false;
    return sp;
  }

  // 解析契约体: { field: type ...; field2: ... } 直到 }
  function parseContractBody() {
    const fields = {};
    if (peek() && peek().t === "{") i++; else fail("sml: @contract 后须契约体 { }");
    while (peek() && peek().t !== "}") {
      if (peek().t === "," || peek().t === ";") { i++; continue; }
      if (peek().t !== "word") fail("sml: 契约字段期望名称, 得 " + peek().t);
      const fkey = peek().v; i++;
      if (peek() && peek().t === ":") i++;
      // 字段可能带块: `addr: { city: str }` —— 这里只接受单层 type 规格
      const sp = parseFieldSpec();
      fields[fkey] = sp;
      // 跳过可选尾随逗号/分号
      if (peek() && (peek().t === "," || peek().t === ";")) i++;
    }
    if (peek() && peek().t === "}") i++;
    return fields;
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
        if (fname === "contract") {
          i++;
          const cname = peek() && peek().v;
          if (!cname) fail("sml: @contract 后须契约名");
          i++;
          let loose = false;
          if (peek() && peek().t === "word" && peek().v === "loose") { loose = true; i++; }
          else if (peek() && peek().t === "word" && peek().v === "strict") { loose = false; i++; }
          const fields = parseContractBody();
          contracts[cname] = { fields, loose };
          continue;
        }
        if (fname === "is") {
          i++;
          const cname = peek() && peek().v;
          if (!cname) fail("sml: @is 后须契约名");
          i++;
          appliedContract = cname;
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
          fragments.set(fname, sub);
        }
        continue;
      }
      const key = (peek() && peek().v);
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
            args.push(peek().t === "str" ? coerceStr(peek().v, fragments) : coerceWord(peek().v, fragments));
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
        setField(key, nxt.t === "str" ? coerceStr(nxt.v, fragments) : coerceWord(nxt.v, fragments));
        i++;
      } else if (colon) {
        setField(key, null);
      } else {
        setField(key, coerceWord(key, fragments));
      }
    }
    if (appliedContract) {
      const c = contracts[appliedContract];
      if (!c) fail("sml: 应用未定义契约 " + appliedContract);
      applyDefaults(c, node);
      const errs = checkContract(contracts, c, node, "");
      if (errs) {
        fail("contract: " + appliedContract + " — " + errs.join("; "));
      }
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
        arr.push(tok.t === "str" ? coerceStr(tok.v, fragments) : coerceWord(tok.v, fragments));
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
export function parseSafe(text) {
  try { return { ok: true, value: parse(text) }; }
  catch (e) {
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
      for (const e of v) {
        out.push("\n" + "  ".repeat(indent + 1) + dumpInline(e));
      }
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
}
