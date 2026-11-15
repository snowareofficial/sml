/* SML 序列化：值树 → SML 文本（含引号判据与转义）。
   从 js/sml.mjs 抽出，`sml.mjs` 再 re-export，对外 API 不变。 */

// ---------------------------------------------------------------------------
// 序列化
// ---------------------------------------------------------------------------

// 纯数字形态（会被 coerceWord 字面量化）。`0x` / `0b` 前缀不算 —— 与 Rust
// `f64::parse` 不认十六进制对齐（Number("0x20") 是 32，直接判会与 Rust 分叉）。
function looksNumber(s) {
  const q = /^[+-]/.test(s) ? s.slice(1) : s;
  if (/^0[xXbB]/.test(q)) return false;
  if (!/^[0-9.]/.test(q)) return false;
  return !Number.isNaN(Number(s));
}

// 是否需要给字符串加引号 —— 与 Rust `sml-value::text_rules::needs_quote` 同判据：
// 空串 / true·false·null·inf·nan / 纯数字 / 注释前缀 / 破坏语法的字符 /
// 首字符非字母下划线 / 含 [A-Za-z0-9_.-] 之外的字符。
// 改前只判空白与 `:#{}"\\` ⇒ `"42"`、`"true"`、`"007"` 全写成裸词，
// 自己 dump 的文本自己读回来就换了类型（台账 S-05）。
function needsQuote(s) {
  if (s === "") return true;
  if (s === "true" || s === "false" || s === "null" || s === "inf" || s === "nan") return true;
  if (looksNumber(s)) return true;
  if (/^(--|\/\/|\/\*|\*\/|\*|_\*)/.test(s)) return true;
  if (/[ \t\n\r:#{},\[\]"\\/*]/.test(s)) return true;
  if (!/^[A-Za-z_\u0080-\uFFFF]/.test(s)) return true;
  return !/^[A-Za-z0-9_.\-\u0080-\uFFFF]+$/.test(s);
}

// 加引号**同时**要转义：`\\ \" \n \t \r \0`，其余控制字符走 `\u00XX`
// —— 与 Rust `quote_if_needed` 的转义集逐条一致。改前只转义 `\` 与 `"`，
// 真换行 / 真 NUL 会以裸字节落进文本（台账 S-06）。
function escapeStr(s) {
  let out = "";
  for (const ch of s) {
    const c = ch.codePointAt(0);
    if (ch === "\\") out += "\\\\";
    else if (ch === '"') out += '\\"';
    else if (ch === "\n") out += "\\n";
    else if (ch === "\t") out += "\\t";
    else if (ch === "\r") out += "\\r";
    else if (c === 0) out += "\\0";
    else if (c < 0x20 || c === 0x7f) out += "\\u" + c.toString(16).toUpperCase().padStart(4, "0");
    else out += ch;
  }
  return out;
}

function quoteIfNeeded(s) {
  const str = String(s);
  if (!needsQuote(str)) return str;
  return '"' + escapeStr(str) + '"';
}

// `allowHead`：只有**键位置**（`stringify` 的单键还原）才允许写块头
// `Type name { … }`；值位置写了它回读必失败 —— 见下面对象分支的注释。
function dumpValue(v, indent, out, allowHead) {
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
    // **值位置一律写扁平对象**：`__type` / `__name` 作为普通键原样写出。
    //
    // 块头形式（`Type name { … }`）只在**键位置**成立 —— 见 `stringify` 的单键还原。
    // 改前这里无条件写块头，于是 `d: { S: S prod { } }` 回读时报 `E-PARSE-006`
    // （值位置不能写裸块头），整份文档 dump 之后自己读不回来（台账 §4.5d）。
    // 键按字典序：Rust 用 BTreeMap，`__name` < `__type` < 普通键。
    const ty = v.__type;
    const keys = Object.keys(v).sort();
    if (keys.length === 0) { out.push("{}"); return; }
    if (allowHead && ty) {
      out.push(String(ty));
      if (v.__name !== undefined) out.push(" " + String(v.__name));
      out.push(" {");
      for (const k of keys) {
        if (k === "__type" || k === "__name") continue;
        out.push("\n" + "  ".repeat(indent + 1) + k + ": ");
        dumpValue(v[k], indent + 1, out);
      }
    } else {
      out.push("\n" + pad + "{");
      for (const k of keys) {
        out.push("\n" + "  ".repeat(indent + 1) + k + ": ");
        dumpValue(v[k], indent + 1, out);
      }
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
    // 与 `dumpValue` 同口径：值位置**不丢** `__type` / `__name`（它们也是数据）。
    const keys = Object.keys(v).sort();
    return "{ " + keys.map((k) => {
      let vs;
      if (typeof v[k] === "string") vs = quoteIfNeeded(v[k]);
      else if (v[k] === null) vs = "null";
      // 嵌套容器**递归展开**：改前写 `[..]` / `{..}` 占位，回读时 `..` 变成字符串元素
      // ⇒ dump 静默丢数据（`!obey[4]` 的 args 会从 [4] 变成 [".."]）。
      else if (typeof v[k] === "object") vs = dumpInline(v[k]);
      else vs = String(v[k]);
      return k + ": " + vs;
    }).join(", ") + " }";
  }
  return "";
}

export function stringify(v) {
  const out = [];
  if (v && typeof v === "object" && !Array.isArray(v)) {
    const all = Object.keys(v);
    // `{ User = { __type = "User", … } }` collapses back to `User name { … }`;
    // otherwise re-parsing adds one extra layer (`User: User …`).
    // ｜ 单键且键名 == 值的 `__type` 时还原成块头。
    if (all.length === 1) {
      const k = all[0];
      const sub = v[k];
      if (sub && typeof sub === "object" && !Array.isArray(sub) && sub.__type === k) {
        dumpValue(sub, 0, out, true);
        out.push("\n");
        return out.join("");
      }
    }
    // 顶层**本身**就是 typed block（根写作 `User u1 { … }`）：保留块头。
    // 缺这一支时根上的 `__type` / `__name` 会被 `continue` 掉，dump 出来只剩 `name: g`，
    // 回读就丢了类型标注（与 Lua `Sml.dump` 的同名分支对齐）。
    if (v.__type) {
      dumpValue(v, 0, out, true);
      out.push("\n");
      return out.join("");
    }
    for (const k of all) {
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
