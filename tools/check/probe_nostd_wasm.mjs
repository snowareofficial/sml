// 在 node 里跑 amberice 的 no_std wasm 探针。
//
// 判据有两条：
//   1. 模块的 import 表必须为空 —— 一旦链进 std / wasi，就会要求外部导入，这里直接判红；
//   2. `run()` 必须返回 0（非 0 = 探针里第几步失败，码见 rust/nostd-probe/src/lib.rs）。
//
// 另外打印各阶段的**累计分配字节**（bump 指针快照的差值）：这是「这套子集到底要多大的堆」
// 的实测答案，也是判断「no-alloc 档值不值得做」的依据。
import { readFileSync } from "node:fs";

const PHASES = [
  "构造值 + canonical",
  "dump（写 .ambr）",
  "load（解回值树）",
  "get（路径取值）",
  "check（逐段校验）",
  "Store::open + get",
  "vectors::verify_self",
  "签名 + 验签 + 回读",
  "加密 + 解密",
  "SHA-256 / SM3",
];

const path = process.argv[2];
if (!path) {
  console.log("用法: node probe_nostd_wasm.mjs <file.wasm>");
  process.exitCode = 2;
} else {
  const bytes = readFileSync(path);
  const mod = new WebAssembly.Module(bytes);

  const imports = WebAssembly.Module.imports(mod);
  if (imports.length !== 0) {
    console.log("失败：模块仍要求外部导入 ——", JSON.stringify(imports));
    process.exitCode = 3;
  } else {
    const inst = new WebAssembly.Instance(mod, {});
    const code = inst.exports.run();
    const status = code === 0 ? "通过" : `失败（第 ${code} 步）`;
    console.log(`imports = 0, run() = ${code} → ${status}`);

    const arena = inst.exports.arena_len();
    const total = inst.exports.allocated();
    const allocs = inst.exports.alloc_count();
    console.log(`竞技场 ${(arena / 1024).toFixed(0)} KiB，实际用 ${(total / 1024).toFixed(1)} KiB，${allocs} 次分配`);
    console.log("阶段            本阶段字节   累计字节");
    let prev = 0;
    for (let i = 0; i < PHASES.length; i++) {
      const at = inst.exports.mark_of(i);
      if (at < 0) break;
      const delta = at - prev;
      console.log(`${PHASES[i].padEnd(16)}${String(delta).padStart(10)}${String(at).padStart(11)}`);
      prev = at;
    }
    process.exitCode = code;
  }
}
