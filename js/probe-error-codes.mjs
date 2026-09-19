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
  // [源码, 期望码或 null（null = 应当解析成功）, 可选 parse 选项]
  ["@version v9\nk: 1\n", "E-FEATURE-004"],
  // 未知名特性：此前 JS 静默加入集合（只有 Rust 报 E-FEATURE-003）—— 跨端差异，现已对齐
  ["@feature enable nosuch\nk: 1\n", "E-FEATURE-003"],
  ["server { @is Nope }\n", "E-CONTRACT-001"],
  ["@contract S { port: int }\nserver { @is S }\n", "E-CONTRACT-003"],
  ["@contract S { port: int }\nserver {\n  @is S\n  port: oops\n}\n", "E-CONTRACT-002"],
  ["@contract S { port: int }\nserver {\n  @is S\n  port: 1\n  extra: 2\n}\n", "E-CONTRACT-004"],
  ["@contract S { ratio: num min 0 max 1 }\nserver {\n  @is S\n  ratio: 2\n}\n", "E-CONTRACT-005"],
  ["include \"nope.sml\"\n", "E-INCLUDE-001"],
  // 空键列表：解析目标列表在 parse() 作用域之外，曾抛宿主 ReferenceError
  // （fail is not defined）而不是带码错误 —— W14 修。
  ["include \"x.sml\" as w { }\n", "E-INCLUDE-005"],
  // W16 余额①：未定义片段引用原先被当普通键（静默退化成字符串），现与 Rust 同码
  ["k: &nope\n", "E-INCLUDE-006"],
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
  // 未闭合**块**：此前 `parseBlock` 循环因 EOF 退出后直接 return（数组路径早有检查）⇒
  // `basic { a: 1`（缺 `}`）被**静默接受**，而 Rust / C / C++ / Lua 全报 E-PARSE-001。
  // 后果最重的是**编辑器诊断**（走 parseSafe）看不见这类错误。现补齐。
  ["basic { a: 1\n", "E-PARSE-001"],
  ["x { y { \n", "E-PARSE-001"],         // 嵌套块未闭合
  ["a { b: 1 }\n", null],                // 正对照：正常闭合的块（`}` 恰为末 token）不许误判
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
  // 特性名白名单的正对照：JS 别名（top-array / bareword-str / escape）与 Rust 的
  // 15 个注册名**并集**才算合法 —— 否则会把既有合法文档误报成"未知特性"
  ["@feature enable top-array\nk: 1\n", null],
  ["@feature enable bareword-str, escape\nk: 1\n", null],   // 逗号分隔也要认
  ["@feature disable env, contract\nk: 1\n", null],
  // —— W16 余额②：未注册指令（位置参数 / 无片段体）⇒ E-PARSE-005 ——
  ["@foo bar { x: 1 }\n", "E-PARSE-005"],
  ["@foo bar\n", "E-PARSE-005"],
  ["@f type: { x: 1 }\n", "E-PARSE-020"],       // 显式参数后缺取值
  ["@f type: A type: B { }\n", "E-PARSE-020"],  // 同一参数重复
  // 正对照：`@foo { }`（无参数带体）是**合法片段定义**，不许被一律判 005 误伤
  ["@foo { x: 1 }\n", null],
  ["@f { x: 1 }\ny: &f\n", null],
  ["@f type: Server name: prod { x: 1 }\ny: &f\n", null],
  // —— W16 余额③：特性门控（此前 env / contract / fragment 全无门控，
  //      只有 include 做了 —— 把 parser 当沙箱用时，「关掉」其实没关）——
  ["k: &f\n", "E-FEATURE-001", { features: ["include"] }],          // fragment 关闭
  ["@f { x: 1 }\n", "E-FEATURE-001", { features: ["include"] }],    // 片段定义需 fragment
  ["@contract C { x: int }\n", "E-FEATURE-001", { features: ["include"] }], // 契约未启用
  ["a { @is C }\n", "E-FEATURE-001", { features: ["include"] }],    // @is 同属 contract
  ["k: $env.NOPE_X\n", "E-FEATURE-002", { features: ["include"] }], // env 关闭（码是 002 不是 001）
  // 正对照：把特性开着就不许误伤
  ["k: $env.NOPE_X\n", null, { features: ["env"] }],
  // —— W16 余额④：模式匹配的预算。JS 是原生 RegExp，运行时插不进计数器，
  //      故预算落在**编译期**（最坏展开估算，见 sml.mjs 的 compilePatternToRe）——
  ["@type name: T { 序列: [ { 组: { 类: 数字, 次: \"+\" }, 次: \"+\" } ] }\n@contract C { x: T }\n",
   "E-LIMIT-002"],                                     // `(a*)*` 形状：1000 × 1000
  ["@type name: T { 序列: [ { 正则: \"^a+$\", 次: \"+\" } ] }\n@contract C { x: T }\n",
   "E-LIMIT-002"],                                     // 内联正则 + 无界量词（经典 ReDoS 形状）
  // 正对照：正常模式（有界量词 / 单独内联正则）必须照常编译
  ["@type name: T { 序列: [ { 类: 数字, 次: 3 } ] }\n@contract C { x: T }\n", null],
  ["@type name: T { 序列: [ { 正则: \"^a+$\", 次: 2 } ] }\n@contract C { x: T }\n", null],
  ["@type name: T { 序列: [ { 名: 段, 类: 数字, 次: 4 } { 类: 空白, 次: \"+\" } ] }\n@contract C { x: T }\n", null],
];

let bad = 0;
for (const [src, want, opts] of CASES) {
  let got = null;
  try {
    parse(src, opts);
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
