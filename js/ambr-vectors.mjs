// SPDX-License-Identifier: MulanPSL-2.0
//! Shared helpers for the `.ambr` cross-end vector tests (decode **and** write).
//! `.ambr` 跨端向量测试的公共件（解码测试与写入测试共用）。
//!
//! Why shared / 为什么要单独一份:
//!   C10 归一化（元数据键前置）与「顶层标量要包一层再解析」都是**规范规则**，
//!   抄两份就会漂移。解码测试与写入测试对拍的是同一份 fixture，规则必须同一份。
//!   ｜ C10 normalisation and the top-scalar wrapping rule are spec rules; one copy only.

import fs from "node:fs";
import { parse } from "./sml.mjs";

export const FIXTURE = new URL("../tests/vectors/amberice-container.v1.json", import.meta.url);

/// C10: the *text* side moves metadata keys to the front, the *byte* side keeps source
/// order (spec §19.5 L5). Normalise both sides before comparing.
/// ｜ C10：文本侧把元数据键前置，字节侧保持源序 ⇒ 对拍前两边都归一化。
const META = ["__type", "__name", "__args", "__featags"];

export function c10(v) {
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

/// The tree `canonical_text` stands for. Top-level scalars cannot be parsed standalone
/// (top-level scalars are `E-PARSE-008`), so they are wrapped in `__v: …` first.
/// ｜ 标量用例先包一层 `__v:` 再解析（顶层标量不可往返）。
export function expectedOf(text) {
  try {
    return parse(text);
  } catch {
    return parse("__v: " + text).__v;
  }
}

export function describe(v) {
  if (v === null) return "null";
  if (Array.isArray(v)) return `array(${v.length})`;
  if (typeof v === "object") return `object(${Object.keys(v).length})`;
  if (typeof v === "number") return `number(${Object.is(v, -0) ? "-0" : v})`;
  return `${typeof v}(${JSON.stringify(v)})`;
}

/// Strict deep equality: number identity via Object.is (so `-0` ≠ `0`), key order matters.
/// 严格深比较：数字用 Object.is（区分 -0），对象键序也比。
export function diff(want, got, path = "$") {
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

export function hexToBytes(hex) {
  if (hex.length % 2 !== 0) throw new Error("hex 长度必须是偶数");
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) {
    out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

export function bytesToHex(b) {
  let s = "";
  for (const x of b) s += x.toString(16).padStart(2, "0");
  return s;
}

/// Load the container fixture, refusing a wrong/missing file loudly.
/// ｜ 读容器向量；格式不对就大声拒绝。
export function loadFixture() {
  const fixture = JSON.parse(fs.readFileSync(FIXTURE, "utf8"));
  if (fixture.format !== "amberice/container-vectors") {
    throw new Error(`向量文件格式不对：${fixture.format}`);
  }
  return fixture;
}
