// SPDX-License-Identifier: MulanPSL-2.0
//! Cross-end decoder test: JS `load()` against `tests/vectors/amberice-container.v1.json`.
//! 跨端解码器测试：JS `load()` 对拍容器向量。
//!
//! Pass criterion ｜ 判据: the decoded tree must equal the tree of `canonical_text`
//! (the same text every other end is asked to reproduce).
//! ｜ 解出的树必须等于 `canonical_text` 代表的树。
//!
//! Run ｜ 运行: `node js/test-ambr-decoder.mjs`

import { load } from "./ambr.mjs";
import { c10, diff, expectedOf, hexToBytes, loadFixture } from "./ambr-vectors.mjs";

const fixture = loadFixture();

let pass = 0;
const failures = [];

for (const c of fixture.cases) {
  let got;
  try {
    got = load(hexToBytes(c.ambr_hex));
  } catch (e) {
    failures.push(`${c.id}: 解码抛错 ${e.message}`);
    continue;
  }
  const want = c10(expectedOf(c.canonical_text));
  const d = diff(want, c10(got));
  if (d) failures.push(`${c.id}: ${d}`);
  else pass++;
}

console.log(`[ambr] ${pass}/${fixture.cases.length} 例通过`);
if (failures.length) {
  for (const f of failures) console.error(`  FAIL ${f}`);
  console.error(`${failures.length} 个失败`);
  process.exit(1);
}
// Negative control: the digest check must actually fire. Flip one digest byte of the first
// segment (table at 40, digest at 49) and expect a throw.
// ｜ 反向用例：改坏一个摘要字节必须报错，否则「验摘要」是假的。
{
  const bytes = hexToBytes(fixture.cases[0].ambr_hex);
  bytes[49] ^= 0xff;
  let threw = false;
  try {
    load(bytes);
  } catch (e) {
    threw = /摘要不符/.test(e.message);
  }
  if (!threw) {
    console.error("  FAIL 改坏摘要后仍解码成功 —— 校验没生效");
    process.exit(1);
  }
  console.log("  ok: 改坏摘要 → 拒绝");
}

console.log("[ambr] ALL OK");
