// 模拟扩展的 ensureSml() 加载路径，确认补全依赖的解析器能否加载
import { pathToFileURL } from "node:url";
import path from "node:path";

const p = path.resolve("src/sml-parse.mjs");
try {
  const m = await import(pathToFileURL(p).href);
  console.log("sml-parse.mjs 加载成功 ✓");
  console.log("导出:", Object.keys(m).join(", "));

  // 补全依赖这三个收集函数
  for (const fn of ["collectContractNames", "collectFragmentNames", "collectKeys"]) {
    console.log(`  ${fn}: ${typeof m[fn] === "function" ? "可用 ✓" : "缺失 ✗"}`);
  }

  if (typeof m.parseSafe === "function") {
    const r = m.parseSafe('名称: 测试\n区划代码: "330106"');
    console.log("parseSafe 试用:", r.ok ? "ok ✓" : "失败 ✗ " + r.error);
  }
} catch (e) {
  console.log("sml-parse.mjs 加载失败 ✗");
  console.log(e && e.message ? e.message : e);
}
