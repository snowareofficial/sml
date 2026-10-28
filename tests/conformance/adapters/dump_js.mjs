// W5：把 .sml 用 **JavaScript 侧** 解析后 dump 成 SML 文本，供跨实现一致性套件比对。
//
// 用法（驱动脚本以 `node dump_js.mjs <file.sml> ...` 调用）：
//   node tests/conformance/adapters/dump_js.mjs <file.sml> ...
//
// 输出协议（与 tools/dump_c.c 同一口径）：
//   "DUMP <路径>\n" + dump 原文       解析成功
//   "ERR  <路径> <错误信息>\n"         解析失败
//
// 两个 JS 侧特有口径：
//   1. 模块路径由**本文件位置**推出（`../../../js/sml.mjs`），不吃本机绝对路径；
//      动态 import 是因为仓库根没有 package.json / "type": "module"。
//   2. JS 的 include 走**虚拟文件表** `parse(text, { files })`，键就是指令里写的那个
//      相对路径字符串。故这里把用例所在目录整棵读进来做成表（键用 `/` 分隔的相对路径），
//      `include "inc/lib.sml"` 才找得到。
import { readFileSync, readdirSync } from "node:fs";
import { join, relative, sep, dirname } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const REPO = fileURLToPath(new URL("../../../", import.meta.url));
const SML_MODULE = pathToFileURL(join(REPO, "js", "sml.mjs")).href;

function collectFiles(root) {
  const out = {};
  const walk = (dir) => {
    for (const ent of readdirSync(dir, { withFileTypes: true })) {
      const p = join(dir, ent.name);
      if (ent.isDirectory()) walk(p);
      else {
        const rel = relative(root, p).split(sep).join("/");
        let text;
        try {
          text = readFileSync(p, "utf8");
        } catch {
          // 目录里可能有被别的进程占住的无关文件（实测 EBUSY）——
          // 读不了就跳过，别让整个用例变成「适配器异常」。
          continue;
        }
        out[rel] = text;
        // 虚拟文件表是**扁平键**（不做相对路径解析），而其它四端按「相对父文件目录」解析。
        // 补一份 basename 别名，让 `inc/chain.sml` 里的 `include "lib.sml"` 也能命中 ——
        // 否则测出来的是本适配器的产物，不是 JS 实现的差异。
        const base = rel.split("/").pop();
        if (!(base in out)) out[base] = text;
      }
    }
  };
  walk(root);
  return out;
}

const { parse, stringify } = await import(SML_MODULE);

for (const path of process.argv.slice(2)) {
  let text;
  try {
    text = readFileSync(path, "utf8");
  } catch (e) {
    process.stdout.write(`ERR ${path} E-IO-001 sml: 读取失败\n`);
    continue;
  }
  const files = collectFiles(dirname(path));
  try {
    const v = parse(text, { files });
    process.stdout.write(`DUMP ${path}\n${stringify(v)}\n`);
  } catch (e) {
    const code = (e && e.code) || "";
    const msg = String((e && e.message) || e);
    process.stdout.write(`ERR ${path} ${code ? code + " " : ""}${msg}\n`);
  }
}
