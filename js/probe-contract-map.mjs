// SPDX-License-Identifier: MulanPSL-2.0
// 契约映射 API 冒烟（JS 侧）：traceContracts 开/关、路径段、to_sml golden、017 仍在。
//
//   node js/probe-contract-map.mjs
//
// 与 rust/sml-parse/src/contract_map.rs 的判据一致：
//   · opt-in —— 不开 traceContracts 时 parse 返回值与旧版**逐字一致**（裸值，无 contractMap）；
//   · 路径段用 { key } / { index } 两种对象（不造点分字符串）；
//   · 按路径排序、无时间戳；to_sml（contractMapToSml）与 Rust ContractMap::to_sml 逐字节一致；
//   · JS 仍「响亮拒绝」多契约（E-CONTRACT-017）—— 映射只记**已成功应用的单契约**。

import { parse, contractMapToSml } from "./sml.mjs";

let bad = 0;
const ok = (cond, msg) => { if (cond) console.log("ok    " + msg); else { bad++; console.log("FAIL  " + msg); } };
const K = (s) => ({ key: s });
const I = (n) => ({ index: n });
const pathEq = (a, b) => {
  if (a.length !== b.length) return false;
  for (let k = 0; k < a.length; k++) {
    const x = a[k], y = b[k];
    const xi = x.index !== undefined, yi = y.index !== undefined;
    if (xi !== yi) return false;
    if (xi) { if (x.index !== y.index) return false; } else { if (x.key !== y.key) return false; }
  }
  return true;
};
const namesAt = (map, path) => { for (const e of map) if (pathEq(e.path, path)) return e.names; return null; };

// —— 钉子 1：off ⇒ 返回值与旧版逐字一致（裸值，contractMap 为 undefined）——
{
  const doc = "@contract Server loose { host: str }\ndatabase { primary { @is Server host: h1 } }\n";
  const off1 = parse(doc);
  const off2 = parse(doc, {});
  const off3 = parse(doc, { traceContracts: false });
  ok(off1 && typeof off1 === "object" && off1.contractMap === undefined, "off(无 opts) 返回裸值，contractMap 为 undefined");
  ok(off2 && off2.contractMap === undefined, "off(空 opts) 返回裸值，contractMap 为 undefined");
  ok(off3 && off3.contractMap === undefined, "off(traceContracts:false) 返回裸值，contractMap 为 undefined");
  ok(off1 && off1.database && off1.database.primary && off1.database.primary.host === "h1",
     "off 裸值形状不变（database.primary.host === h1）");
}

// —— 钉子 2：on ⇒ 返回 { value, contractMap }；value 与 off 逐字节相等（JSON 口径）——
{
  const doc = "@contract Server loose { host: str }\ndatabase { primary { @is Server host: h1 } }\n";
  const on = parse(doc, { traceContracts: true });
  const off = parse(doc);
  ok(on && on.value !== undefined && Array.isArray(on.contractMap), "on 返回 { value, contractMap }");
  ok(JSON.stringify(on.value) === JSON.stringify(off), "on.value 与 off 裸值相等（JSON 口径）");
}

// —— 嵌套键路径：database.primary ⇒ [Key database, Key primary] ⇒ [Server] ——
{
  const doc = "@contract Server loose { host: str }\ndatabase { primary { @is Server host: h1 } }\n";
  const { contractMap } = parse(doc, { traceContracts: true });
  ok((namesAt(contractMap, [K("database"), K("primary")]) || []).join(",") === "Server",
     "嵌套键路径 [database primary] ⇒ [Server]");
}

// —— 数组 Index(0)：items[0]/items[1] ⇒ [Key items, Index n] ⇒ [Item] ——
{
  const doc = "@contract Item loose { id: int }\nitems [ { @is Item id: 1 } { @is Item id: 2 } ]\n";
  const { contractMap } = parse(doc, { traceContracts: true });
  ok((namesAt(contractMap, [K("items"), I(0)]) || []).join(",") === "Item", "数组元素 [items 0] ⇒ [Item]");
  ok((namesAt(contractMap, [K("items"), I(1)]) || []).join(",") === "Item", "数组元素 [items 1] ⇒ [Item]");
}

// —— typed-block 路径：`Server primary { }`（typed-block 需 @feature 开）⇒ [Key Server] ⇒ [Server] ——
{
  const doc = "@contract Server loose { host: str }\n@feature enable typed-block\nServer primary { host: h1 }\n";
  const { contractMap } = parse(doc, { traceContracts: true });
  ok((namesAt(contractMap, [K("Server")]) || []).join(",") === "Server", "typed-block 路径 [Server] ⇒ [Server]");
}

// —— 顶层实体 path: []（顶层 @is）——
{
  const doc = "@contract Top loose { a: int }\n@is Top\na: 1\n";
  const { contractMap } = parse(doc, { traceContracts: true });
  ok((namesAt(contractMap, []) || []).join(",") === "Top", "顶层实体 path [] ⇒ [Top]");
}

// —— to_sml golden（contractMapToSml，与 Rust ContractMap::to_sml 逐字节一致）——
{
  ok(contractMapToSml([]) === "[]\n", "空表 to_sml === \"[]\\n\"");
  const m = [
    { path: [K("database"), K("primary")], names: ["Server"] },
    { path: [], names: ["Top"] },
    { path: [K("items"), I(0)], names: ["Item"] },
    { path: [K("123")], names: ["Numeric"] },   // 数字键必须加引号
  ];
  const want = "[\n  { path: [] contracts: [ Top ] }\n  { path: [ \"123\" ] contracts: [ Numeric ] }\n  { path: [ database primary ] contracts: [ Server ] }\n  { path: [ items 0 ] contracts: [ Item ] }\n]\n";
  const got = contractMapToSml(m);
  ok(got === want, "to_sml golden 逐字节相等（含排序/数字键加引号/path []）");
  if (got !== want) console.log("  want=" + JSON.stringify(want) + "\n  got =" + JSON.stringify(got));
  ok(contractMapToSml(m) === contractMapToSml(m), "to_sml 确定性（同输入两次相等）");
}

// —— 端到端：解析出的 map 过 to_sml，与 golden 同形 ——
{
  const doc = "@contract Top loose { a: int }\n@contract Server loose { host: str }\n@is Top\na: 1\ndatabase { primary { @is Server host: h1 } }\n";
  const { contractMap } = parse(doc, { traceContracts: true });
  const want = "[\n  { path: [] contracts: [ Top ] }\n  { path: [ database primary ] contracts: [ Server ] }\n]\n";
  const got = contractMapToSml(contractMap);
  ok(got === want, "端到端 to_sml（解析→序列化）逐字节相等");
  if (got !== want) console.log("  want=" + JSON.stringify(want) + "\n  got =" + JSON.stringify(got));
}

// —— 多契约输入：JS 仍「响亮拒绝」E-CONTRACT-017（开 trace 也在；绝不静默给树）——
{
  const atIs = "@contract A loose { x: int }\n@contract B loose { y: int }\ns { @is A B x: 1 y: 2 }\n";
  let code = null;
  try { parse(atIs, { traceContracts: true }); } catch (e) { code = e.code || null; }
  ok(code === "E-CONTRACT-017", "@is 多契约 ⇒ E-CONTRACT-017（开 trace 也在）");

  const blockLevel = "@contract A loose { x: int }\n@contract B loose { y: int }\n@feature enable typed-block\nA B item { x: 1 y: 2 }\n";
  let code2 = null;
  try { parse(blockLevel, { traceContracts: true }); } catch (e) { code2 = e.code || null; }
  ok(code2 === "E-CONTRACT-017", "块级多契约 ⇒ E-CONTRACT-017（开 trace 也在）");
}

console.log(bad === 0 ? "ALL OK" : (bad + " FAILED"));
process.exit(bad === 0 ? 0 : 1);
