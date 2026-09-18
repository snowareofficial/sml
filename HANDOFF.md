# 移交文档

> 面向下一个会话。读完即可接手，不必翻聊天记录。
> 本文件主体写于提交 `3995998`（2026-09-18）；那一轮的全部改动**已按主题分 10 笔提交**（清单见 §1.2，`git log --oneline` 可直接对照）。
> **此后又落了九笔**（2026-09-18 ~ 19，`git log` 可查）：W16 的 **C 批**（§15）、
> W16 的 **JS 批**（§16）、**W12**（官网错误码通配 / 深链 + 教科书搜索接入码表，
> 验收脚本 `site/tools/tools_js_check.mjs`）、W16 的 **Rust A 批**（§17.1，含两个新码）、
> **仓库清理**（§17.3）、**私有资产的家 `sml_secret`**（§18，已推送）、
> **W4 ②③ 的 C↔Rust 序列化对齐**（§22.3）、以及**四路并行批**
> （W4 ② ／ `.gitignore` 例外 ／ 文档与 `llms.txt` ／ VSIX 重打，§22.1）
> 与各自带出的文档收口。
> ✅ **VSIX 已是新版**：本机装的就是 `snoware.sml-lang-0.4.2`（含 W16 之后的解析器），
> 0.4.1 已被 `.obsolete` 标 true —— 见 §22.4；编辑器侧的「特别高亮」见 §22.8。
> 当前工作区**干净**，全部测试基线见 §0 与 §5。
> 疑问多数能在这三处找到答案：本文件 §3（规格与实测）、§4（坑）、`TODO.md` §五（任务分解）。

## 0. 现状（一分钟版）

**产品面**：`smlconv` 已更名 **`smltools` 0.2.0**，从单一转换器扩为工具箱 ——
迁移（JSON / TOML / YAML / **XML** ⇄ SML）、`--strip`、`--lint`、目录批量、
**用 SML 自己定制编辑器高亮**。

**W20 / W21 已收口**（见 §12）：**Lua 补齐契约与 include**，不再是"子集实现"（此前
`examples/app.sml` 这类文档在 Lua 里"能解析但树是错的"）；`smltools` 新增两码
（`E-INCLUDE-012` / `E-CLI-008`）并把**码从上游带下来**（删掉按文案猜码的映射）。

**测试面**（本轮实测，数字可直接引用）：

| 套件 | 命令 | 结果 |
|---|---|---|
| Rust 全 workspace | `cargo test --workspace` | **543 通过 / 0 失败**（46 个 target），**rc=0** —— W16 期间复测（见 §14.5：构建期偶发文件占用，重试即过） |
| Rust **serde 套件**（⚠️ 不在上面那条里！） | `cargo test --features serde --test serde_bridge` + `cargo test -p sml-value --features sml,serde` | 10 通过 + 5 单测 + 1 doctest，全 rc=0。**这条必须单独跑**：`tests/serde_bridge.rs` 是 `#![cfg(feature = "serde")]`，而 `cargo test --workspace` **不开 serde** ⇒ 少了它，该套件坏掉两个月都没人发现（§17.2 的教训） |
| 其中 `smltools` | `cargo test -p smltools` | **119 通过 / 0 失败**（bin 74 + 集成 `tests/error_codes.rs` 45；`xml` 子集 26 在 bin 里） |
| C | `python build_check.py --run` | rc=0，`ALL LIMIT TESTS PASSED` + `ALL CODE TESTS PASSED`（CODE **82** 条断言；W16 的 C 批后从 62 涨到 82） |
| C++ | `python cpp/build_verify.py`（脚本内部自己 `cwd=HERE`，从仓库根跑也行） | **CI 里**六 target 全 rc=0（example / CONTRACT / COMMENTS / LIMITS / CODES 110 条 / **RS-BRIDGE**）。⚠️ **本地跑不出这个结果**：RS-BRIDGE 要链接 Rust cdylib，而本机 `rust/target/release` 与 `E:/snoware-target/release` 里**都没有** `libsml.so` / `sml.dll` 一类的产物 ⇒ 本地直接 `python cpp/build_verify.py` 会在这一格 **rc=1**（这是 W9 之后**故意**的 fail-closed，见下）。本地要全绿，先 `cd rust && cargo build --release`（或把 `SML_RUST_LIB` 指向含 cdylib 的目录）；只想看前五个就加 `--allow-skip-rs-bridge`。⚠️ 另：**C 的 `build_check.py` 必须在 `c/` 里跑**（从仓库根跑会 `fatal error: sml.c: No such file or directory`）。⚠️ **RS-BRIDGE 的「假绿」已于 W9（2026-09-19）修掉**：原先缺库时脚本只打印「RS-BRIDGE 跳过」就 `sys.exit(0)`（整体 rc=0），等于门禁失效；现在缺库**直接 rc=1**（fail-closed），`SML_RUST_LIB` 默认值也改成仓库内相对推导（`rust/target/release`，不再写死 `E:/snoware-target`）。CI 里先 `cargo build --release` 再显式传 `SML_RUST_LIB`；本地缺库排查可 `--allow-skip-rs-bridge`。⚠️ 另：**C 的 `build_check.py` 必须在 `c/` 里跑**（从仓库根跑会 `fatal error: sml.c: No such file or directory`） |
| JS 错误码 | `node js/probe-error-codes.mjs` | `ALL OK`（**45 条用例** + 深度闸门 + `parseSafe`；含 W16 余额的 005 / 020 / 006 / 001 / 002 / LIMIT-002 与各自的正对照） |
| JS 四份副本 | `python tools/check_js_copies.py` | 与 `js/sml.mjs` **逐字节一致**（rc=0）；`--fix` 一键同步 |
| Lua | `python lua/run_check.py` | rc=0，`ALL LUA CHECKS PASSED`（入口自检 + `E-IO-001` + **120 条**码用例，含 include 组 38 条） |

> ✅ **`cargo test --workspace` 复核为 `rc=0` / 46 targets / 528 passed / 0 failed**（2026-09-18）。
> 此前记的「rc=1 但 0 失败、未能复现」**已查明，且不是仓库缺陷**。唯一红的是 `swsml-derive`
> 的 doctest 编译（`E0463: can't find crate for proc_macro2 / quote / syn`），而
> `-p swsml-derive --doc` 与 `--workspace --doc` 收到的 rustdoc 命令行**逐字符一致**、每条
> `--extern` 指向的文件都存在 —— 我原先猜的「**feature 统一**」**已被证伪**
> （`cargo tree --duplicates` 里 proc-macro2/quote 单版本、无分叉）。
> 判为**共享 target 目录中该 doctest 所链接构件的瞬时不一致**：`--extern sml=` 指向的是
> **无 hash 的 `libsml.rlib`**，根因是 `swsml ↔ swsml-derive` 的**循环 dev-dependency**，
> cargo 每次构建都报 `output filename collision`（rust-lang/cargo#6313）。实测该文件会被别的
> 配置变体改写（`cargo build -p swsml --no-default-features` 之后 4932198 → 1994448 字节）；
> 共享目录被"半成品构建 / 中断的构建"留下不一致状态时就会 E0463。
> **再遇到时**：`cargo clean -p swsml-derive && cargo test --workspace` 即可恢复。
> 判读仍以 `test result: ok. N passed; 0 failed` 为准，别只看退出码。
> ⚠️ **更正两个数**：① `rust/derive/src/lib.rs` 里的 ``` 只有 **2 处** = **1 个**代码块
> （我曾记成「76 个标记 / 38 个代码块」—— 那是 **PowerShell 把反引号当转义符**造成的假数字，
> 见 §11.3）；② 那条 doctest **本来就在跑**，所以 `doctest = false` 的代价是 **1 条**测试、
> 不是 38 条 —— 结论仍是「别关」，但数量级差了 38 倍。
> **未能确定性复现**（有决定性反向证据 + 机制级证据，但构造不出稳定复现）。

> **CI 已落地（2026-09-19，W9）**：`.github/workflows/ci.yml` 在 GitHub 跑六个 job（`rust` / `rust-serde` / `non-rust` / `guards` / `miri` / `osv`），Gitee 是权威源、GitHub 是镜像 + CI。失败严格传导（**无 `|| true` / 无 `continue-on-error`**）：`rust/osv_check.py` 有洞即失败 + 网络失败 fail-closed（CI 网络抖动可 `--allow-network-error` 豁免，但漏洞默认不可豁免）；`cpp/build_verify.py` 的 RS-BRIDGE 缺库即失败（可 `--allow-skip-rs-bridge` 豁免）。本机已逐条验证非 Rust 各端自检命令（C / C++ / JS 副本 / Lua）全 rc=0；`rust` / `rust-serde` / `miri` / `osv` 需在 Linux CI 跑（osv 依赖外网 OSV API）。⚠️ **分支保护（W9.4）由用户在 GitHub 仓库设置里开启，不入库**，待办见 §2 的 W9-分支保护 行。

**两块新基建**（2026-09-18）：

- **错误码（已全量）**：`errors/codes.sml` 是唯一事实来源 —— 它本身用 SML 写，所以
  `errors/gen_json.py` 能拿 `smltools` 自己校验它，并生成 `site/static/errors.json`
  给官网 `/errors` 页。**139 条码 / 14 个领域 / 分四层**（W20/W21 各加一个：`E-INCLUDE-012`、`E-CLI-008`；以 `errors/gen_codes.py` 的输出为准）（语言层、宿主绑定层、工具层、
  编辑器层）。清点范围与「该报错却静默通过」清单见 `errors/README.md` 的「清点」一节。
  **码是稳定契约，文案不是** —— 这既是 W3（跨端统一）的抓手，也是 W10（把码落到五端）的前置。
- **官网教科书搜索**：`site/tools/gen_search_index.py` 在**构建期**生成 44 页索引，
  前端零依赖本地过滤（中文按子串、英文按词 AND + 加权打分）。与错误码查询共用
  `site/static/site-tools.js`，两个 widget 各按「容器是否存在」自启用；
  `site/build_site.py` 已把两个生成脚本接进构建。

**两个安全修复**（同日，详见 §3.5 与 `TODO.md` 的 W13）：C++ / C 的嵌套深度守卫会失效
（10 万层块嵌套直接打穿栈）、C 在 `err = NULL` 时越界写与空指针写。

## 1. 已完成

### 1.1 已提交（提交顺序）

| 提交 | 内容 |
|---|---|
| `3977b42` | 清仓库欠账：warning 19→0、修 `mod tests` 缺 `#[cfg(test)]`、YAML/测试栈溢出等 |
| `a42de59` | `smlconv` → `smltools`（69 文件引用、站点 URL alias、0.2.0） |
| `5a324f5` | `--from json/yaml` + `--strip` + `--lint` |
| `380c7ad` | TOML 双向（`--to/--from toml`） |
| `a7442ca` | 目录批量（`-i dir -o dir`） |
| `8701b02` | 用 SML 定制高亮（`--to tmlanguage`） |
| `07b180a` | 编辑器定制包（`--to highlight`）：颜色 / 项目内生效 / 扩展编译 / Zed |

### 1.2 本次提交（上一会话成果，按主题分 10 笔）

| 主题 | 提交 | 文件 |
|---|---|---|
| **XML 迁入** | `99c9a8a` | 新 `rust/smltools/src/xml.rs`；`smltools/src/main.rs`（`InputFormat::Xml`、`.xml`/`.svd` 推断、扩展名改单一事实来源）；新 `examples/micro/CH32V103xx.{svd,sml}` |
| **`to_sml` 排版** | `da69898` | `rust/sml-value/src/dump.rs`（扁平才留一行 + 行尾空白 + 深度守卫） |
| **安全修复（W13）** | `973347b` | `c/sml.c`、`c/Makefile`、新 `c/test_limits.c`；`cpp/sml.cpp`、`cpp/build_verify.py`、新 `cpp/test_limits.cpp` |
| **JS 修复** | `d2910b1` | `js/sml.mjs`（契约 `[T]` 简写、嵌套数组静默截断）+ 4 份副本（`site/static/{,lib/}sml.mjs`、`site/public/`、`editors/vscode/src/vendor/sml.mjs`） |
| **`sml-regex`** | `2dce724` | `rust/sml-regex/{Cargo.toml,src/lib.rs}`（量词 off-by-one、`^…$` 锚点松判）、`rust/tests/security.rs` |
| **编辑器** | `8c749cd` | `editors/vscode/src/{extension.js,sml-parse.mjs}`（跳转到定义 + hover 显示契约展开结果）、`editors/vscode/README.md`、新 `editors/zed/` 骨架 |
| **Zed highlights 修正** | `8f07b6c` | `rust/smltools/src/highlight.rs`（谓词带 `@`、标点改匿名 token） |
| **错误码（W11）** | `330972c` | 新 `errors/`（`codes.sml` + `gen_json.py` + `README.md`）、新 `site/content/{zh,en}/errors.md`、`site/static/errors.json`、`site/static/site-tools.js`、`site/themes/…/baseof.html`、`site/build_site.py` |
| **站点搜索** | `a4ba6fc` | 新 `site/tools/gen_search_index.py`、新 `site/content/{zh,en}/search.md`、`site/static/search-index.json` |
| **文档** | `3995998` | `README.md`、新 `README.en.md`、`rust/README.md`、`rust/smltools/README.md`（整篇重写）、`rust/AUDIT_REPORT.md`、`CHANGELOG.md`、`TODO.md`、本文件 |

> 提交前已跑 §5 的四组命令：Rust `528 passed / 0 failed`、C `ALL LIMIT TESTS PASSED`、
> C++ 五个 target 全 `rc=0`。
> 注：`c/build_check.py` 与 `site/public/` 在 `.gitignore` 里，`git add` 会对这些路径报
> 「ignored」告警但**其余文件照常暂存**（退出码为 1）；`site/public/sml.mjs` 是已跟踪的
> 构建产物，需 `git add -f`。

### 1.3 能力矩阵

- **输入**：`sml`（默认）`|json|toml|yaml|xml`，按扩展名自动推断（`.svd` 归 xml）；
  另支持 `-i <目录>` 批量。
- **输出**：`md|json|toml|tmlanguage|highlight|xml|svg|latex|slint|lvgl|html|custom|sml`。
- **其他开关**：`--strip`、`--lint`、`--hugo <dir>`、`--zola <dir>`、`--custom-rules <file>`、
  `--feature <v>`、`-o`（文件或目录）。

## 2. 剩余任务

**任务的唯一权威清单是 `TODO.md` §五（W3–W17）** —— 每条都写清了文件范围、验收标准、
依赖与并行性（✋ 需人工判断 / 🤖 适合派 agent）。这里只给摘要与建议顺序：

| # | 一句话 | 备注 |
|---|---|---|
| **W10** | 错误码**落地到五端**：各端错误对象带 `code`（Rust `ParserError.code` / JS `e.code` / C-ABI 输出码 / C++ / Lua），同一条件五端同码 | 码表已就绪，**无阻塞** |
| **W16** | 「静默清单」逐条判定「改成报错」还是「写进规范允许静默」，回填码表 `status` | ✅ **已完成**（2026-09-18，五端全落地 + 统一收口；见 TODO.md 的 W16 行与 CHANGELOG） |
| **W3** | 顶层标量四实现统一为显式报错（口径已定：**码必须一致、文案不要求逐字**） | W16 的子集 |
| **W4/W5/W6** | C `sml_dump` 与 Rust `to_sml` 逐字节比对 → 跨实现一致性套件 → Lua 侧契约（走 C-ABI） | 三者共享 `c/`、`js/`、`lua/`，**必须串行** |
| **W7** | 解析器一次报多条错误 | |
| **W8** | Zed grammar 编译验证（需联网装 tree-sitter CLI）+ `extension.toml` 指向可用 grammar | grammar 已在 `editors/zed/`，见 §3.4 |
| **W9** | Miri / 安全门禁 / 非 Rust 实现扫描进 CI | ✅ **已完成**（2026-09-19：`ci.yml` + `osv_check.py`/`build_verify.py` 去假绿；详见 §0 的 CI 备注） |
| **W9-分支保护** | GitHub 分支保护（required checks / 禁止直推 main / 撤销权限）由用户在仓库设置里开启，**不入库** | ✋ 待用户决定（核对清单见本会话 W9.4 报告） |
| **W12** | 错误码查询工具接搜索（按码与前缀检索 + 深链 `/errors/#E-PARSE-008`） | 独立 |
| **W14** | JS 空键列表抛 `ReferenceError`（报告函数是 `parse` 的局部量） | 与 W3 同改 `js/sml.mjs`，需串行 |
| **W15** | Lua 补深度上限，对齐 `E-LIMIT-001`（128 层） | 与 W3 同改 `lua/`，需串行 |
| **W17** | C 的嵌套数组被静默丢弃（`parse_array` 不递归） | 数据正确性 |

**建议顺序**：W10 → W16 → W3 → W4 → W5 → W6 → W7 → W9；W14 / W15 与 W3 串行；W8 等 CLI；W12 独立。

> 文档债提醒：`TODO.md` 的勾选状态**历来不可信**（有多项已完成却未勾），一律以代码为准。
> ~~另有两处历史残留的自相矛盾~~ **两处都已清**：
> - `rust/AUDIT_REPORT.md` 的 §二「待修复缺陷」原与上方 §0-* 的「已修复」打架 ——
>   已在该文件顶部与 §二/§三 各加一段「§二是审计当时的结论，不是当前状态」的抬头，
>   **原文保留**（那是证据链，不该删）。别再去逐条改它的正文。
> - `rust/README.md` 的自相矛盾在本会话之前已修。

## 3. 关键规格与实测

### 3.1 `--from xml`（`rust/smltools/src/xml.rs`）

接口 `pub fn parse(text: &str) -> Result<sml::Value, String>`；接线 `InputFormat::Xml`
（`.xml` / `.svd` 自动推断）+ `load_input` 分支。

**映射约定（已实现，26 个单测钉住）**
- 根元素 → 顶层对象的一个键，值为对象；子元素 → 键，**同名兄弟合并为数组**（保序）
- **纯文本元素折叠为字符串**：`<name>PWR</name>` → `name: PWR`（**不套** `_text`）
- 属性 → 该元素的 `_attrs`；元素**同时**有属性/子元素时文本才进 `_text`（`trim` 后非空才写）
- 空元素（`<x/>`、`<x></x>`、只有空白）→ `{}`
- **叶子一律字符串**（XML 无类型，不猜数字/布尔；会被误读成数字/布尔的文本由 `to_sml` 的
  `quote_if_needed` 自动加引号）
- 忽略声明 / 注释 / DOCTYPE（含 `[ … ]` 内部子集）；CDATA 原样入文本
- 实体只认五个具名 + `&#nn;` / `&#xnn;`，**其余报错**（含 `&#xD800;`）
- 命名空间前缀保留（`ns:tag`、`xmlns:xs`）

**错误**：`第 N 行（字符偏移 M）：说明`，一律 `Result::Err`，非测试代码零 panic/unwrap。

**超大输入的三条硬约束**：① 深度上限 128（与 `sml_value::MAX_VALUE_DEPTH` 一致），
10 万层 → 报错返回、不崩（Rust 栈溢出是 abort，`catch_unwind` 接不住）；
② 单趟扫描，游标只前进，整体 O(n)，实体解码只在遇 `&` 时走；
③ **同时报字符偏移** —— XML 常整篇一行，行号会退化成「第 1 行」。

**实测**（release，`examples/micro/CH32V103xx.svd`，432 711 B，CRLF；中位数为 3 次）：

| 输出 | 耗时（中位） | 产物 | 峰值内存 |
|---|---|---|---|
| `--to sml` | 161 ms | 276 669 B（6530 行） | **8.2 MB** |
| `--to json` | 125 ms | 194 243 B | 9.0 MB |

- **往返闭合**：`xml→json` 与 `xml→sml→json` **逐字节相等**（194 243 B）
- **幂等**：对产物再跑 `--format sml` 逐字节不变；工作区那份 `.sml` 与重新生成的完全一致
- 内存放大约 33×（16 MB XML → 529 MB），时间线性（16 MB ≈ 2.0 s）→ 大文件可行，内存按 ~35× 预留
- 深度炸弹 10 万层 → `rc=1` + 明确的「嵌套过深」文案，**不崩溃**
- `--strip` 对 XML 输入是**空操作**（值里没有 `__name`/`__type`），产物字节一致，但峰值内存会上涨

**已知取舍**：名为 `_attrs` / `_text` 的子元素会与保留键同名并被人为并成数组（可预测，不静默覆盖）；
DTD 内部子集自定义的实体不解析；混合内容多段文本按出现顺序拼接后 `trim`；
`to_sml` **不转义**制表符与换行（`"` 与 `\` 会转义），故多行文本在产物里是真的跨行 ——
合法且能往返，但用 `diff` 看迁移结果时要有心理准备。

**还没做**：迭代式解析 / 分批。当前实测时间线性、内存 33×，尚未暴露瓶颈 —— 按「先量再改」不动。

### 3.2 `to_sml` 排版规则（`rust/sml-value/src/dump.rs`）

数组元素与顶层非对象值：**扁平才留一行，含容器就展开多行**（扁平 = 直接子项全是标量）。
动机是 CMSIS-SVD 迁移时一个 `peripheral` 被压成 **15 644 字符的单行**，无法阅读/diff。
净效果（同一 SVD）：最长行 **15 644 → 311**、行宽中位 121 → 32、`>500` 字符行 71 → **0**、
行尾空白 3 → **0**；产物 277 440 → 276 669 B；峰值 13.5 → **8.2 MB**；往返仍逐字节一致。
**这是可观察的行为变更**（下游若有逐字节 golden 需同步），已写进 CHANGELOG。

⚠️ **两处跨端差异（2026-09-19 用户拍板：一条「约定」、一条「注明」，都写进 README）**：

1. **对象键序不保证（约定）**：Rust 的 `Value::Object` 是 `BTreeMap` ⇒ 序列化**按键排序**；
   C / C++ / JS / Lua **保源序**。C↔Rust 的 parity 里 19/19 文件都命中这条。
   **约定写法**：「对象是映射、不是序列；要保序用数组」—— 已进 `README.md` / `README.en.md`
   的「跨实现差异与约定」、`llms.txt`（Key facts）、`site/content/zh/book/ch12-smltools.md:93`
   （原先写的是「已知限制」，改成了引用约定）。**不改实现**（Rust 换保序映射会牵动
   契约 / include / `@for` 一串按 BTreeMap 写的地方）。
2. **引号策略（差异，注明即可）**：Rust 的 `to_sml` 给含特殊字符的裸键/值加引号（中文标点、`%`、
   看起来像数字的 `1.1`、`0x20`）；C / C++ 只认空白与 `:` `#` `{` `}`。两种写法都能被各自读回。
   ⚠️ **但有保真后果，文档里必须写清**：宽松策略下「看起来像数字的字符串」回读会**被重新归类**
   （`schemaVersion: 1.1` 回来是**浮点**）→ 需要严格保真请显式加引号。**不统一实现**。

### 3.3 错误码体系（`errors/`）

- **形状** `E|W|I - <领域> - <三位序号>`，序号只增不回收；沿用仓库既有先例（qsm 的 `E-ID-001`）。
- **`codes.sml` 是唯一事实来源**，且它本身是 SML 文档 —— 写坏了 `gen_json.py` 当场报错（自检）。
- **两处易误读，已在文件头写明**：① `status` 描述的是**行为是否已实现**，与「是否带上码」
  无关（后者是 W10，全表现在都还没带码）；② `coverage` 现为「全量」。
- **书写纪律**（SML 结构字符）：所有文本值加引号；引号内**避开**反斜杠、半角引号、
  `${…}`（在 SML 字符串里是**插值模板**）、半角冒号（用全角「：」）。第一版就是被这几样拒了三次。
- 加码流程见 `errors/README.md`；`site/build_site.py` 已自动跑生成脚本。

### 3.4 编辑器定制包（`--to highlight` / `--to tmlanguage`）

- 定制文件 schema：`directives` / `elements` / `types` / `colors` / `rules`（`rules` 支持
  `after` / `before` **锚点**）
- 基线 `rust/smltools/assets/baseline.tmLanguage.json`（`include_str!` 内嵌）；
  **锚点名从基线动态读取**，不硬编码
- TextMate `patterns` **顺序敏感**：锚点写错必须**报错并列可用锚点**，绝不静默追加到末尾
  （末尾恰是「被前面规则吃掉、永不生效」的位置）
- `--to highlight` 出 5 份产物：`syntaxes/`、`vscode/settings.fragment.json`（项目内就地生效）、
  `themes/`、`zed/highlights.scm`、`zed/themes/sml.json`
- **Zed 用 Tree-sitter**（不吃 TextMate scope）：`.scm` 里方言指令用 `(#match? …)` 谓词限定，
  **谓词必须带 `@`**（grammar 的 `directive` 节点文本是 `@form`，写成 `^form$` 会静默不上色）；
  标点要用匿名 token 列表而非 `(punctuation)`（grammar 里没这个节点）。
- `editors/zed/` 已落盘：`extension.toml`、`languages/sml/{config.toml,highlights.scm}`、
  `grammars/sml/{grammar.js,test/parse/*.sml}`、README。**剩两件**（= W8）：
  ① `extension.toml` 里 grammar 是**占位值** —— Zed 只认 `repository` + `rev`，没有 `path`，
  monorepo 只能 `file://` 绝对路径，正式发布需把 `grammars/sml` 拆成独立仓库；
  ② `grammar.js` 未编译验证（本机无 tree-sitter CLI，需联网，两次尝试被中断）。

### 3.5 C / C++ 两个安全修复（W13）

1. **嵌套深度守卫会失效**（两端同一个病，两个层次）：
   - 第一层：C++ 的 `parse_block` 遇到子块**直接递归**，绕过带守卫的 `parse_value`，深度计数不增长；
   - 第二层（修完第一层才暴露）：守卫超限时把 `depth` **复位为 0**，而复位**不会让已压上的栈帧退回**，
     外层循环随即又从 0 往下钻，「每 128 层一轮」反复压栈 —— 10 万层照样崩。
   - **C 也中招**：它的 `parse_block` 明明是带守卫的 wrapper（审计据此判过「C 无此洞」），
     但同一个「复位不能收手」让它一样被打穿（实测段错误）。
   - **修法**：让控制流真正**向上退出** —— C++ 新增 `aborted` 中止标志 + 各层循环 break；
     C 复用既有的 `ps->failed` 同样 break（Rust 当年是靠 `Err` 的 `?` 传播，三者是同一件事）。
2. **C 在 `err = NULL` 时越界写 / 空指针写**：`sml.h` 明写 `err` 允许为 NULL，而
   ① 版本错误分支把 **1 字节**复合字面量当缓冲、却拿调用方给的 `errsz` 当写入上限
   （`sml_parse(text, NULL, 256)` 就能写 255 字节踩栈）；② 另有 21 处 `snprintf(errbuf, …)`
   在 `errbuf` 为 NULL 时是空指针写。全部收敛到 `set_err()` 助手：缓冲区为空则**一个字节都不写**。
3. **回归用例**（新文件，已接进既有 runner）：`cpp/test_limits.cpp`、`c/test_limits.c` ——
   10 万层块 / 数组 / 交替嵌套**报错返回而不崩**、100 层照常解析、`err=NULL` 与 `errsz=0`
   四条路径一个字节都不写。**行为变更**：C 的契约校验失败现在会立即中止解析（原先会把余下全文
   解析完再丢弃），对外仍是「返回 NULL + 错误文案」，只是文案以**第一条**为准。

## 4. 关键坑（都踩过，别重复）

1. `sml::Value` **实现了 `Drop`** → 不能按值 match 移出内部字段（**E0509**）；用引用或 clone。
2. `Value` 只有 `as_str()` / `as_float()` —— **没有** `as_array()` / `len()`。
3. `jsonify` 输出**键按字典序**（`Value::Object` 基于 `BTreeMap`）→ 迁移往返键序会变，
   已记入 CHANGELOG 作为已知限制。
4. `mod tests` **必须**带 `#[cfg(test)]`，否则非测试构建下测试专用 import / 辅助函数被误报
   unused/dead_code，`cargo fix` 会**删掉它们**（本次真删了 `tmpdir`，`cargo test` 直接编译失败）。
5. 深嵌套测试要放进**显式大栈线程**（`std::thread::Builder::stack_size`）—— Rust 测试线程默认
   仅 2 MB，5 万层嵌套会**测试自己**先溢出。
6. **PowerShell 的 AMSI 会崩**（`System.AccessViolationException`，本轮又中了一次，
   连 `python -c "…"` 都可能被扫到）。规避：把脚本**写成 `.py` 文件再 `python file.py` 执行**；
   纯文本命令也用 `.py` 包一层。
7. 非 ASCII 处理：`sanitize_filename` / `sanitize_section` 要保留 Unicode 字母数字，
   否则中文标题会变成 `______`。
8. **`code-explorer` 子代理是只读的**（只有 lsp/search/read）—— 需要写盘的活别派给它；
   但**派它做清点极划算**（W11 的 5 端错误点清点就是三个并发只读 agent 干的）。
9. **CRLF 会被 SML 解析器静默吃掉**：`sml-parse/src/scan.rs` 的指令预扫（`@version` / `@feature`）
   用 `str::lines()` 按行重建**整篇**文本，而 `lines()` 会剥掉 CRLF 里的 `\r` ——
   **字符串字面量里的 CR 也跟着丢**（实测 `xml→sml→json` 与 `xml→json` 差 485 处 / 970 B）。
   `xml.rs` 按 XML 1.0 §2.11 在解析前归一 `\r\n` / `\r` → `\n`（既是规范要求，也让往返闭合）。
   **若日后其它迁入格式（YAML/TOML 的引号内 CR）出现同类损失，根因在此，先去 `scan.rs`，别在解析器里瞎找。**
10. `Value` 的 `Drop` 还有两处必踩：① 从它里面按值移出字段用 `std::mem::take` / `mem::replace`；
    ② `match cur { … other => … }` 当 `cur: &mut Value` 时，模式绑定会**把引用移走**（E0382），
    必须写成 `match &mut *cur { … }` reborrow。
11. 实测脚本**别把临时产物写进仓库**（本轮一度在 `rust/_xmlout` 落下 4 个文件）。用 `os.environ['TEMP']`；
    本机**没有 psutil**，测峰值内存走 ctypes 的 `kernel32.OpenProcess` + `psapi.GetProcessMemoryInfo`
    （进程退出后 `PeakWorkingSetSize` 仍可读，只要句柄是退出前开的）。
12. `--strip` 对 XML 输入是**空操作**，但峰值内存会明显上涨（要在持有旧树的同时建新树）。
13. **`rust/qsm` 与 `rust/crystalic` 在本工作区里没有源码**（只有 `target/` 与 `_*.py` 脚本，
    没有 `Cargo.toml`、没有 `src`）。它们既不属 `cargo test --workspace`（members 不含），
    也无法在此回归。**「改了 sml-value 要跑 qsm/crystalic 回归」这条旧经验在当前工作区不成立** ——
    别据此声称「下游全绿」，只能声明「本工作区无下游可测」。
14. **「有守卫」不等于「守得住」**：C/C++ 的深度守卫原先都是「超限 → 报错 → 把 depth 复位为 0 →
    返回空值」。复位是**无效的收手** —— 它不让已压上的栈帧退回，外层又往下钻，反复压栈照样崩。
    真正的收手必须让控制流**向上退出**（C 用 `ps->failed`、C++ 用 `aborted`、Rust 靠 `?`）。
    推论：审计里「某实现有守卫」这类结论**不能只看有没有守卫**，要看超限后是否真的停止下降。
15. **C 的 `parse_array` 不递归**：数组元素只处理块 / 字符串 / 裸词，遇到 `[` 直接跳过并丢弃。
    所以「嵌套数组」在 C 里既不报错也不递归 —— 给它写深度用例会误判成「守卫失效」，
    其实是**根本没解析**（已登记为 W17，数据正确性）。
16. **批量替换的缩进陷阱**（本轮踩到）：同一段代码在不同分支里缩进可能不同（16 空格 vs 12 空格），
    `replace_all` 只命中缩进一致的那些 —— 我漏掉的正好是 `key { ... }` 那条**主路径**，
    靠事后 grep 才逮住。**改完必须回 grep 一遍剩下的旧写法。**
17. **判据类代码写完要用数据验一次**：写 `to_sml` 排版判据时，递归版（「子孙全是标量」）**恒为真**
    —— 任何对象的子孙最终都落到标量。结果编译通过、测试全绿、产物**一个字节都没变**。
    **测试全绿恰恰是「没生效」的伪装**：改排版/判据这类「输出可见」的活，必须比对**产物**
    （行宽、字节数），不能只看测试通过。
18. **`to_sml` 的每条递归写入路径都要各自的深度守卫**：`dump_value`→`dump_block` 有守卫，
    新加的 `dump_element` 直接递归自己（数组套数组）时**没有** —— 实测 `0xC00000FD`（栈溢出）。
    加任何新的递归写入口，第一件事是抄一份 `if depth > MAX_VALUE_DEPTH`。

## 5. 验证命令

```bash
cd rust
cargo test --workspace                 # 528 通过 / 0 失败（全集；qsm/crystalic 不在 members 里）
cargo test -p smltools                 # 65 个（xml 占 26）
cargo test -p sml-value --features sml # to_sml 排版的回归（行宽 / 行尾空白 / 深度守卫）
cargo check --workspace                # 应为 0 warning（只剩 "hard linking" 环境噪声）
```

```bash
# C / C++（各有独立 runner，都会编译并运行回归用例）
cd c   && python build_check.py --run    # 语法检查 + example + test_limits（ALL PASSED）
cd cpp && python build_verify.py         # example + contract + comments + limits + rs-bridge
make -C c test                           # 等价入口：sml_demo + test_limits
```

```bash
# 迁移与检查
smltools --from json --to sml -i app.json         # 也支持 toml / yaml / xml
smltools --from xml  --to sml -i chip.svd         # .xml / .svd 可省 --from
smltools --from json --to sml -i conf.d -o out/   # 目录批量
smltools --lint -i doc.sml
smltools --to json --strip -i doc.sml
smltools --from sml --to highlight -i my-dialect.sml -o out/
```

```bash
# 官网数据（两个生成脚本，site/build_site.py 会自动调用）
python errors/gen_json.py             # errors/codes.sml -> site/static/errors.json（含校验）
python errors/gen_json.py --check     # 只校验不写文件（CI 用）
python site/tools/gen_search_index.py # content/**/*.md -> site/static/search-index.json
python site/build_site.py             # 完整构建（含上面两步 + Hugo + EPUB）

# 官网小工具（W12：错误码通配 / 深链 + 教科书搜索接入码表）—— 不需要浏览器
node site/tools/tools_js_check.mjs    # 10 条断言（极小 DOM 桩 + vm 里真跑脚本）
python site/tools/_w12_disc.py        # 判别实验：HEAD 版必须红（7 条）

# JS（探针 + 四份副本一致性 + 副本冒烟）
node js/probe-error-codes.mjs         # ALL OK（45 条用例）
python tools/check_js_copies.py       # 四份副本与 js/sml.mjs 逐字节一致（--fix 同步）
node js/_w16_copies_smoke.mjs         # 直接对副本跑 9 条断言（站点 / 扩展各一遍）
```

```bash
# CI（W9，2026-09-19）：本地对照 .github/workflows/ci.yml 的等价命令
cd rust && python osv_check.py        # 有洞即失败；网络失败 fail-closed（可 --allow-network-error 豁免）
cd rust && python miri_check.py --test c_abi --timeout 300   # 需 nightly + miri
cd cpp && SML_RUST_LIB="$(pwd)/../rust/target/release" python build_verify.py  # RS-BRIDGE 缺库即失败（可 --allow-skip-rs-bridge）
cd rust && cargo build --release -p smltools && errors/gen_json.py && errors/gen_codes.py  # 错误码生成物幂等
python tools/check_private_assets.py  # 私有资产守卫（需 fetch-depth:0）
```

> ⚠️ **CI 与本地跑法要点**：① `build_verify.py` 的 RS-BRIDGE 必须先有 Rust cdylib（先 `cargo build --release`，产物名以 `rust/Cargo.toml` 的 `crate-type=["rlib","cdylib"]` + `[lib] name="sml"` 为准 → Linux `libsml.so`）；② `gen_json.py` / `gen_codes.py` 依赖 `smltools` 二进制（在 PATH 或 `rust/target/release/` 下）；③ `check_private_assets.py` 要全历史，本地需 `git fetch --unshallow` 才等价于 CI 的 `fetch-depth: 0`。

**环境注意（两条，本轮都踩过）**：
- PowerShell 下**不要**直接跑这些命令去比字节：AMSI 会随机崩，默认编码也可能不是 UTF-8。
  一律写成 `.py` 文件用 `python file.py` 跑，输出写到 `%TEMP%` 再读。
- 控制台是 **GBK**：**别直接 print 中文** —— 用 `json.dumps(..., ensure_ascii=True)`
  或先写文件再读，否则 `UnicodeEncodeError` 会把整条命令打断。

## 6. 发布纪律（用户定，开工/发版前先看）

- **任何对外发布前必须先告知用户**（`cargo publish`、站点发布、仓库打 tag / release 都算）。
  `CHANGELOG.md` 的 `## [未发布]` 可以随时累积，但**把它变成版本号那一步必须等用户点头**；
  `Cargo.toml` 的版本号也别在没通知的情况下改。
- **VSCode 扩展（VSIX）上架市场：暂缓**（上架流程麻烦）。现状是本地 `npm run package` 打包后手动装；
  README 里对「未上架」的说明是**有意为之**，不是待补的短板。
- Zed 扩展的 grammar 要**拆成独立仓库**才能正式发布，该拆分同属「发布」动作，同样先告知。
- 例外：`sml-regex` 按纪律升过 `0.1.0-alpha.3`（语义变更，见 CHANGELOG）—— alpha 阶段的内部
  版本号随语义走，不算「对外发布」。

## 7. 下个会话的第一件事

1. **`git push`** —— 属对外动作，**由用户自己执行**（他的原话是「然后我push」）。
   本地领先 `github/main` **52 笔**（`git rev-list --count github/main..HEAD` 现查得）。
   AI **不要**代推；推之前提醒他 §6 的发版清单（版本号、`Cargo.toml` 版本要求同步）。
2. **W10 已收口到四端**（Rust / JS / C / C++ / C-ABI 全带码，见 `errors/README.md` 的落地进度表），
   **W18 已插队修完**（§9）。W10 剩下的尾巴只有两处，而且都**不是本仓库内能做完的**：
   - ~~**Lua 做不了**~~ **已完成（见 §10）**：前一条断言「`.soup` 是编译产物、源码不在本仓库」
     是**错的** —— 它就是纯 Lua 源码，`luajit lua/main.lua` 直接就能跑。
     用户给的 Soup 工程地址（`gitee.com/snoware/soup`）因此**不需要了**。
   - **`smltools` 的部分输出**仍只有文案 —— 这现在是 W10 唯一的尾巴。
3. **然后 W16 → W3**（静默清单判定 → 顶层标量统一）。W16 必须**先出判定表**再动实现，
   否则会在四个实现里来回改。W18 的修法给了 W16 一个可复用的范式：
   **先让判别实验变红，再动实现**（见 §9）。
4. 用户若给更极端的 XML（>64 MB 或多文件目录），先按 §4-11 的方法量时间 / 内存**再决定**
   是否上迭代式解析 —— 不要凭感觉重构。

## 8. W10 第一步（已完成，提交 `b04375d`）与环境事故

### 8.1 做完的：码表 → 生成链路 → Rust 全量带码

- **生成链路**：`errors/gen_codes.py` 从唯一事实来源 `errors/codes.sml` 生成三份产物
  （`rust/sml-codes/src/codes.rs`、`js/sml-codes.mjs`、`c/sml_codes.h`）。
  `--check` 只校验不写（CI 用）。**改码表后必须重跑**，否则
  `rust/tests/error_codes.rs::generated_codes_match_registry` 会当场拦住。
- **新 crate `rust/sml-codes`**（零依赖）：码常量 + `SmlError`（`code()` / `message()` /
  `Display` 把码缀在文案后 / `From<SmlError> for String` 只取文案）。
  后者是关键：既有 `Result<_, String>` 的调用方 `?` 一行都不用改。
- **Rust 一侧已全量带码**：`sml-lex`(a3)、`sml-include`(a2)、`sml-contract`(a3)、
  `sml-parse`(a3) 的语言层错误点全改完（词法 7 + 片段/include 11 + 契约 11 + 语法/特性/上限 48）。
  新码 `E-EXT-008`（外置指令执行失败）。
- **C-ABI**：`sml_error` 加 `code_str[16]`（真实错误码）；`classify` 改成**读码**而不是
  猜文案关键词；`c/sml_rs.h`、`cpp/sml_rs.{hpp,cpp}` 同步。
- **测试**：`rust/tests/error_codes.rs` 9 个用例，23 条「触发条件 → 期望码」，
  外加「生成物 vs 码表」一致性。全 workspace **473 passed / 0 failed**。

**两条踩过的坑**：
1. `SmlError::new(码, "字面量".into())` 会 E0283（`.into()` 失去推断目标）——
   直接传字面量即可，别再套 `.into()`。
2. 码表要**按语义条件**而不是按「报错位置」划：`enum` 取值不在列表内原本落进
   Rust 的通用类型错误，与 JS 的「未知枚举值」不是同一个码。落地时把
   `E-CONTRACT-006` 单拆出来，否则「同因同码」当场破功。

**W10 的现状（收口）**：

- **Rust ✅ / JS ✅ / C ✅ / C++ ✅ / Lua ✅ / C-ABI ✅ 已全部带码**
  （C 23 个码、C++ 27 个码、Lua 7 个适用码）。
  JS 见 §8.3；C/C++ 由两个 agent 并行完成，**做法、裁决与踩过的坑见 §8.6**。
- **Lua 也已带码（见 §10）**；`smltools` 的部分输出仍只有文案（未做）。
- ~~**Lua 侧：卡住，原因是硬的**~~ —— **这条已作废**（2026-09-18 查证时判断错了）。
  当时写的理由是「`lua/` 下只有 `main.lua` 与 `lua/lib/sml.soup`，而 **`.soup` 是编译产物、
  这个仓库里没有它的源码**，`MANIFEST.json` 也没写源在哪；记忆里那个 Soup 工程路径
  （`~/Downloads/lua-5.5.1/lua`）**已不存在**」。
  **实际**：`.soup` 只是 Soup 的**打包扩展名**，内容是**没被编译的纯 Lua 源码**
  （489 行，带人的注释），`luajit lua/main.lua` 当场就能跑。整条"阻塞"是**按扩展名猜出来的**。
  保留这段是为了记住教训：**「改不了」的结论也必须实测过才能写下来**。
- ~~`errors/README.md` 的码表状态回填与 `CHANGELOG.md` 条目~~ **已完成**（提交见 `git log`）。
  ⚠️ 一处**容易想错**的地方已写进 `codes.sml` 表头与 README：`status` 描述的是
  **行为**（各端报不报这个错），与「带没带码」是两件事 —— 所以 W10 落地完
  **不会**让任何一条 `status` 自动变 `done`。别去批量改它。

### 8.3 JS 一侧（已完成，提交 `912a608`）

`fail(msg, pos)` → `fail(code, msg, pos)`（22 个调用点补码）；新增模块级
`throwCode(code, msg)` 接管模式引擎/词法层的 15 处 `throw new Error`；
契约校验 `checkContract` 的 `errs.push(字符串)` → `errs.push({code, msg})`（12 处原因），
两处「汇总后一次抛出」用**第一条的码**、文案仍是全部原因的拼接；
`parseSafe` 多返回一个 `code`。四份副本已同步（逐字节一致）。

测试：`js/probe-error-codes.mjs`（`node js/probe-error-codes.mjs` → ALL OK），
与 `rust/tests/error_codes.rs` 是同一组条件。`errors/gen_codes.py` 也补上了反向校验：
JS/C/C++/Lua 里**手写的**码字面量必须都在 `codes.sml` 里。

有意保留的跨端差异（用例里写 `want = null`，**待 W16 判定**）：JS 未定义片段引用
被当普通键、未知特性被静默加入集合。别把它们当成本次的漏做。

### 8.5 ⚠️ 顺带挖出的既有 P0：C++ `@include` 会毁文档（**不是 W10 引入**，登记为 W18）

> **✅ 已修**（用户批准插队）：修法与验证见 §9。下面这段是当时的**发现记录**，保留原样 ——
> 尤其是最后那条「动手顺序」，事后被判别实验证实是对的。

派 agent 做 C++ 侧时，为查 `E-INCLUDE-002` 顺手实测出来的。`cpp/sml.cpp` 的 `@include` 展开：

- 拿 `git show HEAD:cpp/sml.cpp` 编同一个探针，**输出逐字相同** → 既有缺陷，非本会话引入。
- 症状：`a.sml = "@include \"b.sml\"" + "from_a: 1"` 解析后 `from_a` **丢失**，树里只剩一个垃圾键
  （形如 `include = "include"`）；目标文件自己的字段同样丢。
- 根因已定位：把目标 token **插到路径 token 之前**后又 `st.i++`，跳过插入段**首个 token**（嵌套的 `@`），
  于是 `include` 退化成裸块键，把后面字段当参数吞掉。
- **附带更麻烦的一层**：环检测的 `include_stack` push 完立刻 pop，**永远是空的** ——
  自包含/互包含根本检测不到，`E-INCLUDE-002` 实际不可能触发。
- **⚠️ 动手顺序**：off-by-one 恰好压住了无限展开，**只改索引会把它变成真死循环**；
  而「严格见过即拒」会误伤合法的菱形包含。正确修法是**链栈**（只判当前路径，允许菱形），
  必须与索引修正一起改。细节见 `TODO.md` 的 W18。

### 8.6 C / C++ 带码：两个 agent 并行做的，以及我拍了哪些板

派了两个**可写盘**的 agent（团队 `w10`：`c-native` 只碰 `c/`、`cpp-native` 只碰 `cpp/`，
文件不重叠所以能并行）。**只按只读 agent 派是没用的** —— `code-explorer` 没有写权限，
要写盘必须用带 `name` 的团队模式。

**约定（写进任务书，两端一致）**：只用 `c/sml_codes.h` 的宏、**不手打字符串**；
码作**消息前缀**写进同一个 `err` 缓冲（`E-XXX-NNN 文案`，因为 `sml_parse(text, err, errsz)`
没有放码的槽位）；**必须保住 W13 的性质**（`err==NULL` / `errsz==0` 时一个字节都不写）；
拿不准的不许猜、列出来问我；不许 commit / git add。

**我拍的板（都有实测依据）**：

1. **超 i64 整数字面量落 `Str`，不是 Float** —— Rust 的 `coerce_word` 就是这么做的。
   cpp-native 一开始引 `c_abi.rs` 的 **JSON 桥**当依据，**引错了入口**，被我驳回。
2. **`1e400` 落 `Float(inf)`**（Rust 与 C 都这样）—— C++ 原归成 `Str`，是唯一的异类。
3. **契约 `min`/`max` 边界**改 `strtod` + `endptr`：C 原先把非法边界当 0，
   `max abc` 报的是 **`E-CONTRACT-005` 这个错码**，比静默更坏。
4. **批准补 `E-PARSE-001`**（未闭合块/数组）—— 判据：码表 `impls` 已声明该端、且它
   **不在** README 的静默清单里，属「声明与实现不符」；**C 与 C++ 必须同码**。
5. **挂起**：`E-IO-002`（「空输入算不算错」要五端统一口径，属 W16）、`E-FEATURE-004`
   （要真正实现 `@version` 校验 = **加特性**）、`E-INCLUDE-002`（绑在 W18 的架构修复上）。
6. 静默清单里的东西（未闭合块注释、顶层标量…）**一概不发码**，归 W16 —— 两位都主动引
   `errors/README.md` 的纪律来反对扩范围，这个判断我认可。

**两位的自我纠错质量很高**（值得延续）：主动更正过「PARSE 组数报错」「被断言为不可达的用例
其实可达」「一律查 ERANGE 会破坏现在正确的 inf」，还各自写了**判别实验**（把修复还原成旧实现、
证明测试会红）。**正是靠这些才敢按他们的报告拍板。**

**踩过的坑**：

- 两个 agent 都被**意外回收过一次**（原因不明）。**重派用同名**（`c-native` / `cpp-native`）
  能带回历史；任务书里要写「**先把可验证的东西跑起来再补细节**，每完成一批就发一次状态」——
  这样半路被收走也不会留下不可用的半成品（第一次被收走时 `test_codes` 还没接进构建）。
- 它们改完 `cpp/sml.cpp` 后工作区会脏，**我提交时必须逐文件 `git add`，不能 `-A`**。
- `.codebuddy/`（团队数据）已加进 `.gitignore`。

### 8.2 环境事故（**务必转告用户**）

- 本机 **C 盘（400 GB）已 100% 写满**，工作区只剩不到 20 MB 可用。表现：
  编辑器写文件报 **ENOSPC**（`rust/sml-parse/src/parser.rs` 曾被**截断成 0 字节**，
  已从 git 恢复并用脚本重做；教训：**磁盘紧张时不要直接改大文件**，先 `git checkout` 恢复
  再用脚本一次性重放），shell 偶发无法执行、命令输出丢失。
- 已回收 757 MB（`rust/crystalic/target` 542 MB、`rust/target_verify` 154 MB、
  `rust/qsm/target-linux` 60 MB —— 都是构建产物），但**很快又被外部进程吃光**。
  工作区内已无可回收的大目录（`_*.py`、`dist/`、`tools/` 都是真内容，别删）。
- 真正的大头在工作区之外：`C:\Users\sakeen\AppData` 106 GB、`Desktop` 41 GB、
  `Videos` 16 GB、`.lmstudio` 10 GB。**这些都要用户自己决定**。
- `.cargo/registry/cache` 只有 463 MB 且被环境的 safe-delete 闸门挡住（>500 文件要确认），
  不是主因，别在它身上花时间。

**给下一会话的规矩**：动手前先确认 C 盘有空间；写文件失败要立刻 `git status` 看有没有文件被截断。

---

## 9. W18：C++ `@include` 修复（已完成）

**用户批准插队**（W10 收口后的第一件事）。只改一个实现文件（`cpp/sml.cpp`）+ 一个测试文件
（`cpp/test_codes.cpp`），但判决依据是**判别实验**，不是"我改完了"。

### 9.1 改法：照 Rust/C 的架构，而不是"修那个索引"

把 include 从「边解析边往 `st.toks` 中间插 token」挪到**解析之前的一次递归展开**
（新函数 `expand_includes`），链栈只装「根 → 当前」这一条路径：

- **环检测**：命中链栈即成环 → `E-INCLUDE-002`。**只判当前路径 ⇒ 菱形包含合法** ——
  这正是 W18 条目里那条警告的正解（「见过即拒」会误伤菱形）。
- **嵌套深度 32** → `E-INCLUDE-004`（与 Rust `MAX_INCLUDE_DEPTH` / C `MAX_INC_DEPTH` 同值）。
- **全局展开次数 10000** → `E-LIMIT-003`：**深度上限挡不住菱形包含的 2^N 膨胀**，
  而 C++ 原先**完全没有**这道闸。用例用一个 20 层「每层包含下一层两次」的链打出来
  （2^20 次读取 ≫ 10000）。
- 子文件的基准目录换成**它自己的所在目录**（与 Rust/C 一致）—— 原先一律相对根目录，
  子目录里的链式包含会找不到文件。
- 顺带三处同源静默：基准目录不可解析原被 `weakly_canonical`（只做词法规范化）消解成
  "目录里没这个文件"、报出 `E-INCLUDE-001`（**错码**）→ 改用严格 `canonical` 报
  `E-INCLUDE-010`；子文件的词法错误原被**丢弃**、残段照插（未闭合字符串 ⇒ 静默截断的文档）
  → 报 `E-INCLUDE-011`；超深包含原为静默跳过（字段凭空消失）→ `E-INCLUDE-004`。
- `parse_block` 里的 include 分支只剩「吃掉三个 token」（`include_dir` 为空 = 按 API 约定
  关闭 include）。`PState` 的 `include_dir` / `include_stack` 两个字段随之删除 ——
  解析器不必再知道 include 存在，那个"永远是空的死栈"也就无处可藏。

### 9.2 判别实验（这一步才是验收）

脚本把 **HEAD 版 `sml.cpp`** 取出来，配**同一份新用例**编译运行：

| 实现 | 结果 |
|---|---|
| 新（含修复） | rc=0，**0 失败**，`ALL CODE TESTS PASSED`（CODES 共 **80** 条断言） |
| HEAD（W10 之后、W18 之前） | rc=1，**14 条红** |

旧版那 14 条里最有信息量的是**四格「静默通过」**：自包含、互包含、超深包含、膨胀炸弹
在旧实现上**全都成功返回、没有任何错误**。这正面证实了 §8.5 那条警告 ——
off-by-one 恰好压住了无限展开（旧版**没挂**），所以"只改索引"确实会把它变成真死循环。

一条**反例用例**要单独说：菱形包含（a→b、a→c、b 与 c 都→d）必须**合法**。它是用来挡住
「把环检测写成见过即拒（集合而不是链栈）」这种修法的 —— 只写正向用例的套件挡不住它。
顺带钉住一个语义：叶子被包含两次 ⇒ 字段在 token 流里出现两次 ⇒ 按 SML 的重复键规则
**合并成数组**（`from_d` 是 `[4, 4]`，不是 `4`）。我第一版断言写的就是 4，跑出来才发现 ——
**是断言错了，不是实现错了**。这类"测试先红"的时刻，先怀疑断言。

### 9.3 复验命令（我实际跑的）

```bash
cd cpp && python build_verify.py          # 六 target 全 rc=0（CODES 80 条全过）
cd cpp && g++ -std=c++17 -Wall -Wextra -I. -o t_codes.exe test_codes.cpp sml.cpp
                                          # 零新增告警（-Wall -Wextra 比 build_verify 更严）
```

`errors/codes.sml` 回填了 5 条的 `impls`（`E-INCLUDE-002/004/010/011` + `E-LIMIT-003`；
C++ 从 22 个码涨到 **27 个**），两个生成器已重跑（`errors.json` 仍是 137 条 —— 只改了
`impls`/`note`，不动码）。

---

## 10. W10 最后一块：Lua 带码（已完成）

### 10.1 先纠一个错前提

上一会话把 Lua 标成「❌ 本仓库做不了」，理由是「`lua/lib/sml.soup` 是编译产物」。
**这个前提是错的**：`.soup` 只是 Soup 的**打包扩展名**，内容是**没编译过的纯 Lua 源码**
（489 行：`local Sml = {}` / `tokenize` / `parse_block` / `Sml.load` / `Sml.dump`，
开头还带着人的注释）。实测 `luajit lua/main.lua` 当场跑通（本机
`C:\msys64\ucrt64\bin\luajit.EXE`），自检输出 `self-test: a=1 b.c='hi'`。

**「编译产物」是按扩展名猜的，没跑过就写进了移交文档** —— 这个仓库已经因为"按声明反推"
吃过一次亏（W10 的 `impls` 声明与实现不符），这是第二次。教训已写进 TODO 的 W10 更正条里。

### 10.2 做法：三件与其它端不同的事

适用 Lua 的码只有 **7 条**（`E-PARSE-001/003/006/012`、`E-FEATURE-004`、`E-IO-001`、
`E-INTERNAL-001`）—— 因为 Lua 的实现面本来就窄（无契约、无 include）。

1. **`error(msg, 0)` 的 level=0 是必须的**。Lua 的 `error()` 默认会往消息**前面插位置信息**
   （`lua/lib/sml.soup:229: ...`），那会把码挤到消息中间，调用方按「首个空格之前」取码就会
   拿到 `lua/lib/sml.soup:229:` 这种东西 —— 码等于白加。判别实验的日志把这点印得很清楚。
2. **pcall 出口要判断「已经带码了，就别再包一层」**：`Sml.load` 里的 `has_code(msg)` 命中就
   把内层消息原样透出（它比外层具体得多），否则才归 `E-PARSE-012`（兜底码）。
3. **`E-INCLUDE-001` 从 `impls` 移除了 `lua`**：Lua 没有 include 语法，本条对它不适用。
   码表里那句「Lua 的宿主入口报文件不存在」是**归类错误** —— 宿主入口读的是文档本身，
   归 `E-IO-001`（与 C 的 `sml_parse_file` 同一格）。

### 10.3 判别实验 **加上** 全仓扫描（两步都要）

**判别实验**：同一份 `lua/test_codes.lua` 配 HEAD 版实现 → **11 通过 / 15 失败**；
配新实现 → **26 通过 / 0 失败**。旧版那 15 条里最要紧的是**四条 "parsed OK but should fail"**：
未闭合块、未闭合嵌套块、未闭合数组、`:` 当键名 —— 全是**静默**的。

**全仓扫描**（比判别实验更能抓到问题）：把仓库里**41 个 `.sml`** 同时喂给新/旧两份实现，
对比结论。old 28 OK / 13 ERR，new 26 OK / 15 ERR。**两个文件从 OK 变成 ERR**，逐个查过：

| 文件 | 用了什么 | 为什么不是误伤 |
|---|---|---|
| `examples/app.sml` | `@is` ×1、`include` ×3 | Lua 全不支持 |
| `SML_政务数据密级标注规范_报送稿.sml` | `@is` ×2、`@contract` ×2 | Lua 全不支持 |

两者**此前都是"能解析"但树是错的**：`include "x.sml"` 被当成裸块键，把后面到第一个 `{`
的内容全吞进片段体。所以这不是"新检查误伤了合法文档"，而是**把「静默给错树」改成了
「响亮地拒绝」**。剩下 13 个 ERR 的文件改动前后都是 ERR，只是从笼统的 `parse:` 变成了具体码。

**这一步值得单列**：只跑判别实验的话，我会以为"用例全绿 = 改对了"。全仓扫描才抓到
「新严格性会改变**真实文档**的结论」这一类影响。**动一个解析器，就要拿真实语料扫一遍。**

### 10.4 扫描顺带挖出的能力缺口：W20

Lua 的契约 / include **完全没实现**（`@is Service` 被当成片段定义、把紧随的 `name` 当
"类型/名字"参数吃掉）。这是**既有缺口、非 W10 引入**，但 W10 让它从静默变成报错。
要不要在 Lua 里补契约 / include 是**产品决定**，已登记 **W20**，等用户拍板。

### 10.5 复验命令

```bash
python lua/run_check.py        # rc=0 / ALL LUA CHECKS PASSED（入口自检 + E-IO-001 + 26 条码用例）
luajit lua/test_codes.lua      # 直接跑也行（会自动按脚本位置找 lib.sml）
python errors/gen_codes.py     # 反向校验会扫 lua/lib/sml.soup 里的码字面量
```




---

## 11. 清掉 4 项便宜活（W14 / W15 / W19 / W10 尾巴）+ 一次跨端边界统一

派了 4 个**可写盘**的 team agent（`cheap4`：`w10-tail` / `w14-js` / `w15-lua` / `w19-flaky`），
文件范围划死、共享文档（`errors/**`、`CHANGELOG`、`TODO`、`HANDOFF`）由 lead 独占 —— 4 个 agent
同时改那几处必然打架。

**验收纪律（这一轮真正起作用的三条）**：
1. **不许 agent 自己 commit / git add**，提交全由 lead 做。
2. **每个交付都必须带判别实验**（新用例在改动前先跑成红的）。`w10-tail` 写得很漂亮：
   40 条端到端用例改动前 **39 条红**，唯一绿的那条「本来就带码」正好当反证。
3. **lead 必须独立复验，不许直接采信报告。** 三次都验出了东西：
   - `w14-js` 报告的"改动前抛 `ReferenceError`"只覆盖了 `parse()`；我补跑 `parseSafe()`
     发现它**静默吞成 `ok:false` + 无码** —— 两条 API 都中招，后者更危险。
   - `w15-lua` 的边界存疑，它诚实写了"Rust 那条是**读源码推的**、没实测"。我实测后**推翻了它
     的归因**（不是"Lua/JS 齐、Rust 差一格"，而是 **2:2 分裂**）。
   - 我给 C++ 补边界钉时它**当场红了**，查下去发现 C++ 的**数组**入口白送一层（见 §11.1）。

### 11.1 跨端深度边界：从"差一格"查到"内部自相矛盾"

用**闭合**嵌套逐格扫五端（这一步必须闭合 —— 不闭合会被 `E-PARSE-001` 抢答）：

| 端 | 旧：块 | 旧：数组 |
|---|---|---|
| Rust / C | 127 | 128 |
| JS | 128 | 128 |
| C++ | 128 | **129** |
| Lua | 128 | 不递归 |

- Rust/C 用 `>=`：128 层就报，**而文案写「超过 128 层」，自相矛盾**；
- C++ 的 `key: [ ... ]` **直接调 `parse_array`**，绕过 `parse_value` 的守卫 ——
  与 W13 修的"块嵌套绕过守卫"是同一个洞，当时只补了块。

统一为「文档根不计层，128 放行 / 129 报」，并**逐格钉住**四个文件。

**教训（这轮最值钱的一条）**：我最初的五端扫描**只测了块嵌套**。要不是给 C++ 加了一条数组的
边界钉，那个"数组白送一层"会一直藏着。**扫一个维度就以为扫完了，是这类验证最常见的漏洞。**

### 11.2 还给下一会话留了个坑位

`w10-tail` 找出三处"没有家"的东西（两个候选新码 + 一个 panic），我**没让它自行加码** ——
加码是改对外契约。已登记为 **W21**，含"`E-INCLUDE-001` 现在被当成已知错码用着"这件事。

另外它挑出 `errors/codes.sml` 顶部那句「C / C++ / Lua 还只有文案」**是我写的、且已过期**
（README 的进度表是对的）。这类"文档自己打自己脸"比代码 bug 更容易骗过下一会话，已改，
并在该行写明「与 README 冲突时以 README 为准」。


### 11.3 W19 收口：查了，但**不该改**（以及我两个错误的更正）

`w19-flaky` 的结论是「**不可复现、非仓库缺陷、未改任何文件**」—— 这是一个"查了但不该改"的正确结局。
它做了我上一轮**没做到**的一步：把 `-p swsml-derive --doc` 与 `--workspace --doc` 收到的
rustdoc 命令行**逐字符抓出来对比**，确认每条 `--extern` 指向的文件都存在 ⇒ **我原先猜的
「feature 统一」被证伪**。真正的机制是共享 target 里**无 hash 的 `libsml.rlib`** 被别的配置变体
污染（`swsml ↔ swsml-derive` 循环 dev-dependency ⇒ cargo `output filename collision`，#6313）。
每条都见 §0 的注记。

**它更正了我两个错误，两个都值得记：**

1. **「76 个 ``` 标记 / 38 个代码块」是假数字。** 真实是 **2 个标记 = 1 个代码块**。
   成因：我把计数写在 `python -c "..."` 里，**PowerShell 把反引号当转义符**改写了我递过去的
   代码 —— 它甚至能复现出同样的假值 76，换成 `.py` 文件立刻变回 2。
   教训：**数字一旦经过一层工具，就要用另一种方式复核**；"看起来像是量过的"最危险。
   （它当时是在纠正**我任务书里的这个数**，而且我原本据它写了"38 个示例没人跑"的结论。）
2. **我替它写的理由「有单测 ⇒ 上游改文案会响亮失败」也是错的。** 那两条单测喂的是**自己的
   字面量**，钉的是**映射逻辑**、不是上游措辞 ⇒ 上游一改文案，码会**静默**退化成 `E-CLI-007`。
   **是它主动把这句话收回来的** —— 如果我按它去回答用户"这里安全"，就是拿错的事实做决策。
   现已在 `rust/smltools/src/main.rs` 的测试模块里补**真实上游错误**触发的加固用例（见下条提交）。

**它顺带查出的三件事（登记，不属本轮）**：
- `sml-value/src/serde_bridge.rs:128` 的 doctest **永不编译**（`#[cfg(feature = "serde")]` 默认不开）
  —— 全仓唯一一条"名义存在、实际从不运行"的测试；
- 每次构建 4 条 `output filename collision` 告警；根治要拆循环 dev-dep，代价是 derive 那条
  唯一真实的 doctest ⇒ **得不偿失，需用户拍板**；
- 它把全仓 doctest 面摸清了：**4 条会编译通过 / 2 条显式 `ignore` / 9 条非 Rust / 1 条静默死掉**。

**它还撞出一个我的操作失误**：我为了测五端深度边界，往 `rust/tests/` 写了个临时探针
`_tmp_depth_probe.rs` 又删掉，它正好撞上 cargo "列进 target 又被删"的竞态。
**教训：临时探针别写进 `tests/` 目录**（那里会被 cargo 当 target 扫描），写 `target/` 或 OUT_DIR。

---

## 12. W20 / W21：两个 agent 并行，以及 agent 被收走两次之后怎么接手

团队 `w2021`，2 个**可写盘**的 agent：`lua-contract` 只碰 `lua/`、`w21` 只碰
`rust/smltools/` 与 `rust/src/emit/`。**共享文档（`errors/**`、`CHANGELOG`、`TODO`、
`HANDOFF`）由我独占** —— 两个 agent 同时改那几处必然打架，这条每次都省事。

### 12.1 落地的东西（细节见 CHANGELOG）

- **W20 一阶段**：Lua 补契约（`@contract` / `@is` + 默认值回填），13 条码逐条用 `smltools`
  **实跑**核实（不是读源码推的）。顺带修一个**静默测试 bug**：`expect_ok` 不返回值 ⇒
  8 条值断言被整段跳过、套件照样全绿 —— **靠"新增断言数与总数对不上"发现**。
  这个信号值得一直盯着：**全绿不等于跑了**。
- **W20 二阶段**：Lua 补 include，与 Rust/C++ 同架构（**解析前**展开成文本再整体词法）。
  一处**刻意的端间差异**：Lua 不报 `E-INCLUDE-011`（它整体词法，子文件里的未闭合字符串
  走正文的静默清单，已实测）—— 已写进码表 `note`，免得被当漏做补上。
- **W21**：新增 `E-INCLUDE-012` / `E-CLI-008`；`toml.rs::descend` 的 `unreachable!()` 改
  `Err`；**根治**：`emit` 后端自己带码，删掉按文案前缀猜码的映射（旧做法会让上游一改文案
  就**静默**退化成 `E-CLI-007`，没有任何告警）。另删 6 处**重复工具前缀**并加不变式断言。

### 12.2 agent 被意外收走两次：接手时该把预算花在哪

`w21` 与 `lua-contract` 各被收走过一次，**原因不明**。两次的处理一样，且都有效：

1. **先看落盘状态，不看它的报告** —— 报告可能根本来不及发（`lua-contract` 这次就是）。
2. **先跑测试**，判断盘上的是"可运行的半成品"还是"坏掉的半成品"。
3. **预算优先给判别实验，而不是补文档** —— 只有判别实验能回答"这些用例真的钉住实现了吗"。

`lua-contract` 被收走时**实现与用例都在**，缺的只是它自己的验收报告。我只补了一件事：
把 `lua/lib/sml.soup` 换成 **HEAD 版**跑同一份套件 ⇒ **26 条红**（含 10 条 include），
随后按字节还原（sha256 一致）。**这一步比读它改的代码有用得多。**

同轮还有一件"文档自己打自己脸"要顺手收拾：`lua/test_codes.lua` 文件头的
「故意不测」清单里还写着「Lua 没有 include 语法，码表已把 lua 从 impls 移除」——
能力补上后这条必须作废，否则下一会话会照它去"补"一个已经有了的东西。
**改能力时，先 grep 一遍"没有 X / 不适用 X"的表述。**

### 12.3 两条被反复验证的规矩

- **"全绿"必须能反驳自己**。本轮靠别的信号发现过三次问题：`expect_ok` 不返回值
  （断言数与总数对不上）、`test_include_codes` 有没有**接进 runner**（先数 `ok:` 行）、
  `drive()` 里的不变式（把前缀加回去必须变红）。**断言要放在所有用例都必经的出口上。**
- **报告里的输出要标明"哪一次构建"**。我和 `w21` 在同一分钟量同一个二进制、结论相反，
  唯一原因是它贴的是**修前**那次构建的输出。它自己也认了这个教训：
  **贴 CLI 输出一律注明"改动前/后 + 是否已 rebuild"。**

---

## 13. W17：C 的嵌套数组（已修，P0 数据正确性）

只改 `c/sml.c` 一处 + 两个测试文件，判决依据仍是**判别实验**。

### 13.1 缺陷比登记的更坏：会凭空造键

TODO 原登记「嵌套数组被静默丢弃」。实测更糟 —— `[` 落到兜底 `else { next(ps); }` 被丢掉，
**内层的 `]` 于是被外层当成结束符**，剩下的 token 交给外层块解析、被当成**键名**：

| 输入 | 改前（都不报错） | 期望（Rust/JS 实测） |
|---|---|---|
| `m: [ [ a ] ]` | `{"m":["a"]}` | `{"m":[["a"]]}` |
| `m: [ 1, [2, 3], 4 ]` | `{"m":[1,2,3],"4":4}` ← **假键** | `{"m":[1,[2,3],4]}` |
| `m: [ [a], [b] ]` | `{"m":["a"]}` | `{"m":[["a"],["b"]]}` |
| `a: ` + 100 层 `[..]` | `{"a":[]}` | 正确嵌套 |

假键比"少一层"更坏：**它会被下游当真实数据**（还能过契约校验）。三端里只有 C 错。

### 13.2 递归与守卫必须同时给（否则把 bug 换成崩溃）

C 的深度守卫原先只在 `parse_block`。加 `[` 递归时**同时**把 `parse_array` 拆成
守卫 wrapper + inner（与 `parse_block` 共用 `ps->depth`，与 Rust 的 depth 同口径）。
**只加递归不给闸 = 用栈溢出换静默错解** —— 栈溢出在 C 里是段错误，接不住。
边界仍是 128 放行 / 129 报，数组与块同口径（数组那两格是 W17 才有的）。

### 13.3 判别实验，以及"哪条断言真的能抓 bug"

HEAD 版 `sml.c` + 新用例 ⇒ `test_codes.c` **7 条红**（形状）+ `test_limits.c` **1 条红**
（129 层），新实现 0 红。
⚠️ **128 层那格改前改后都通过**（改前也"成功"，只是内容被吞空）—— 它的作用是「守卫不能
误伤」，**没有判别力**。把两种断言的作用分别写进注释，免得后人以为「全绿就是有用」。

### 13.4 顺带查明、**故意未改**的端间差异

C 的数组里出现多余 `}` 仍**静默跳过**（`m: [ } ]` → `{"m":[]}`，JS 同样；Rust 报
`E-PARSE-003`）。不属 W17 范围，已写进 `errors/README.md` 的静默清单，归 W16 判定。

### 13.5 环境变化（**务必转告用户**）

动手时发现 **`rust/target` 与 `E:\snoware-target\debug` 都不在了**（后者只剩 `scan`），
即 Rust 侧现在**每次都是冷构建**；且 **C: 只剩 2.9 GB**（§8.2 记过这个风险）。
本轮的 Rust 构建一律用 `CARGO_TARGET_DIR=E:\snoware-target`（E: 有 81 GB），
**别用默认的 `rust/target`（在 C 盘）** —— 否则有填满 C 盘的风险。

---

## 14. W16（「静默清单」逐条落地）—— **进行中，交接要点**

用户裁决见 §14.1，逐条判定表在 **`errors/silence-decisions.md`**（含四端实测矩阵）。
本节的用处：**下一位接手者不需要重新测量任何现状**，照着 §14.3 的清单逐批做即可。

### 14.1 用户裁决（已定，2026-09-18）

| 问题 | 裁决 |
|---|---|
| 静默清单做多少 | **全做** —— 让各端在这些条件下与 Rust **同码**报错 |
| `sml-regex` 非法/超长模式静默「不匹配」 | **报错，自动新增码** |
| YAML 未知转义 | **收紧，但不要污染 SML**（只在迁移层） |
| `&undefined` 片段引用 | **必须报错**（"这种不应该出现，堪比 `void`"）⇒ Rust 现在是对的，另三端对齐 |

**「顶层标量」判据（已实测定死）**：顶层**恰好一个标量 token** ⇒ `E-PARSE-008`。
四端改前一致地把 `42` 造键成 `{"42":42}`；`hello world`（两 token）值能往返 ⇒ **不算**；
带指令的顶层标量（token 数 > 1）**不报** —— 有意保守，宁漏不误伤。

### 14.2 已落地（两笔，都有判别实验）

| 提交 | 内容 |
|---|---|
| `a8a37df` | **`E-PARSE-008` 接线（Rust）**：它此前是**死码**（全仓只有常量定义 + doctest 引用，没有一处 `SmlError::new(E_PARSE_008, …)`），而 `42` 被静默造键。判别实验：摘掉检查 ⇒ `top_level_scalar_needs_a_container` 红；还原后 11 passed |
| `5bd0b64` | **JS 第一批 9 条**：未闭合字符串/块注释、未知转义、`\u` 非法、数组里多余的 `}`、闭合符错配、顶层多余的 `}`/`]`、未闭合数组、顶层标量。判别实验：换回 HEAD 版 JS ⇒ **10 条红**（正是新增用例）；全仓 41 个 `.sml` 扫描新增 FAIL = 1（未跟踪遗留探针） |

`5bd0b64` 顺带修掉一个**既有 P0**：`@feature` 会把**整份文档吞成 `{}`**（`collectFeatures` 是行级读的，
而解析器那分支靠"遇到 `}` 才停"猜边界，`tokenize` 早已丢换行）—— 实测 `_probe2.sml` 旧 JS 得 `{}`、
Rust 得完整树。改法：**词法前剥掉整行**（同 Rust 的 `strip_features`）。

### 14.3 逐批清单（**码已定，截至 2026-09-18 全部完成**）

> ⚠️ 本节是交接时的逐批清单（码已定）。截至 2026-09-18，**JS 余额 / C 10 条 / Lua 6 条 / C++ 6 条 / Rust 6 条 / 统一收口 已全部完成**，逐批的判别实验与全仓扫描记录见 §15 / §16 / §17.1 与 CHANGELOG 的各端行为变更段，落地结论见 TODO.md 的 W16 行。下面保留原清单作为「当时为什么这么分」的来龙去脉。

**JS 余额 4 条**
- 未注册指令 → `E-PARSE-005`。⚠️ Rust 的规则比直觉细：`@foo { }`（无参数带体）是**合法片段定义**；
  只有**位置参数**形式（`@foo bar { }`）与"没有片段体"才报 005。别一刀切。
- 未定义片段引用 `x: &nosuchfrag` → `E-INCLUDE-006`。⚠️ 现有套件 `["k: &nope\n", null]`
  **钉住了旧的静默行为**，实现时要一起改（并把它从「本来就静默」清单里划掉）。
- 特性门控（`env` / `contract` / `fragment` 无门控，只有 include 有）→ `E-FEATURE-001`。
- 模式匹配加步数预算 → `E-LIMIT-002`。

**C 10 条**（`c/sml.c`；用例加进 `c/test_codes.c` / `c/test_limits.c`，跑 `python c/build_check.py --run`）
- LEX 四条：未闭合字符串 `E-LEX-001`、未闭合块注释 `E-LEX-002`/`E-LEX-003`、未知转义 `E-LEX-004`。
- 数组里多余的 `}` → `E-PARSE-003`、闭合符错配 → `E-PARSE-002`、未注册指令 → `E-PARSE-005`、
  未定义片段引用 → `E-INCLUDE-006`、顶层标量 → `E-PARSE-008`。
- **`sml_parse_json()` / `sml_parse()` 失败要写 `err`**（现在返回 NULL 但 err 为空，调用方分不清
  「空结果」与「出错」）—— 能判具体码就写，兜底 `E-PARSE-012`。
- ⚠️ 守住 W13 的性质：`err == NULL` 或 `errsz == 0` 时**一个字节都不许写**（`test_limits.c` 在钉）。

**Lua 6 条**（`lua/lib/sml.soup`；跑 `python lua/run_check.py`）
- 同 C 那一组，另：⚠️ `a { ] }` 现在报 **`E-PARSE-003`（错码）**，要改成 `E-PARSE-002`（同因同码）。
- 码的写法保持 `error(msg, 0)` + 码作消息前缀。

**C++ 6 条**（`cpp/sml.cpp`；跑 `python cpp/build_verify.py`）
- **先实测现状**（判定表里 C++ 那几格写的是「需实测」，别照表假设）。

**Rust 侧 6 条**
- regex 非法模式 → **新增 `E-PARSE-025`**（用户已授权「自动新增码」）；模式过长 → `E-LIMIT-007`
  （`impls` 现为 `[js]`，要加 `rust`）；步数预算耗尽 → `E-LIMIT-002`。
  ⚠️ **`E-LIMIT-002` 是「声明与实现不符」**：`impls: [rust]`，而 `sml-regex` 实测是**静默返回 false**。
- C-ABI 的 JSON 入口失败要写 `err`。
- serde 桥 `u64` 超 `i64` 静默变浮点（**丢精度**）→ `E-DERIVE-002`（或按保真口径保留为字符串，先判断设计意图）。
- `sml-value` 序列化深度超限静默降级为占位文本 → `E-LIMIT-004`。
- YAML 未知转义 → `E-MIGRATE-013`（**只在 `rust/smltools` 的迁移层收紧，不许改 SML 解析器/值模型**）。

**收口（最后一起做）**：`errors/codes.sml` 按端回填 `impls` + 重跑两个生成器、CHANGELOG 的
**行为变更段**逐条写明（这批会让一批畸形文档开始报错）、`errors/README.md` 的「静默清单」改写成
判定结果、TODO 的 W16 行、以及 `errors/silence-decisions.md` 标注落地进度。

### 14.4 方法（这轮验证有效的三条，别丢）

1. **判别实验是唯一验收标准**：把修复还原成旧实现，**同一份用例必须红**。
   本轮两次都靠它立住（E-PARSE-008 的 `if false && …` 摘除法、JS 的 HEAD 版替换法）。
2. **每批都做全仓 `.sml` 扫描**：行为变更期唯一的安全网。本轮靠它抓到
   `@feature` 吞文档（旧实现 `{}`）与三处遗留探针。
3. **逐批单独提交**：agent 随时会被收走，**落盘才算进度**。

### 14.5 环境（**务必转告用户**，本轮实测）

- ⚠️ **agent 团队被整体拆掉 3 次**（`lua-contract`、`w21`、整个 `w16` 队）。
  `.codebuddy/teams/<name>/` 会**整个消失**；`w16` 那 5 个成员**一个字节都没写**（除 js 动了两格）。
  **结论：这个环境里别依赖 agent** —— 由 lead 自己按端逐批做，慢但每批都可验证。
- ⚠️ **Rust target 目录被清过**：`rust/target` 与 `E:\snoware-target\debug` 都不在了（后者只剩 `scan`），
  即 Rust 每次都是**冷构建**；且 **C 盘只剩约 2.9 GB**（§8.2 记过这个风险）。
  ⇒ **一律 `CARGO_TARGET_DIR=E:\snoware-target`**（E: 有 81 GB），别用默认目录（会写 C 盘）。
  本轮据此跑 `cargo build -p smltools` / `--release --all-features` / `test --workspace` 都成功。
- ⚠️ **`cargo test --workspace` 会因文件占用失败**：`failed to rename archive file ... (os error 5)`；
  **等几秒重试即过**（同 `verify_rs.py` 对 `os error 32` 的重试逻辑）。别当成代码问题。
- ⚠️ **PowerShell 会 AMSI 崩溃**（`AccessViolationException`，本轮崩了 3 次），
  且 `>` 重定向默认写 **UTF-16**（读的时候会看到乱码/匹配不上）。
  **用 Python 驱动子进程最稳**（本轮所有验证脚本都是这么写的，放在 `E:\smltmp\`）。
- **现状数字**：Rust 全量 `rc=0` / 46 targets / **538 passed / 0 failed / 2 ignored**；
  JS `node js/probe-error-codes.mjs` → `ALL OK`；码表 **139 条**。

### 14.6 下一批的第一件事

~~**C 侧那 10 条**~~ **✅ 已完成（2026-09-18，见 §15）**；
~~**JS 余额 4 条**~~ **✅ 已完成（2026-09-18，见 §16）** **Lua 那 6 条** ✅ **已完成**（见 CHANGELOG「Lua：W16 末段 6 条」；其中 `a { ] }` 已改报 `E-PARSE-002`）；
**C++ 那 6 条** ✅ **已完成**（见 CHANGELOG「C++：W16 末段 6 条」）；
**Rust 那 6 条** ✅ **已完成**（见 §17.1）；
**统一收口**（码表 `impls` 回填 + 两生成器重跑、`CHANGELOG.md` 各端行为变更段、`errors/README.md` 静默清单终稿、TODO 的 W16 行、本文件 §14 末段同步）✅ **已完成（2026-09-18）**。W16 整轮收口，进度见 TODO.md 的 W16 行。

### 14.7 顺带发现、**未改**（不属 W16，另行登记）

- **数组里的裸块 `Type { }` 两端不同**：JS 得 `["rect", {…}]`（类型名成了独立元素），
  Rust 得 `[{__type:"rect", …}]`（合并进元素）。属数据形状差异。
- **`_probe_for.sml` 那类** `@for` + 片段混写：JS 报 `E-PARSE-003`、Rust 报 `E-PARSE-006`。
- 仓库根有一批**未跟踪**的遗留探针（`_probe2.sml` / `_probe3.sml` / `_probe_for.sml` / `_lvgl_test/`），
  W5 的待办里写了要收编后删除 —— 它们会让"全仓扫描"的 FAIL 计数带上噪声（本轮已按此逐条判读）

---

## 15. W16 的 C 批（已完成，2026-09-18）

**只改两个文件**：`c/sml.c` + `c/test_codes.c`（外加码表与文档的收口）。C 侧的码从 **23 → 34**。
逐条记录也在 `errors/silence-decisions.md` 的 §4（含落地进度表与全仓扫描判定表）。

### 15.1 实现（8 处）

| # | 位置 | 改法 |
|---|---|---|
| 1 | `lexer` 结构 | 新增 `failed` 标志 + `lex_err()` 助手（**只记第一条**，缓冲区为空时一个字节都不写 —— 与 W13 的 `set_err` 同约定） |
| 2 | 词法：字符串 | EOF 未闭 → `E-LEX-001`；未知转义 → `E-LEX-004`（改前 `default` 把该字符收下、**连反斜杠一起丢**）；`\u` 位数不足/非十六进制/花括号未闭合/代理区 → `E-LEX-005`（改前不足四位照收，静默变控制字符） |
| 3 | 词法：`/*` 与 `_*` | EOF 未闭 → `E-LEX-002` / `E-LEX-003`（改前吃到文件结尾，**后面整篇凭空消失**） |
| 4 | `sml_parse` 入口 | 见 `lx.failed` **立即返回 NULL**（否则后续语法错会覆盖第一个错的码 —— 与 Rust 的 `tokenize()` 短路同义）；另加 `E-PARSE-008` 判据 = `lx.n == 2 && toks[0] 是 WORD/STR`（C 的 token 流末尾固定有 `T_EOF`，故「恰好一个标量」= n == 2，与 Rust 的判据逐字一致） |
| 5 | `parse_block_inner` 结束符 | 顶层遇 `}`/`]` → `E-PARSE-003`；本层期望 `}` 却遇 `]` → `E-PARSE-002`（改前两格都静默） |
| 6 | `parse_array_inner` | 数组里多余 `}` → `E-PARSE-003`（改前落兜底 `else` 静默跳过；其余孤立 token 仍静默，**不扩范围**） |
| 7 | 指令/片段分支 | 位置参数形式与「没有片段体」→ `E-PARSE-005`（与 Rust 的 `is_param` 判据一致）；**顺带**支持显式参数 `type:` / `name:`（含 `E-PARSE-020` 两条：参数后缺取值、参数重复） |
| 8 | `coerce_word` | 未命中的 `&name` → `E-INCLUDE-006`（改前 `return sml_new_str(w)` 静默退化成字符串）；另把 `err_unclosed` / `apply_or_fail` 加上 `ps->failed` 闸，坐实「**第一条错误为准**」 |

**两条「顺带对齐」（不做就会把合法文档拒掉）**：① `@contract X strict { … }` —— Rust/JS 接受
`strict`（与默认等价），C 只认 `loose`，于是契约体被整个跳过、`strict { … }` 变成数据块；
② 片段显式参数 `@foo type: X name: Y { … }`（Rust 的 v4 写法，C 只支持已废弃的位置参数）。

### 15.2 判别实验（验收标准）

`c/_w16_cmp.py codes` —— 把 **HEAD 版 `sml.c`**（`git show HEAD:c/sml.c`，脚本自己写快照）
配**同一份新 `test_codes.c`** 编译运行：

| 实现 | 结果 |
|---|---|
| 新（含修复） | rc=0，**0 失败**，`ALL CODE TESTS PASSED`（断言 **82** 条） |
| HEAD | rc=1，**20 条红**（rc 报 `20 FAILURES`） |

`c/_w16_cmp.py limits`：HEAD 与新版都 rc=0 ⇒ **W13 的性质（10 万层不崩、`err=NULL`/`errsz=0`
一个字节都不写）保住了**。计数不能只看"全绿"：本轮**数了 `ok:` 行**（82），
防的是「断言写在跑不到的出口上」那类假绿。

### 15.3 全仓扫描（41 个 `.sml`，走 `sml_parse_file`）

`c/_w16_scan.py before|after|diff`：**OK 29 → 22、FAIL 12 → 19**，7 条 OK→FAIL 逐条查过：

| 文件 | 改后码 | 改前「OK」的真面目 |
|---|---|---|
| `examples/for_when.sml` | `E-PARSE-005` | 解析结果是**空树**（C 从未实现 `@feature`/`@when`/`@for`） |
| `examples/slint/login.sml` | `E-PARSE-003` | 树是**错的**；**Rust 对同一文件也失败**（`E-PARSE-006`） |
| `_gov_demo.sml` | `E-CONTRACT-004` | 因 `]` 提前中断、后半篇被静默丢弃才「OK」（C 无「键位置裸 `&name` 合并」） |
| `_probe2/_probe3/_probe_for/_for_probe.sml` | `E-PARSE-005` | 未跟踪遗留探针（W5 待办里说要收编后删除） |

⇒ **没有一条是「原本正确的文档被误伤」**。判定依据用的工具是 `c/_w16_one.c`（打印码）与
`c/_w16_dump.c`（打印解析结果）—— 「原先 OK」必须掏出**当时的树**来看，不能只凭「它没报错」。

**两个坑**：① `_w16_scan.py` 的 `parse()` 必须 `os.path.normcase` 归一化路径 —— 两次运行的
盘符大小写可能不同（`c:` vs `C:`），不归一化会把 41 个文件**全部**判成「消失 + 新增」（踩过）；
② 临时探针 `.sml` 别落在仓库根或 `c/` —— 会被 `find_sml()` 当语料扫进去，污染 FAIL 计数。

### 15.4 顺带查明、**未改**（另行登记）

- **C 没有指令注册表**：`@feature` / `@when` / `@for` 这些**它不实现的指令**与拼错的指令在
  token 流上同形，一并落 `E-PARSE-005` —— 方向是「响亮拒绝」而不是静默丢行，但提示会指向
  「拼写」（粒度不足）。已写进该条码的 `note`。
- **C 无「键位置裸 `&name` 合并进当前块」**（Rust 有，`parser.rs` 的裸片段合并）⇒ `_gov_demo.sml`
  报 `E-CONTRACT-004`。补齐属**能力**问题，不属 W16。
- **`examples/slint/login.sml` 两端都解析不了**：它依赖反引号表达式，而 Rust 与 C 的词法器都会
  把表达式里的半角引号 / `#` 当结构字符切开（`#` 起注释时**不 flush 当前词**）——
  **同因不同码**（Rust `E-PARSE-006`、C `E-PARSE-003`），归 W3/W16 的后续话题。

### 15.5 本轮用到的脚本（都在 `c/`，`_` 开头不入库）

`_w16_cmp.py`（判别实验：probe / codes / limits 三档）、`_w16_scan.py`（全仓扫描 before/after/diff，
已修 normcase 坑）、`_w16_one.c`（打印单个文件的码，用于按行二分）、`_w16_dump.c`（打印解析结果，
用于判定「原先的树是不是错的」）、`_w16_count.py`（数 C 侧用到的码数，回填 README 用）、
`_w16_head_sml.c` / `_w16_sml_before.c`（HEAD 快照，可随时重生成）。

---

## 16. W16 的 JS 批（余额 4 条 + 顺带修「四份副本漏同步」）（已完成，2026-09-18）

改动面：`js/sml.mjs`、`js/probe-error-codes.mjs`、**四份副本**（同步）、
新 `tools/check_js_copies.py`（**已跟踪**的闸门）+ 码表/文档收口。

### 16.1 实现（4 条 + 1 个安全修复）

| # | 条件 | 改前 | 改后 |
|---|---|---|---|
| ① | `@foo bar { x: 1 }`（位置参数）/ `@foo bar`（无片段体） | 被当片段收下 / 整行丢掉 | `E-PARSE-005` |
| ② | `@f type: { }`、`@f type: A type: B { }` | 静默丢掉整条定义 / 解析出错误的树 | `E-PARSE-020` |
| ③ | `x: &nosuchfrag` | 退化成字符串 `&nosuchfrag` | `E-INCLUDE-006` |
| ④ | 特性门控：`@contract` / `@is` / 片段定义 / `&` 引用 / `$env.X` | **全无门控**（只有 include 做了） | `contract`、`fragment` ⇒ `E-FEATURE-001`；`env` ⇒ **`E-FEATURE-002`**（与 Rust 的 `coerce_word` 同码，别混成 001） |
| ⑤ | 模式预算 | 编译成原生 RegExp，`(a*)*` 可灾难性回溯 | `E-LIMIT-002`（**编译期**最坏展开估算，`PATTERN_STEP_BUDGET = 1e5`） |

**实现要点**：
- `coerceWord` / `coerceStr` 各多收一个 `feats` 参数（6 个调用点全在 `parse()` 内，同一批改完）。
- 片段分支重写为「显式参数 `type:` / `name:`（仅当紧跟冒号时才算参数）+ 否则 `{` 才算有体」
  —— 与 Rust 的 `is_param` 判据一致；⚠️ **`@foo { … }` 仍是合法片段定义**（正对照在探针里）。
- ④ 是**安全修复**：`parse(text, {features})` 当沙箱用的调用方此前会以为已经关掉了契约/片段/env，
  实际照样执行。改法照 Rust：`@contract` 与 `@is` 都查 `Feature::Contract`。
- ⑤ 的粒度必须说清：JS 是**原生 RegExp**，运行时插不进步数计数器（Rust 的 `sml-regex` 是自研
  回溯引擎才计得了步）⇒ JS 的预算落在**编译期**：顺序求和、量词求积（`*`/`+` 按 `QUANT_MAX` 代理）、
  内联正则按 `REGEX_SRC_MAX` 计入。同因同码、条件粒度不同，已写进码表 `note`。

### 16.2 ⚠️ 顺带修的真缺陷：四份副本漏同步（不是本批引入）

W16 的 JS 首批（`5bd0b64`）只改了 `js/sml.mjs`，**四份副本一份都没同步**。取证：

| 对象 | sha256 前 16 位 | 字节 |
|---|---|---|
| `js/sml.mjs`（HEAD） | `5bc3469f482b38e3` | 62721 |
| 四份副本（改前） | `00fff0fc40589203` | 57263 |

且四份副本与 `git show b82dd4a:js/sml.mjs` **逐字节相同** ⇒ 它们停在 W14 那一版，
即「首批的修复从未进过站点与扩展」。后果：官网 Playground（`site/static/sml.mjs`）与
VSCode 扩展（`editors/vscode/src/vendor/sml.mjs`）**继续用旧解析器**。

处置：① 新增**已跟踪**的 `tools/check_js_copies.py`（默认校验四份副本与 `js/sml.mjs`
逐字节一致、不一致 rc=1；`--fix` 一键同步）——把「靠人记着」换成闸门；
② 四份副本已同步；③ `js/_w16_copies_smoke.mjs` **直接 import 副本本身**跑 9 条断言
（站点与扩展各一遍）——**只比字节证明不了「加载的那一份在运行时确实报新码」**。

### 16.3 验收与全仓扫描

- **判别实验**：HEAD 版 `js/sml.mjs`（脚本自动 `git show` 出快照 `js/_head_sml.mjs`）
  跑**同一份**新探针 ⇒ **13 条红**；新实现 `ALL OK`（45 条用例 + 深度闸门 + `parseSafe` 交码）。
- **全仓 41 个 `.sml` 扫描**（`js/_w16_scan.mjs`：同进程里 HEAD 与 NEW 各扫一遍，只打印结论变化）：
  OK 30 → 26、变化 6 个 —— 4 个未跟踪遗留探针 + `examples/for_when.sml`（都依赖 JS 不实现的
  `@feature`/`@when`/`@for`）+ `examples/app.sml`（改动前后**都失败**，码从 `E-CONTRACT-001`
  变成更准确的 `E-INCLUDE-006`）。**无一是「原本正确的文档被误伤」**。
- 驱动脚本 `js/_w16_disc.py`（判别 + 副本冒烟 + 字节一致 + 全仓扫描，一次跑完）。

### 16.4 ⚠️ 顺带查明、**未改**：JS 的 include 架构与其它三端不同（已登记为 **W23**）

JS 的 include 是「把子文件**单独 parse** 再合并数据」，**不携带子文件的片段表 / 契约表 / 类型表**；
Rust / C++ / Lua 都是**解析前文本展开**（W20 的 Lua 就是这个架构）。后果：`include` 之后的
`&name`、以及「契约写在被包含文件里」的 `@is`，在 JS 下必然失败（`examples/app.sml` 正是这一格）——
W16 只把「静默给错串」改成「响亮拒绝」，**能力没补**。修它等于重做 include（W23，✋ 需拍板）。

### 16.5 下一批（Lua）的入口

`lua/lib/sml.soup` + `python lua/run_check.py`（当前 rc=0 / 120 条）；6 条清单在 §14.3，
其中 `a { ] }` 现在报 **`E-PARSE-003`（错码）**、要改成 `E-PARSE-002`；码的写法保持
`error(msg, 0)` + 码作消息前缀。判别实验的现成做法：把 `lua/lib/sml.soup` 换成 HEAD 版
跑同一份套件（W20 期间用过，记得随后按 sha256 还原）。

---

## 17. W16 的 Rust A 批（6 条）+ 仓库清理（已完成，2026-09-18）

### 17.1 Rust A 批（改动面：`sml-regex` / `sml-value` / `sml-include` / `smltools` / `src/c_abi.rs`）

| 条件 | 改前 | 改后 | 码 |
|---|---|---|---|
| 受限正则**非法**（量词前无原子、`[` 未闭合、`\` 结尾） | 一律判「不匹配」 | `compile_regex_checked` → `RegexError::Illegal`（`sml-include` 映射） | **新码 `E-PARSE-025`** |
| 受限正则**过长**（> 256） | 占位成「永不匹配」 | `RegexError::TooLong` | `E-LIMIT-007` |
| **步数预算耗尽** | 静默判「不匹配」 | `regex_matches_checked` → `RegexError::Budget` | `E-LIMIT-002` |
| `sml-value` 序列化**深度超限** | 写占位文本 `/* …深度超限… */ null` | `to_sml_checked`（**迭代式**预扫深度，避免自己爆栈） | `E-LIMIT-004` |
| serde 桥 **u64 超 i64** | 静默变 f64（丢精度） | `visit_u64` / `serialize_u64` 报错 | `E-DERIVE-002` |
| YAML **未知转义** | 宽松保留（写错的 YAML 静默搬进 SML） | `unescape_double` 返回 `Result`（只在迁移层收紧） | **新码 `E-MIGRATE-018`** |
| C-ABI **JSON 入口失败** | 只给 NULL、无诊断 | 新符号 `sml_dump_err(json, err)`（旧 `sml_dump` 签名不动，新符号并存） | `E-PARSE-012` / `E-LIMIT-004` |

**口径要点**（三处「同因同码、粒度不同」，都写进了码表 `note`）：
① **引擎层不带码**：`sml-regex` 零依赖，只报 `RegexError`（原因），码由 `sml-include` 映射；
② **serde 桥与 `sml-value` 的码只在文案里**（`[E-DERIVE-002]` / `[E-LIMIT-004]`）—— 这两个
crate 刻意零依赖，而 serde 的错误类型本就是 message-only；门面 `sml::to_sml_checked`
会把码**提成结构化字段**；
③ `to_sml` **仍不失败**（40+ 处调用方依赖「总能给你一份文本」），只有 `to_sml_checked` 报错；
用户路径上 `E-LIMIT-004` 是**防御性**的（解析层的 128 层上限通常先拦）。

**验收**：`sml-regex` 的宽松入口（= 旧行为）与 `*_checked`（新行为）**在同一份输入上同时
断言**（`rust/tests/security.rs::regex_failures_report_codes`）⇒「改前确实静默」被钉进测试；
YAML 用**改动前的 release 二进制**跑同一份探针：旧 `rc=0` 静默保留 `\d`、新 `rc=1` +
`E-MIGRATE-018`，正对照两侧输出逐字节相同（`errors/_w16_yaml_disc.py`）；
`cargo test --workspace` **543 passed / 0 failed**；`--features serde` 那两条套件
（`serde_bridge` 10 条）也真跑过。

### 17.2 顺带修好的三处「测试自己坏了、却没人知道」

1. `rust/tests/c_abi.rs` 的 `CSmlError` **镜像缺 `code_str`**（W10 漏同步）⇒ `fill` 写到结构
   之外 16 字节（栈上越界写，UB，恰好没炸）。
2. `tests/serde_bridge.rs::from_str_enum_variants` 依赖顶层裸词，被 W16 的 `E-PARSE-008`
   改动作废；它只在 `#![cfg(feature = "serde")]` 下编译，`cargo test --workspace` **跑不到**
   ⇒ 早红无人知。**教训**：feature-gated 的套件要单独列进验证命令（见 §5）。
3. `sml-value/src/serde_bridge.rs` 的 doctest 引用门面 crate 的 API ⇒ **从来没编译过**；
   本轮真跑当场红，已改成真测 `from_value`。

### 17.3 仓库清理（同一轮，用户要求）

- **敏感件排查（决定性结论）**：某个内部报送件里的人名 **只出现在 4 个工作区文件**里；
  **git 全历史 145 个提交、2372 个对象（含不可达）、`.git` 文本文件里都没有** ⇒
  **不需要 filter-repo**（这 4 个文件早被 `.gitignore` 的「私有报送件」段挡住）。
  4 个文件已**移出仓库**到 `C:\Users\sakeen\Desktop\sml-私有报送件-已移出\`（移动而非删除）。
- **`_` 前缀杂物**：243 个未跟踪文件 + `_lvgl_probe/`、`_lvgl_test/` 两个目录已归档到
  `%TEMP%\sml-underscore-archive-20260918\`（保留相对路径，**要恢复就整体搬回**）；
  4 个 `__pycache__` 删除。**保留 7 个已跟踪的 `_` 文件**（Hugo `_index.md` ×4 + 站点工具 ×3）
  与 `_default` / `_lib` 目录（Hugo / Pages 结构）。
- `_gov_demo.sml` → **已跟踪**夹具 `rust/tests/fixtures/gov_demo.sml`（它被
  `rust/tests/gov_demo.rs` 真读，留在 `**/_*` 之下等于「测试只在本人机器上过」）。
- ⚠️ **归档规则当场就被修正过一次（同日晚些时候）—— 记下来，别再犯**：
  第一版规则是「`_` 开头一律归档」，**这是错的**。查下去发现大量 `_` 文件是**在用的工具 /
  被文档当命令引用**：`_sync_playground.py` 与 `_sync_wasm.py` 是站点四份副本的**同步入口**
  （`js/sml.mjs` 的注释里写着「务必运行」）、`_build_wasm.py`、`_check_*.py`、`_w16_cmp.py` /
  `_w16_scan.py` / `_w12_disc.py` 是 HANDOFF/§15.5/§16.5 里**列出的可复现命令**、
  `rust/qsm/**` 下是整套 qsm-acl 工具（`qsm-acl/web/README.md` 直接引用其中两个）。
  而 `.gitignore` 的 `**/_*` 早就把它们挡在版本库之外 ⇒ **删它们对「仓库整洁」零贡献，
  只有反作用**（丢工具 + 文档指向不存在的文件）。
  **修正后的规则**：**源代码 / 脚本（`.py` `.mjs` `.c` `.cpp` `.js` `.lua` `.sh`）+ 整个
  `_lvgl_probe/` `_lvgl_test/` 测试台 = 保留**（共 **206 个文件已全部还原原位**）；
  只有**纯产物**（日志、转储 `.txt`、`__pycache__`、编译出的 `.exe`/`.o`、`.svg`/`.png`、
  SquareLine `.xml`）留在归档 —— **46 个**，在 `%TEMP%\sml-underscore-archive-20260918\`。
- **教训（给下一位 agent）**：判据不是「文件名长什么样」，而是「**有没有代码/文档在引用它**」
  + 「**是不是纯产物**」。而且我第一遍查引用用的是 `git grep`（**只覆盖已跟踪文件**），
  **盲区正是 `.gitignore` 里的目录** —— `rust/qsm/**` 整个被忽略，里面「脚本 A 调脚本 B」
  当时根本看不见。补扫**全工作区**（含被忽略的文件）才查出来。查完再删，别先删再查。
- 清理后**四套回归全 rc=0**（C `build_check.py --run` / JS `probe-error-codes.mjs` + 副本一致 /
  Lua `run_check.py` / Rust `cargo test --workspace`）。

---

## 18. 私有资产的家：内网 `sml_secret`（2026-09-18）

**起因**：敏感资产（报送件 / 内部报告）此前只能「躺在主库工作区 + 靠 `.gitignore` 挡」
—— **没有版本、没有备份**；而主库 `sml` 是公开的（GitHub / Gitee）。用户给了内网可信服务器
上的私有库地址（Gitea 1.27.3 @ `10.16.144.2:3000`，组织 `CrystalicCore`，库名 `sml_secret`）。

### 18.1 已做

| 项 | 说明 |
|---|---|
| 资产归集 | 7 个：报送稿 `md` / `sml` / `sml.txt` / **`pdf`（上一轮文本扫描漏掉的二进制件）**、`报送邮件.txt`、`SML图形管线项目_全量文档合集.md`、`SML_数字字面量保真性审计报告.md` ⇒ 全部**移出主库工作区**，进 `C:\Users\sakeen\Desktop\sml_secret`（主库工作区现在**一个都不留**） |
| 私库初始化 | `git init -b main`；README 写清「用途 / 资产清单 / 纪律 / 含个人信息不要外发」；`.gitignore` 挡 Office 临时件；首次提交 **`1b68545`** |
| 远程 | `git remote add origin http://10.16.144.2:3000/CrystalicCore/sml_secret.git` |
| 主库守卫 | 新增 `tools/check_private_assets.py`：查**历史**（含已删文件）/ **索引** / `.gitignore` 规则是否在位；当前 **OK**（只写文件名模式，**不写任何个人信息**） |
| 主库文档 | `.gitignore` 私有段的注释改成「正主在私库」；TODO 新增 §三·九 |

**移动前查过两件事**（避免拆坏东西）：① 这些名字在**任何提交的历史对象里都不存在**
（`git rev-list --all --objects` 逐名比对，全无）⇒ **不需要 filter-repo**；
② 已跟踪文件里对这些名字**只有 `.gitignore` 本身**提到 ⇒ 可安全移动。

### 18.2 首次 push ✅ **已完成（2026-09-18，用户执行）**

远端 `main` 已到位：本地 `refs/remotes/origin/main` = **`1b68545`**，与本地 `main` 同步
（`git status -sb` 显示 `## main...origin/main`、无 ahead）。私库的 7 个资产现在**在可信服务器上有版本、有备份**。

> 注：我这边的 `git ls-remote` 仍报 `unable to get password from user` —— 因为本机
> `credential.helper=manager-core` 而 **GCM 没装**，**我这条非交互通道**拿不到凭据；
> 你自己终端里输口令那次是成功的（`push -u` 把 `origin/main` 跟踪分支写下来了）。
> 后续如果想让**非交互**也能推（脚本 / CI），再按下面三条里挑一条配好。

<details><summary>原先记录的三条路（保留备查）</summary>

非交互试探失败：本机 `git config --global credential.helper = manager-core`，但
**GCM 并没装** ⇒ `git: 'credential-manager-core' is not a git command` + `fatal: unable to get password from user`。

**端口实测**（决定用哪条路）：`3000` 开放（Gitea HTTP，**明文**）、**`22` 开放（SSH）**、
`443` / `8443` / `80` / `2222` 全部关闭。所以：

1. **首选 SSH**：把 `~/.ssh/id_*.pub` 加到 Gitea「设置 → SSH 密钥」，
   `git remote set-url origin ssh://git@10.16.144.2:3000/CrystalicCore/sml_secret.git`，
   再 `git push -u origin main`（Gitea 的 SSH 若不在 22 端口，按它的文档换）；
2. **次选 Token**：Gitea「设置 → 应用 → 生成令牌」，push 时密码填令牌；
3. 最省事：`cd Desktop\sml_secret && git push -u origin main` 直接输账号口令
   —— 但那是 **HTTP 明文**，只在内网做，且**建议尽早换 SSH / 让服务器开 HTTPS**。

</details>

**端口实测结论**（保留）：`3000` 开放（Gitea HTTP，**明文**）、**`22` 开放（SSH，首选）**、
`443` / `8443` / `80` / `2222` 全部关闭；`~/.ssh/known_hosts` 里已有 `10.16.144.2`（以前连过），
但 `~/.ssh` 下**当前没有密钥对** —— 要配 SSH 得先 `ssh-keygen` 再把公钥贴进 Gitea。
**待办（不急）**：让「非交互推送」可用（SSH 密钥 或 装 GCM / 用 Token），并把 3000 的明文换成
HTTPS —— 现在能用，只是安全性和自动化上还有欠账。

### 18.3 纪律（下一位 agent / 未来的你）

- 新的敏感件**只进私库**，不要再放进主库工作区；主库 `.gitignore` 那几条规则与
  `tools/check_private_assets.py` **都不许删**（后者可进 CI）。
- 私库含**个人信息**（报送邮件里的姓名）：不要 clone 到不受控的机器、不要截图外发。
- `Desktop\sml_secret` 与 `Desktop\sml` 是**两个互不包含**的仓库 —— 别在其中之一里
  对另一个做 `git add`（比如别把 `sml_secret` 放进主库目录内）。

## 19. Zed 扩展语法状态（2026-09-19，W8 收口）

**结论**：Zed 的 Tree-sitter 语法**已编译验证通过**，生成物已入库；但「正式发布」所需的
独立仓库**尚未创建/push**，故扩展此刻不能开箱加载 —— 属预期。

**验证（本机，tree-sitter-cli@0.22.6）**：
```bash
cd editors/zed/grammars/sml
tree-sitter generate                       # 生成 src/parser.c（≈53 KB）/ grammar.json / node-types.json
tree-sitter parse test/parse/basic.sml     # 0 ERROR / 0 MISSING
tree-sitter parse test/parse/advanced.sml  # 0 ERROR / 0 MISSING
```

**已入库（monorepo，`editors/zed/grammars/sml/`）**：`grammar.js`（源）、`src/`（生成物）、
`bindings/`、`Cargo.toml`、`package.json`、`binding.gyp` 等 tree-sitter 脚手架，以及
`test/parse/{basic,advanced}.sml` + 其 README。

**发布仓库决策（用户 2026-09-19 定）**：
- 源真相留在 monorepo；另开独立镜像仓库 `snoware/tree-sitter-sml`（只含 grammar 子树，不含其它），
  供 `editors/zed/extension.toml` 的 `[grammars.sml]` 引用。
- `extension.toml` 已填 `repository = "https://gitee.com/snoware/tree-sitter-sml"`，
  `rev` 已填该仓库首版 commit `b72396d37ebb90f9d05bb52937827b8b1265d72a`（2026-09-19 用户回传）。
- 本地开发要立刻见效：`extension.toml` 顶部注释里有 `file://` + 本机绝对路径的写法。

**待办（用户侧）**：在 gitee 建空仓库 `snoware/tree-sitter-sml` → 把
`Desktop/tree-sitter-sml/`（本机已备好镜像内容）push 上去 → 把首个 commit 短 sha 发给 agent
填进 `rev`。详见 `TASK-hy3-w8-w9.md` §W8.4 / §W8.5。


---

## 20. CI 宿在哪 / 分支保护怎么办（2026-09-19 定）

`ci.yml` 已经能在 GitHub 上跑（`rust` / `rust-serde` / `non-rust` / `guards` / `miri` / `osv` 六个 job），
但**「跑得起来」≠「挡得住」**。本仓库的拓扑决定了这件事比看上去麻烦：

- **Gitee 是权威源**，日常提交推 Gitee；
- **GitHub 是镜像 + CI**（`sync-from-gitee.yml` 单向同步），CI 只在 GitHub 上跑；
- ⇒ 你往 Gitee 直接推时，**GitHub 侧的分支保护（required checks / 禁止直推）根本拦不到你**。

### 19.1 三条路（按推荐度排序）

| # | 方案 | 效果 | 代价 |
|---|---|---|---|
| **1（推荐）** | **自建 Gitea 上跑 CI + 开保护**：把仓库推一份到 `http://10.16.144.2:3000`（Gitea **1.27.3**，已实测支持 Actions），工作流放 `.gitea/workflows/*.yaml`（语法与 GitHub Actions 基本一致，job/steps/uses 几乎可照抄），管理员开启 Actions 并注册一个 act_runner（跑在你自己的机器上）；分支保护在 Gitea 仓库「设置 → 分支」里开（require status checks / 需 PR / 限制直推） | CI 与门禁**都在你自己机器上**；保护作用在**权威源**上，真拦得住；不依赖任何会员 | 要维护一个 runner（一次性的，之后基本不管） |
| 2 | **把开发主战场挪到 GitHub**：在 GitHub 上用 PR 开发，靠 sync 工作流拉回 Gitee；GitHub 分支保护开起来就有效 | 零额外成本，`ci.yml` 现成 | 改变你的提交习惯；Gitee 仍是权威源时两边要盯 |
| 3 | **保持现状（CI 当告警）**：不开保护，CI 红了看邮件/网页 | 零成本 | 拦不住任何东西 —— 门禁的价值只剩「事后知道」 |

> **Gitee Go 不在候选里**：它要会员（用户 2026-09-19 指出），且这条路同样不解决「保护要作用在权威源上」的问题。

### 19.2 required checks 怎么配（开了保护之后再定）

- **先列**：`rust` / `rust-serde` / `non-rust` / `guards` —— 这些都快（分钟级）、且失败基本都是真问题。
- **先别列**：`miri`（最长 60 分钟）、`osv`（依赖外网 OSV API，偶发抖动）。
  让它们跑成**报警**；等观察一段时间确认稳定，再把 `osv` 提为 required。
- **理由**：门禁的全部价值在「挡住真的坏提交」。一旦它开始因为环境抖动误挡，
  人就会习惯性绕过（force push / 直接关掉检查）—— 那才是门禁真正的死法。

### 19.3 首次跑红的预期

这套测试是几周内在 **Windows** 上长出来的（路径、换行、shell 假设都可能埋着），
**Linux 上第一次跑很可能会红，而且多数不是 W9 的错**。分诊顺序：
`guards`（最轻，先确认基础环境） → `rust` / `rust-serde` → `non-rust` → `osv` → `miri`。


---

## 21. W4 首次比对：C `sml_dump` ↔ Rust `to_sml`（2026-09-19，**只量未改**）

**为什么要有这件事**：C 与 Rust 各有一套序列化器，两边都「解析成功、输出看着都对」，
但**输出可以不一样**。这类差异不会让任何一端的测试变红，只会等到跨端交换数据时才炸。

**装置（已入库，可复用）**：

```bash
python tools/check_dump_parity.py            # 全仓 *.sml（排除 `_*` 临时探针）
python tools/check_dump_parity.py --show 3   # 每个不一致文件多打几处差异
```

- `tools/dump_c.c`：C 侧只有 `sml_parse_file` + `sml_dump`、**没有**公开 dump CLI，
  所以这个小程序是比对必需的那一半（编译由 Python 驱动自动完成）。
- Rust 侧用 `smltools --to sml`（默认找 `rust/target/release`，可用 `SMLTOOLS` 覆盖）。
- 退出码 `1` = 存在**两端都解析成功但输出不同**的项（即真差异）。

**首次结果（2026-09-19，语料 34 个 `.sml`）**：完全一致 **0**；不一致 **19**；
C 解析失败 **15**（解析层缺口，如 `@feature`/`@when`、`include` 读取、契约必填、
`@version v4`、`examples/slint/login.sml` 的反引号问题 —— **不算**序列化差异）。

已识别的四类差异（按「是不是已定的口径」排）：

| # | 现象 | 例 | 判定 |
|---|---|---|---|
| 1 | **`键:` 后接块时的行尾空格**：C 写 `topic: `（带尾随空格），Rust 写 `topic:` | `examples/doc-demo/*.sml`、`examples/lvgl/*.sml`、`examples/advanced_inc/*.sml` | **Rust 侧已改、C 没跟上**（见 CHANGELOG「`to_sml` 不再在『键: 后接块』时于行尾留空格」）⇒ 照改 |
| 2 | **块/数组「行内 vs 展开」**：C 把含容器的数组压成一行，Rust 展开多行 | `examples/doc-demo/guide.sml`、`examples/micro/CH32V103xx.sml` | **同上**：Rust 的新排版规则「扁平才留一行，含容器就展开多行」⇒ 照改 |
| 3 | **引号策略**：C 对含特殊字符（中文、`:`、`*`）的裸键不加引号，Rust 加 | `examples/common.sml`：`等价，仅书写风格不同: */` ↔ `"等价，仅书写风格不同": "*/"` | 涉及**回读保真**（Rust 更稳），要判定后统一 |
| 4 | **键顺序**：`editors/zed/grammars/sml/test/parse/basic.sml` 首行 C 是 `firstName: John`、Rust 是 `address:` | — | **未知**，下一步单独查（是插入序 vs 别的原因） |

**下一步（未做）**：先修 1、2（口径已定、风险低），再判定 3，最后查 4；
每修一类就重跑 `check_dump_parity.py`，看 19 条收敛到多少。

> **✅ 1 与 2 已做完（2026-09-19，见 §22.3）**：本轮把「行数与 Rust 一致」做到 **19/19**、
> 总行数差距 **13343 → 0 行**。**3（引号）与 4（键顺序）仍未改** —— 它们不影响行数，
> 且 4 是**数据保真级**问题（要判定「哪一端该改」），不是顺手能改的。

---

## 22. W4 收口 + 四路并行批（2026-09-19）

### 22.1 四路并行 agent：产出、我的复核方式、提交

| 提交 | 谁 | 做了什么 | 我**独立复核**的方式（不采信自述） |
|---|---|---|---|
| `76bbb46` | w4b | **W4 ②**：C 的数组/对象元素「行内 vs 展开」对齐 Rust；抽出 `is_flat()` / `dump_element()`；顺带修 `tools/dump_c.c` 的 Windows 文本模式 | `cd c && python build_check.py --run`（rc=0 / ALL PASSED）+ `check_dump_parity.py`（行数一致 **7/19 → 10/19**、总差距 13343 → 48） |
| `65b5bfd` | ign | **`.gitignore` 的 `**/_*` 补 9 条精确例外**（`site/static/_headers`、tree-sitter python 绑定的 `__init__.py(i)`、VS Code 6 个发布闸门脚本）；**只加 `!`、不删规则、不放宽既有规则** | 用**退出码**逐条复核（文本会骗人，见 §22.2 第 3 条）：应放行的 9 个不再忽略、应忽略的 10 个仍忽略；`numstat` = **+32/0** |
| `4d34098` | docs | `llms.txt` + 官网中英 13 个页面 + 编辑器 README 跟上本轮功能 | 它的报告**没落盘**（见下方 ⚠️），我自己读 diff 复核：15 个文件代码围栏全部成对、+169/−11 |
| `60fbffa` | vsix | 重打 `editors/vscode/sml-lang-0.4.2.vsix`（包内解析器 45466 → 69404 B） | 解包后**包内 vs 工作区**逐字节比（`src/vendor/sml.mjs` = 69404 B / sha256 前缀 `70f1ee47`）；`git check-ignore` 确认该包未被忽略 |

⚠️ **面板上三个 ✗ 不是它们失败**：产出都在磁盘上，是这台机器的 **PowerShell/AMSI
`AccessViolationException`** 在运行末尾打断（`docs` 连报告都没来得及落盘，所以我只能读 diff 复核）。
**下一位**：派并行任务时要求「**每完成一步就写报告**」，别攒到最后一步。

### 22.2 三条方法论级的教训（这轮真踩出来的，别丢）

1. **「`vendor/` == `js/sml.mjs`」证明不了「包是新的」**：这两件事在本仓库是**分离**的
   （同步脚本会跑，但**没人重打包**）。判据必须落到「**包内 vs 工作区**」——
   旧 0.4.2 就是这么放过期的：**09-07 打的包、09-18 才补提交进库、源码在提交当晚又改了一次**。
2. **查「有没有被引用」不能用 ripgrep / 编辑器全局搜索**：它们遵守 `.gitignore`，会把
   **被测文件自己**漏掉（两次得到「0 引用」的**假信号**）。必须 `git grep`（只搜已跟踪文件）。
   —— 这条同时解释了第一轮 `_` 文件清理为何误判：`.gitignore` 里的 `rust/qsm/**` 是盲区，
   里面「脚本 A 调脚本 B」当时根本看不见（§17.3）。
3. **`git check-ignore -v` 的文本会骗人**：命中 `!` 行表示「**不再**被忽略」。
   判据看**退出码**（`git check-ignore -q` 的 rc）—— 只看文本会把结论判反。

### 22.3 W4 ②③：C ↔ Rust 序列化对齐（`c/sml.c`，本轮）

**装置**：`python tools/check_dump_parity.py`（§21：语料 34 个、C 解析失败 15 个不计入、
Rust 失败 0）。口径：只看「两端都解析成功」的 **19** 个。

| 指标 | 改前 | 改后 |
|---|---|---|
| 「行数与 Rust 一致」 | 10 / 19 | **19 / 19** |
| 总行数差距（19 个文件合计） | **13343 行** | **0 行** |

**② 行内/展开**（`76bbb46`）：判据 = Rust `dump.rs::is_flat` —— **直接子项全是标量**才算扁平，
**只看一层、不递归**（递归版是**恒真判据**；Rust 侧当初写这段时真踩过：编译与测试全绿、
只是完全没生效）。C 侧新增 `is_flat()` / `dump_element()`，`dump_object_body` / `dump_array_body`
抽出复用（与 Rust 的 `dump_object_body` / `dump_block` / `dump_element` 同构）。
**量具本体还有个坑**：`tools/dump_c.c` 的 stdout 在 Windows 默认是**文本模式**，`\n` → `\r\n`
⇒ 逐字节比对**每个文件都必判不一致**（内容里自带 `\r\n` 的文档还会被二次翻译成 `\r\r\n`，
`yuntianming_original.sml` 的行数因此虚高 293 行）。已 `_setmode(_O_BINARY)`。

**③ A 类（dumper 丢掉 `__type`/`__name`）**：C 把这两个键当「内部标记」跳过，Rust 原样输出。
后果不只是少两行 —— **只有元数据的块被判成「空体」⇒ 输成 `{}`**（元数据静默消失）。
现在 `dump_object_body` / `dump_inline` / `sml_dump` 三处**都不筛键**；`obj_has_body()` 的语义
也从「排除元数据键后还有没有键」改成「**是不是非空对象**」（与 Rust `starts_inline` 的
「空对象才同行」对齐，`{}` / 标量仍保留 `: `）。顶层按 Rust `to_sml` 分叉：
`sml_obj_get(v, "__type")` 命中 ⇒ 按块渲染（`\n{` + 逐键 + `}`）。

**③ B 类（解析器把裸块拆成三个数组元素）**：新增 `try_parse_array_bare_block()`，判据与写法
**照抄** Rust 的 `bare_block_ahead()` + `parse_bare_block()`。⚠️ 两个必须照抄的点：
① 入口**只认 `T_WORD`**（Rust 的 `Some(Tok::Str(_))` 分支直接当字符串元素 ⇒ `[ "sec" { } ]`
两端都**不是**块）；② **先 `next()` 消费类型名、再收参数** —— 漏了这一步，类型词会被当成
第一个参数（实测得到 `__name: section` + `__args: [情节]`，我当场踩过一次）。

**顺带补齐「裸块参数不丢」**（Rust P1-3 的行为）：两个以上参数原先**被静默丢弃** ⇒ 现在
首个进 `__name`、其余进 `__args`；**键位置与数组位置同构**（改前键位置连「恰好两个参数」
都既不写 `__name` 也不写 `__args`）。这条是新发现的行为缺口，不是 A/B 表里列的。

**判别实验**（脚本在 `%TEMP%\w4abc_diff.py`，**未入库**）：用 `git show HEAD:c/sml.c` 另编一个
dumper，与改动后在**同一份用例**上对照：

| 用例 | 改前（HEAD） | 改后（工作区） |
|---|---|---|
| `topic 云天明童话 { label: a }` | `topic:` + `label: a`（**元数据丢**） | 带 `__type: topic` / `__name: 云天明童话` |
| `m: [ screen login { width: 320 } ]` | `screen`、`login`、`{ width: 320 }` **三个元素** | 一个块对象 `{ width: 320, __type: screen, __name: login }` |
| `server web prod { x: 1 }`（键位置） | 只有 `x: 1`（**`web`/`prod` 丢**） | `__name: web` + `__args: [prod]` |
| 正对照 `m: [ hello world ]` / `m: [ "sec" { … } ]` / `k: { x: 1 }` | — | **两侧逐字节相同** |

**复验命令**：`cd c && python build_check.py --run`（`=== ALL PASSED ===`）+
`python tools/check_dump_parity.py`（汇总行应显示「行数已一致: 19」）。

**剩余两类 —— 2026-09-19 用户已拍板：都不改实现，只写进文档**：

1. **引号策略 → 「注明」**：C 的 `needs_quote()` 只认空白与 `:`/`#`/`{}`，比 Rust 宽松 ——
   `等价，仅书写风格不同: */`（C）↔ `"…": "*/"`（Rust）、`schemaVersion: 1.1`（C）↔ `"1.1"`（Rust）、
   `hex: 0x20` / `oct: 0o17` / `big: 1_000`（C 裸写、Rust 全加引号）。
   ⇒ 已写进 README 中英的「跨实现差异与约定」+ §3.2，**含一条必须写清的保真后果**：
   宽松策略下「看起来像数字的字符串」回读会**被重新归类**（`schemaVersion: 1.1` 回来是浮点）——
   需要严格保真就显式加引号。
2. **键顺序 → 「约定」**：Rust 的 `Value::Object` 是 `BTreeMap`（键排序输出），C / C++ 保**源序**
   —— **19/19 全命中**。约定 = 「**对象是映射、不是序列；要保序用数组**」，落进
   README 中英 / `llms.txt`（Key facts）/ `ch12-smltools.md` / §3.2。**Rust 侧不动**
   （换保序映射会牵动契约 / include / `@for` 一串按 BTreeMap 写的地方）。

### 22.4 VSIX：✅ **已是新版（2026-09-19 实测确认）**

**过程**：原先本机装的是 `snoware.sml-lang-0.4.1`（`src/vendor/sml.mjs` = 45466 B ⇒ W16 之前的
解析器）；重打 0.4.2 后用户已安装。**实测确认**（不是「应该装好了」）：

| 项 | 值 |
|---|---|
| 已安装 | `snoware.sml-lang-0.4.2`，`src/vendor/sml.mjs` = **69404 B / sha256 `70f1ee47…`** |
| 旧版状态 | `.obsolete` = `{"snoware.sml-lang-0.4.1": true}`；`extensions.json` 记的是 version **0.4.2** |
| 结论 | 编辑器加载的就是新版（0.4.1 目录只是还没被清掉） |

**用「已安装的那份」代码真跑一遍**（`~/.vscode/extensions/snoware.sml-lang-0.4.2/src/sml-parse.mjs`，
不采信「文件在不在」）：

| 能力 | 结果 |
|---|---|
| 定义跳转 `findDefinition(Server, contract)` / `(base, fragment)` | ✅ `{line:0,col:10,length:6}` / `{line:6,col:1,length:4}`；不存在的名字 → `null` |
| 契约展开 `contractInstance` | ✅ `{key:"web", value:{host:"example.com", port:8080, tls:false}}` —— **默认值是解析器真填的** |
| 悬浮 `contractHoverMarkdown` | ✅ 声明段 + 「**填入默认值后的结构** —— 来自块 `w`（第 5 行）」+ 实例 |
| 诊断 / 补全名字集合 | ✅ `[]` / 契约+片段+键全在 |
| 对照：0.4.1 的 `sml-parse.mjs` | ❌ `findDefinition` ✗ `contractInstance` ✗ `contractHoverMarkdown` ✗（**所以「没展开、没跳转」恰好是 0.4.1 的表现**） |

⚠️ **「契约展开看不到」的四个常见原因（都写进扩展 README 了）**：
1. **没重载窗口**（装完扩展必须 Reload，否则扩展宿主还是旧的）。
2. **文档没全绿** —— 展开那一半要求整份文档通过校验（语法 **和** 契约；`contractInstance`
   内部就是 `parseSafe(text)`）⇒ 有任何错误时只显示声明 + 「_未找到可展开的实例_」。
   **先看问题面板。**
3. **光标不在契约名上**（要落在 `@contract Server` 或 `@is Server` 的 `Server`，且该契约
   在**本文档**声明）。
4. **写法不对**：`@is` 要写在**块内**；`web @is Server { }`（块名后紧跟）**不是合法语法** ——
   Rust 报 `E-PARSE-012`、JS 报「多余的结束符号 }」（C/C++ 碰巧接受，但契约语义也不对）。
   **这是排查时我自己先踩的坑**：拿这种写法去测，结论会指向「扩展坏了」。

**装法（留档，重装/换机器时用）**：

```bash
code --install-extension editors\vscode\sml-lang-0.4.2.vsix --force   # 直接装已重打的包
cd editors\vscode && npm run install-local                            # 会先重打包再装（需联网拉 vsce）
```

**版本号口径**（你 2026-09-19 定）：**保持 0.4.2** —— 未上架、手动 `--force` 安装，
且升版本要**同步 4 处**（`package.json` 的 `version` 与 `install-local`、中英 `README` 各一处），
漏一处就立刻制造新漂移。

### 22.5 C++ 同步对齐（`cpp/sml.cpp`，同一批做的）

**为什么多做这一件**：C 改完后用**同一套判据**量了 `cpp/`（它有公开 API `Parser::to_sml`），
发现它**同病、而且多缺一整类**。既然「`__type`/`__name` 要往返」已定为口径，C++ 是兄弟实现，
就一并做 —— **C 已改好，可当逐行参照**。

| 缺口 | C++ 改前 | 改后 |
|---|---|---|
| ② 行内 vs 展开 | 数组元素里的对象**一律**压成 `{ k: v }` 一行（压根没有 `is_flat`/`dump_element`） | 与 Rust `dump.rs` / C `sml.c` **逐函数对应**：`is_flat` / `starts_inline` / `dump_object_body` / `dump_array_body` / `dump_element` / `dump_inline` / `dump_value` / `to_sml` |
| ③ A 类 | dumper **五处**跳过 `__type`/`__name`（含 `has_body` 判据）⇒ 元数据丢；**只有元数据的块被当成空体输成 `{}`** | 全都不筛键；顶层按 Rust `to_sml` 分叉（`v->has("__type")` ⇒ 按块渲染） |
| ③ B 类 | 数组位置裸块被拆成多个元素（`parse_array` 里只有「当标量」一条路） | 新增 `bare_block_ahead()` / `parse_bare_block()`，判据与写法**照抄 Rust** |
| 参数 | 两个以上参数静默丢弃 | 首个 ⇒ `__name`、其余 ⇒ `__args`（**只做了这一半**，见 §22.6） |

⚠️ **本实现把引号串也存成 `Word` token**（`coerce` 里靠 `t[0]=='"' && t.back()=='"'` 判断），
而 Rust 里引号串是独立的 `Tok::Str`（那条分支**不试裸块**）⇒ `bare_block_ahead` 必须**显式排掉**
引号串，否则 `[ "sec" { x: 1 } ]` 会被当成「类型名 `"sec"` 的裸块」。**我第一版就踩了**，
靠用例 t10 当场抓到。

**量具（本轮新写，未入库）**：C++ 没有 parity 工具，仿 `check_dump_parity.py` 写了个极小的
C++ dumper 探针（`%TEMP%\w4cpp_measure.py`），跑同一份语料：

| 指标 | 改前 | 改后 |
|---|---|---|
| 行数与 Rust 一致 | 11 | **24** |
| **行数不同** | **15** | **2** |
| C++ 解析失败 | 7 | **7（未新增）** |

剩下 2 个**都不是本轮引入**，且都已登记（§22.6）：`examples/common.sml`（判据过宽旧账）与
`examples/secrets.sml`（`$` 独立 token 旧账）。

⚠️ **量具本身又踩了同一个坑**：C++ 探针的 `std::cout` 在 Windows 也是**文本模式**，
`yuntianming_original.sml`（内容自带 `\r\n`）被二次翻译成 `\r\r\n` ⇒ 虚高 293 行（741 vs 448）
—— 与 §22.3 里 `tools/dump_c.c` 那处**是同一个错**。已 `_setmode(_O_BINARY)` 修掉。
**下次给任何「dump 到 stdout」的量具都先加这一行**（C 侧已修、C++ 侧是本轮踩的）。

**C++ 原生回归**：`cd cpp && python build_verify.py --allow-skip-rs-bridge` ⇒
`CONTRACT / COMMENTS / LIMITS / CODES` 全 rc=0。⚠️ 带 `--allow-skip-rs-bridge` 是因为本机
`rust/target/release` 下**没有 Rust cdylib**（`SML_RUST_LIB` 默认指仓库内相对路径，而我们的
构建产物在 `E:\snoware-target`）—— 这是 W9 把 RS-BRIDGE 改成 fail-closed 之后的**正常行为**，
不是本轮引入的失败；要跑那一条得先 `cargo build --release` 出 cdylib。

### 22.6 C++ 三处遗留缺陷 —— ✅ **已修（用户拍板「开吧」后按正确顺序修完）**

「收紧即炸」是面镜子：把**键位置**的裸块判据收紧成 Rust 的 `bare_block_ahead` 之后，
一批文件从「静默错解」直接变成 `E-PARSE-006` **解析失败** —— 说明那个过宽判据一直在**吞**它们。
三条都**不是本轮引入**，2026-09-19 按「**先修 ③、再收紧 ①**」的顺序全部修完：

| # | 缺陷 | 修法 | 证据（改前 → 改后） |
|---|---|---|---|
| ① | **键位置裸块判据过宽** + 参数收集**贪心**（吃到 `{`/`}`/`,`，中间任何 token 都算参数） | 判据换成 Rust 的 `bare_block_ahead`；参数只收**词**且经 `coerce` | `examples/common.sml`：C++ **14 行 → 1 行**（= Rust） |
| ② | **`$` 是独立 token** ⇒ 值位置 `$env.X` 退化成「`$` + `env.X`」，值成 null、`env.X` 掉到键位置**多出一个键** | tokenizer **不再切 `$`**（Rust 里 `$` 就是普通词字符；`coerce_word` 里早有 `$env.` 分支，一直没被走到） | `examples/secrets.sml`：`resendApiKey: null` + 凭空多出的键 → **`""`（三个键都对）**，5 行 → **6 行 = Rust** |
| ③ | **`@contract X strict {` 的 `strict` 不被消费** ⇒ 后面那个 `{` 不再是「紧跟契约名」⇒ 整条契约声明被跳过 | 消费 `loose` **与** `strict`（`allow_extra` 分别 true / false） | `gov_demo.sml` 的契约面恢复（错误码回到基线的 `E-CONTRACT-004`，不再是 `E-PARSE-006`） |

**顺带修掉一处真实的静默松口**（修 ③ 时暴露）：契约体**未闭合**原先静默通过
（`if (…) st.i++;` —— 有 `}` 才吃、没有就算了）⇒ 现在报
**`E-PARSE-001 契约体未闭合（缺少结束符号 }）`**，与 Rust 同格；`@contract C {` /
`loose` / `strict` 三种写法都验过（正常闭合的仍解析成功）。

**结果**：C++ ↔ Rust 的「**行数不同 2 → 0**」（26/26 一致，剩余差异全是引号 + 键序 ——
即 §22.3 里已拍板的那两条）；6 个原生 target 保持 rc=0。**8 个解析失败**里：
3 个连 Rust 也拒（`advanced.sml`×2、`slint/login.sml`），5 个与 **C 侧同格**
（`app.sml` / `for_when.sml` / `ui.sml` / `lvgl_demo.sml` / `gov_demo.sml`，都是 C/C++
共同的能力缺口，见 §22.7 与既有登记）。

**定位手法的教训（保留）**：定位分歧要用「HEAD 版 vs 新版**逐行前缀**对比」，判据是
「**两边不同**」而**不是**「新版失败」—— 截断本身会产生「未闭合块」的假失败
（我第一次二分就被这个骗了，差点把结论搞反）。脚本 `%TEMP%\w4cpp_bisect2.py`（未入库）。
另外 `w4cpp_measure.py` 的计数有个陷阱值得记：**C++ 解析失败的文件不会被拿去问 Rust**
（`continue` 在跑 Rust 之前），所以「Rust 失败 0」只代表「C++ 成功的那些里 Rust 都没失败」。

### 22.7 ⚠️ 本轮新登记的两件事（**定位未改**，需拍板）

1. **`examples/slint/login.sml` 是坏样例 —— 所有实现都拒（含 Rust）**：
   它写 `` text: `root.busy ? "登录中…" : "登录"` ``，**反引号里的三目冒号**会被切成结构
   `Colon`、落到键位置 ⇒ Rust / C++ 报 `E-PARSE-006`（C 报 E-PARSE-003）。而该文件**自己的头注释**
   宣称后端支持三目 `` `a ? b : c` ``（`rust/src/emit/slint.rs` 那条约定）⇒ **样例与实现不符**。
   注意根因：**反引号不是字符串定界符** —— Rust / C / C++ 都把它当**普通裸词字符**，
   所以 `` `#0f1117` `` 里的 `#` 还会起注释（值只剩一个反引号）；「原样输出 Slint 表达式」
   只有**裸词内不含空白/冒号**时才走得通。⇒ 两条路：**改样例**（用引号串或拆键）
   或**给语言补「反引号原始串」**。**需拍板**。
2. **行级 `&frag` 展开（splice）：Rust 有，C / C++ 没有** —— 实测 Rust：
   `@base { a: 1 b: 2 }` + `w { c: 3` + `&base }` ⇒ `{"w":{"a":1,"b":2,"c":3}}`（字段**并入父块**）；
   C / C++ 把裸词当键名 ⇒ 多出一个 `&base` 键，**严格契约下报 `E-CONTRACT-004`**
   （`rust/tests/fixtures/gov_demo.sml` 正是这一格；C 侧同码）。
   ⚠️ **顺带发现 README 写反了**：`README.md` / `README.en.md` 关于「块内裸写 `&base`
   **不展开**」的说明与**参照实现的实际行为相反**（`TODO.md` 里那条 `[x]` 也是照这个错前提
   勾掉的）⇒ 本轮**已更正 README 中英**（写明 Rust 是 splice、C/C++ 尚未实现，属跨实现差异），
   并登记 C/C++ 的补齐任务。

### 22.8 VSCode 扩展：特别高亮（临时探照灯）+ 自检升级（2026-09-19）

**需求（用户原话）**：「提供选择 → 右键 → 自定义当前工作区特别高亮」。

**做法**：新模块 `editors/vscode/src/special-highlight.js`（`extension.js` 里一行接线），
搜索逻辑放桥接层 `sml-parse.mjs::findOccurrences` —— **纯函数、可脱离 VSCode 用 node 直测**
（与悬浮/跳转同一个取舍：能在 node 里测的逻辑，别绑在编辑器 API 上）。

| 项 | 内容 |
|---|---|
| 命令 | `sml.specialHighlight`（右键菜单，`when = editorHasSelection && editorLangId == sml`）、`sml.clearSpecialHighlight`（`when = sml.specialHighlightActive`，由代码 `setContext` 维护） |
| 交互 | 再触发同一个词 = 取消；状态栏 `N 处 / M 文件`（触上限标**已截断**），点状态栏即清除 |
| 配置 | `sml.specialHighlight.include`（默认 `**/*.sml`，遵循 `files.exclude`）/ `.caseSensitive` / `.wholeWord` |
| 上限 | 文件 500 / 命中 20000 / 单文件 4 MB（超了**明说截断**，不假装搜全） |

**三段刻意取舍**（写进代码注释与 README，别被「优化」掉）：① **字面**匹配 —— 选中
`(`、`*`、`[` 也按字面找，不当正则（当正则会少命中甚至抛异常）；② **不做语义判断** ——
注释/字符串里的同名文字同样点亮（文本级探照灯的价值在**可预期**，「聪明」在这里是负资产）；
③ 只给**可见编辑器**上色（decorations 的 API 限制），其余文件仍计入统计、打开时按缓存补上。

**踩到的两个环境事实**：① `@types/vscode`（`^1.80`）里**没有 `findTextInFiles`** ⇒ 改用
`workspace.findFiles` + `workspace.fs.readFile`（有类型、SML 工作区小，够用）；
② 扩展自检脚本原先用 `path.resolve("src/…")`，**只有恰好 cd 到扩展目录时才跑得对** ⇒
改成从 `import.meta.url` 推出扩展根目录再 `chdir`。

**顺带升级自检（这一节的隐藏价值）**：`scripts/_verify_ext.mjs` 之前**只打印 ✗ 却始终
`exit 0`** ⇒ `_prepublish.mjs` 里那一步永远显示 ok，**门槛形同虚设**。现在它有：
① `findOccurrences` **七条断言**（跨行定位 / `wholeWord` / 大小写 / 字面特殊字符 / `max` / 空词）；
② **「声明了却没实现的命令」闸门** —— 拿 `package.json` 的 `commands` + `menus` 逐个反查
`registerCommand`（这类 bug 只在用户点下去时才暴露为 command not found，静态就能查，必须查）；
③ 三个 `.js` 的 `node --check` 语法闸门；④ **失败即 rc=1**。

**验证链**：`node scripts/_verify_ext.mjs`（EXT VERIFY ALL PASS）→ `node scripts/_prepublish.mjs`
（PREPUBLISH ALL PASS）→ 重打 VSIX（**20 项 / 134476 B**；包内逐字节核对：除 `readme.md` 的
链接改写外全一致，`src/vendor/sml.mjs` 仍是 69404 B / `70f1ee47…`）。

### 22.9 「重启窗口后还是没反应」→ 造了个**假 VSCode 宿主**，并给扩展装上自检

**背景**：用户报「重启窗口后悬停/选择高亮还是没有」。这类问题此前**无从取证** ——
仓库的扩展测试只跑桥接层（`sml-parse.mjs`），**从不跑 `extension.js`**：provider 有没有注册、
hover 到底返回什么、命令有没有接线，全是盲区。于是本轮造了一个假宿主（`%TEMP%\fakehost.mjs`，
**未入库**）：用 `Module._load` 把 `require("vscode")` 换成 mock，然后**真的 activate 扩展**、
真的调 provider 与命令。

**假宿主当场给出的结论**（全部来自**真代码**，不是猜）：

| 检查 | 结果 |
|---|---|
| `activate()` | 不抛异常 ✓；注册 completion/hover/definition/formatting 各 1、命令 6 条 |
| 悬浮（文档**有**契约错误） | 返回 Hover，但**只有声明段** —— 与设计一致（展开要求文档全绿） |
| 悬浮（文档**干净**） | **两段都在**：声明 + 「填入默认值后的结构 —— 来自块 `web`（第 9 行）」+ 实例（`port: 8080` / `tls: false` 是解析器真填的） |
| 定义跳转 | ✓ 返回 `{line:0, col:10}`（指向 `@contract Server`） |
| 特别高亮 | ✓ 装饰真的落到编辑器（可见文档命中 2 处 → 2 个 range），`sml.specialHighlightActive` 置位 ✓ |
| 自检命令 | ✓ 输出含包内指纹 `69404 B / 70f1ee476ab684fc`、命令注册、语言模式、文档校验失败原因 |

⚠️ **假宿主自己先骗了我一次**：它缺 `Position.translate`，于是定义跳转抛 `TypeError`，
看起来像**扩展有 bug**。去 `@types/vscode` 里核（`Position.translate(lineDelta?, characterDelta?)`
**是真实 API**，d.ts:357/366）才发现是宿主不完整 —— **先核实 API 再改代码**，否则会「修」掉一个
不存在的问题。另一次是 `findFiles` 返回的 Uri 被二次包裹，导致「工作区 0 命中」的假信号。

**因此给扩展补了自检**（`SML: 自检` + 「输出 → SML」面板）：把上表那几项逐条摊给用户 ——
扩展版本、**包内解析器指纹**、命令注册、语言模式、文档校验结果、光标下的词是否契约名、
有无可展开实例（以及**为什么没有**）、能否跳转。悬浮取不到实例时也把原因写进面板
（同一文档版本只解释一次）。**这条同时把「装的是旧包」变成可自查**：以前只能人肉解包比对
（§22.2 第 1 条）。

**给下一位**：`%TEMP%\fakehost.mjs` 是现成的「扩展无 VSCode 集成测试」骨架，要扩就扩它
（mock 里缺 API 就补 mock，**先去 d.ts 确认真实签名**）；`_verify_ext.mjs` 里那条
「声明了却没实现的命令」闸门也是这轮加的，别再让它退化成只打印 ✗ 却 exit 0。

### 22.10 顺着「核对已安装的包」又挖出一个老 bug：**命令面板里一条 SML 命令都没有**

**经过**：用户装完重打的 VSIX 后我按老规矩核「**包内 vs 工作区**」，发现 `package.json`
包内 4250 B ≠ 工作区 4834 B。先别慌 —— 那是 vsce 的**正常行为**（用 tab 重新序列化 + 加
`__metadata` 键），逐行 diff 确认**语义一致**，不是漂移。但在核对「包内声明了哪些命令」时
发现：**`contributes.commands` 根本不存在**，6 条命令全写在**顶层 `commands`** ——
而顶层 `commands` **不是有效贡献点，VS Code 静默忽略**。

后果：**命令面板（F1）里搜不到任何 SML 命令**。右键菜单那几条能用，是因为
`contributes.menus` 是有效的、且菜单引用命令**不要求**命令被声明 —— 典型的「能用一半」，
所以一直没人报。

⚠️ **真正值得记的是「为什么闸门没拦住」**：`_verify_ext.mjs` 那关读的是 **`pkg.commands`**
（顶层）—— **它校验的正是那个没人看的键**。于是「声明了却没实现」这条断言永远在
错误的对象上运行，一路绿灯。**教训**：「测试通过」必须先问「**测的是不是该测的东西**」
（与 §22.2 第 1 条同族：判据落在哪儿，决定了你能不能看见问题）。

**已修**：命令整块搬进 `contributes.commands`（26 行纯位移，diff 干净）；闸门改**双向断言**
（声明→实现、实现→声明，各自互为子集）+ 新增回归闸门「**顶层 `commands` 必须不存在**」。
**反向联调**（`%TEMP%\gate_redteam.py`，原子备份/还原）验证闸门真拦得住：把顶层 `commands`
加回去 ⇒ `✗ 命令声明在 contributes.commands（顶层 commands 是无效键）`、rc=1。

⚠️ **顺手踩的坑**：Windows 的 `cmd`/`findstr` 会把 `A && B ; C` 链式命令**后半段吃掉当参数**
（`FINDSTR: Cannot open ;`），于是「造坏→跑闸门→还原」这种原子操作被拆散，`package.json`
真的被留在坏状态里（备份也备了坏状态）。**结论**：需要「改→测→还原」的联调，一律写进
**一个 python 脚本**里用 `try/finally` 保证还原，别用 shell 链。

### 22.11 「悬停/右键全无反应」的查法 + 「字段组合高亮」的真 bug（2026-09-19）

**先查资料（用户要求）**，三条结论都改变了判断：

| 出处 | 结论 | 对本仓库的意义 |
|---|---|---|
| [Activation Events](https://code.visualstudio.com/api/references/activation-events) | **1.74 起**，你自己贡献的语言/命令/视图/自定义编辑器**会自动激活**，不必写 `onLanguage` | `activationEvents: onLanguage:sml` 冗余但无害；**但语言模式不是 `sml` 时，就什么都不激活** |
| [Contribution Points](https://code.visualstudio.com/api/references/contribution-points) | `contributes.menus` 的 `when` 只影响**该菜单**；`editor/context` 默认分组 `navigation` 排最前（我们在 `navigation@10/11`）；**菜单引用命令不要求命令先声明在 `contributes.commands`** | 与 §22.10 完全吻合：命令面板那条路死了，右键那条路仍活着 |
| [Writing a VS Code extension in ES modules (2025)](https://jan.miksovsky.com/posts/2025/03-17-vs-code-extension.html) | CJS 入口 + **动态 `import()` 载入 ESM** 是社区标准做法（`vscode` 模块只能在 CJS 侧取） | 本扩展「CJS + 动态 import 桥接层」的架构**不是问题源**，可以放心 |

**① 「字段组合的高亮有问题」= 真 bug，已修**：`#key` 整个 match 被 `^\s*` 锚在**行首** ⇒
同一行的第二个及以后的字段一律不着色。用仓库现成的**真实 Oniguruma 探针**
（`scripts/_verify_tokenize.mjs` 的骨架，拷成 `_probe_key.mjs`，`_` 开头不入库）实测：

| 写法 | 改前 | 改后 |
|---|---|---|
| `web { host: a, port: 8080 }` | `host✗ port✗` | `host✓ port✓` |
| `web { host: a port: 8080 }`（无逗号） | `host✗ port✗` | `host✓ port✓` |
| `address { city: Shanghai  zip: "200120" }`（语料原句） | `city✗ zip✗` | `city✓ zip✓` |
| `m: [ { a: 1, b: 2 } ]` | `a✗ b✗` | `a✓ b✓` |
| `a: 1 b: 2` | `b✗` | `b✓` |
| 负向 `m: [ hello, world ]` / `issuer: https://x` / `# 注释里 foo:` | 均无色 | **仍无色** ✓ |

三分支 + 两道保险：`(?<!:)` 挡住值里的冒号（`url: https://…` 的 `https` 不着键色）、
`(?=:)` 挡住数组裸值。**注意**：`_verify_grammar.mjs`（JS 侧镜像）原来读 `repo.key.match`，
改成分支数组后它**静默漏检**（报 `数据键名 -> (none)`）—— 这说明「闸门要跟着结构走」，
已改成多分支联合 + 全局扫描 + 新增 3 条用例。

**② 「悬停/右键都无效」的头号真身：文件没被当成 SML**。语言模式 ≠ `sml` ⇒ provider 不被调用、
右键项被 `when` 藏掉、语法也不生效 —— **三件事一起坏**，与「扩展坏了」无法区分。已加：
`workspaceContains:**/*.sml` 激活事件（否则语言模式不对时扩展压根不激活，检查跑不到 =
鸡生蛋）+ 语言守卫（发现 `.sml` 却是别的语言模式就弹警告并给「设为 SML」按钮）+
自检面板报 `SML 语言已注册：✓/✗`（✗ = 扩展没被加载）。假宿主里做了**全链路验证**：
`plain.sml`（languageId=`plaintext`）→ 警告弹出 → 点「设为 SML」→ `languageId` 变 `sml` ✓。

⚠️ **新踩的坑（比上次那个更隐蔽）**：`修改→跑闸门 | findstr ... && 打包` —— **管道会把退出码
吃掉**（`A | findstr` 的 rc 是 findstr 的）。于是 `_prepublish.mjs` 明明报了
`FAIL grammar 正则层 / 1 FAILED`（真闸门，`exit(failed?1:0)`），后面的 `vsce package`
**照样执行了**，只有人眼能从输出里看出不对。**结论**：依赖退出码的串联，一律**不加管道**；
要过滤输出就重定向到文件、跑完再读文件。

### 22.12 🔴 本扩展**从来没有真正激活过**：顶层读了 VS Code 1.138 已移除的 API

**用户反馈**：「悬停无效」。此时语言模式已确认是 `SML`（状态栏截图），禁用列表里也只有
`snoware.soup-lang`（不是 sml-lang）。**别再猜了 —— VS Code 把一切都写在磁盘上**：

| 去哪读 | 能读到什么 |
|---|---|
| `%APPDATA%\Code\logs\<会话>\window*/exthost\exthost.log` | **扩展激活记录**：`ExtensionService#_doActivateExtension snoware.sml-lang, startup: false, activationEvent: 'onLanguage:sml'`，紧跟 `[error] Activating extension … failed due to an error:` + **完整调用栈** |
| `…\window*/exthost\output_logging_*/<n>-<面板名>.log` | **每个输出面板的落盘**（我们自己那个会叫 `*-SML.log`）。⚠️ **没有这个文件 = 扩展从未激活**（面板只在 activate 时创建） |
| `%APPDATA%\Code\User\globalStorage\state.vscdb`（sqlite） | `extensionsIdentifiers/disabled` —— **被禁用的扩展**（本次排除了 `snoware.soup-lang`） |
| `…\workspaceStorage\<hash>\state.vscdb` | 工作区级禁用 / 受信任状态 |

**证据链**（全部来自上面第一、二行）：

```
2026-09-19 07:34:21.617 [info] ExtensionService#_doActivateExtension snoware.sml-lang, activationEvent: 'onLanguage:sml'
2026-09-19 07:34:21.622 [error] Activating extension snoware.sml-lang failed due to an error:
2026-09-19 07:34:21.622 [error] TypeError: Cannot read properties of undefined (reading 'Snippet')
    at Object.<anonymous> (…\snoware.sml-lang-0.4.2\src\extension.js:78:47)   ← 模块**加载**阶段
```
日志最早一条是 **0.4.1 / 09-18 20:30**（`…0.4.1\src\extension.js:55:47`，同一句），
0.4.2 也一样 ⇒ **这个扩展从来没有成功激活过**。而且没有 `*-SML.log` ⇒ 输出面板也没建。

**根因**：第 78 行第 47 列正是 `insertTextFormat: vscode.InsertTextFormat.Snippet,` 的
`.Snippet` 访问点 ⇒ **`vscode.InsertTextFormat` 在 VS Code 1.138 里根本不存在**。查证（本机
`d:\Microsoft VS Code\7debcd0e2a\resources\app`）：
* `out\vs\workbench\api\node\extensionHostProcess.js`（宿主 bundle）里 `InsertTextFormat` **0 次**，
  而 `CompletionItemKind` 3 次、`DiagnosticSeverity` 1 次；
* 自带的 `out\vscode-dts\vscode.d.ts` 里**没有** `enum InsertTextFormat`（`enum CompletionItemKind` 有）。

**为什么自检/假宿主都没发现**：`@types/vscode@^1.80` 是**编译期**口径（它当然有该枚举），
而我的假宿主是我照着 `@types` 手写的 mock —— **我把不存在的 API 也 mock 进去了**，
于是「跑得好好的」。**假宿主的 API 面必须对齐目标宿主，而不是对齐类型声明。**

**已修**：`INSERT_SNIPPET = 2` / `INSERT_PLAIN = 1`（线上协议数值）+ `vscode.CompletionItemKind`
改经 `CK(name, fallback)`；顶层护栏注释；`_verify_ext.mjs` 加「**已被移除的 API**」黑名单
（现含 `InsertTextFormat`，先剥注释再匹配）；假宿主**故意删掉** `InsertTextFormat`（对齐 1.138），
顶层再读它就当场 `activate` 抛异常 —— 这条回归闸门以后能自动抓住同类问题。

**给下一位的三条结论**：
1. **顶层（模块作用域）永远不要直接读 `vscode.<枚举>.<成员>`** —— 宿主会移除枚举，一读就炸**整个模块**，
   而且 `activate` 连执行机会都没有，报错只在日志里。要么经 helper 取、要么用协议数值。
2. `engines.vscode: ^1.80.0` 只保证**不会装到更老的**宿主上，**不保证**你用到的 API 在新宿主上还在。
   定期拿**目标宿主**的 `vscode.d.ts`（`<安装目录>\<hash>\resources\app\out\vscode-dts\vscode.d.ts`）
   把 `src/*.js` 里所有 `vscode.<名字>` 抽出来对一遍（本次脚本：`%TEMP%\audit_api.py`，未入库）。
3. 排查「扩展没反应」**先读 `exthost.log` 与 `output_logging_*`**，别再从界面猜：前者给你激活失败与
   调用栈，后者「有没有那个面板文件」直接说明激活有没有走到创建面板那一步。
