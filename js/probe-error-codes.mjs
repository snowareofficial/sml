// SPDX-License-Identifier: MulanPSL-2.0
// 「同因同码」冒烟（JS 侧）：错误码挂没挂上 `e.code`。
//
//   node js/probe-error-codes.mjs
//
// 与 `rust/tests/error_codes.rs` 是同一组条件，**期望码必须一致** ——
// 这正是 W10 的意义：各端措辞可以不同，码相同就是同一件事。
//
// 注意 `want === null` 的几条：它们在 JS 侧**本来就静默通过**（见
// `errors/README.md` 的「该报错却静默通过」清单），不是这次改造漏了。
// 那些是 W16「静默清单逐条判定」的对象，判定之前不要在这里假装它们有码。

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
  // JS 侧未定义片段引用被当普通键（Rust 报 E-INCLUDE-006）—— 跨端差异，待 W16 判定
  ["k: &nope\n", null],
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
