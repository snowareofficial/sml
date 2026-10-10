// probe-multi-contract.mjs —— 「多契约叠加（叠加 / mixin）」现状探针
//
// 目的：把候选写法在当前 JS 实现上**真跑一遍**，得到事实而不是推测。
// 只读探针，不改任何语义。对应 Rust 侧的同族探针 rust/examples/probe_capability.rs
// （H / H3 / M 三格），本文件把同一批写法按**叠加**的需求逐条摊开 ——
// 结论见 TODO.md 的「新增议题：多契约叠加（叠加 / mixin）」。
//
// 用法：
//     node js/probe-multi-contract.mjs
//
// 现状（本次实测，2026-10）：
//     @is tls                 ✅ 正常（基线）
//     @is tls, retry          ❌ 逗号被静默跳过，`retry` 掉进键位置 ⇒ 错树（被严格模式撞出 E-CONTRACT-003）
//     @is tls retry           ❌ 同上（第二个名字掉进键位置）
//     @is <tls, retry>        ❌ E-CONTRACT-001「应用未定义契约 <tls」（`<` 是普通词字符，被粘进词）
//     @<tls, retry>           ❌ E-PARSE-005（不是合法指令）
//     同块两条 @is            ❌ **后者覆盖前者**（tls 被丢），报的却是"未声明字段"，看不出是覆盖
import { fileURLToPath } from "node:url";

const mod = await import(new URL("./sml.mjs", import.meta.url).href);
const parseSafe = mod.parseSafe ?? mod.default?.parseSafe;
const parse = mod.parse ?? mod.default?.parse;

const cases = [
  ["基线 · 单契约 @is tls", `@contract tls { cert_ref: str required }
mail {
  @is tls
  cert_ref: "pem"
}
`],
  ["A · `@is tls, retry`（逗号）", `@contract tls { cert_ref: str }
@contract retry { max_attempts: int default 3 }
mail {
  @is tls, retry
  cert_ref: "pem"
  max_attempts: 5
}
`],
  ["B · `@is tls retry`（空格）", `@contract tls { cert_ref: str }
@contract retry { max_attempts: int default 3 }
mail {
  @is tls retry
  cert_ref: "pem"
}
`],
  ["C · `@is <tls, retry>`（尖括号）", `@contract tls { cert_ref: str }
@contract retry { max_attempts: int default 3 }
mail {
  @is <tls, retry>
  cert_ref: "pem"
}
`],
  ["D · 同块两条 `@is`", `@contract tls { cert_ref: str }
@contract retry { max_attempts: int default 3 }
mail {
  @is tls
  @is retry
  cert_ref: "pem"
  max_attempts: 5
}
`],
  ["E · `@<tls, retry>`（更早的提案写法）", `@contract tls { cert_ref: str }
@contract retry { max_attempts: int default 3 }
sender mail {
  @<tls, retry>
  cert_ref: "pem"
}
`],
  ["F · 叠加后写未声明字段（严格性归谁）", `@contract tls { a: str }
@contract retry { b: int default 3 }
x {
  @is tls, retry
  a: hello
  zzz: 1
}
`],
  ["G · 两个契约同名字段（冲突）", `@contract p1 { a: str }
@contract p2 { a: int }
x {
  @is p1, p2
  a: hello
}
`],
];

let failed = 0;
for (const [label, src] of cases) {
  process.stdout.write(`=== ${label} ===\n`);
  try {
    const out = parseSafe ? parseSafe(src) : { ok: true, value: parse(src) };
    if (out && out.ok === false) {
      process.stdout.write(`[ERR] code=${out.code ?? "-"} ${out.error ?? out.message ?? JSON.stringify(out)}\n`);
      failed++;
    } else {
      process.stdout.write(`[OK] ${JSON.stringify(out.value ?? out)}\n`);
    }
  } catch (e) {
    process.stdout.write(`[THROW] ${e.code ?? "-"} ${e.message}\n`);
    failed++;
  }
  process.stdout.write("\n");
}
process.stdout.write(`共 ${cases.length} 例：失败/报错 ${failed} 例 —— 叠加现状就是「全部不可用」\n`);
