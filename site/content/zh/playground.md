---
title: "SML Playground"
---

# SML Playground { ❄ }

在浏览器里直接解析 SML —— 支持契约校验、片段继承、include 转义。基于最新 JavaScript 实现（`sml.mjs`，零依赖 ESM）。

<style>
  .pg-wrap { display: flex; gap: 16px; flex-wrap: wrap; }
  .pg-pane { flex: 1 1 360px; min-width: 300px; }
  .pg-pane h3 { margin: 6px 0; color: #0d47a1; }
  textarea#src {
    width: 100%; height: 420px; font-family: "SFMono-Regular", Consolas, monospace;
    font-size: 13px; line-height: 1.5; padding: 10px; border: 1px solid #cfd8dc;
    border-radius: 8px; background: #0d1117; color: #e6edf3; resize: vertical;
    tab-size: 2;
  }
  pre#out {
    width: 100%; height: 420px; overflow: auto; margin: 0; padding: 10px;
    border: 1px solid #cfd8dc; border-radius: 8px; background: #0d1117; color: #e6edf3;
    font-size: 13px; line-height: 1.5; white-space: pre-wrap; word-break: break-word;
  }
  pre#out.error { color: #ff8a80; border-color: #b71c1c; }
  .pg-bar { margin: 8px 0; font-size: 13px; color: #555; }
  .pg-bar .ok { color: #2e7d32; font-weight: 600; }
  .pg-bar .bad { color: #c62828; font-weight: 600; }
  .pg-samples { margin: 10px 0; }
  .pg-samples button {
    background: #1565C0; color: #fff; border: 0; padding: 6px 12px; margin-right: 8px;
    border-radius: 6px; cursor: pointer; font-size: 13px;
  }
  .pg-samples button:hover { background: #0d47a1; }
</style>

<div class="pg-samples">
  <span style="font-size:13px;color:#555;">示例：</span>
  <button data-sample="contract">契约校验</button>
  <button data-sample="fragment">片段继承</button>
  <button data-sample="array">数组/枚举</button>
  <button data-sample="bad">故意出错</button>
</div>

<div class="pg-wrap">
  <div class="pg-pane">
    <h3>SML 源码</h3>
    <textarea id="src" spellcheck="false"></textarea>
  </div>
  <div class="pg-pane">
    <h3>解析结果</h3>
    <pre id="out">（输入左侧内容…）</pre>
  </div>
</div>
<div class="pg-bar" id="bar"></div>

<script type="module">
  import { parse, parseSafe, stringify } from "/sml.mjs";

  const SAMPLES = {
    contract:
`@contract Cfg loose {
    api_key: str
    port:    int default 8080 min 1 max 65535
    debug:   bool default false
    mode:    enum(active, disabled) default active
}
@is Cfg
api_key: re_abc123
port: 9090
debug: true
mode: active`,
    fragment:
`@base { region: cn-north-1 timeout: 30 }
service auth { &base port: 7100 name: auth-svc }
service billing { &base port: 7200 name: billing-svc }
features: [ logging metrics tracing ]
database: {
    url: "postgres://localhost/app"
    pool: { min: 2 max: 16 }
}`,
    array:
`@contract U { kind: enum(active,disabled) tags: array[str] ? }
@is U
kind: active
tags: [ a b c ]`,
    bad:
`@contract Cfg strict { api_key: str port: int }
@is Cfg
api_key: re_xxx
port: 99999
extra: oops`,
  };

  const src = document.getElementById("src");
  const out = document.getElementById("out");
  const bar = document.getElementById("bar");

  function render() {
    const text = src.value;
    const r = parseSafe(text);
    if (r.ok) {
      out.classList.remove("error");
      out.textContent = JSON.stringify(r.value, null, 2);
      bar.innerHTML = '<span class="ok">✓ 解析成功</span> · ' +
        Object.keys(flatten(r.value)).length + " 个叶子字段";
    } else {
      out.classList.add("error");
      let msg = r.error;
      if (r.position) msg += `  (行 ${r.position.line + 1}, 列 ${r.position.col + 1})`;
      out.textContent = "✗ " + msg;
      bar.innerHTML = '<span class="bad">✗ 解析失败</span>';
    }
  }

  function flatten(o, acc = {}) {
    if (o && typeof o === "object" && !Array.isArray(o)) {
      for (const k of Object.keys(o)) {
        if (k === "__type" || k === "__name") continue;
        if (o[k] && typeof o[k] === "object") flatten(o[k], acc);
        else acc[k] = o[k];
      }
    }
    return acc;
  }

  src.addEventListener("input", render);
  document.querySelectorAll(".pg-samples button").forEach((b) => {
    b.addEventListener("click", () => {
      src.value = SAMPLES[b.dataset.sample] || "";
      render();
    });
  });

  // 初始示例
  src.value = SAMPLES.contract;
  render();
</script>
