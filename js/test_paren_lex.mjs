// 两端一致性对照：用例与 rust/tests/_tmp_paren_probe.rs 逐条相同，
// 输出格式也对齐（ok/ERR + 结果），便于 diff。
import { parseSafe } from "./sml.mjs";

const cases = [
  ["括号裸词值", "备注: (重要)\n公式: f(x)\n"],
  ["enum(逗号)", "@contract D { level: enum(公开, 内部, 机密) }\nd { @is D\n level: 机密 }\n"],
  ["enum(空格)", "@contract D { level: enum(公开 内部 机密) }\nd { @is D\n level: 机密 }\n"],
  ["enum越界", "@contract D { level: enum(公开, 内部, 机密) }\nd { @is D\n level: 绝密 }\n"],
  ["enum[方括号]", "@contract D { level: enum [ 公开 内部 机密 ] }\nd { @is D\n level: 机密 }\n"],
  ["@is type(X)", "@contract 办事人 strict {\n姓名: str\n}\n@is type(办事人)\n姓名: 张三\n"],
  ["@is type(X) 违规", "@contract 办事人 strict {\n姓名: str\n手机: str\n}\n@is type(办事人)\n姓名: 张三\n"],
  ["@is type 原名", "@contract type strict {\n姓名: str\n}\n@is type\n姓名: 张三\n"],
  ["契约名含括号", "@contract 办事人 strict {\n姓名: str\n}\n@is 办事人\n姓名: 张三\n"],
];

for (const [name, src] of cases) {
  const r = parseSafe(src);
  const shown = r.ok ? JSON.stringify(r.value) : r.error;
  console.log(`${r.ok ? "ok  " : "ERR "}  ${name.padEnd(16)} ${shown}`);
}
