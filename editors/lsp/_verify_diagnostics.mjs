// LSP 诊断验收：**真起服务进程、真走 stdio**，断言两类诊断都发得出来。
//
// 钉三件事（`docs/swsml/spec/weakdoc.md` §3 / §5）：
// 1. `@weakdoc <名>` 找不到 profile ⇒ 发一条 **Information** 且带码 `W-CLI-001`；
// 2. 同名 `.smlex` 在项目的 `.smlex/profiles/` 里 ⇒ **不发**这条提示（发现真生效）；
// 3. 语法错仍是 Error（不能被新加的提示挤掉或改级别）。
//
// 用法：node editors/lsp/_verify_diagnostics.mjs（失败退出码 1）

import { spawn } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const SERVER = path.join(HERE, "server.mjs");

let failures = 0;
function check(ok, label, extra = "") {
  if (!ok) failures++;
  console.log(`  ${ok ? "✓" : "✗ 失败"} ${label}${extra ? "  " + extra : ""}`);
}

/** 起一个服务进程，发若干消息，收集 stdout 原文后退出。 */
function runServer(messages) {
  return new Promise((resolve) => {
    const child = spawn(process.execPath, [SERVER], { stdio: ["pipe", "pipe", "pipe"] });
    let out = "";
    let err = "";
    child.stdout.on("data", (b) => { out += b.toString("utf-8"); });
    child.stderr.on("data", (b) => { err += b.toString("utf-8"); });
    child.on("close", () => resolve({ out, err }));
    for (const msg of messages) {
      const body = JSON.stringify(msg);
      child.stdin.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`);
    }
    // initialize 之后给服务一点时间处理 didOpen，再关 stdin 让它自然退出
    setTimeout(() => child.stdin.end(), 300);
  });
}

function frame(body) {
  return `Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`;
}

const DOC_MISSING = "@version v1\n@feature enable type.weak\n@weakdoc no-such-profile-xyz\nk: 1\n";
const DOC_FOUND = "@version v1\n@feature enable type.weak\n@weakdoc api-docs\nk: 1\n";
const DOC_BAD = "@version v1\nk: {\n";

function initMsg(rootDir) {
  return {
    jsonrpc: "2.0",
    id: 1,
    method: "initialize",
    params: {
      processId: process.pid,
      rootUri: pathToFileURL(rootDir).href,
      capabilities: {},
    },
  };
}

function openMsg(uri, text) {
  return {
    jsonrpc: "2.0",
    method: "textDocument/didOpen",
    params: { textDocument: { uri, languageId: "sml", version: 1, text } },
  };
}

/** 从 stdout 原文里抠出最后一条 publishDiagnostics 的 diagnostics 数组。 */
function lastDiagnostics(out) {
  const idx = out.lastIndexOf("textDocument/publishDiagnostics");
  if (idx < 0) return null;
  const start = out.indexOf("[", idx);
  if (start < 0) return null;
  // 简单的括号配平（诊断里不会有嵌套数组以外的结构）
  let depth = 0;
  for (let i = start; i < out.length; i++) {
    if (out[i] === "[") depth++;
    else if (out[i] === "]") {
      depth--;
      if (depth === 0) {
        try {
          return JSON.parse(out.slice(start, i + 1));
        } catch {
          return null;
        }
      }
    }
  }
  return null;
}

// —— ① 缺 profile ⇒ W-CLI-001（Information=3）——
{
  const dir = mkdtempSync(path.join(tmpdir(), "sml_lsp_missing_"));
  const { out, err } = await runServer([
    initMsg(dir),
    openMsg(pathToFileURL(path.join(dir, "a.sml")).href, DOC_MISSING),
  ]);
  const diags = lastDiagnostics(out);
  check(Array.isArray(diags), "收到了 publishDiagnostics", err.slice(0, 200));
  const hit = (diags || []).find((d) => d.code === "W-CLI-001");
  check(!!hit, "缺 profile ⇒ 发 W-CLI-001", JSON.stringify(diags));
  check(hit && hit.severity === 3, "级别是 Information（不阻断）", String(hit && hit.severity));
  check(hit && /no-such-profile-xyz/.test(hit.message), "文案点名 profile", hit && hit.message);
  // 文档第 3 行是 `@weakdoc`（0 起第 2 行）—— 提示必须落在它上面，不能落到别处
  check(hit && hit.range.start.line === 2, "定位到 @weakdoc 那一行", JSON.stringify(hit && hit.range));
  rmSync(dir, { recursive: true, force: true });
}

// —— ② 项目里有同名 .smlex ⇒ 不发提示 ——
{
  const dir = mkdtempSync(path.join(tmpdir(), "sml_lsp_found_"));
  mkdirSync(path.join(dir, ".smlex", "profiles"), { recursive: true });
  writeFileSync(path.join(dir, ".smlex", "profiles", "api-docs.smlex"), 'name: "api-docs"\n');
  const { out, err } = await runServer([
    initMsg(dir),
    openMsg(pathToFileURL(path.join(dir, "a.sml")).href, DOC_FOUND),
  ]);
  const diags = lastDiagnostics(out);
  check(Array.isArray(diags), "收到了 publishDiagnostics", err.slice(0, 200));
  check(!(diags || []).some((d) => d.code === "W-CLI-001"),
    "profile 存在 ⇒ 零提示", JSON.stringify(diags));
  rmSync(dir, { recursive: true, force: true });
}

// —— ③ 语法错仍是 Error（不被新提示挤掉）——
{
  const dir = mkdtempSync(path.join(tmpdir(), "sml_lsp_bad_"));
  const { out } = await runServer([
    initMsg(dir),
    openMsg(pathToFileURL(path.join(dir, "a.sml")).href, DOC_BAD),
  ]);
  const diags = lastDiagnostics(out);
  check((diags || []).some((d) => d.severity === 1), "语法错仍是 Error", JSON.stringify(diags));
  rmSync(dir, { recursive: true, force: true });
}

console.log(failures === 0 ? "\nLSP DIAGNOSTICS ALL PASS" : `\n${failures} 项失败`);
process.exit(failures ? 1 : 0);
