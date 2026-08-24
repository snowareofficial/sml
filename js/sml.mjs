// SML — Soup Markup Language (JavaScript 实现, ESM)
//
// 纯 JS、零依赖，Node ≥14 与浏览器均可用。语法与 Soup 生态的
// lib/sml.soup (Lua) 及 sml-rs (Rust) 对齐：
//   裸词字符串 / 引号串（转义 + $env 内联）/ true/false/null / 数字 /
//   块 key { } / 裸块 type name { } / 数组 [ ] / 逗号可选 / # 注释 /
//   @name { } 片段定义 & 引用。
//
// API:
//   parse(text)   -> value | throws        （解析 SML 文本）
//   parseSafe(text) -> { ok, value|error }  （安全版，不抛异常）
//   stringify(v)  -> string                 （序列化回 SML，round-trip）
//   dump(v)       -> string                 （同 stringify）

// ---------------------------------------------------------------------------
// 词法
// ---------------------------------------------------------------------------

function tokenize(text) {
  const toks = [];
  const n = text.length;
  let i = 0;
  let buf = "";
  const flush = () => {
    if (buf !== "") { toks.push({ t: "word", v: buf }); buf = ""; }
  };
  while (i < n) {
    const c = text[i];
    if (c === "#") {
      while (i < n && text[i] !== "\n") i++;
    } else if (c === '"') {
      flush();
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
      toks.push({ t: "str", v: s });
    } else if ("{}[]:,".includes(c)) {
      flush();
      toks.push({ t: c });
      i++;
    } else if (" \t\n\r".includes(c)) {
      flush();
      i++;
    } else { buf += c; i++; }
  }
  flush();
  return toks;
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

// ---------------------------------------------------------------------------
// 解析（递归下降）
// ---------------------------------------------------------------------------

export function parse(text) {
  const toks = tokenize(text);
  const fragments = new Map();
  let i = 0;
  const peek = () => toks[i];

  function parseBlock(closing) {
    const node = {};
    const setField = (k, v) => {
      if (node[k] === undefined) node[k] = v;
      else if (Array.isArray(node[k])) node[k].push(v);
      else node[k] = [node[k], v];
    };
    while (i < toks.length) {
      const tok = peek();
      if (tok.t === "}" || tok.t === "]") {
        if (closing === tok.t) { i++; break; }
        break;
      }
      if (tok.t === ",") { i++; continue; }
      if (tok.t === "@") {
        // @name [type [name]] { ... } 片段定义
        i++;
        const fname = (peek() && peek().v) ?? (() => { throw new Error("sml: @ 后需片段名"); })();
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
      // key
      const key = (peek() && peek().v);
      if (key === undefined) throw new Error("sml: 期望键");
      i++;
      let colon = false;
      if (peek() && peek().t === ":") { colon = true; i++; }
      // 裸块预扫描: 无冒号且后继是词, 可能 `type name { }`
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
        // key 本身即值（片段引用/裸词）
        setField(key, coerceWord(key, fragments));
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

  return parseBlock(null);
}

export function parseSafe(text) {
  try { return { ok: true, value: parse(text) }; }
  catch (e) { return { ok: false, error: String(e && e.message || e) }; }
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

// Node 直接运行自检: node sml.mjs
if (typeof process !== "undefined" && typeof import.meta !== "undefined" &&
    process.argv[1] && import.meta.url.endsWith(process.argv[1].split(/[\\/]/).pop())) {
  const t = "name: John\nage: 27\naddress: { city: NY }\ntags: [ dev tools ]\n";
  const v = parse(t);
  console.log("parsed:", JSON.stringify(v));
  console.log("stringify:\n" + stringify(v));
  const r = parseSafe("{ unclosed");
  console.log("safe (bad input):", JSON.stringify(r));
}
