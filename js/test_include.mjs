// test_include.mjs — include / import 的**行为**回归：行级收集、glob、点分模块名、`re:`。
//
// 为什么单开一个（而不并进 probe-error-codes.mjs）：那个脚本是**对 HEAD 老实现**的 A/B
// 对照（它 import `_head_sml.mjs`），**钉不住当前实现**。而 include 这一块的坏法很隐蔽：
// 曾经整条指令把后面几行的内容一起吞进去，结果是 `ok = true`、零诊断，**只是树错了**
// （顶层键丢失 + 命名空间被拼成 `waimportcommon`）。所以这里按「树的形状 + 错误码 +
// 错误位置」三样断言，而不是只断言"没报错"。
//
//   node js/test_include.mjs
import { parseSafe } from "./sml.mjs";

let bad = 0;
const check = (ok, msg, extra = "") => {
  if (!ok) bad++;
  console.log(`${ok ? "ok  " : "FAIL"}  ${msg}${ok || !extra ? "" : "   ← " + extra}`);
};

const FILES = {
  "sub/a.sml": "ka: 1\n",
  "sub/b.sml": "kb: 1\n",
  "sub/widget_x.sml": "kx: 1\n",
  "sub/widget_y.sml": "ky: 1\n",
  "ui/buttons.sml": "bt: 1\n",
  "common.sml": "shared: 1\n",
};
const P = (src, opts) => parseSafe(src, { files: FILES, ...(opts || {}) });
const keys = (v) => Object.keys(v).sort().join(",");

// ① 行级收集：多行指令各自生效，**下一行的键不许被吞**（这就是原 P0）
{
  const r = P('import "sub/a.sml"\nx: 1\nimport "sub/b.sml"\ny: 2\n');
  check(r.ok, "① 多行 include 解析成功", r.error);
  check(
    r.ok && r.value.x === 1 && r.value.y === 2 && r.value.ka === 1 && r.value.kb === 1,
    "① 四行的内容都在（没有把后续行吞进指令）",
    r.ok && keys(r.value)
  );
}

// ② `import "x.sml"` ≡ `include "x.sml"`：带扩展名 ⇒ 顶层内联（不挂命名空间）
{
  const r = P('import "common.sml"\nx: 1\n');
  check(r.ok && r.value.shared === 1 && !r.value.common, '② import "x.sml" 顶层内联', r.ok && keys(r.value));
}

// ③ 点分模块名（`import ui.buttons`）：点当目录层级 + 默认挂命名空间
{
  const r = P("import ui.buttons\n");
  check(
    r.ok && r.value.ui && r.value.ui.buttons && r.value.ui.buttons.bt === 1,
    "③ import ui.buttons ⇒ ui/buttons.sml 且挂 ui.buttons",
    r.ok && JSON.stringify(r.value)
  );
}

// ④ glob：命中多个文件；与已引入的同名键**聚合为数组**（Rust 是文本内联，同语义）
{
  const r = P('@feature enable glob-include\ninclude "sub/widget_*.sml"\nimport "sub/widget_x.sml"\n');
  check(r.ok, "④ glob 解析成功", r.error);
  check(
    r.ok && Array.isArray(r.value.kx) && r.value.kx.length === 2 && r.value.ky === 1,
    "④ glob 命中 2 个 + 同名键聚合为数组",
    r.ok && JSON.stringify(r.value)
  );
}

// ⑤ glob 命中 0 个 ⇒ **静默展开为空**（与 Rust `glob_or_regex_dir` 同语义）
{
  const r = P('@feature enable glob-include\ninclude "sub/nope_*.sml"\nx: 1\n');
  check(r.ok && keys(r.value) === "x", "⑤ glob 0 命中：静默（与 Rust 一致）", r.ok && keys(r.value));
}

// ⑥ `re:` 模式里的 `\.` **照文档写**即可（按原文取模式，不经字符串转义 ⇒ 不报 E-LEX-004）
//    ⚠️ 模式里的**目录部分**决定在哪个目录里找（与 Rust 的 `glob_or_regex_dir` 同口径）
//    —— 裸模式只看「主文件所在目录」，所以这里的 widget 文件在 `sub/` 下，模式也要写 `sub/`。
{
  const r = P('@feature enable regex-include\ninclude re:"sub/widget_.*\\.sml"\n');
  check(r.ok, '⑥ re:"sub/widget_.*\\.sml" 解析成功（不报 E-LEX-004）', r.error);
  check(r.ok && r.value.kx === 1 && r.value.ky === 1, "⑥ 正则命中两个 widget_*", r.ok && keys(r.value));
  const bare = P('@feature enable regex-include\ninclude re:"widget_.*\\.sml"\n');
  check(bare.ok && keys(bare.value) === "",
    "⑥ 裸模式只在主文件所在目录找（不递归到 sub/）", bare.ok && keys(bare.value));
}

// ⑦ 受限正则的语义细节：`^…$` 锚、`\d` 是字面 d、匹配对象是**文件名**、`*` 不当 glob
{
  const r1 = P('@feature enable regex-include\ninclude re:"sub/^widget_x\\.sml$"\n');
  check(r1.ok && keys(r1.value) === "kx", "⑦ ^…$ 只命中 widget_x", r1.ok && keys(r1.value));
  const r2 = P('@feature enable regex-include\ninclude re:"sub/[a-z]+"\n');
  check(r2.ok && keys(r2.value) === "ka,kb,kx,ky", "⑦ 模式匹配的是**文件名**（[a-z]+ 命中的是名字）",
    r2.ok && keys(r2.value));
  const r3 = P('@feature enable regex-include\ninclude re:"\\d"\n');
  check(r3.ok && keys(r3.value) === "", "⑦ \\d 是字面 d（受限正则没有字符类转义）", r3.ok && keys(r3.value));
  const r4 = P('@feature enable glob-include\ninclude "sub/*_x.sml"\n');
  check(r4.ok && keys(r4.value) === "kx", "⑦ glob 正对照仍按 `*` 通配", r4.ok && keys(r4.value));
}

// ⑧ 错误码 + **位置都落在指令那一行**
{
  const cases = [
    ['include re:"widget"\n', "E-FEATURE-001", 0, "正则特性未开"],
    ['@feature enable regex-include\nimport re:"widget_.*\\.sml" { kx }\n', "E-INCLUDE-009", 1, "部分引用不能配正则"],
    ['@feature enable regex-include\ninclude re:"[0-9"\n', "E-PARSE-025", 1, "字符类未闭合"],
    ['@feature enable regex-include\ninclude re:"' + "x".repeat(300) + '"\n', "E-LIMIT-007", 1, "模式过长"],
    ['include "sub/missing.sml"\n', "E-INCLUDE-001", 0, "目标不存在"],
    ['@feature enable glob-include\ninclude "sub/[a-z]*.sml"\n', null, 1, "glob 正对照"],
  ];
  for (const [src, want, line, name] of cases) {
    const r = P(src);
    const got = r.ok ? null : r.code;
    check(got === want, `⑧ ${name} ⇒ ${want}`, got);
    if (want) {
      check(
        r.position && r.position.line === line,
        `⑧ ${name} 的位置落在那条指令上（第 ${line + 1} 行）`,
        JSON.stringify(r.position)
      );
    }
  }
}

console.log(bad === 0 ? "ALL PASS" : `${bad} FAILED`);
process.exit(bad === 0 ? 0 : 1);
