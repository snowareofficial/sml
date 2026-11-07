# Lua 端（`lua/`）

纯 Lua 的 SML 实现 + `.ambr` 读写。**无外部依赖**（写入除外，见下）。

Pure-Lua SML implementation plus `.ambr` reading/writing. No external dependencies,
except the writer (see below).

## 支持范围 / Support matrix

| 运行时 | 版本 | 读 `.sml` | `.ambr` 解码 | `.ambr` 写入 | `i64` 精确 | `f64` 精确 |
|---|---|---|---|---|---|---|
| PUC Lua | 5.1 / 5.2 | ✅ | ✅ | ❌ | ❌ double | ❌ 不支持 |
| PUC Lua | 5.3 / 5.4 / 5.5 | ✅ | ✅ | ❌ 未实现 | ✅ integer | ✅ `string.unpack` |
| LuaJIT | 2.0 / 2.1 | ✅ | ✅ | ✅ FFI | ❌ double | ✅ `ffi` |

「精确」指解码后的值能无损表示原字面量；不能时**行为不同**，见下一节。

"Exact" means the decoded value can represent the literal losslessly. Where it cannot,
the behaviour differs — see below.

## 以版本命名的差异 / Differences named by version

| 标签 | 触发条件 | 后果 |
|---|---|---|
| `lua51` | 无位运算、无 `string.unpack`、无 `ffi` | `i64` 超 2^53 丢精度（varint 走 float 回退）；遇到 `f64` **显式报错**「解 f64 需要 LuaJIT 的 ffi」，不猜值 |
| `lua53` | 有 `<<` / `>>` / `|`、有 `string.unpack("<d")` | `varint` 走位运算（精确到 2^63-1）；`f64` 走 `string.unpack`（精确）。**不需要 LuaJIT** |
| `luajit` | `require("ffi")` 成功 | `f64` 走 `ffi`（精确）；`i64` 仍是 double（普通 number 超 2^53 丢精度）；额外获得 `.ambr` **写入**能力（调 `amberice_ffi` cdylib） |

> 检测方式：**运行时探测**，不看 `_VERSION` 字符串。
> `lua53` 用 `load("return 1 << 1")` 试探（5.1 是语法错误，返回 nil）；
> `f64` 用 `string.unpack` 是否存在；
> `luajit` 用 `pcall(require, "ffi")`。
> ｜ Detection is by runtime probing, not by `_VERSION`.

## 为什么 5.3+ 不能写 `.ambr`（但可以）

`.ambr` 写入需要 canonical 编码器（值 → `STRTAB` + 值块）与段表/摘要。LuaJIT 走 FFI
调 Rust cdylib（`lua/lib/ambr_write.lua`），5.3+ 目前没有纯 Lua 编码器。

5.3+ **具备实现它的语言能力**（integer 子类型 + 位运算 + `string.pack`），只是还没写。
`lua/lib/ambr_write.lua` 在没有 `ffi` 时会明确返回
`AMBR: LuaJIT ffi unavailable — writes need LuaJIT`，不静默降级。

Writing needs a canonical encoder plus the segment table and digests. LuaJIT calls the
Rust cdylib through FFI; 5.3+ has the language features to do it natively but no
implementation yet. The writer module reports that clearly instead of degrading silently.

## 文件 / Files

| 文件 | 作用 |
|---|---|
| `lib/sml.lua` | 解析器 / 序列化 / `.ambr` 解码（单文件，约 1800 行） |
| `lib/ambr_write.lua` | 可选写入模块（LuaJIT FFI） |
| `sml_errors.lua` | 错误码表（生成物，见 `errors/gen_codes.py`） |
| `test_ambr.lua` / `test_ambr_write.lua` / `test_codes.lua` | 对拍与错误码自检 |
| `run_check.py` | 统一入口：跑上面三个 |

> 扩展名 2026-11-07 由 `.soup` 改为 `.lua`（内容未变，`git mv` 保留历史）。

## 运行 / Run

```bash
# 依赖：luajit（或 lua5.1+；写入需要 luajit）
python lua/run_check.py

# 单独跑
AMBR_LIB=path/to/amberice_ffi.dll luajit lua/test_ambr_write.lua
```

## 已知限制 / Known limits

- `lua51` 的 `i64` 精度：varint 走 float 回退，>2^53 会丢。这是运行时能力，不是 bug；
  5.3+ 精确。
- `luajit` 的 `i64` 精度：普通 `number` 是 double；要精确需全程 `ffi` cdata，成本高于收益。
- 写入需要 `amberice_ffi` cdylib（`cargo build -p amberice-ffi --features sml`）。
