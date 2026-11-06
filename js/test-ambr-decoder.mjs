// SPDX-License-Identifier: MulanPSL-2.0
//! Cross-end decoder test: JS `load()` against `tests/vectors/amberice-container.v1.json`.
//! 跨端解码器测试：JS `load()` 对拍容器向量。
//!
//! Pass criterion ｜ 判据: the decoded tree must equal the tree of `canonical_text`
//! (the same text every other end is asked to reproduce). Scalar cases cannot be parsed
//! standalone (top-level scalars are `E-PARSE-008`), so they are wrapped in `__v: …` first.
//! ｜ 标量用例先包一层 `__v:` 再解析（顶层标量不可往返）。
//!
//! Run ｜ 运行: `node js/test-ambr-decoder.mjs`

import fs from "node:fs";
import { load } from "./ambr.mjs";
import { parse } from "./sml.mjs";

const FX = new URL("../tests/vectors/amberice-container.v1.json", import.meta.url);

function expectedOf(text) {
  try {
    return parse(text);
  } catch (e) {
    // Top-level scalar: wrap it in a key so the parser accepts it.
    // 顶层标量：包一层键再解析。
    return parse("__v: " + text).__v;
  }
}

// C10: the *text* side moves metadata keys to the front, while the *byte* side keeps
// source order (spec §19.5 L5). Normalise both sides before comparing, otherwise every
// typed block / featag case fails on key position alone.
// C10：文本侧把元数据键前置，字节侧保持源序（§19.5 L5）⇒ 对拍前两边都按 C10 归一化。
const META = ["__type", "__name", "__args", "__featags"];

function c10(v) {
  if (Array.isArray(v)) return v.map(c10);
  if (v && typeof v === "object") {
    const out = {};
    for (const k of META) {
      if (Object.prototype.hasOwnProperty.call(v, k)) out[k] = c10(v[k]);
    }
    for (const k of Object.keys(v)) {
      if (!META.includes(k)) out[k] = c10(v[k]);
    }
    return out;
  }
  return v;
}

function describe(v) {
  if (v === null) return "null";
  if (Array.isArray(v)) return `array(${v.length})`;
  if (typeof v === "object") return `object(${Object.keys(v).length})`;
  if (typeof v === "number") return `number(${Object.is(v, -0) ? "-0" : v})`;
  return `${typeof v}(${JSON.stringify(v)})`;
}

/// Strict deep equality: number identity via Object.is (so `-0` ≠ `0`), key order matters.
/// 严格深比较：数字用 Object.is（区分 -0），对象键序也比。
function diff(want, got, path) {
  if (typeof want === "number" && typeof got === "number") {
    return Object.is(want, got) ? null : `${path}: 期望 ${describe(want)}，实得 ${describe(got)}`;
  }
  if (want === null || got === null || typeof want !== "object" || typeof got !== "object") {
    if (want === got) return null;
    return `${path}: 期望 ${describe(want)}，实得 ${describe(got)}`;
  }
  const isArr = Array.isArray(want);
  if (isArr !== Array.isArray(got)) {
    return `${path}: 期望 ${describe(want)}，实得 ${describe(got)}`;
  }
  if (isArr) {
    if (want.length !== got.length) {
      return `${path}: 长度 ${want.length} vs ${got.length}`;
    }
    for (let i = 0; i < want.length; i++) {
      const d = diff(want[i], got[i], `${path}[${i}]`);
      if (d) return d;
    }
    return null;
  }
  const wk = Object.keys(want);
  const gk = Object.keys(got);
  if (wk.length !== gk.length || wk.some((k, i) => k !== gk[i])) {
    return `${path}: 键序/键集不同\n    期望 ${JSON.stringify(wk)}\n    实得 ${JSON.stringify(gk)}`;
  }
  for (const k of wk) {
    const d = diff(want[k], got[k], `${path}.${k}`);
    if (d) return d;
  }
  return null;
}

function hexToBytes(hex) {
  if (hex.length % 2 !== 0) throw new Error("hex 长度必须是偶数");
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) {
    out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

const fixture = JSON.parse(fs.readFileSync(FX, "utf8"));
if (fixture.format !== "amberice/container-vectors") {
  console.error(`[!] 向量文件格式不对：${fixture.format}`);
  process.exit(2);
}

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
  const d = diff(want, c10(got), "$");
  if (d) failures.push(`${c.id}: ${d}`);
  else pass++;
}

console.log(`[ambr] ${pass}/${fixture.cases.length} 例通过`);
if (failures.length) {
  for (const f of failures) console.error(`  FAIL ${f}`);
  console.error(`${failures.length} 个失败`);
  process.exit(1);
}
console.log("[ambr] ALL OK");
