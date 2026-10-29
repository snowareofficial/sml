// 契约字段符号修饰符迁移（fix/drop-symbolic-contract-modifiers，df6e396）：
// 废弃的契约字段符号修饰符 ?(optional) 与 =(default) 必须**响亮**报 E-PARSE-006
// （与 Rust / Lua / C / C++ 同码）。单词形式 optional / default 仍可用，且
// default 仍回填。每条负向都配正对照，防「把码改成一报到底」误伤合法文档。
//
// 运行：node js/test-contract-modifiers.mjs
// 风格参照 js/test_type_vs_contract.mjs（import { parse, parseSafe } from "./sml.mjs"）。
import { parse, parseSafe } from "./sml.mjs";

let bad = 0;
const ok = (label) => console.log("ok   " + label);
const fail = (label, detail) => {
  bad++;
  console.log("FAIL " + label + (detail ? "  ->  " + detail : ""));
};

// 期望解析抛错，且错误信息以 "E-PARSE-006" 开头
function expectReject(tag, src) {
  try {
    parse(src);
    fail(tag, "解析成功，但未报错（期望 E-PARSE-006）");
  } catch (e) {
    const msg = String((e && e.message) || e);
    if (msg.startsWith("E-PARSE-006")) ok(tag + "  ->  " + msg);
    else fail(tag, "错误码非 E-PARSE-006，实得 " + msg);
  }
}

// 期望解析成功
function expectOk(tag, src) {
  const r = parseSafe(src);
  if (r.ok) ok(tag);
  else fail(tag, r.error);
}

// ① 负向：遗留符号形式 ? 必须报 E-PARSE-006
expectReject("契约遗留 ? 修饰符（应为 optional）",
  "@contract S { x: str? }\nserver { @is S }\n");

// ② 负向：遗留符号形式 = 必须报 E-PARSE-006
expectReject("契约遗留 = 修饰符（应为 default）",
  "@contract S { p: int = 5432 }\nserver { @is S }\n");

// ③ 正对照：迁移后的单词写法必须解析成功
expectOk("契约 optional 等价写法",
  "@contract S { x: str optional }\nserver { @is S }\n");

// ③ 续：default 等价写法必须解析成功，且缺字段时回填默认值到 server.p
{
  const r = parseSafe(
    "@contract S { p: int default 5432 }\nserver { @is S }\n");
  if (!r.ok) {
    fail("契约 default 等价写法（应解析成功）", r.error);
  } else if (r.value && r.value.server && String(r.value.server.p) === "5432") {
    ok("契约 default 等价写法  ->  server.p 回填 5432");
  } else {
    const got = r.value && r.value.server && r.value.server.p;
    fail("契约 default 等价写法 server.p 应回填 5432", "实得 " + JSON.stringify(got));
  }
}

console.log(bad === 0 ? "ALL OK" : bad + " FAILED");
process.exit(bad === 0 ? 0 : 1);
