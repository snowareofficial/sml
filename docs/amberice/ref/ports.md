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
| 7 | Python | FFI（ctypes） | ✅ 21/21 **逐字符** | 本次 | `bindings/python/ambr.py` |
| 8 | C# | FFI（P/Invoke） | ⬜ 待做 | | 同一套读路径符号，见下方配方 |
| 9 | Go | FFI（cgo） | ⬜ 待做 | | 同上 |
| 10 | Ruby | FFI（Fiddle） | ⬜ 待做 | | 同上 |
| 11 | FFI 面 | **独立 crate** `amberice-ffi`（cdylib） | ✅ 7 个导出符号，`rust/amberice/include/amberice.h` 为契约 | 本次 | `rust/amberice-ffi/`；`amberice` 保持纯 rlib |

> FFI 端**刻意只到「文本」为止**：更友好的对象 API（值/路径/类型）等 `swml` 定稿后再一次性加，
> 免得五个绑定先把一套临时 API 抄死。现在这层只承诺「字节进、canonical 文本出」。

> Java 现状说明：`bindings/java` 里的 `Sml` 是 FFM 调 Rust cdylib（不是原生实现）；
> `Ambr` 是**纯 Java**，不加载任何本地库 —— 这正是用户说的「java 这种可能应该做」。

## 还差什么

只剩 **C# / Go / Ruby 三端 FFI**（配方见下节）。表里已达标的那几端不要重做。

摘要侧另有一条欠账：五端只算 SHA-256，`0x31 SM3` / `0x32 BLAKE3` 的文件会被误报「摘要不符」。
补 `hash_id` 分派（能算则验、不能算则明确跳过）排在 D44 后续，先登记不扩散。

- FFI 面已经是**独立 crate** `rust/amberice-ffi`（cdylib），`amberice` 保持纯 rlib；
  不要再给 `amberice` 加 `ffi` feature —— 它没有，也不该有。
- 补齐后：把 `spec/tools.md` 的解码器行改成「全端 + FFI」，N03 / T7 收口。

## 剩余 FFI 端的三步配方

共同前提：`cargo build -p amberice-ffi` 产出 `amberice_ffi.dll` / `libamberice_ffi.so`。

1. 按 `rust/amberice/include/amberice.h` 声明读路径所需符号（完整清单在头文件）：
   - `uint32_t ambr_caps(void)` —— 先查能力位
   - `char *ambr_text(const unsigned char *buf, size_t len, char *err, size_t errsz)`
   - `void ambr_free_str(char *p)`
   - 写入路径另需 `ambr_pack_text` / `ambr_free_bytes`（`sml` feature）；宿主缓冲用
     `ambr_alloc` / `ambr_dealloc`
2. 传字节、收文本：长度必须显式传（`.ambr` 里必然有 NUL）；失败返回 NULL、`err` 里是
   `AMBR: …`（NUL 结尾，可截断）。
3. 对拍：把返回的文本与 `tests/vectors/amberice-container.v1.json` 的 `canonical_text`
   **逐字符**比较——比解成对象再比更严，同时钉住 canonical 发射。

C# `[DllImport("amberice")]`；Go `import "C"` + `C.GoString`；Ruby `Fiddle::Function`。
三端都只需 ~40 行，不需要值模型：要树就用各自的 `loads(text)`。

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
