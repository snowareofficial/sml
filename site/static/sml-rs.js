// SPDX-License-Identifier: MulanPSL-2.0
// sml-rs.js — 在浏览器里调用 Rust 实现（swsml 编译的 wasm, C-ABI）
//
// 为什么需要它：
//   js/sml.mjs 受 JavaScript 语言能力限制，无法呈现 Rust 的真实行为
//   （详见 js/sml.mjs 顶部的「与 Rust 实现的已知差异」A 类：
//    A1 无法区分 1.0 与 1；A2 大整数阈值 2^53 vs 2^63）。
//   本模块把 Rust 引擎搬到浏览器，让 Playground 能直接对照两种引擎。
//
// 加载方式：裸 WebAssembly.instantiate —— sml.wasm 只导出 C-ABI 函数，
// 不依赖 wasm-bindgen，因此不需要任何胶水代码。
//
// 用法:
//   const rs = await loadSmlRs("/sml.wasm");
//   const r = rs.parse("a: 1.0\nb: 330106201503071234\n");
//   // r = { ok, json, sml, error, position }

const enc = new TextEncoder();
const dec = new TextDecoder("utf-8");

// sml_rs.h：
//   #define SML_F_V3_ALL (0xFFFFFFFFu)   全部特性
// 必须显式传它。sml_loads 按 flags 过滤特性（默认基线 0 **不含**
// SML_F_CONTRACT），而 sml_parse 内部走的是全特性 —— 若这里传 0，
// 带 @contract 的文档会出现「JSON 解析成功、值树加载失败」的诡异不一致。
const SML_F_V3_ALL = 0xffffffff;

// CSmlError 布局（wasm32，见 rust/src/c_abi.rs）
//   code: i32 @0 / line: i32 @4 / column: i32 @8 / position: usize @12
//   source: [i8;128] @16 / text: [i8;256] @144   —— 共 400 字节
const ERR_SIZE = 400;
const ERR_OFF = { code: 0, line: 4, column: 8, source: 16, text: 144 };

/**
 * 把 JS 字符串写进 wasm 线性内存，返回 [指针, 字节数]（调用方负责释放）。
 *
 * 注意：wasm32-unknown-unknown 不导出 malloc（宿主默认是 wasm-bindgen 生态）。
 * 本 crate 刻意不依赖 wasm-bindgen，故由 Rust 侧的 sml_alloc 提供分配能力。
 */
function putStr(mod, s) {
  const bytes = enc.encode(s);
  const n = bytes.length + 1; // +1 留给 NUL
  if (typeof mod.exports.sml_alloc !== "function") {
    throw new Error("sml.wasm 未导出 sml_alloc，请用最新构建（见 _sync_wasm.py）");
  }
  const p = mod.exports.sml_alloc(n);
  const mem = new Uint8Array(mod.exports.memory.buffer, p, n);
  mem.set(bytes);
  mem[n - 1] = 0; // C 字符串需要 NUL 结尾
  return [p, n];
}

/** 从 wasm 线性内存读取 NUL 结尾的 C 字符串。 */
function getStr(mod, p) {
  if (!p) return null;
  const mem = new Uint8Array(mod.exports.memory.buffer);
  let end = p;
  while (mem[end] !== 0) end++;
  return dec.decode(mem.subarray(p, end));
}

/**
 * 加载 Rust 引擎。
 * @param {string} url wasm 文件路径
 * @returns {Promise<{parse: Function, version: Function}>}
 */
export async function loadSmlRs(url) {
  const res = await fetch(url);
  if (!res.ok) throw new Error("加载 wasm 失败: HTTP " + res.status);
  const buf = await res.arrayBuffer();
  const { instance: mod } = await WebAssembly.instantiate(buf, {
    // swsml 的 C-ABI 不需要任何宿主导入；留空以便将来扩展。
    env: {},
  });
  const ex = mod.exports;

  if (typeof ex.sml_parse !== "function") {
    throw new Error("sml.wasm 未导出 sml_parse，请确认编译的是 swsml 的 cdylib");
  }

  /**
   * 用 Rust 引擎解析 SML 文本，给出两种视图。
   *
   *   sml  —— **推荐**。to_sml 结果，是 Rust 值模型的无损呈现：
   *           能看出 1.0 与 1 的区别（Float 保留小数点）、科学计数法是否
   *           保住（1e10）、哪些值其实是字符串（带引号，如 "0571"）。
   *   json —— 与 Playground 现有展示一致，但**有损**：
   *           ① JSON 只有 number，Int/Float 的区别会消失
   *           ② 超出 2^53 的整数经 JSON.parse 即被改坏
   *              （Rust 输出 330106201503071234，JS 读回 ...071200）
   *           因此凡是要看「真实值」，一律以 sml 视图为准。
   *
   * 注：wasm 未导出对象键名遍历，故无法逐字段给出 typeof；
   *     sml 视图已隐含类型信息（小数点 / 引号），足够使用。
   */
  function parse(text) {
    const [pin, pinLen] = putStr(mod, text);
    const pErr = ex.sml_alloc(ERR_SIZE); // 用于取回错误详情
    let pJson = 0;
    let pSml = 0;
    let pRoot = 0;
    try {
      // 1) 值树 + 序列化（SML 视图，无损）—— 主路径
      const root = ex.sml_loads(pin, SML_F_V3_ALL, pErr);
      if (root) {
        pRoot = root;
        pSml = ex.sml_dumps(root, 0);
      }
      const sml = getStr(mod, pSml);

      // 2) JSON 视图（有损，仅作对照）
      pJson = root ? ex.sml_parse(pin) : 0;
      const json = getStr(mod, pJson);

      if (!root) {
        return {
          ok: false, json: null, sml: null,
          error: readErr(mod, pErr) || "解析失败（Rust 引擎未给出原因）",
        };
      }
      return { ok: true, json, sml, error: null };
    } finally {
      if (pJson) ex.sml_free_str(pJson);
      if (pSml) ex.sml_free_str(pSml);
      if (pRoot) ex.sml_free(pRoot);
      if (ex.sml_dealloc) {
        ex.sml_dealloc(pin, pinLen);
        ex.sml_dealloc(pErr, ERR_SIZE);
      }
    }
  }

  /** 从 CSmlError 内存块读出错误信息（含行列，与 Playground 现有风格一致）。 */
  function readErr(mod, p) {
    if (!p) return null;
    const mem = mod.exports.memory.buffer;
    const code = new Int32Array(mem, p + ERR_OFF.code, 1)[0];
    const line = new Int32Array(mem, p + ERR_OFF.line, 1)[0];
    const col = new Int32Array(mem, p + ERR_OFF.column, 1)[0];
    const text = getStr(mod, p + ERR_OFF.text);
    if (!text) return null;
    return {
      code, line, column: col, message: text,
      toString() {
        return line > 0 ? `${text} (line ${line}, col ${col})` : text;
      },
    };
  }

  function version() {
    const p = ex.sml_version();
    return p == null ? "(unknown)" : getStr(mod, p);
  }

  return { parse, version, exports: ex };
}
