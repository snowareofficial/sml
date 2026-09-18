// 校验 HL-cfg.sml 能被 SML 解析器正确解析成高亮组
import { readFileSync } from "node:fs";
import { parseSafe } from "../src/sml-parse.mjs";

const RESERVED = new Set([
  "@contract", "@is", "@version", "@feature", "@when", "@for",
  "include", "import", "as", "in", "loose", "strict",
  "str", "int", "num", "bool", "any", "array", "enum",
  "default", "min", "max", "required", "optional",
  "true", "false", "null",
]);

const cfg = `# 示例配置
密级词 {
    words: [ 公开 内部 秘密 机密 绝密 ]
    color: "#ff9f43"
    bold: true
}

部门名 {
    words: [ 公安分局 人社局 城管局 ]
    color: "#4ec9b0"
}

# 故意混入官方关键字，应被忽略
冲突组 {
    words: [ str default 自定义词 ]
    color: "#ff0000"
}
`;

const r = parseSafe(cfg);
if (!r.ok) {
  console.log("配置解析失败 ✗", r.error);
  process.exit(1);
}
console.log("配置解析成功 ✓\n");

let fail = 0;
const groups = {};
const skipped = [];
for (const [name, val] of Object.entries(r.value || {})) {
  if (!val || typeof val !== "object" || Array.isArray(val)) continue;
  const words = Array.isArray(val.words) ? val.words.map(String) : [];
  const kept = [];
  for (const w of words) {
    if (RESERVED.has(w.toLowerCase())) { skipped.push(`${name}.${w}`); continue; }
    kept.push(w);
  }
  groups[name] = { kept, color: val.color, bold: val.bold === true };
  console.log(`组 ${name}: words=[${kept.join(", ")}] color=${val.color} bold=${val.bold === true}`);
}

console.log(`\n被忽略的官方关键字冲突: ${skipped.join(", ")}`);

const checks = [
  ["密级词 保留 5 个中文词", groups["密级词"]?.kept.length === 5],
  ["部门名 保留 3 个词", groups["部门名"]?.kept.length === 3],
  ["冲突组 只剩 自定义词", JSON.stringify(groups["冲突组"]?.kept) === JSON.stringify(["自定义词"])],
  ["str 被拦截", skipped.includes("冲突组.str")],
  ["default 被拦截", skipped.includes("冲突组.default")],
  ["color 正确读取", groups["密级词"]?.color === "#ff9f43"],
  ["bold 正确读取", groups["密级词"]?.bold === true],
];
console.log("");
for (const [label, ok] of checks) {
  if (!ok) fail++;
  console.log(`${ok ? "ok  " : "FAIL"}  ${label}`);
}
console.log(fail === 0 ? "\n全部通过" : `\n${fail} 项失败`);
process.exit(fail ? 1 : 0);
