// SPDX-License-Identifier: MulanPSL-2.0
//! Cross-end writer test: JS `pack()` against `tests/vectors/amberice-container.v1.json`.
//! 跨端写入器测试：JS `pack()` 对拍容器向量。
//!
//! Criteria per case ｜ 每例判据:
//!   1. 往返树一致 —— `load(pack(canonical_text))` 等于 `canonical_text` 代表的树（按 C10 归一）；
//!   2. 文本一致   —— `canonicalText(pack(text))` 回到同一份 canonical 文本；
//!   3. 固定点     —— `pack(canonicalText(pack(text)))` 与 `pack(text)` 逐字节相同；
//!   4. 字节一致   —— `pack(canonical_text)` 与 fixture 的 `ambr_hex` 逐字节相同。
//!      仅对**键序不受 C10 影响**的用例要求：canonical 文本把 `__type`/`__name`/`__featags`
//!      前置，而字节侧保持源序（spec §19.5 L5），故 `typed-block` / `featags-nested`
//!      这两例经文本往返后键序必然不同 —— 它们改由判据 3（固定点）兜住。
//! ｜ Byte identity is only asserted where C10 does not reorder keys; the two metadata
//!   cases are covered by the fixed-point criterion instead.
//!
//! Run ｜ 运行: `node js/test-ambr-write.mjs`（先 `python js/build_ambr_wasm.py`）

import { load } from "./ambr.mjs";
import { AmbrError, CAP_WRITE, loadAmbrWriter } from "./ambr_write.mjs";
import { bytesToHex, c10, diff, expectedOf, hexToBytes, loadFixture } from "./ambr-vectors.mjs";

function fail(lines) {
  for (const l of lines) console.error(`  FAIL ${l}`);
  console.error(`${lines.length} 个失败`);
  process.exitCode = 1;
}

function expectThrow(label, fn, type, pattern) {
  try {
    fn();
  } catch (e) {
    if (e instanceof type && pattern.test(e.message)) {
      console.log(`  ok: ${label} → ${e.message.slice(0, 60)}`);
      return true;
    }
    fail([`${label}：类型或文案不对（${e.name}: ${e.message}）`]);
    return false;
  }
  fail([`${label}：本应抛错却成功`]);
  return false;
}

async function main() {
  const fixture = loadFixture();

  let writer;
  try {
    writer = await loadAmbrWriter();
  } catch (e) {
    console.error(`[ambr-write] 加载 wasm 失败：${e.message}`);
    console.error("  先跑：python js/build_ambr_wasm.py");
    process.exitCode = 2;
    return;
  }

  const caps = writer.caps();
  console.log(
    `[ambr-write] wasm 已加载：caps=0x${caps.toString(16)}` +
      `（READ=${(caps & 1) !== 0} WRITE=${(caps & CAP_WRITE) !== 0}）`
  );
  if ((caps & CAP_WRITE) === 0) {
    console.error("  FAIL 该 wasm 是只读构建（缺 sml feature）—— 写入无从验证");
    process.exitCode = 2;
    return;
  }

  let round = 0;
  let textSame = 0;
  let fixedPoint = 0;
  let bytesEqual = 0;
  const reordered = [];
  const failures = [];

  for (const c of fixture.cases) {
    const id = c.id;
    let packed;
    try {
      packed = writer.pack(c.canonical_text);
    } catch (e) {
      failures.push(`${id}: pack 抛错 ${e.message}`);
      continue;
    }

    // 1) 往返：用**独立的** JS 解码器读回，不让写入器自证
    //    ｜ Round-trip through the standalone JS decoder, not through the writer itself.
    let srcTree;
    try {
      srcTree = load(hexToBytes(c.ambr_hex)); // fixture 原树的键序（字节侧源序）
    } catch (e) {
      failures.push(`${id}: fixture 解码失败 ${e.message}`);
      continue;
    }
    let got;
    try {
      got = load(packed);
    } catch (e) {
      failures.push(`${id}: 解码 pack 出的字节失败 ${e.message}`);
      continue;
    }
    const want = expectedOf(c.canonical_text);
    const d = diff(c10(want), c10(got));
    if (d) failures.push(`${id}: ${d}`);
    else round++;

    // 2) 文本契约 + 3) 固定点
    let canon;
    try {
      canon = writer.canonicalText(packed);
    } catch (e) {
      failures.push(`${id}: canonicalText 抛错 ${e.message}`);
      continue;
    }
    if (canon === c.canonical_text) textSame++;
    else failures.push(`${id}: canonicalText 与 canonical_text 不同`);

    const again = writer.pack(canon);
    if (bytesToHex(again) === bytesToHex(packed)) fixedPoint++;
    else failures.push(`${id}: 重打包不是固定点（${again.length} B vs ${packed.length} B）`);

    // 4) 字节契约：只在文本侧没有发生 C10 键序前置、且 fixture 没有旁路段时要求逐字节相同。
    //    `typed-schema`（第 22 例）的 fixture 额外带 TYPED / SCHEMA 段，而文本→字节
    //    写入器只会产出 VALUE 段 —— 旁路段按规格「未知段可跳过」，不可能也不应该
    //    被复刻，故改由判据 1/2/3（往返 / 文本 / 固定点）兜住。
    //    ｜ Byte identity only where C10 did not reorder keys and the fixture has no
    //    skippable side segments: `typed-schema` carries TYPED/SCHEMA that a text writer
    //    cannot (and per spec need not) reproduce.
    const NO_BYTE_IDENTITY = new Set(["typed-schema"]);
    const orderShifted = diff(want, srcTree) !== null;
    if (orderShifted || NO_BYTE_IDENTITY.has(id)) {
      reordered.push(id);
    } else if (bytesToHex(packed) === c.ambr_hex) {
      bytesEqual++;
    } else {
      failures.push(
        `${id}: 字节与 fixture 不同（${packed.length} B vs ${c.ambr_hex.length / 2} B）`
      );
    }
  }

  const n = fixture.cases.length;
  console.log(
    `[ambr-write] 往返 ${round}/${n}，文本一致 ${textSame}/${n}，` +
      `固定点 ${fixedPoint}/${n}，字节一致 ${bytesEqual}/${n - reordered.length}` +
      (reordered.length ? `（不要求逐字节：${reordered.join(", ")}）` : "")
  );

  if (failures.length) {
    fail(failures);
    return;
  }

  // Negative controls：数据错误是 AmbrError（带 `AMBR:` 前缀），调用方用错类型是 TypeError。
  // ｜ Negative controls: data errors are AmbrError (`AMBR:` prefix); misuse is TypeError.
  const ok = [
    expectThrow("非法 SML", () => writer.pack("a: [ 1"), AmbrError, /^AMBR: /),
    expectThrow("空输入", () => writer.pack(""), AmbrError, /^AMBR: \[E-AMBR-001\] empty input/),
    expectThrow("非字符串", () => writer.pack(42), TypeError, /String/),
    expectThrow("坏字节回读", () => writer.canonicalText(new Uint8Array(40)), AmbrError, /^AMBR: /),
  ].every(Boolean);

  if (ok) console.log("[ambr-write] ALL OK");
}

await main();
