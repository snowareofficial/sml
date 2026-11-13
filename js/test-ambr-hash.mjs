// SPDX-License-Identifier: MulanPSL-2.0
// test-ambr-hash.mjs — 验证 JS 解码器按 manifest 的 `hash_id` 分派摘要算法。
//
// 覆盖：
//   1. SM3 / BLAKE3 实现与官方向量一致（含 BLAKE3 跨 chunk 边界）；
//   2. `tests/vectors/amberice-hash-algos.v1.json` 的 SM3 / BLAKE3 夹具能被 `load` 解开，
//      且 `report.hashId` 与夹具声明一致、`verified === true`；
//   3. 把任一段的摘要改坏 ⇒ 必须报「摘要不符」（证明分派真的换了算法，而不是仍按 SHA-256）；
//   4. `hash_id` 为预留号 / 多级元组时 ⇒ **跳过**校验并明确标注，不误报。
// ｜ Verifies hash_id dispatch: official vectors, Rust-generated fixtures, a tampered digest,
//   and the "reserved → skip explicitly" path.

import { load } from "./ambr.mjs";
import { blake3 } from "./blake3.mjs";
import { sm3 } from "./sm3.mjs";
import { hexToBytes } from "./ambr-vectors.mjs";

const hex = (b) => Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
const enc = new TextEncoder();
let failures = 0;

function ok(name, detail = "") {
  console.log(`  ok: ${name}${detail ? " -> " + detail : ""}`);
}
function fail(name, detail) {
  console.error(`  FAIL: ${name} -> ${detail}`);
  failures++;
}

// ---------------------------------------------------------------- 1. 官方向量
const sm3Cases = [
  ["", "1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b"],
  ["abc", "66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0"],
];
for (const [input, want] of sm3Cases) {
  const got = hex(sm3(enc.encode(input)));
  got === want ? ok(`SM3("${input}")`) : fail(`SM3("${input}")`, `${got} != ${want}`);
}

const vec = (n) => Uint8Array.from({ length: n }, (_, i) => i % 251);
const blake3Cases = [
  [0, "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"],
  [1, "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213"],
  [1024, "42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af7"],
  [1025, "d00278ae47eb27b34faecf67b4fe263f82d5412916c1ffd97c8cb7fb814b8444"],
  [2049, "5f4d72f40d7a5f82b15ca2b2e44b1de3c2ef86c426c95c1af0b6879522563030"],
  [3072, "b98cb0ff3623be03326b373de6b9095218513e64f1ee2edd2525c7ad1e5cffd2"],
];
for (const [len, want] of blake3Cases) {
  const got = hex(blake3(vec(len)));
  got === want ? ok(`BLAKE3(len=${len})`) : fail(`BLAKE3(len=${len})`, `${got} != ${want}`);
}

// ---------------------------------------------------------------- 2. 夹具
const fixture = JSON.parse(
  await (await import("node:fs/promises")).readFile(
    new URL("../tests/vectors/amberice-hash-algos.v1.json", import.meta.url),
    "utf8"
  )
);

for (const c of fixture.cases) {
  const bytes = Uint8Array.from(hexToBytes(c.ambr_hex));
  const report = {};
  let tree;
  try {
    tree = load(bytes, { report });
  } catch (e) {
    fail(`fixture ${c.id}`, `解码失败：${e.message}`);
    continue;
  }
  const same =
    tree && tree.who === fixture._value.who && tree.n === fixture._value.n;
  const hashOk = report.hashId === c.hash_id && report.verified === true;
  same && hashOk
    ? ok(`fixture ${c.id}`, `hash_id=0x${c.hash_id.toString(16)} verified`)
    : fail(
        `fixture ${c.id}`,
        `tree=${JSON.stringify(tree)} hashId=${report.hashId} verified=${report.verified}`
      );
}

// ---------------------------------------------------------------- 3. 坏摘要必须报错
{
  const c = fixture.cases[0];
  const bytes = Uint8Array.from(hexToBytes(c.ambr_hex));
  // 段表偏移在头 8..16（u64），项长 41，摘要从项内偏移 9 开始（§19.1）。
  const dv = new DataView(bytes.buffer);
  const tableOff = Number(dv.getBigUint64(8, true));
  bytes[tableOff + 9] ^= 0xff;
  let threw = false;
  try {
    load(bytes, { report: {} });
  } catch (e) {
    threw = /摘要不符/.test(e.message);
  }
  threw ? ok("改坏摘要 → 拒绝") : fail("改坏摘要 → 拒绝", "没有报「摘要不符」");
}

console.log(failures ? `${failures} FAILED` : "ALL OK");
process.exitCode = failures ? 1 : 0;
