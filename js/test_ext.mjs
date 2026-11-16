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

// 外置修饰符（P13）：与 Rust `Modifier{apply,check,check_group}` 同形。
{
  const mods = {
    nonempty: {
      apply(spec, v) {
        if (typeof v !== "string") return "取值须为字符串";
        spec.def = v; // 解析期可改写字段规格（Rust `apply` 同款）
        return null;
      },
      check(spec, v) {
        return typeof v === "string" && v.length > 0 ? null : "不得为空串";
      },
    },
    groupfail: {
      checkGroup(view) {
        return view.owner === "n" ? "钉子：check_group 失败" : null;
      },
    },
  };
  const withMods = { ...ext, modifiers: mods };
  check("修饰符 apply 改写规格",
    codeOf("@contract C { title: str nonempty x }\nd { @is C }\n", withMods) === "OK",
    "apply 设了 default，应通过");
  check("修饰符 check 失败落 011",
    codeOf("@contract C { title: str nonempty x }\nd { @is C title: \"\" }\n", withMods) === "E-CONTRACT-011",
    "应报 E-CONTRACT-011");
  check("修饰符 check_group 失败落 011",
    codeOf("@contract C { n: int groupfail 1 }\nd { @is C n: 1 }\n", withMods) === "E-CONTRACT-011",
    "应报 E-CONTRACT-011");
  check("E-EXT-002 内置修饰符同名",
    codeOf("a: 1\n", { modifiers: { min: () => true } }) === "E-EXT-002",
    "应报 E-EXT-002");
  // 零扩展 = 零影响：未注册修饰符仍走**旧路径**（被当成下一个字段的开始），
  // 于是 `nope` 成了字段名、`1` 成了类型 ⇒ 缺字段 003。不因为接了注册点就改变旧行为。
  check("未注册修饰符保持旧路径",
    codeOf("@contract C { n: int nope 1 }\nd { @is C n: 1 }\n", {}) === "E-CONTRACT-003",
    "应维持未扩展时的既有结果");
}

console.log(failures === 0 ? "\nEXT TESTS PASSED" : `\n${failures} FAILURES`);
process.exit(failures === 0 ? 0 : 1);
