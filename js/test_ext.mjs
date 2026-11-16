// test_ext.mjs — 外置扩展点（ext API）回归（JS 侧）
//
// 钉：① 元数据块不进主树；② emit 合并；③ 外置类型合法/非法（⇒ E-CONTRACT-007）；
//     ④ 注册期校验（与内置同名 ⇒ E-EXT-002）；⑤ 零扩展与旧入口一致。
import { parse } from "./sml.mjs";

let failures = 0;
function check(tag, cond, detail) {
  if (cond) console.log(`  ok: ${tag}`);
  else { console.log(`FAIL: ${tag} — ${detail}`); failures++; }
}

const codeOf = (src, opts) => {
  try { parse(src, opts); return "OK"; }
  catch (e) { return (e && e.code) || "(no code)"; }
};

const ext = {
  // 注：JS 侧的既有约定是回调返回 `{ emit: 对象 }`（C / C++ / Lua 是「直接返回对象」）。
  directives: {
    form: () => {},
    emitme: () => ({ emit: { meta: 42 } }),
  },
  types: {
    image: (v) => (typeof v === "string" ? true : "image 须为附件 id 字符串"),
  },
};

{
  const v = parse("a: 1\n@form { x: 1 }\nb: 2\n", ext);
  check("元数据块不进主树",
    v && v.form === undefined && v.x === undefined && v.a === 1 && v.b === 2,
    JSON.stringify(v));
}
{
  const v = parse("a: 1\n@emitme { }\n", ext);
  check("emit 合并进所在块", v && v.meta === 42, JSON.stringify(v));
}
{
  check("外置类型（合法）",
    codeOf('@contract C { img: image }\nx { @is C img: "att-1" }\n', ext) === "OK",
    "应通过");
}
{
  check("外置类型（不合法）",
    codeOf("@contract C { img: image }\nx { @is C img: 123 }\n", ext) === "E-CONTRACT-007",
    "应报 E-CONTRACT-007");
}
{
  check("E-EXT-002 内置指令同名",
    codeOf("a: 1\n", { directives: { contract: () => {} } }) === "E-EXT-002",
    "应报 E-EXT-002");
}
{
  check("E-EXT-002 内置类型同名",
    codeOf("a: 1\n", { types: { str: () => true } }) === "E-EXT-002",
    "应报 E-EXT-002");
}
{
  check("零扩展与旧入口一致",
    codeOf("a: 1\nb: 2\n", {}) === "OK", "应通过");
}

console.log(failures === 0 ? "\nEXT TESTS PASSED" : `\n${failures} FAILURES`);
process.exit(failures === 0 ? 0 : 1);
