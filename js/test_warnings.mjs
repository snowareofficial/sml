// test_warnings.mjs — 解析期诊断（warning）回归（JS 侧）
//
// 钉住 W-PARSE-001（重复键提升数组）与 W-PARSE-002（片段展开覆盖父块键）。
// 值树由 conformance 套件比对；这里只断言诊断码 + 文案 + 位置。
import { parseSafe } from "./sml.mjs";

let failures = 0;
function check(tag, cond, detail) {
  if (cond) {
    console.log(`  ok: ${tag}`);
  } else {
    console.log(`FAIL: ${tag} — ${detail}`);
    failures++;
  }
}

{
  const r = parseSafe("a: 1\na: 2\n");
  const w = r.warnings;
  check("重复键产出 W-PARSE-001",
    r.ok && w.length === 1 && w[0].code === "W-PARSE-001" &&
      Array.isArray(r.value.a) && r.value.a.length === 2,
    JSON.stringify({ ok: r.ok, w }));
}

{
  const src = "@base { x: 1 y: 2 }\nw {\n  x: 9\n  &base\n}\n";
  const r = parseSafe(src);
  const w = r.warnings;
  const line = w[0] && typeof w[0].pos === "number"
    ? src.slice(0, w[0].pos).split("\n").length : null;
  check("片段覆盖产出 W-PARSE-002",
    r.ok && w.length === 1 && w[0].code === "W-PARSE-002" &&
      w[0].message.includes("x") && w[0].message.includes("&base") &&
      r.value.w.x === 1 && line === 4,
    JSON.stringify({ ok: r.ok, w, line, value: r.value }));
}

console.log(failures === 0 ? "\nWARNINGS TESTS PASSED" : `\n${failures} FAILURES`);
process.exit(failures === 0 ? 0 : 1);
