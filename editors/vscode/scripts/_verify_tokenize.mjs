// 发布前实测：用 VSCode 真实的 TextMate 引擎（vscode-textmate + Oniguruma）
// 对 showcase.sml 做 tokenize，验证高亮行为——而不是只用 JS RegExp 测正则。
//
// 断言：
//   1. 词中 @（a@b.c / ops@example.com）不得命中「片段声明」scope
//   2. 行首 @（@base / @is / @contract）必须正常命中声明/调用/指令 scope
//   3. 模式关键字（序列 / class）与字符类（数字 / digit）有色
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";

const require = createRequire(import.meta.url);
const path = require("node:path");
const tm = require("vscode-textmate");
const onig = require("vscode-oniguruma");

const wasmPath = path.join(path.dirname(require.resolve("vscode-oniguruma")), "onig.wasm");
const wasmBin = readFileSync(wasmPath).buffer;
await onig.loadWASM(wasmBin);

const pkgPath = require.resolve("../package.json");
const here = path.dirname(pkgPath);
const grammarPath = path.join(here, "syntaxes", "sml.tmLanguage.json");
const grammarJson = JSON.parse(readFileSync(grammarPath, "utf-8"));

const registry = new tm.Registry({
  onigLib: Promise.resolve({
    createOnigScanner: (sources) => new onig.OnigScanner(sources),
    createOnigString: (s) => new onig.OnigString(s),
  }),
  // 本项目无跨 grammar 依赖：无论 registry 内部请求什么 path，都返回同一份 grammar
  loadGrammar: async () => grammarJson,
});

const grammar = await registry.loadGrammar("source.sml");

if (!grammar) {
  console.error("grammar 加载失败");
  process.exit(1);
}

const src = readFileSync(path.join(here, "..", "..", "showcase.sml"), "utf-8");
const lines = src.split(/\r?\n/);

let failed = 0;
function check(name, cond, detail) {
  if (!cond) failed++;
  console.log(`${cond ? "ok  " : "FAIL"}  ${name}${detail ? "  " + detail : ""}`);
}

/** 对单行 tokenize（INITIAL 栈） */
function tokensOfLine(line) {
  return grammar.tokenizeLine(line, tm.INITIAL).tokens;
}

/** 多行 tokenize（维护 ruleStack），返回每行的 tokens */
function tokenizeLines(lines) {
  let stack = tm.INITIAL;
  return lines.map((line) => {
    const r = grammar.tokenizeLine(line, stack);
    stack = r.ruleStack;
    return r.tokens;
  });
}

/** 找覆盖 needle 子串的 token 的 scopes（可能多个 token 拼成 needle） */
function scopesCovering(line, needle, tokens) {
  const toks = tokens ?? tokensOfLine(line);
  const start = line.indexOf(needle);
  if (start < 0) return null;
  const end = start + needle.length;
  const hit = toks.filter((t) => t.startIndex < end && t.endIndex > start);
  return hit.map((t) => t.scopes.join(" "));
}

// ---- 断言 1：词中 @ 不是片段声明 ----
{
  const s = scopesCovering("    to: a@b.c", "@b");
  check("词中 @b 不含 fragment 声明色", s !== null && !s.join(" ").includes("entity.name.function"), s?.join(" | "));
  const s2 = scopesCovering("    cc: ops@example.com", "@example");
  check("词中 @example 不含 fragment 声明色", s2 !== null && !s2.join(" ").includes("entity.name.function"), s2?.join(" | "));
}

// ---- 断言 2：行首 @ 正常（@ 与名字可能拆成两个 token，检查名字部分）----
{
  const s = scopesCovering("@base {", "base");
  check("行首 @base 的名字命中片段声明色", s !== null && s.join(" ").includes("entity.name.function"), s?.join(" | "));
  const s2 = scopesCovering("@is Contact", "@is");
  check("行首 @is 命中调用色", s2 !== null && s2.join(" ").includes("keyword.control"), s2?.join(" | "));
  const s3 = scopesCovering("@contract User {", "@contract");
  check("行首 @contract 命中声明色", s3 !== null && s3.join(" ").includes("keyword.other.declaration"), s3?.join(" | "));
}

// ---- 断言 2b：@is type(契约名) 类型标注形式 ----
{
  const s = scopesCovering("@is type(办事人)", "type");
  check("@is type(...) 的 type 命中标注关键字色", s !== null && s.join(" ").includes("keyword.other.declaration"), s?.join(" | "));
  const s2 = scopesCovering("@is type(办事人)", "办事人");
  check("@is type(...) 的契约名命中引用色", s2 !== null && s2.join(" ").includes("variable.other.member"), s2?.join(" | "));
  // 裸名形式不受影响
  const s3 = scopesCovering("@is 办事人", "办事人");
  check("@is 裸名形式的契约名仍为引用色", s3 !== null && s3.join(" ").includes("variable.other.member"), s3?.join(" | "));
}

// ---- 断言 3：模式关键字与字符类（必须在 @type 块上下文内测）----
{
  const lines = [
    "@type name: 手机号 {",
    "    序列: [",
    "    { 名: 号, 类: 数字, 次: 4 }",
    "    class: digit",
    "}",
  ];
  const all = tokenizeLines(lines);
  const s = scopesCovering(lines[1], "序列", all[1]);
  check("块内模式关键字「序列」命中 keyword.other.pattern", s !== null && s.join(" ").includes("keyword.other.pattern"), s?.join(" | "));
  const s2 = scopesCovering(lines[3], "digit", all[3]);
  check("块内字符类 digit 命中 support.type.primitive", s2 !== null && s2.join(" ").includes("support.type.primitive"), s2?.join(" | "));
  const s3 = scopesCovering(lines[2], "数字", all[2]);
  check("块内字符类「数字」命中 support.type.primitive", s3 !== null && s3.join(" ").includes("support.type.primitive"), s3?.join(" | "));
}

// ---- 全文 tokenize 不抛异常 ----
{
  let threw = null;
  try {
    let stack = tm.INITIAL;
    for (const line of lines) {
      const r = grammar.tokenizeLine(line, stack);
      stack = r.ruleStack;
    }
  } catch (e) {
    threw = e;
  }
  check(`showcase.sml 全文 tokenize（${lines.length} 行）无异常`, threw === null, threw ? String(threw) : "");
}

console.log(failed === 0 ? "\nALL PASS（Oniguruma 实测）" : `\n${failed} FAILED（Oniguruma 实测）`);
process.exit(failed ? 1 : 0);
