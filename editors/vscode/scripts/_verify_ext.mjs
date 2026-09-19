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
  // include / import 的编辑器导航（跳转 / 悬停 / 补全）依赖它取路径与**行内列区间**
  // ——行为验证在 `_verify_nav.mjs`，这里只钉"确实导出了"。
  "parseIncludeTargets",
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

// —— 命令「声明 ↔ 实现」双向闸门 + 无效贡献点回归闸门 ——
//   ① 声明了却没实现（`contributes.commands` 或菜单里出现、代码却没 registerCommand）
//      ⇒ 用户点下去才炸「command not found」；
//   ② 实现了却没声明（代码 registerCommand、`contributes.commands` 里没有）
//      ⇒ 命令**不会出现在命令面板**（右键菜单仍可能可用，因为菜单不要求声明）；
//   ③ 顶层 `commands` 这个**无效贡献点**必须不存在 —— 本仓库真踩过：6 条命令全写在
//      顶层，VS Code 静默忽略 ⇒ 命令面板里搜不到任何 SML 命令；而旧闸门恰好也在读
//      顶层 `pkg.commands`，等于「校验了一个没人看的键」，于是一路绿灯。
try {
  const pkg = JSON.parse(readFileSync("package.json", "utf-8"));
  check(!("commands" in pkg), "命令声明在 contributes.commands（顶层 commands 是无效键）");
  const declared = (pkg.contributes?.commands || []).map((c) => c.command);
  const menuCmds = Object.values(pkg.contributes?.menus || {}).flat().map((x) => x.command);
  const jsFiles = readdirSync("src").filter((f) => f.endsWith(".js"));
  const src = jsFiles.map((f) => readFileSync(path.join("src", f), "utf-8")).join("\n");
  for (const id of [...new Set([...declared, ...menuCmds].filter(Boolean))]) {
    check(src.includes(`registerCommand("${id}"`), `命令已实现：${id}`);
  }
  for (const id of [...new Set([...src.matchAll(/registerCommand\(\s*"([^"]+)"/g)].map((m) => m[1]))]) {
    check(declared.includes(id), `命令已声明（命令面板可见）：${id}`);
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

// —— 悬浮「块名」与「特殊颜色」所依赖的桥接函数 ——
// 这两条都是用户直接反馈过的：① 悬停块名什么都不显示；② 特殊颜色要能只对语法单元生效。
try {
  const m = await import(pathToFileURL(path.join(ROOT, "src", "sml-parse.mjs")).href);
  const doc = [
    "@contract Server {",
    "  host: str",
    "  port: int default 5432",
    "}",
    "",
    "database {",
    "  primary {",
    "    @is Server",
    "    host: db1.internal",
    "  }",
    "}",
    "# 注释里的 Server 不该被算作契约单元",
  ].join("\n");
  // ① 块名悬停：能给出路径 + 契约 + 契约应用后的结构
  const md = m.blockHoverMarkdown(doc, 6, m.collectContractNames(doc));   // 第 7 行 `primary {`
  check(!!md && md.includes("database.primary"), "块名悬停给出完整路径", md ? md.split("\n")[0] : "返回 null");
  check(!!md && md.includes("Server"), "块名悬停给出它应用的契约");
  check(!!md && md.includes("5432"), "块名悬停给出契约填充后的结构（默认值 5432）");
  // ② 嵌套块也能取到实例（此前只认顶层）
  const inst = m.contractInstance(doc, "Server");
  check(!!inst && inst.path.join(".") === "database.primary", "契约实例支持嵌套块", inst ? inst.path.join(".") : "null");
  // ③ 单元识别
  check(m.detectUnitKind(doc, "Server") === "contract", "识别契约名", String(m.detectUnitKind(doc, "Server")));
  check(m.detectUnitKind(doc, "host") === "key", "识别键名", String(m.detectUnitKind(doc, "host")));
  check(m.detectUnitKind(doc, "没这个词") === null, "普通词不识别为单元");
  // ④ 单元级定位：注释里的同名文字**不得**被算进去
  const occ = m.findUnitOccurrences(doc, "Server", "contract");
  check(occ.length === 2, "契约单元定位只认语法位置（@contract + @is，注释不算）", `命中 ${occ.length} 处`);
  const occText = m.findUnitOccurrences(doc, "Server", "text");
  check(occText.length === 3, "普通词定位把注释里的也算上", `命中 ${occText.length} 处`);
  // ⑤ 字段级：解析 / 悬浮 / 跳转（用户点名「各个字段能不能也有类似支持」）
  // 契约里除 `host` 外都给默认值 / 可选：这样每个块都能通过校验，才取得到「契约填充后的值」。
  // （缺必填字段时契约校验会失败，取不到值 —— 那是设计，不是 bug：宁可少显示也不显示错。）
  const crlf = [
    "@contract Address { city: str }",
    "",
    "@contract Server {",
    "    host: str                              # 必填（默认 required）",
    "    port: int default 5432                 # 缺失时填 5432",
    "    tags: [str] optional",
    "    status: enum [ active standby ] default active",
    "    weight: num min 0 max 100 default 10",
    "    mode: enum(active, off) default active",
    "    address: Address",
    "}",
    "",
    "database {",
    "    primary {",
    "        @is Server",
    "        host: db1.internal",
    "        address { city: Beijing }",   // 行内块：必须**不**影响外层路径（曾多弹一层）
    "    }",
    "    replica {",
    "        @is Server",
    "        host: db2.internal",
    "        port: 5433",
    "        address { city: Shanghai }",
    "    }",
    "}",
  ].join("\r\n");   // ⚠️ 故意用 CRLF：`. ` 不匹配 `\r`，注释剥离曾因此整条失配
  const flds = m.contractFields(crlf, "Server");
  check(flds.length === 7, "CRLF 下契约字段全部解析出来", `解析出 ${flds.length} 个`);
  const port = flds.find((f) => f.name === "port");
  check(!!port && port.type === "int" && port.default === "5432", "字段类型/默认值解析正确",
    port ? `${port.type} default ${port.default}` : "无 port");
  check(!!port && port.comment === "缺失时填 5432", "行尾 `#` 注释解析为字段说明", port ? String(port.comment) : "");
  const status = flds.find((f) => f.name === "status");
  check(!!status && Array.isArray(status.enum) && status.enum.length === 2, "enum [ ... ] 解析为枚举值",
    status ? JSON.stringify(status.enum) : "");
  const weight = flds.find((f) => f.name === "weight");
  check(!!weight && weight.min === "0" && weight.max === "100", "min/max 区间解析正确");
  const mode = flds.find((f) => f.name === "mode");
  check(!!mode && Array.isArray(mode.enum) && mode.enum.join(",") === "active,off", "enum(...) 内联括号形式也解析",
    mode ? JSON.stringify(mode.enum) : "");
  // 声明处悬浮：给规格 + 行尾说明
  const mdDecl = m.fieldHoverMarkdown(crlf, "port", port.line);
  check(!!mdDecl && mdDecl.includes("`int`") && mdDecl.includes("5432") && mdDecl.includes("> 缺失时填 5432"),
    "契约声明处悬浮给出类型/默认值/说明");
  // 数据区悬浮：给规格 + 当前值（replica 里写的是 5433，与默认值不同）
  const keyLine = crlf.split("\r\n").findIndex((l) => /^\s*port:\s*5433\s*$/.test(l));
  const mdUse = m.fieldHoverMarkdown(crlf, "port", keyLine);
  check(!!mdUse && mdUse.includes("5433") && mdUse.includes("显式写的"), "数据区悬浮给出当前值", mdUse ? mdUse.split("\n").pop() : "null");
  // 行内块不得破坏“所在块”的定位（曾把 database 弹掉 ⇒ 路径只剩 ["replica"]、值取不到）
  const rep = m.enclosingBlock(crlf, keyLine);
  const bpi = m.blockPath(crlf, rep.line);
  check(!!bpi && bpi.path.join(".") === "database.replica", "行内块之后路径仍完整", bpi ? bpi.path.join(".") : "null");
  check(m.contractOfBlock(crlf, rep.line, m.collectContractNames(crlf)) === "Server", "找到块应用的契约");
  // 跳转：数据区的键 → 契约里的字段声明
  const fd = m.findFieldDefinition(crlf, "port", keyLine);
  check(!!fd && fd.line === port.line, "数据区的键可跳到契约字段声明", fd ? `第 ${fd.line + 1} 行` : "null");

  // ⑥ HL-cfg 分组能往返（特殊颜色写文件靠它）
  const g = { contract_Server: { words: ["Server"], color: "#ff9f43", unit: "contract" } };
  const back = m.parseSafe(m.stringify(g));
  check(
    back.ok && back.value.contract_Server && back.value.contract_Server.color === "#ff9f43" &&
      back.value.contract_Server.unit === "contract",
    "HL-cfg 分组 stringify→parse 往返一致"
  );
} catch (e) {
  check(false, "块名悬停 / 特殊颜色桥接函数", String(e && e.message));
}

// —— 「已被新版 VS Code 移除的 API」闸门 ——
// 为什么必须有：顶层读一个**已不存在**的枚举成员，会让**整个扩展模块加载失败** ——
// `activate` 从不执行、provider / 命令 / 输出面板全都不注册，而报错只落在「扩展主机」日志里。
// 界面上表现就是「悬停 / 右键菜单 / 特别高亮全都没反应」，与「扩展坏了」无法区分（极难自查）。
// 本仓库真栽过：VS Code **1.138** 的扩展宿主里没有 `vscode.InsertTextFormat`
// （宿主 bundle 里 0 次出现、自带 vscode.d.ts 里无 `enum InsertTextFormat`），而
// `insertTextFormat: vscode.InsertTextFormat.Snippet` 写在模块顶层 ⇒ 从 0.4.1 起
// **一次都没激活成功**（HANDOFF §22.12）。
{
  const banned = [
    ["InsertTextFormat", "1.138 起宿主里已无此枚举；顶层读它会炸掉整个模块 ⇒ 改用数值 2 / 1（见 extension.js 的 INSERT_SNIPPET / INSERT_PLAIN）"],
  ];
  // 先剥掉注释，免得注释里提到被禁名字就误报
  const strip = (t) => t.replace(/\/\*[\s\S]*?\*\//g, "").replace(/(^|[^:])\/\/[^\n]*/g, "$1");
  const files = readdirSync("src").filter((f) => f.endsWith(".js") || f.endsWith(".mjs"));
  for (const [name, why] of banned) {
    const hit = files.filter((f) => new RegExp(`vscode\\.${name}\\b`).test(strip(readFileSync(path.join("src", f), "utf-8"))));
    check(hit.length === 0, `未使用已被移除的 API：vscode.${name}`, hit.length ? `出现在 ${hit.join("、")} —— ${why}` : "");
  }
}

// —— 新增模块的语法闸门（node --check：不执行、只解析）——
for (const f of ["src/special-highlight.js", "src/highlight.js", "src/extension.js"]) {
  const r = spawnSync(process.execPath, ["--check", f], { encoding: "utf-8" });
  check(r.status === 0, `语法检查 ${f}`, r.status === 0 ? "" : (r.stderr || "").split("\n")[0]);
}

console.log(failed === 0 ? "\nEXT VERIFY ALL PASS" : `\n${failed} 项失败`);
process.exit(failed ? 1 : 0);
