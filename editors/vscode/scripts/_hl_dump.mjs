// 一次性探针：把一段 SML 逐 token 打出来（文本 + 最内层 scope），用来看「哪里没上色」。
// 判定「没上色」= 最内层 scope 只有 source.sml。
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";

const require = createRequire(import.meta.url);
const path = require("node:path");
const tm = require("vscode-textmate");
const onig = require("vscode-oniguruma");

const wasmPath = path.join(path.dirname(require.resolve("vscode-oniguruma")), "onig.wasm");
await onig.loadWASM(readFileSync(wasmPath).buffer);

const here = path.dirname(require.resolve("../package.json"));
const grammarJson = JSON.parse(readFileSync(path.join(here, "syntaxes", "sml.tmLanguage.json"), "utf-8"));

const registry = new tm.Registry({
  onigLib: Promise.resolve({
    createOnigScanner: (s) => new onig.OnigScanner(s),
    createOnigString: (s) => new onig.OnigString(s),
  }),
  loadGrammar: async () => grammarJson,
});
const grammar = await registry.loadGrammar("source.sml");

const src = process.argv[2] ? readFileSync(process.argv[2], "utf-8") : readFileSync(0, "utf-8");
let stack = tm.INITIAL;
const lines = src.split(/\r?\n/);
lines.forEach((line, ln) => {
  const r = grammar.tokenizeLine(line, stack);
  stack = r.ruleStack;
  const plain = [];
  const row = r.tokens.map((t) => {
    const text = line.slice(t.startIndex, t.endIndex);
    const inner = t.scopes[t.scopes.length - 1];
    if (inner === "source.sml" && text.trim()) plain.push(text.trim());
    const mark = inner === "source.sml" && text.trim() ? "!!" : "";
    return `${mark}${JSON.stringify(text)}<${inner}>`;
  });
  console.log(String(ln + 1).padStart(3) + " | " + row.join(" "));
});
