# `.ambr` 配套工具

> **产品线** amberice ｜ **状态** current ｜ **最后核对** 2026-11-09
>
> 格式的可用性有一半在工具面：`info` / `ls` / `cat` / `check` / `get` / `history` /
> `text` / `tree` / `pack` / `unpack` / `vectors` / `verify` / `sign` / `append` /
> `compact` / `keygen` / `delegate`，以及 `smltools` 的文本互转与五端解码器。
> 完整清单以 `amberice --help` 为准（CLI 兜底提示也指向它）。
> 格式本体见 `format.md`；决策记录见 `../decisions.md`；元数据在 `docs/meta.sml`。
>
> **章节编号是 `amberice` 产品线的全局编号**，不是本文件内的连续编号：第 0–4, 8–9, 11, 18–21, 23–25 节在 `format.md`；第 5–7, 13–14, 17 节在 `smldb.md`；第 10, 12, 15–16 节在 `../decisions.md`。
> 这样跨文件的交叉引用（`§19.9`、`§21.1b` 之类）永远指同一件事。

---

## 22. 配套工具（D38 · 用户 2026-11-04 点名「还得有配套工具」）

> 原则：**格式的价值有一半在工具**。没有 `info` / `verify` / `cat`，`.ambr` 就是个打不开的黑盒。

| 工具 / 子命令 | 用途 | 阶段 | v1 必须 |
|---|---|---|---|
| `amberice info <f>` | 一眼看清：edition/fmt/段表/能力位/是否签名、是否加密 | T2 | ✅ **已实现**（T1 提前；能力位/签名状态待 T2/T4 补） |
| `amberice ls <f> [--segments]` | 列段与外来载荷（id/长度/算法） | T2 | ✅ **已实现**（并列出 `payloads[]` 的名字与长度） |
| `amberice cat <f> [<path>] [--schema]` | 无路径输出整份；有路径按路径取值（`a.b[0]`）；`--schema` 按内嵌 `SCHEMA` 渲染 `TYPED`（D45） | T3 | ✅ **已实现** |
| `amberice history <f> <path> --at N` | 第 N 层为止的值（历史回放） | T4 | ✅ **已实现**（从 `cat --at` 拆出） |
| `amberice get <f> <path> [--bytes]` | 零拷贝取一棵子树（不解全树），`--bytes` 出原始字节 | T3 | ✅ **已实现**（不依赖 `DIR`：用 `subtree_len` 跳兄弟） |
| `amberice text <f> [--path P]` | 整份或某一子树的 canonical 文本 | T3 | ✅ **已实现** |
| `amberice check <f>` | 结构校验：段哈希、层链、能力位、截断位置 | T2 | ✅ **已实现**（段哈希/截断；层链待 T4、能力位待 `format.md §19.9`） |
| `amberice verify <f> [--trust HEX64]` | 验链 + 验签，打印每层结论；`--trust` 把链头钉在外部锚点上 | T4 | ✅ **已实现**（防整条链被剥离） |
| `amberice sign / append / delegate` | 签名、追加补丁、换钥授权 | T4 | ✅ **已实现**（`keygen` 也在，`sign` 默认原地写） |
| `amberice pack / unpack` | 把多文件打成 `.ambr`（`EMBED` + `payloads[]`），或反向取出 | T2 | ✅ **已实现**（`unpack` 拒绝目录穿越；`pack --zstd` 段级压缩） |
| `amberice tree <f>` | 打印层链（index/作者/补丁/签名数/授权） | T4 | ✅ **已实现** |
| `amberice vectors --gen/--check` | 生成/校验跨端测试向量：`amberice-canonical.v1.json`（节点编码）+ `amberice-container.v1.json`（整份 `.ambr`，2026-11-06 加） | T0 / T7 | ✅ **已实现**（另带「自检」：往返 + 文本一致 + `dump` 两次同字节） |
| `amberice bench <f>` | 读路径基准（mmap 打开、单子树取、解帧、解密） | T5 | 建议 |
| `amberice migrate --to fmt=N` | `fmt` 升级骨架：只支持 `fmt=0`；有层必须 `--seed` 重签（D49） | T5+ | ✅ 骨架 |
| `amberice compact` | 合并多层（会**重签**，D8 的例外情况；丢弃历史、旧锚点需重建） | T4 | ✅ **已实现** |
| `smltools --to amberice` / `--from amberice` | 与 SML 文本互转（D27 的 A 路） | T6 | ✅ |
| `smltools --to amberice --dir [N]` | 同时写可选 `DIR` 路径索引（`DumpOptions.dir` / `dir_depth`，D43）；缺省不写 | T6 | ✅ |
| `smltools --to amberice --keep-source` | 带 `SOURCE` 段 | T6 | 建议 |
| JS / C / C++ / Lua / Java 解码器 | 只读 `VALUE` + 透传载荷（D12 五端可读） | T7 / N03 | ✅ **五端 + 纯 Java 全通（2026-11-06）**：JS `js/ambr.mjs`、C `c/ambr.c`、C++ `cpp/ambr.cpp`、Lua `Sml.ambr_load`、Java `sml.Ambr` —— 各 22/22 + 语义抽检，四个端已进各自检查脚本。**FFI 绑定（Python/C#/Go/Ruby）2026-11-10 实测全通（22/22 + 往返）** |
| VSCode 扩展（只读） | 直接打开 `.ambr` 浏览树 / 悬停看段信息 | T7+ | 建议 |

**顺序**：`info`/`ls`/`check` 跟着 T2 长出来；`cat`/`verify`/`sign`/`append` 跟 T3/T4；`vectors` 是 T0 的**前置**（没有向量就不知道 canonical 对不对）。

