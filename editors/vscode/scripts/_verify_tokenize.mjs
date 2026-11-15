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

const src = readFileSync(path.join(here, "..", "..", "examples", "showcase.sml"), "utf-8");
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

// ---- 断言 2c：裸块头（I-26）----
// ⚠️ 判据必须看**最内层** scope：`scopesCovering` 返回整条 scope 栈，祖先里可能
// 恰好含目标词而形成假通过（实测第一版就假通过了）。
function innerScopesAt(line, needle) {
  const toks = tokensOfLine(line);
  const start = line.indexOf(needle);
  if (start < 0) return null;
  const end = start + needle.length;
  return toks.filter((t) => t.startIndex < end && t.endIndex > start)
             .map((t) => t.scopes[t.scopes.length - 1]);
}
{
  // 块头里的**契约词**（首词的类型词 + 其余被应用的契约）一律引用色，
  // 与 `@is A, B` 同色；`@contract A` 声明处才是 entity.name.type。
  const cases = [
    ["xs: [ A B item {", "A B", "数组内同行"],
    ["    A B item {", "A B", "数组内跨行"],
    ["    A !t tagged {", "A", "featag 区前的首词"],
    ["A B item {", "A", "键位置（首词）"],
  ];
  for (const [line, needle, label] of cases) {
    const s = innerScopesAt(line, needle);
    check(`${label}：块头契约词=引用色`, s !== null && s.join(" ").includes("variable.other.member")
          && !s.join(" ").includes("entity.name.type"), s?.join(" | "));
  }
  // 声明处（@contract A）与应用处（块头 A）必须不同色，否则「声明 vs 应用」分不出来
  const decl = innerScopesAt("@contract A {", "A");
  check("契约声明处=类型色（与应用处不同）", decl !== null && decl.includes("entity.name.type.sml"), decl?.join(" | "));
  // 普通标量数组不该被这条吃掉（没有 `{`）
  const s3 = innerScopesAt("xs: [ a b ]", "a b");
  check("标量数组不吃裸块规则", s3 !== null && !s3.join(" ").includes("entity.name.section"), s3?.join(" | "));
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

// ---- 断言 4：同一行多个字段（「字段组合」/ 内联对象）每个键都要着色 ----
// 回归背景：#key 原先整个 match 被 ^\s* 锚在行首 ⇒ `web { host: a, port: 8080 }` 里
// 只有行首那个词着色，同行的 host / port 全无色（真实引擎实测过）。
{
  const cases = [
    "web { host: example.com, port: 8080 }",     // 逗号分隔
    "web { host: example.com port: 8080 }",      // 无逗号（SML 分隔符可省）
    'address { city: Shanghai  zip: "200120" }', // 语料 showcase_contract.sml 原句
    '{ type: home   number: "212 555-1234" }',   // 语料 showcase.sml 原句
    "m: [ { a: 1, b: 2 } ]",                     // 数组里的内联对象
  ];
  for (const line of cases) {
    const toks = tokensOfLine(line);
    const bad = [];
    const re = /([A-Za-z_\u4e00-\u9fa5][\w.\-\u4e00-\u9fa5]*)\s*:/g;
    let m;
    while ((m = re.exec(line)) !== null) {
      const s = m.index, e = m.index + m[1].length;
      const sc = toks.filter((t) => t.startIndex < e && t.endIndex > s).map((t) => t.scopes[t.scopes.length - 1]);
      if (!sc.includes("variable.other.member.sml")) bad.push(m[1]);
    }
    check(`同行多字段全部着色：${line}`, bad.length === 0, bad.length ? "无色：" + bad.join("、") : "");
  }
  // 负向 1：数组里的裸值不是键
  const s1 = scopesCovering("m: [ hello, world ]", "hello");
  check("数组裸值不着键色", s1 !== null && !s1.join(" ").includes("variable.other.member"), s1?.join(" | "));
  // 负向 2：值里的冒号（URL）不是键
  const s2 = scopesCovering("issuer: https://sso.swebase.cn", "https");
  check("值里的 URL 不着键色", s2 !== null && !s2.join(" ").includes("variable.other.member"), s2?.join(" | "));
  // 负向 3：注释里的 `词:` 不是键（注释规则必须优先）
  const s3 = scopesCovering("# SML: state: NY", "state");
  check("注释里的词不着键色", s3 !== null && !s3.join(" ").includes("variable.other.member"), s3?.join(" | "));
}

// ---- 断言 5：数组里的裸块（必须**多行** tokenize；上下文只在 `[` 之后成立）----
// 回归背景：数组里的 `A {` 曾被 #key 的「行首键」分支抢走（应为裸块头的类型色）；
// `A !t tagged {` 的块名 `tagged` 因词 run 被 featag 区打断而无色。
{
  const doc = [
    "xs: [",
    "    A B item {",
    "        a: 1",
    "    }",
    "    A {",
    "        a: 4",
    "    }",
    "    A !t tagged {",
    "        a: 5",
    "    }",
    "    { plain: 1 }",
    "]",
  ];
  const toks = tokenizeLines(doc);
  const inner = (ln, needle) => {
    const line = doc[ln];
    const start = line.indexOf(needle);
    if (start < 0) return null;
    const end = start + needle.length;
    return toks[ln].filter((t) => t.startIndex < end && t.endIndex > start)
                    .map((t) => t.scopes[t.scopes.length - 1]);
  };
  const tA = inner(1, "A");
  check("多词块头：首词也是契约（引用色，与 B 同色）", tA !== null
        && tA.includes("variable.other.member.sml") && !tA.includes("entity.name.type.sml"), tA?.join(" | "));
  const tB = inner(1, "B");
  check("多词块头：中间的契约词=引用色（与 A 同色）", tB !== null
        && tB.includes("variable.other.member.sml") && !tB.includes("entity.name.type.sml"), tB?.join(" | "));
  const t1n = inner(1, "item");
  check("多词块头：最后一个词是块名（第三种颜色）", t1n !== null
        && t1n.includes("entity.name.section.sml") && !t1n.includes("entity.name.type.sml"), t1n?.join(" | "));
  const t2 = inner(4, "A");
  check("数组内单契约 `A {`：A 是引用色、不是块名", t2 !== null
        && t2.includes("variable.other.member.sml") && !t2.includes("entity.name.section.sml"), t2?.join(" | "));
  const t3 = inner(7, "tagged");
  check("featag 区之后的块名=块名色（不是契约色）", t3 !== null
        && t3.includes("entity.name.section.sml") && !t3.includes("entity.name.type.sml"), t3?.join(" | "));
  const t4 = inner(10, "plain");
  check("数组里的内联对象键仍着键色", t4 !== null && t4.includes("variable.other.member.sml"), t4?.join(" | "));
}

// ---- 断言 6：`@is` 多契约 / 块头三段式逗号 / featag 声明的 default ----
{
  const line = "@is A loose, B";
  const sB = innerScopesAt(line, "B");
  check("@is 多契约：第二份契约名=引用色", sB !== null && sB.includes("variable.other.member.sml"), sB?.join(" | "));
  const sLoose = innerScopesAt(line, "loose");
  check("@is 就地严格度=修饰符色", sLoose !== null && sLoose.includes("storage.modifier.sml"), sLoose?.join(" | "));
  const sComma = innerScopesAt(line, ",");
  check("@is 多契约：逗号=分隔符色", sComma !== null && sComma.includes("punctuation.separator.sml"), sComma?.join(" | "));

  const head = "Service !critical !obey[4], prod {";
  const sHeadComma = innerScopesAt(head, ",");
  check("块头三段式的逗号=分隔符色", sHeadComma !== null && sHeadComma.includes("punctuation.separator.sml"), sHeadComma?.join(" | "));
  const sProd = innerScopesAt(head, "prod");
  check("块头逗号之后的块名=块名色", sProd !== null && sProd.includes("entity.name.section.sml"), sProd?.join(" | "));

  // featag 声明只在契约体内成立（`#featag-decl` 由 `#contract-block` 引入），故多行 tokenize
  const cdoc = ["@contract X {", "    !density: str default compact -> layout.density", "}"];
  const ctoks = tokenizeLines(cdoc);
  const at = (ln, needle) => {
    const line = cdoc[ln];
    const start = line.indexOf(needle);
    if (start < 0) return null;
    const end = start + needle.length;
    return ctoks[ln].filter((t) => t.startIndex < end && t.endIndex > start)
                    .map((t) => t.scopes[t.scopes.length - 1]);
  };
  const sDef = at(1, "default");
  check("featag 声明的 default=修饰符色（不再被并进类型）", sDef !== null && sDef.includes("storage.modifier.sml"), sDef?.join(" | "));
  const sStr = at(1, "str ");
  check("featag 声明的类型仍着类型色", sStr !== null && sStr.includes("support.type.primitive.sml"), sStr?.join(" | "));
  const sColon = at(1, ":");
  check("featag 声明的冒号=分隔符色", sColon !== null && sColon.includes("punctuation.separator.key-value.sml"), sColon?.join(" | "));

  // 契约字段位：自定义类型名（@type 注册）要着类型色，内置类型仍是原始类型色
  const cdoc2 = ["@contract C {", "    at: 时刻", "    port: int", "}"];
  const ct2 = tokenizeLines(cdoc2);
  const at2 = (ln, needle) => {
    const line = cdoc2[ln];
    const start = line.indexOf(needle);
    if (start < 0) return null;
    const end = start + needle.length;
    return ct2[ln].filter((t) => t.startIndex < end && t.endIndex > start)
                  .map((t) => t.scopes[t.scopes.length - 1]);
  };
  const sCustom = at2(1, "时刻");
  check("契约字段位：自定义类型名着类型色", sCustom !== null && sCustom.includes("entity.name.type.sml"), sCustom?.join(" | "));
  const sBuiltin = at2(2, "int");
  check("契约字段位：内置类型仍是原始类型色", sBuiltin !== null && sBuiltin.includes("support.type.primitive.sml"), sBuiltin?.join(" | "));

  // 裸词里的 `-1`（`cn-north-1`）不该被数字规则切出一个数字 token
  const sRegion = innerScopesAt("region: cn-north-1", "cn-north-1");
  check("裸词里的 -1 不被切成数字", sRegion !== null && !sRegion.join(" ").includes("constant.numeric"), sRegion?.join(" | "));
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
