// 校验 grammar：JSON 合法性 + 声明/调用/指令分色 + @type 模式关键字 + 中文支持
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

// 锚定到本脚本所在目录，避免被其它 cwd（如仓库根）调用时找不到 grammar 文件。
const HERE = dirname(fileURLToPath(import.meta.url));
const g = JSON.parse(readFileSync(join(HERE, "..", "syntaxes", "sml.tmLanguage.json"), "utf-8"));
const repo = g.repository;
console.log("JSON OK  scopeName =", g.scopeName, "\n");

const D = repo.directive.patterns;

// 按 comment 关键字定位规则 —— 此前用数组下标（D[0]/D[1]…），一旦在数组中间
// 插入新规则，下标就会整体错位，导致「grammar 其实没问题、脚本却全红」的假失败。
const pick = (pats, key) => {
  const r = pats.find((p) => (p.comment ?? "").includes(key));
  if (!r) throw new Error(`grammar 中找不到 comment 含「${key}」的规则`);
  return r;
};

const rules = {
  "契约声明 @contract": new RegExp(repo["contract-block"].begin),
  "类型声明 @type": new RegExp(repo["type-block"].begin),
  "契约调用 @is（类型标注）": new RegExp(pick(D, "类型标注形式").match),
  "契约调用 @is": new RegExp(pick(D, "调用契约：@is Name").match),
  "指令 @version": new RegExp(pick(D, "版本声明").match),
  "指令 @feature": new RegExp(pick(D, "特性开关").match),
  "指令 @when": new RegExp(pick(D, "条件：@when").begin),
  "指令 @for": new RegExp(pick(D, "循环：@for").match),
  "指令 include": new RegExp(pick(D, "import/include").begin),
  "片段声明 @name": new RegExp(pick(D, "片段声明 @name").match),
  "片段调用 &name": new RegExp(repo["fragment-ref"].match),
  "模式关键字": new RegExp(repo["pattern-keyword"].match),
  "字符类": new RegExp(repo["pattern-class"].match),
  "契约字段": new RegExp(repo["contract-field"].match),
  "枚举": new RegExp(repo["contract-enum"].match),
  "类型": new RegExp(repo["contract-type"].match),
  "修饰符": new RegExp(repo["contract-modifier"].match),
  // `#key` 是多分支（行首 / 紧跟 `{` 或 `,` / 空格分隔的后续键），JS 侧合并成
  // 一个**全局**联合体做镜像检查：只要该行里**任意一个**匹配的组含有期望词，就算命中。
  "数据键名": new RegExp(
    (repo.key.patterns ? repo.key.patterns.map((p) => "(?:" + p.match + ")") : [repo.key.match]).join("|"),
    "g"
  ),
};

const cases = [
  ["@contract 事项 strict {", "契约声明 @contract", "事项"],
  ["@type name: 手机号 {", "类型声明 @type", "手机号"],
  ["@type name: date {", "类型声明 @type", "date"],
  ["@is 事项", "契约调用 @is", "事项"],
  ["@is type(事项)", "契约调用 @is（类型标注）", "事项"],
  ["@version v4", "指令 @version", "v4"],
  ["@feature enable for", "指令 @feature", "for"],
  ["@when $env.DEBUG == \"1\"", "指令 @when", "@when"],
  ["@for h in a b {", "指令 @for", "in"],
  ["include \"ui.sml\" as ui", "指令 include", "include"],
  ["@base {", "片段声明 @name", "base"],
  ["  &base", "片段调用 &name", "base"],
  // 邮箱中的 @ 不应被当作片段声明（这些串不应命中 @ 片段声明规则）
  ["to: a@b.c", "片段声明 @name", "@b", true],
  ["cc: ops@example.com", "片段声明 @name", "@example", true],
  // 模式关键字（中文）
  ["    序列: [", "模式关键字", "序列"],
  ["    类: 数字", "模式关键字", "类"],
  ["    次: 4", "模式关键字", "次"],
  ["    直到: 行尾", "模式关键字", "直到"],
  ["    可选: true", "模式关键字", "可选"],
  // 模式关键字（英文）
  ["    seq: [", "模式关键字", "seq"],
  ["    class: digit", "模式关键字", "class"],
  ["    times: 4", "模式关键字", "times"],
  ["    any-of: [", "模式关键字", "any-of"],
  // 字符类（中英）
  ["    类: 数字", "字符类", "数字"],
  ["    类: 字母", "字符类", "字母"],
  ["    class: digit", "字符类", "digit"],
  ["    class: alpha", "字符类", "alpha"],
  // 契约
  ["名称: str", "契约字段", "名称"],
  ["密级: enum [ 公开 内部 秘密 机密 绝密 ] default 公开", "枚举", "公开 内部 秘密 机密 绝密"],
  ["承诺时限: int min 1 max 90", "类型", "int"],
  ["承诺时限: int min 1 max 90", "修饰符", "min"],
  ["区划代码: \"330106\"", "数据键名", "区划代码"],
  // 同一行多个字段（「字段组合」）：非行首的键也要命中
  ["web { host: a, port: 8080 }", "数据键名", "port"],
  ['address { city: Shanghai  zip: "200120" }', "数据键名", "zip"],
  ["m: [ { a: 1, b: 2 } ]", "数据键名", "b"],
];

let fail = 0;
for (const [src, rule, expect, negated] of cases) {
  const re = rules[rule];
  re.lastIndex = 0;
  const hits = [];
  if (re.global) {
    let x;
    while ((x = re.exec(src)) !== null) {
      hits.push(x);
      if (x[0].length === 0) re.lastIndex++;   // 防零宽匹配死循环
    }
  } else {
    const x = src.match(re);
    if (x) hits.push(x);
  }
  // 全局规则下「任意一个匹配的任意一组含期望词」即算命中（同一行多字段就靠这个）
  let got = "";
  for (const x of hits) {
    for (let i = 0; i < x.length; i++) if ((x[i] || "").includes(expect)) got = x[i];
    if (!got && x[0].includes(expect)) got = x[0];
  }
  const ok = negated ? hits.length === 0 : hits.length > 0 && got.includes(expect);
  if (!ok) fail++;
  const tag = negated ? " (期望不匹配)" : "";
  console.log(`${ok ? "ok  " : "FAIL"}  ${rule.padEnd(18)} ${src.padEnd(52)} -> ${got || (hits[0] ? hits[0][0] : "(none)")}${ok && negated ? " ✓未命中" : ""}`);
}

// 分色断言：声明与调用必须不同 scope
const declScope = repo["contract-block"].beginCaptures["2"].name;
const callScope = pick(D, "调用契约：@is Name").captures["2"].name;
const fragDecl = pick(D, "片段声明 @name").captures["2"].name;
const fragCall = repo["fragment-ref"].captures["2"].name;
const typeDecl = repo["type-block"].beginCaptures["2"].name;
console.log("\nscope 检查:");
console.log(`  契约名 声明=${declScope} 调用=${callScope} ${declScope !== callScope ? "OK" : "FAIL"}`);
console.log(`  片段名 声明=${fragDecl} 调用=${fragCall} ${fragDecl !== fragCall ? "OK" : "FAIL"}`);
console.log(`  类型名 声明=${typeDecl} ${typeDecl === declScope ? "OK(与契约名同类)" : "WARN"}`);
if (declScope === callScope || fragDecl === fragCall) fail++;

console.log(fail === 0 ? "\nALL PASS" : `\n${fail} FAILED`);
process.exit(fail ? 1 : 0);
