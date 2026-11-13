# 多端 `.ambr` 解码器 · 进度台账（2026-11-10）

> 用户出门前指示：**一次走多步**，用时钟命令记录时间，单开文件记进度（本文件）。
> 回来时先看这张表，再看每步的提交号。

**判据（所有端一致）**：`tests/vectors/amberice-container.v1.json` 的 22 个 `ambr_hex`
全部解码成功，且各自端内的语义抽检通过。真值来源 = Rust 的 `canonical_text`。

## 状态表

| # | 端 | 方式 | 状态 | 提交 | 备注 |
|---|---|---|---|---|---|
| 1 | Rust | 原生（参考实现） | ✅ 已有 | — | `amberice` |
| 2 | JS | 纯 JS | ✅ 22/22 | `5c4eeb1` | 已进 CI |
| 3 | C | 纯 C99 | ✅ 22/22 + 5 抽检 | `cebdc4c` | `build_check.py --run` |
| 4 | C++ | 纯 C++17 | ✅ 22/22 + 5 抽检 | `b47d342` | `cpp/build_verify.py` |
| 5 | Lua | 纯 Lua（f64 需 LuaJIT ffi） | ✅ 22/22 + 5 抽检 | 已入库 | `lua/run_check.py` |
| 6 | Java | **纯 Java**（不再依赖 cdylib） | ✅ 22/22 + 5 抽检 | 已入库 | `bindings/java/.../Ambr.java` |
| 7 | Python | FFI（ctypes） | ✅ 22/22 **逐字符** | 已入库 | `bindings/python/ambr.py` |
| 8 | C# | FFI（P/Invoke） | ✅ 22/22 逐字符 + 往返 | `e91ce7a` | `bindings/csharp/Amberice.cs`；`dotnet run --project Sml.csproj`；**2026-11-13 复测 22/22 全绿** |
| 9 | Go | FFI（cgo） | ✅ 22/22 逐字符 + 往返 | `e91ce7a` | `go test ./amberice`；Windows 需 `CGO_LDFLAGS` 指到 cdylib 目录；**2026-11-13 复测 22/22** |
| 10 | Ruby | FFI（Fiddle） | ✅ 22/22 逐字符 + 往返 | `e91ce7a` | `AMBR_LIB=… ruby test_amberice.rb`；**2026-11-13 复测 22/22 + 22 往返** |
| 11 | FFI 面 | **独立 crate** `amberice-ffi`（cdylib） | ✅ 7 个导出符号，`rust/amberice/include/amberice.h` 为契约 | 已入库 | `rust/amberice-ffi/`；`amberice` 保持纯 rlib |

> 提交列只记首次落地；「已入库」不锚定具体提交号，台账不随提交推进改写。
> 要追某一端的历史用 `git log -- <端目录>`。

> FFI 端**刻意只到「文本」为止**：更友好的对象 API（值/路径/类型）等 `swml` 定稿后再一次性加，
> 免得五个绑定先把一套临时 API 抄死。现在这层只承诺「字节进、canonical 文本出」。

> Java 现状说明：`bindings/java` 里的 `Sml` 是 FFM 调 Rust cdylib（不是原生实现）；
> `Ambr` 是**纯 Java**，不加载任何本地库 —— 这正是用户说的「java 这种可能应该做」。

## 还差什么

C# / Go / Ruby 三端代码在 `e91ce7a` 入库，2026-11-10 的实测收口记在 `082032a`
（当时向量为 **21 例**，提交消息即「实测 21/21」）。第 22 例（TYPED / SCHEMA）由
`8fcf26e` 加入，当前 `amberice-container.v1.json` 为 22 例。
**2026-11-13 已补 22 例复测并全部进 CI**：Go `TestVectors` / `TestPackRoundTrip`、
Ruby `22/22 read + 22 round-trip`、C# `dotnet run` 全绿（连同 Java 纯实现一起进 CI）。
表中 22/22 现在是**实测口径**，不再是「沿用全端口径」。
状态表 11 行全部达标，`spec/tools.md` 的解码器行按「全端 + FFI」维护，N03 / T7 已收口。

错误码（2026-11-13）：`errors/codes.sml` 新增独立 `AMBR` 域 8 条（`E-AMBR-001`…`008`，
英文文案在 `errors/msg.en.json`），Rust 侧 `amberice::AmbrError` 提供 `code()` / `kind()` /
`render()`；C-ABI 的 `err` 缓冲统一为 `AMBR: [E-AMBR-xxx] <detail>`，宿主按方括号里的码
分支即可，不再靠文本前缀猜。详见 `docs/amberice/ref/swsml-alignment.md`。

摘要侧的 `hash_id` 分派（2026-11-13 复核，供裁决）：

| 端 | 分派现状 | 遇到 `0x31 SM3` / `0x32 BLAKE3` |
|---|---|---|
| Rust `amberice` | ✅ `hash.rs::digest_by_id` 按 manifest 的 `hash_id` 走，三个算法都实现；未知 id 明确拒绝 | 正常验 |
| FFI 五绑定（C# / Go / Ruby / Python / Java FFM 路径） | 走 Rust `ambr_text`，继承上表 | 正常验 |
| JS | ✅ 2026-11-13 起解 manifest 取 `hash_id`，`0x30/31/32` 分别走 `sha256.mjs` / `sm3.mjs` / `blake3.mjs`；预留号与多级元组**明确跳过并标注**（`report.skipped`） | 正常验 |
| C | ✅ 2026-11-13 起解 manifest 取 `hash_id`，`0x30/31/32` 分别走 SHA-256 / SM3 / BLAKE3；预留号与多级元组跳过并置 `digest_skipped`（`c/ambr.c`） | 正常验 |
| C++ | ✅ 2026-11-13 起解 manifest 取 `hash_id`，按 `0x30/31/32` 分派；此前**完全没验摘要**（不是写死 SHA-256），本批补上校验 + 分派（`cpp/ambr.cpp`） | 正常验 |
| Java 纯实现 / Lua | ❌ 不读 manifest 的 `hash_id`（Java 无摘要校验，Lua 只有写侧 SHA-256） | 未验 / 误报 |

⇒ 欠账不是「Rust 没分派」，而是**独立解码器没跟上**。可选两档：① 在各端补 SM3 / BLAKE3
实现后按 `hash_id` 验；② 先做「不能算则明确跳过并标注」（D44 后续），不误报但也别假装验过。
**本批不动**，等裁决。
编号空间（2026-11-13）：`format.md §20.3` 把 §20 的区间定为**分组命名空间**并多留了号：
哈希组 `0x30–0x3F` 全部 16 号已登记（`0x33` 国密 SM 抗量子、`0x34` 通用 PQC、
`0x35–0x3D` 未分配槽、`0x3E` 扩展命名空间转义、`0x3F` 私有实验），另在签名 PQC /
KEM / 混合组各留 `0x62` / `0x83` / `0xA2` 给国密 PQC。所有预留号读端一律
`NotImplemented`，永不回收；组边界（`0x2F` / `0x40`）仍是 `Unknown`。
多级转义已按元组落地：`hash_id: [0x3E, ns_version, sub…]`（版本号在前、每级 2 字节、
最多 4 级），`Manifest::hash_path` 能往返保存，但 `digest` 仍报 `NotImplemented(0x3E)`
—— 只登记、不算摘要。KDF 组已按裁决迁到 `0xB0–0xBF`（`Argon2id` = `0xB1`，
旧 `0x51` 仅 KDF 语境只读兼容）。

D45 的 `SCHEMA` / `TYPED` 渲染：JS 已通（`js/test-ambr-typed.mjs`）、C 已通
（`sml_ambr_load_typed` + `c/tests/test_ambr_typed.c`，已进 `build_check.py`）、C++ 已通
（`ambr_load_typed` + `cpp/tests/test_ambr_typed.cpp`，已进 `build_verify.py`）、Lua 已通
（`Sml.ambr_load_typed` + `lua/test_ambr_typed.lua`，已进 `run_check.py`）、Java 已通
（`Ambr.loadTyped` + `AmbrTest.java` 内的断言）。五端渲染均已通过；容器向量已加
`typed-schema` 用例（22 例），D45 收口。

- FFI 面已经是**独立 crate** `rust/amberice-ffi`（cdylib），`amberice` 保持纯 rlib；
  不要再给 `amberice` 加 `ffi` feature —— 它没有，也不该有。
- 三端**已进 CI**（2026-11-13，见 `.github/workflows/ci.yml` 的 non-rust job）；
  Java 纯实现也一并接入。本地命令仍见下节，便于离线复核。

## C# / Go / Ruby 的构建与运行

共同前提：`cargo build -p amberice-ffi --features sml` 产出 `amberice_ffi.dll` / `libamberice_ffi.so`。
实测（2026-11-10，Windows / MSVC Rust + MSYS2 gcc 16.1）：

```text
C#   : dotnet run --project Sml.csproj              （PATH 加 <target>/debug）
Go   : CGO_LDFLAGS=-L<target>/debug go test ./amberice
Ruby : AMBR_LIB=<target>/debug/amberice_ffi.dll ruby test_amberice.rb
```

Go 若链接失败，再按平台说明生成导入库（`gendef` + `dlltool`）；本次 MSYS2 gcc 直接链接通过。

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
