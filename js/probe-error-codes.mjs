// SPDX-License-Identifier: MulanPSL-2.0
// 「同因同码」冒烟（JS 侧）：错误码挂没挂上 `e.code`。
//
//   node js/probe-error-codes.mjs
//
// 与 `rust/tests/error_codes.rs` 是同一组条件，**期望码必须一致** ——
// 这正是 W10 的意义：各端措辞可以不同，码相同就是同一件事。
//
// 注意 `want === null` 的两类含义：
//   1) 合法形态的**正对照**（应当解析成功）；
//   2) 仍在 JS 侧静默通过、且**尚未判定**的静默点 —— 那些是 W16 的对象，
//      判定之前不要在这里假装它们有码（用户已裁决「全做」，正在逐条落地）。
// W16 已把 LEX / PARSE 那一批（未闭合字符串与块注释、未知转义、\u 非法、
// 多余的结束符号、闭合符错配、未闭合数组、顶层标量）改成显式报码，见下面的用例。

import { parse, parseSafe } from "./sml.mjs";

const CASES = [
  // [源码, 期望码或 null（null = 本端本就静默）]
  ["@version v9\nk: 1\n", "E-FEATURE-004"],
  // JS 侧未知特性被静默加入集合（Rust 报 E-FEATURE-003）—— 跨端差异，待 W16 判定
  ["@feature enable nosuch\nk: 1\n", null],
  ["server { @is Nope }\n", "E-CONTRACT-001"],
  ["@contract S { port: int }\nserver { @is S }\n", "E-CONTRACT-003"],
  ["@contract S { port: int }\nserver {\n  @is S\n  port: oops\n}\n", "E-CONTRACT-002"],
  ["@contract S { port: int }\nserver {\n  @is S\n  port: 1\n  extra: 2\n}\n", "E-CONTRACT-004"],
  ["@contract S { ratio: num min 0 max 1 }\nserver {\n  @is S\n  ratio: 2\n}\n", "E-CONTRACT-005"],
  ["include \"nope.sml\"\n", "E-INCLUDE-001"],
  // 空键列表：解析目标列表在 parse() 作用域之外，曾抛宿主 ReferenceError
  // （fail is not defined）而不是带码错误 —— W14 修。
  ["include \"x.sml\" as w { }\n", "E-INCLUDE-005"],
  // JS 侧未定义片段引用被当普通键（Rust 报 E-INCLUDE-006）—— 跨端差异，待 W16 判定
  ["k: &nope\n", null],
  // —— W16 落地：LEX / PARSE 的静默点改为显式报码（与 Rust 同码） ——
  ["k: \"abc\n", "E-LEX-001"],          // 未闭合字符串（原先静默吃进文件剩余部分）
  ["k: 1\n/* 未闭合\n", "E-LEX-002"],   // 未闭合块注释
  ["k: 1\n_* 未闭合\n", "E-LEX-003"],   // 未闭合块注释（_* *_）
  ["k: \"a\\qb\"\n", "E-LEX-004"],       // 未知转义（原先原样保留 —— 静默改数据）
  ["k: \"\\u12\"\n", "E-LEX-005"],       // \u 位数不足（原先抛宿主 RangeError，无码）
  ["m: [ } ]\n", "E-PARSE-003"],         // 数组里多余的 }（原先静默得 {"m":[]}）
  ["a { ] }\n", "E-PARSE-002"],          // 闭合符错配（原先静默得 {"a":{}}）
  ["a: 1\n}\n", "E-PARSE-003"],          // 顶层多余的 }（原先静默忽略）
  ["a: [1, 2\n", "E-PARSE-001"],         // 未闭合数组
  ["42\n", "E-PARSE-008"],               // 顶层标量（原先造键 {"42":42}）
  // —— W16 正对照：合法形态**不许被误伤**（null = 应当解析成功） ——
  ["k: \"a\\nb\\t\\\"c\\\\d\"\n", null],
  ["k: \"\\u4e2d\\u{1F680}\"\n", null],
  ["m: [ 1, [2, 3], 4 ]\n", null],
  ["a: 1\nb: 2\n", null],
  ["hello world\n", null],               // 两 token，值可往返 ⇒ 不算顶层标量
  ["42: x\n", null],
  // `@feature` 整行在词法前剥掉：此前靠「遇到 } 就停」猜边界，会把整份文档吞成 {}
  ["@feature enable for\nsvg {\n  w: 1\n}\n", null],
];

let bad = 0;
for (const [src, want] of CASES) {
  let got = null;
  try {
    parse(src);
  } catch (e) {
    got = e.code || null;
  }
  const ok = got === want;
  if (!ok) bad++;
  console.log(`${ok ? "ok  " : "FAIL"}  期望=${want} 实得=${got}  ${JSON.stringify(src)}`);
}

// 深度闸门（E-LIMIT-001）
let deep = "";
for (let i = 0; i < 200; i++) deep += "a { ";
let deepCode = null;
try {
  parse(deep);
} catch (e) {
  deepCode = e.code;
}
if (deepCode !== "E-LIMIT-001") {
  bad++;
  console.log(`FAIL  深度闸门 期望=E-LIMIT-001 实得=${deepCode}`);
} else {
  console.log("ok    深度闸门 E-LIMIT-001");
}

// 报错走的是宿主异常还是带码错误，用 parseSafe 看一眼
const r = parseSafe("server { @is Nope }\n");
if (r.ok || r.code !== "E-CONTRACT-001") {
  bad++;
  console.log(`FAIL  parseSafe 未把码交出来（code=${r.code}）`);
} else {
  console.log("ok    parseSafe 交出了 code");
}

console.log(bad === 0 ? "ALL OK" : `${bad} FAILED`);
process.exit(bad === 0 ? 0 : 1);
