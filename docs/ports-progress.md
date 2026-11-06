# 多端 `.ambr` 解码器 · 进度台账（2026-11-06）

> 用户出门前指示：**一次走多步**，用时钟命令记录时间，单开文件记进度（本文件）。
> 回来时先看这张表，再看每步的提交号。

**判据（所有端一致）**：`tests/vectors/amberice-container.v1.json` 的 21 个 `ambr_hex`
全部解码成功，且各自端内的语义抽检通过。真值来源 = Rust 的 `canonical_text`。

## 状态表

| # | 端 | 方式 | 状态 | 提交 | 备注 |
|---|---|---|---|---|---|
| 1 | Rust | 原生（参考实现） | ✅ 已有 | — | `amberice` |
| 2 | JS | 纯 JS | ✅ 21/21 | `5c4eeb1` | 已进 CI |
| 3 | C | 纯 C99 | ✅ 21/21 + 5 抽检 | `cebdc4c` | `build_check.py --run` |
| 4 | C++ | 纯 C++17 | ✅ 21/21 + 5 抽检 | `b47d342` | `cpp/build_verify.py` |
| 5 | Lua | 纯 Lua（f64 需 LuaJIT ffi） | ✅ 21/21 + 5 抽检 | 本次 | `lua/run_check.py` |
| 6 | Java | **纯 Java**（不再依赖 cdylib） | ✅ 21/21 + 5 抽检 | 本次 | `bindings/java/.../Ambr.java` |
| 7 | Python | FFI（ctypes） | ⬜ 待做 | | 需先有 FFI 面（见 11） |
| 8 | C# | FFI（P/Invoke） | ⬜ 待做 | | 需先有 FFI 面 |
| 9 | Go | FFI（cgo） | ⬜ 待做 | | 需先有 FFI 面 |
| 10 | Ruby | FFI（Fiddle） | ⬜ 待做 | | 需先有 FFI 面 |
| 11 | FFI 面 | `amberice` 出 C ABI | ⬜ 待做 | | 让 7–10 共用一套 `ambr_*` |

> Java 现状说明：`bindings/java` 里的 `Sml` 是 FFM 调 Rust cdylib（不是原生实现）；
> `Ambr` 是**纯 Java**，不加载任何本地库 —— 这正是用户说的「java 这种可能应该做」。

## 需要按顺序做的事

1. C++ 解码器 + `test_ambr.cpp` + 接进 `cpp/build_verify.py`
2. Lua 解码器 + 对拍（`lua/` 目前无 `.ambr` 支持）
3. **FFI**：新建 `rust/amberice-ffi`（cdylib）或给 `amberice` 加 `ffi` feature，
   导出 `ambr_load(bytes,len,&err) -> *mut Value`、`ambr_free`、`ambr_info`？
   ⚠️ 值模型不能跨 crate 共享：FFI 返回的是 handle，取字段用 `ambr_get(handle, path)`。
4. Java 纯实现（`Ambr.java`：头/段表/LEB128/VALUE → `Sml.Value`）
5. Python / C# / Go / Ruby：优先 FFI（ctypes / P/Invoke / cgo / Fiddle），
   每端一个「21 例」测试脚本；没有 FFI 产物时跳过而不是假绿。
6. 全部完成后：`docs/amberice-smldb-spec.md` 解码器行改成「五端 + FFI」，
   N03 / T7 收口。

## 已知坑（各端照抄，别再踩）

1. **段表项的 `len` 在偏移 5**（`id:u8 comp:u8 enc:u8 flags:u16 len:u32 sha256:32`），不是 4。
2. **元数据键两侧顺序不同**：字节侧源序、`canonical_text` 按 C10 前置 ⇒ 对拍前按
   `__type/__name/__args/__featags` 归一化。
3. **`Vec<u8>` 不能同时有泛型与特化**（Rust 无特化）⇒ 字节块用 `Bytes` newtype。
4. **段表按 id 升序时 manifest(0x04) 不是最后**（TYPED 0x70 在后）⇒ 补零条件是「不是最后一段」。
5. C / C++ 顶层标量在 JSON 对拍时要包 `__v:` 再解析（`E-PARSE-008`）。

## 时钟

| 时间 | 动作 |
|---|---|
| 2026-11-06 20:46 | 侦察 bindings/StarLand；确认 Java 纯实现源码在 bindings/java |
| 2026-11-06 20:5x | 开始 C++ 解码器 |
