// 模拟扩展的 ensureSml() 加载路径，确认补全/悬浮/跳转/特别高亮依赖的解析器函数都在，
// 并给纯函数（尤其 findOccurrences）上真断言。
//
// 两条纪律：
//  · **失败即退出码 1**（此前只打印 ✗ 却始终 exit 0，于是 `_prepublish.mjs` 里那一步
//    永远显示 ok —— 门槛形同虚设）。
//  · **不依赖当前工作目录**：从脚本自身位置推出扩展根目录再 chdir（此前用
//    `path.resolve("src/…")`，只有「恰好 cd 到扩展目录」时才跑得对）。
import { pathToFileURL, fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";

const ROOT = path.dirname(path.dirname(fileURLToPath(import.meta.url)));   // scripts/ 的上级
process.chdir(ROOT);

let failed = 0;
const check = (ok, label, extra = "") => {
  if (!ok) failed++;
  console.log(`  ${ok ? "✓" : "✗ 失败"} ${label}${extra ? "  " + extra : ""}`);
};

const m = await import(pathToFileURL(path.resolve("src/sml-parse.mjs")).href);
console.log("sml-parse.mjs 加载成功 ✓");
console.log("导出:", Object.keys(m).join(", "));

// 补全 / 悬浮 / 跳转 / 特别高亮依赖的函数
for (const fn of [
  "collectContractNames", "collectFragmentNames", "collectKeys",
  "findDefinition", "contractDeclaration", "contractInstance", "contractHoverMarkdown",
  "findOccurrences", "diagnose", "parseSafe", "stringify",
]) {
  check(typeof m[fn] === "function", `${fn} 可导出`);
}

if (typeof m.parseSafe === "function") {
  const r = m.parseSafe('名称: 测试\n区划代码: "330106"');
  check(r.ok, "parseSafe 试用", r.ok ? "" : String(r.error));
}

// —— findOccurrences 断言（行列从 0 起；字面匹配；wholeWord / 大小写 / 上限）——
if (typeof m.findOccurrences === "function") {
  const eq = (a, b) => JSON.stringify(a) === JSON.stringify(b);
  const f = m.findOccurrences;
  check(eq(f("x\nyx", "x"), [{ line: 0, col: 0, length: 1 }, { line: 1, col: 1, length: 1 }]),
    "跨行定位", JSON.stringify(f("x\nyx", "x")));
  check(eq(f("a b a\nba", "a", { wholeWord: true }),
    [{ line: 0, col: 0, length: 1 }, { line: 0, col: 4, length: 1 }]),
    "wholeWord 排掉 `ba` 里的 a", JSON.stringify(f("a b a\nba", "a", { wholeWord: true })));
  check(f("Ab ab", "ab", { caseSensitive: true }).length === 1, "默认区分大小写");
  check(f("Ab ab", "ab", { caseSensitive: false }).length === 2, "可选不区分大小写");
  check(f("a(b a(b", "a(b").length === 2, "特殊字符按字面（不当正则）");
  check(f("a a a", "a", { max: 2 }).length === 2, "max 上限生效");
  check(f("abc", "").length === 0, "空词返回空");
}

// —— 「声明了却没实现的命令」闸门 ——
// 这类 bug 只在用户点下去时才暴露（command not found）；静态就能查出来，必须查。
try {
  const pkg = JSON.parse(readFileSync("package.json", "utf-8"));
  const declared = [
    ...(pkg.commands || []).map((c) => c.command),
    ...Object.values(pkg.contributes?.menus || {}).flat().map((x) => x.command),
  ].filter(Boolean);
  const jsFiles = readdirSync("src").filter((f) => f.endsWith(".js"));
  const src = jsFiles.map((f) => readFileSync(path.join("src", f), "utf-8")).join("\n");
  for (const id of [...new Set(declared)]) {
    check(src.includes(`registerCommand("${id}"`), `命令已实现：${id}`);
  }
  const whens = Object.values(pkg.contributes?.menus || {})
    .flat()
    .map((x) => x.when || "")
    .join(" ");
  for (const key of new Set(
    (whens.match(/[a-z][A-Za-z0-9]*\.[A-Za-z0-9.]+/g) || []).filter((k) => k.startsWith("sml."))
  )) {
    check(src.includes(`"${key}"`), `上下文键被设置：${key}`);
  }
} catch (e) {
  check(false, "package.json 命令 / 菜单一致性检查", String(e && e.message));
}

// —— 新增模块的语法闸门（node --check：不执行、只解析）——
for (const f of ["src/special-highlight.js", "src/highlight.js", "src/extension.js"]) {
  const r = spawnSync(process.execPath, ["--check", f], { encoding: "utf-8" });
  check(r.status === 0, `语法检查 ${f}`, r.status === 0 ? "" : (r.stderr || "").split("\n")[0]);
}

console.log(failed === 0 ? "\nEXT VERIFY ALL PASS" : `\n${failed} 项失败`);
process.exit(failed ? 1 : 0);
