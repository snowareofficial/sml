# amberice ↔ swsml 对齐面（单一事实来源 + 守护）

> 2026-11-13 建立。目的不是「把两套 API 做成一样」—— `.ambr` 是容器格式，
> `sml_*` 是解析/值树 C-ABI，二者**刻意不同**；本页只管那些**必须从 swsml 抄过去、
> 抄错就会静默出坏数据**的事实，并把「v5 时手动跟一手」变成可执行判据。
>
> 守护：`python tools/check/check_amberice_align.py`（纯标准库、只读，CI guards job 已接入）。
> 任一副本与单一事实来源不符 ⇒ 退出码 1。

## 1. 必须对齐的事实

| 面 | swsml 单一事实来源 | amberice 侧副本 | 判据（脚本里的检查） |
|---|---|---|---|
| 文档 edition | `rust/sml-feature/src/lib.rs` 的 `EDITIONS`（最后一个 `Supported` 的 major）+ `Edition::CURRENT` | `rust/amberice/src/container.rs::EDITION`、`lua/lib/ambr_pack.lua::EDITION`、`tests/vectors/amberice-container.v1.json` 每条 `ambr_hex` 头第 5–6 字节（**little-endian u16**） | 四处同为 `4` / `0x0004`；当前 `5.0` 是 `Dev`，**不许**提前写 5 |
| 包版本 | `rust/Cargo.toml` 的 `[workspace.package] version`（`0.6.9-dev.0`） | `rust/amberice/Cargo.toml` 用 `version.workspace = true`；`main.rs` 用 `env!("CARGO_PKG_VERSION")` | 无需守护：没有手抄副本。`sml_rs.h` / 绑定头注释里的旧版本串已删除 |
| 特性位表 | `sml-feature::FEATURES`（20 位）+ `c/sml_rs.h` 的 `SML_F_*` | 五门绑定的 `SML_F_*` / `SMLF_*`（此前只镜像 0..11 + 19，缺 12..18） | C 头与五门绑定各有一套完整 `0..19`；本批已补齐 12..18 |
| 错误码条目数 | `errors/codes.sml` 的 `impls:` 条目（167） | **amberice 刻意不复制码表**（见 §2） | 三份生成物 `COUNT` 与码表一致（顺带守护 swsml 自身漂移） |
| 容器向量 | `rust/amberice/src/vectors.rs` 生成 | `tests/vectors/amberice-canonical.v1.json`（21 例）、`amberice-container.v1.json`（22 例） | `cargo run -p amberice --features preserve_order -- vectors --check`（当前 rc=0）；向量是**冻结生成物**，勿手改 |
| `@version` 字面量 | `version.md` §1：`vN` / `N` / `N.0` / `N0` / 代号 | 向量里的 `source` 全是简单值，**不解析 `@version`** | 不适用；不要把 swsml 的版本解析器搬进 amberice |

## 2. 刻意不同（不是欠账，别「对齐」掉）

| 面 | 差异 | 理由 |
|---|---|---|
| C-ABI 符号 | amberice 是 `ambr_caps` / `ambr_text` / `ambr_pack_text` / `ambr_free_str` / `ambr_free_bytes` / `ambr_alloc` / `ambr_dealloc`（7 个，`rust/amberice/include/amberice.h`）；swsml 是 `sml_*` | 两套独立 ABI；amberice-ffi 是单独 crate，D3 规定 `amberice` 只依赖值模型 |
| 能力位 | `AMBR_CAP_READ = 1<<0` / `AMBR_CAP_WRITE = 1<<1`（`rust/amberice-ffi/src/lib.rs`），C / JS / Lua 写入端有镜像 | 这是 **amberice-ffi 自己的能力位**，与 swsml 的 20 位 `FEATURES` 无关、数值碰巧相近而已。守护只验「各端互相一致」，不与 `SML_F_*` 比 |
| 错误码域 | **已落地（2026-11-13）**：`errors/codes.sml` 新增独立 `AMBR` 域 8 条（`E-AMBR-001`…`008`），`errors/msg.en.json` 有英文 title/msg，`errors/gen_js_errors.py::DOMAIN_EN` 有英文域名 | amberice **自成一体**：不复用语言层 `E-PARSE-*` / `E-LEX-*`。Rust 侧 `amberice::AmbrError`（`code()` / `kind()` / `render()`），C-ABI 与 wrapper 的错误串统一 `AMBR: [E-AMBR-xxx] <detail>`，宿主按码分支。详见 `docs/amberice/ref/ports.md` |
| 文件入口 | swsml 五门绑定有 `load_file_full`；amberice 绑定只吃字节 | 容器格式没有「文档文件 + include」概念 |

## 3. 本次修掉的漂移

1. **五门绑定特性位缺 12..18**（`when` / `for` / `typed-block` / `case-insensitive` /
   `block-name` / `type.featag` / `lex.ident`）—— 调用方想按位传这些特性时根本拿不到常量。
   现全部补齐，并由守护钉住。
2. **头注释手抄包版本**：`c/sml_rs.h`、`cpp/sml_rs.*` 写 `swsml 0.6.4`，
   五门绑定写 `swsml 0.6.3`，而实际是 `0.6.9-dev.0`。改为「由 `sml_version()` 提供，不写死」。
3. **过期数字**：`TODO.md` 的 146 条码改为引用 `COUNT`；`scan.rs` 的「16 个特性」、
   `features.md` 的「17 个特性」改为 20；CI 步骤名「41 例」→ 60、容器向量注释「21 例」→ 22
   （Lua 纯写入仍是 canonical 21 例，未改）。
4. **Ruby 新常量缩进**：补位时曾把常量写到模块外（顶格），`Sml::SML_F_WHEN` 会取不到 ——
   已移回模块缩进内，守护正则允许缩进并做全量位数校验。
5. **澄清了长期被记作「C# 既有缺口」的那一格**：`ambr pack` 报
   `this build is read-only`，此前一直被当成 `amberice-ffi` 只读的实现缺口。实测是
   **构建产物没带 `sml` feature**：按 CI 的方式
   `cargo build -p amberice-ffi --features sml` 重建后，`bindings/csharp` 整组
   `ALL OK`；显式给 `AMBR_LIB` 后 `tools/check/check_ambr_write.py` 也是
   22/22 读 + 22/22 往返。⇒ 属**构建链对齐**，不是代码缺陷。
6. **构建顺序陷阱（本批实测复现）**：`cargo run -p smldocs` 会把 `sml.dll` 按
   **不带 `sml` feature** 的图重建一遍，随后五门绑定的 `sml_dump_err` 全报
   `E-INTERNAL-001`。跑完文档工具后必须
   `cargo build -p swsml --features sml` 再测绑定。两个 crate 各有自己的
   写入开关（`swsml` :: `sml`，`amberice-ffi` :: `sml`），都带上了写入对拍才全绿。

## 4. 支持平台对齐（2026-11-13）

`ports.md` 自点的「C# / Go / Ruby 扩到 22 例后未重测、三端不在 CI」已补：

| 端 | 本轮实测 | CI |
|---|---|---|
| C# | `dotnet run --project Sml.csproj` 全绿（ambr 22 例） | ✅ non-rust job |
| Go | `TestVectors` 22/22 逐字符、`TestPackRoundTrip` 22 往返 | ✅ |
| Ruby | `22/22 read, 22 round-trip` | ✅ |
| Java（纯实现） | `sml.AmbrTest` | ✅ 一并接入 |

加上原有的 C / C++ / JS / Lua / Python / Rust，状态表 11 行现在都在 CI 里。
首次 Linux CI 运行会验证 `LD_LIBRARY_PATH` / `CGO_LDFLAGS` 的路径写法（本地 Windows
已用等价的 `PATH` / `AMBR_LIB` / `CGO_LDFLAGS` 跑通）。

**wasm 也补齐**：`amberice-ffi` 新增 `allow-32bit` feature 转发（只在
`js/build_ambr_wasm.py` 的 wasm 构建命令里开），重编 `js/build/amberice_ffi.wasm` 后
JS 写入器的解码错误同样带 `[E-AMBR-002]`。`js/test-ambr-write.mjs` 里
`typed-schema` 的既有假红也一并修正：该 fixture 带 TYPED / SCHEMA 旁路段，文本写入器
只产 VALUE，按「未知段可跳过」不该要求逐字节相同。

## 5. 仍待裁决 / 后续（不在本批）

- **v5 发布时**（用户 2026-11-13 裁决「先等等」）：`Edition::CURRENT` 升到 `V5` 后，
  守护会立刻变红，要求同步 `container.rs::EDITION`、Lua `EDITION` 与**重生成容器向量**
  （`amberice vectors --gen`）。这是有意的「红」：把静默漏改变成显式放行。
- **AMBR 码的语义细化**：当前 8 码是边界级分类；若程序需要区分「坏魔数」与「段截断」，
  可新增细码（只增不改），不改已有 8 码的含义。
- `docs/amberice/ref/ports.md` 的 `hash_id` 分派表：Rust 与 FFI 五绑定已按 manifest 分派
  `0x30/31/32`；**独立解码器**（JS / C / C++ / Java 纯实现 / Lua）仍写死 SHA-256，
  遇到 SM3 / BLAKE3 会误报「摘要不符」。补实现还是先「明确跳过」属待裁决，见该页。
- `0x33` 已永久预留给国密 SM 系列抗量子哈希（`format.md §20.3`、`hash.rs::SmPqcDigest`）：
  登记编号但未实现，读端报 `HashError::NotImplemented(0x33)`，绝不退回 SHA-256。
- `docs/amberice/spec/format.md` 内若干「已落地 / 未做」自相矛盾的段落、
  `decisions.md` 的待裁决条目：属 amberice 自身文档治理，另行处理。
