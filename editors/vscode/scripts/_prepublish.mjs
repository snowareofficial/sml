// 发布前硬门槛：逐功能真实验证，全绿才允许打包 vsix。
//
// 背景：曾出现过「正则用 JS RegExp 验证通过，但 VSCode 实际用的 Oniguruma
// 行为不同」的漏网——发布前必须在**真实引擎**上把功能用一遍。
//
// 用法：node scripts/_prepublish.mjs   （任一步失败即退出码 1，禁止打包）

import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const path = require("node:path");
const here = path.dirname(require.resolve("../package.json"));

const steps = [
  ["grammar 正则层（JS RegExp）", "scripts/_verify_grammar.mjs"],
  ["tokenize 层（Oniguruma 真实引擎）", "scripts/_verify_tokenize.mjs"],
  ["HL-cfg 自定义高亮", "scripts/_verify_hlcfg.mjs"],
  ["扩展 API 与命令一致性", "scripts/_verify_ext.mjs"],
  // ⚠️ 这一步是 2026-09-19 补的：此前从没有人**真跑过 activate()**，于是「顶层读了宿主里
  // 已不存在的 API ⇒ 整个模块加载失败」这类致命错误一路漏到用户机器上（HANDOFF §22.12）。
  ["扩展激活（mock 对齐 VS Code 1.138）", "scripts/_verify_activate.mjs"],
];

// showcase.sml 必须在扩展诊断所用的 JS 引擎上零错误解析
function checkShowcase() {
  const src = readFileSync(path.join(here, "..", "..", "showcase.sml"), "utf-8");
  const r = spawnSync(process.execPath, ["-e", `
    import("./js/sml.mjs").then(({ parseSafe }) => {
      const r = parseSafe(require("node:fs").readFileSync(process.argv[1], "utf-8"));
      if (!r.ok) { console.error(r.error); process.exit(1); }
    });
  `, path.join(here, "..", "..", "showcase.sml")], { cwd: path.join(here, "..", ".."), encoding: "utf-8" });
  return { ok: r.status === 0, out: r.stderr || r.stdout || "" };
}

let failed = 0;
for (const [name, script] of steps) {
  const r = spawnSync(process.execPath, [path.join(here, script)], { encoding: "utf-8" });
  const ok = r.status === 0;
  if (!ok) failed++;
  console.log(`${ok ? "ok  " : "FAIL"}  ${name}`);
  if (!ok) {
    const tail = (r.stdout + r.stderr).split(/\r?\n/).filter((l) => l.includes("FAIL")).slice(0, 4);
    for (const l of tail) console.log("      " + l);
  }
}

{
  const { ok, out } = checkShowcase();
  if (!ok) failed++;
  console.log(`${ok ? "ok  " : "FAIL"}  showcase.sml 在 JS 引擎零错误解析${ok ? "" : "  " + out.slice(0, 160)}`);
}

console.log(failed === 0 ? "\nPREPUBLISH ALL PASS —— 允许打包" : `\n${failed} 项失败 —— 禁止打包`);
process.exit(failed ? 1 : 0);
