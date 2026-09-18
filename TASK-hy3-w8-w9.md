# TASK-hy3-w8-w9：W8（Zed 扩展）+ W9（安全门禁进 CI）

> **这份文档是给「不需要判断力」的执行者写的。** 每条都给了：**改哪个文件哪一行**、
> **改成什么**、**跑什么命令**、**期望看到什么**、**不符怎么办**。
> 行号是 **2026-09-18 提交 `d88c5b7` 时的实测值**；读到的行对不上 ⇒ **停下问人**，不要猜。
>
> 两个任务**互相独立**，可以分开做、分开提交。**建议先做 W8**（不碰 CI，风险更低）。
>
> **本文件里有两处「决定权不在你手里」的地方**（W8-5 的 `extension.toml` 取值、
> W9-4 的分支保护），都标了 🛑 **停下问人**。除此之外不许自己发挥。

---

## 0. 铁律（违反就是失败）

1. **不许自己发挥**：不重构、不「顺便优化」、不改与任务无关的行。
2. **不许 `git add -A` / `git add .`**：只 `git add <你明确改过的文件>`。
3. **不许 `git push`**：提交完就停。CI 要推送后才会跑 —— 那一步是**用户**做的。
4. **不许动**：`.codebuddy/`、`rust/qsm/**`、`Desktop\sml_secret\`（私有资产库）、
   `.gitignore` 里「私有报送件 / 内部报告」那一段。
5. **不许动 GitHub 仓库设置**（分支保护、Secrets、Actions 权限）—— 那是用户的事。
6. **临时文件一律以 `_` 开头**（`.gitignore` 的 `**/_*` 会挡住它们，不会污染仓库）。
7. 任何一步**实际输出与本文档不符** ⇒ 停下，把「命令 + 完整输出」贴出来问人。
8. 一次只做一条，做完立刻验证并贴证据，**不要攒着一起改**。

---

## 1. 环境与开工前检查

```bash
cd c:\Users\sakeen\Desktop\sml
git status --short              # 必须干净；有东西就先停下问人
python tools/check_private_assets.py    # 必须 OK
```

本机已有的工具（**已实测**）：

| 工具 | 位置 / 版本 | 备注 |
|---|---|---|
| node / npx / npm | `D:\SWE\nodejs\`（node v24.13.1） | tree-sitter CLI 走 `npx` |
| cargo / rustc | `C:\Users\sakeen\.cargo\bin`（1.98.1 stable） | Miri 需要 **nightly** |
| gcc / g++ | msys64（gcc 16.1.0） | 跑 C/C++ 用；PATH 里要先加 `C:\msys64\ucrt64\bin` |
| luajit | `C:\msys64\ucrt64\bin\luajit.EXE` | Lua 用例 |
| **tree-sitter CLI** | **没装**（PATH 里没有） | W8-1 解决 |

⚠️ **C 盘只剩约 1.6 GB**：Rust 构建必须 `set CARGO_TARGET_DIR=E:\snoware-target`；
`npx` 下载的 tree-sitter CLI 只有几 MB，可以直接下。

---

# 任务 W8：Zed 扩展（`editors/zed/`）

## W8.0 现状（已实测，别重复劳动）

| 东西 | 状态 |
|---|---|
| `editors/zed/extension.toml` | 存在（1653 B）。`id/name/version/schema_version/description` 齐全；**`[grammars.sml]` 里是占位值**（`file:///REPLACE/...`、`rev = "REPLACE_WITH_COMMIT_SHA"`）—— 这是**有意为之**，见该文件第 18–19 行注释 |
| `editors/zed/languages/sml/config.toml` | 齐全（`grammar = "sml"`、后缀、注释符、缩进、word_characters） |
| `editors/zed/languages/sml/highlights.scm` | 存在（1241 B），与 `grammar.js` 靠**节点名**耦合 |
| `editors/zed/grammars/sml/grammar.js` | 存在（5204 B），**手写、从未编译过** |
| `grammars/sml/src/*` | **全都不存在**（`grammar.json` / `parser.c` / `node-types.json`）⇒ 从没跑过 `tree-sitter generate` |
| 语料 | `grammars/sml/test/parse/{basic.sml, advanced.sml}` + `README.md` |
| `package.json` / `Cargo.toml` / `binding.gyp` | 都不存在 |

**验收标准（来自 `TODO.md` 的 W8 行，原文）**：
> `tree-sitter generate && tree-sitter parse test/parse/*.sml` 无 `ERROR`；`extension.toml` 指向可用 grammar

---

## W8.1 装 CLI（钉版本，别用 latest）

```bash
cd c:\Users\sakeen\Desktop\sml\editors\zed\grammars\sml
npx --yes tree-sitter-cli@0.22.6 --version
```

- **为什么钉 `0.22.6`**：0.25 起的 CLI 要求仓库里有 `tree-sitter.json`，会顺带引入一批
  新文件；本任务只想验证语法，不想改仓库布局。若你要用别的版本，**在报告里写明版本号**。
- **期望**：打印出版本号（如 `tree-sitter 0.22.6`）。
- **不符怎么办**：若报网络错 / 找不到包 ⇒ 停下贴输出。若某版本要求 `tree-sitter.json`
  ⇒ 停下问人（不要自己造那个文件）。

## W8.2 生成 parser

```bash
cd c:\Users\sakeen\Desktop\sml\editors\zed\grammars\sml
npx --yes tree-sitter-cli@0.22.6 generate
```

**期望**：生成下列文件（逐个 `ls` 确认，并把大小记下来）：

| 文件 | 说明 |
|---|---|
| `src/grammar.json` | 语法 JSON（几十 KB） |
| `src/node-types.json` | 节点类型（几十 KB） |
| `src/parser.c` | **生成的 C 解析器（1–3 MB）** ← 见 W8-4 的决策点 |
| 可能还会生成 | `binding.gyp`、`Cargo.toml`、`package.json`、`bindings/`（不同版本不同） |

- **若 `generate` 报语法错误**（它很啰嗦，会指到 `grammar.js` 的某条规则）：
  说明 `grammar.js` 有问题（比如 tree-sitter 的正则不支持环视 —— README 的「已知限制 3」
  已有记载）。**先只改报错点，改完重跑**；不许顺手重写规则。
- **⚠️ 不许改节点名**：`highlights.scm` 按节点名匹配，`grammar.js` 文件头有一张
  「节点名契约」表。改名 = 高亮静默失效。要改名就得同时改 `highlights.scm` 并说明理由。

## W8.3 用两份语料验证「无 ERROR」

```bash
cd c:\Users\sakeen\Desktop\sml\editors\zed\grammars\sml
npx --yes tree-sitter-cli@0.22.6 parse test/parse/basic.sml
npx --yes tree-sitter-cli@0.22.6 parse test/parse/advanced.sml
```

**期望**：输出是语法树（S-expression），**全文不含 `ERROR`**（也留意 `MISSING`）。

- **判定口径**：`grep -i "ERROR"` 应无命中；`MISSING` 视为**警告**，出现就要在报告里列出
  （说明某条规则缺了可选/必需子节点）。
- **有 ERROR 怎么办**：逐条定位 —— 命令会打印形如
  `(ERROR (…))` 的片段并给出行列位置；打开 `test/parse/<那一个>.sml` 看那一行的写法，
  再去 `grammar.js` 补规则。**每次只改一处，改完重跑两条命令**。
  若某个 ERROR 你判断需要改动已定的节点名契约 ⇒ **停下问人**。
- **必须留证据**：把两条命令的完整输出存成
  `%TEMP%\w8_parse_basic.txt` / `%TEMP%\w8_parse_advanced.txt`（报告里要贴）。

## W8.4 🛑 决策点之一：生成的 `src/parser.c` 要不要提交？

**事实**：Zed 的扩展在安装时要**编译 grammar**，所以 grammar 仓库里**必须有生成的
`src/parser.c`**（这是 tree-sitter 生态惯例）。代价是本仓库会大 ~1–3 MB（`parser.c`
是巨大的 C 文件）。

**你的动作**：**先不要提交**，报告里给出 `src/parser.c` 的**确切字节数**与
`git status --short` 看到的新文件清单，**然后停下问人**（用户要决定：提交进 monorepo，
还是把 `grammars/sml/` 拆成独立仓库 `snoware/tree-sitter-sml` 再提交 —— 见 W8.5）。

## W8.5 🛑 决策点之二：`extension.toml` 的 `[grammars.sml]` 填什么

**事实（已查官方文档 + 现状注释）**：Zed 文档里 `[grammars.<name>]` **只记录
`repository` + `rev`**，没有 `path` 字段（本地开发允许 `file://`）。而本仓库是
**monorepo**（grammar 在 `editors/zed/grammars/sml`）⇒ 只有两条路：

| 方案 | 填法 | 适用 |
|---|---|---|
| A. 本地开发 | `repository = "file:///C:/Users/<你>/Desktop/sml/editors/zed/grammars/sml"`、`rev = "<该目录所属仓库的 HEAD 短 sha>"` | 只在本机 Zed 里看得见效果。**写死本机绝对路径，不适合提交到公开仓库** |
| B. 正式发布 | 把 `grammars/sml/` 拆成独立仓库（建议 `snoware/tree-sitter-sml`），然后 `repository = "https://gitee.com/snoware/tree-sitter-sml"`、`rev = "<该仓库某个 commit sha>"` | 给用户装扩展时用 |

**你的动作**：**不要自己选**。在报告里把上面两行填法**原文粘出来**，并给出当前
`git rev-parse --short HEAD` 的值，**停下问人**。⚠️ 现在文件里的占位值是**故意**留的
（该文件 18–19 行注释 + README「已知限制 2」），你把它填成某台机器的路径 = 犯错。

## W8.6 文档收口（W8-2/W8-3 全绿之后）

| 改哪 | 怎么改 |
|---|---|
| `editors/zed/README.md` | 「已知限制 1」现在写着「**未在本机跑过 `tree-sitter generate`**…`grammar.js` 是按规范手写、未编译验证的」。改成**事实陈述**：哪天、用哪个 CLI 版本、跑了两条什么命令、结果「两份语料均无 `ERROR`」。**不要再写「未验证」** |
| `CHANGELOG.md` | `## [未发布]` 的 `### 新增`（或 `### 文档 / 工具链`）加一小段：Zed grammar 首次通过 `tree-sitter generate` + `parse` 验证（附 CLI 版本与语料） |
| `TODO.md` | W8 行改成 `✅ **已完成（日期）**`，并把「待办」描述换成实测结论；`extension.toml` 那半句**按用户决定的结果**写（若还没定，写「占位值待用户决定，见 TASK-hy3-w8-w9.md §W8.5」） |
| `HANDOFF.md` | 追加一节（或往 §0 基线表补一行）：Zed 侧现状 + 复现命令 |

## W8.7 验收清单

- [ ] `npx --yes tree-sitter-cli@<版本> --version` 有输出（版本记进报告）
- [ ] `generate` 成功，`src/grammar.json`、`src/node-types.json`、`src/parser.c` 存在
- [ ] `parse test/parse/basic.sml` 与 `parse test/parse/advanced.sml` **均无 `ERROR`**（输出留档）
- [ ] `grammar.js` 若被改：节点名契约（文件头那张表）**没动**；动了就必须同步 `highlights.scm`
- [ ] `editors/zed/README.md` 的「已知限制 1」已改成已验证的事实
- [ ] CHANGELOG / TODO / HANDOFF 三处收口
- [ ] 🛑 两处决策点（W8.4 的 `parser.c`、W8.5 的 `extension.toml`）**已停下问人**，没有擅自填值
- [ ] 提交按主题一笔（`editors/zed/**` + 文档），**逐文件 add**，**没 push**

---

# 任务 W9：安全门禁进 CI

## W9.0 现状（已实测）

| 东西 | 状态 |
|---|---|
| `.github/workflows/` | **只有 `sync-from-gitee.yml`**（Gitee→GitHub 单向镜像）。**没有任何测试/门禁 workflow** |
| 权威源 / CI 在哪跑 | 注释写明：**Gitee 是权威源，GitHub 是镜像 + CI** ⇒ CI 文件放在 GitHub 侧（就是本目录） |
| `rust/miri_check.py` | 存在（147 行）。**逐用例**跑 `cargo +nightly miri test --test <目标> -- --exact <名字>`，带超时，写 `miri_result.txt`，**FAIL/TIMEOUT 时 `return 1`** ✓ |
| `rust/osv_check.py` | 存在（51 行）。查 `Cargo.lock` 依赖的 OSV 漏洞，**⚠️ 只打印 `OK` / `WARN`，永远 `rc=0`** —— 作为门禁它现在**挡不住任何东西** |
| 非 Rust 实现的自检脚本 | `c/build_check.py`（`CC="gcc"`，跨平台 ✓，**但必须在 `c/` 里跑**）、`cpp/build_verify.py`（第 6 行把 `C:\msys64\ucrt64\bin` 塞进 PATH，Linux 上无害；**⚠️ 但会假绿**：RS-BRIDGE 失败仍 `sys.exit(0)`、且默认 `SML_RUST_LIB` 写死 `E:/snoware-target/release` —— 见 **W9.1b**，实测过）、`js/probe-error-codes.mjs`、`tools/check_js_copies.py`、`lua/run_check.py`（按 `luajit→lua5.4→lua54→lua` 找解释器 ✓） |
| 私有资产守卫 | `tools/check_private_assets.py` ✓（依赖 **git 全历史**） |

**验收标准（来自 `TODO.md` 的 W9 行，原文）**：
> CI 里跑得起来，失败能挡住合并

## W9.1 先修 `osv_check.py`：让它真的会失败

**改哪**：`rust/osv_check.py`（整文件 51 行）
**现状（已读代码确认）**：结尾是
```python
if not vulns:
    print("OK: OSV 未发现任何已知漏洞")
else:
    print(f"WARN: 发现 {len(vulns)} 条：\n")
    ...
```
—— 没有 `sys.exit(1)`，所以 `rc` 恒为 0。

**改成**（保守、可回退）：

1. 末尾补上退出码：发现漏洞 ⇒ `sys.exit(1)`；
2. 加一个开关 `--allow-vuln`（或 `--warn-only`）供本地排查时用（CI 不带它 ⇒ 默认失败）；
3. **网络失败不要静默变 OK**：现在单个包查询失败只打 `[!] … 查询失败` 然后 `continue`
   —— 那是「查不到 ≠ 没漏洞」。改成**计数**，结束时若 `查询失败数 > 0` 也报失败
   （保守口径 = fail-closed；若你担心 CI 网络抖动，就加 `--allow-network-error` 明确豁免）。

**怎么验**：
```bash
cd c:\Users\sakeen\Desktop\sml\rust
python osv_check.py ; echo "rc=$?"         # 当前依赖干净 ⇒ 期望 rc=0
python osv_check.py --allow-vuln ; echo $?  # 开关能用
```
并**构造一次失败**来证明它真能挡（最容易的办法：临时把 `--fail-on-vuln` 逻辑对着一个
已知有洞的假清单跑；或用 `python -c` 直接调它的判断函数）—— 报告里要写出你**怎么证明
「有洞时 rc≠0」**的。**不许**只说「加了 sys.exit(1)」。

## W9.1b 再修 `cpp/build_verify.py`：它会**假绿**（实测）

**这是我在写这份文档时实测出来的**，也是 W9「失败能挡住合并」的前置条件 ——
一个「失败也返回 0」的脚本进了 CI，门禁就是摆设。

**改哪**：`cpp/build_verify.py`（79 行）

**现状（已读代码 + 实测确认）**：

```python
67: RUST_LIB = os.environ.get("SML_RUST_LIB", r"E:/snoware-target/release")
68: rc = run(["g++", ..., "-L" + RUST_LIB, "-lsml"])
70: if rc == 0:
74:     print("RS-BRIDGE rc=%d" % rc2)
75:     if rc2 != 0: sys.exit(rc2)
76: else:
77:     print("RS-BRIDGE 跳过: 未找到 Rust cdylib (设置 SML_RUST_LIB 指向 cargo target/release)")
79: sys.exit(0)
```

两个问题：

1. **fail-open（假绿）**：RS-BRIDGE 的编译/链接**失败**时，只打印一句「RS-BRIDGE 跳过」
   然后一路走到 `sys.exit(0)` ⇒ **整体 rc=0**。实测：把仓库根与 `cpp/` 里各跑一次，
   两次都出现 `ld.exe: … No such file` + `collect2.exe: error: ld returned 1 exit status`，
   而**两次的返回码都是 0**（前面四个 target 打印 `rc=0`，RS-BRIDGE 那格静默跳过）。
   ⇒ 也就是说：**现在「六 target 全 rc=0」这句话是不成立的，命令却在报成功**。
2. **机器相关的硬编码**：默认值 `E:/snoware-target/release` 是**本机**路径
   （`rust/` 是 §三·六 清理过一轮「硬编码绝对路径 = PII/环境耦」的，这里是个漏网的同类）。

**改成**：

- RS-BRIDGE 不允许静默跳过：**要么真跑成功，要么整体失败**。
  若确实需要「本地没构建 Rust 库时先跳过」，就加**显式开关**（例如 `--allow-skip-rs-bridge`），
  **CI 不带这个开关**（于是 CI 里缺库 = 失败，而不是绿）。
- `SML_RUST_LIB` 的默认值别再写死盘符：改成**仓库内相对推导**
  （例如 `<repo>/rust/target/release`，用 `HERE`/`__file__` 推），或**没设就报错退出**。
- ⚠️ 别顺手改前五个 target 的逻辑（它们**是会传导 rc 的**：`if rc2 != 0: sys.exit(rc2)`）——
  本任务只修 RS-BRIDGE 这一格 + 那个默认值。

**CI 里怎么配套**（写进 `non-rust` job 或单独 job）：

```bash
cd rust && cargo build --release          # 产物名以 rust/Cargo.toml 为准，自己确认
SML_RUST_LIB="$PWD/rust/target/release" python cpp/build_verify.py
```

⚠️ **产物名要你自己核实**（`-lsml` 对应 `libsml.a` / `sml.dll` / `libsml.so`，
以 `rust/Cargo.toml` 的 `crate-type` 与 `[lib] name` 为准）—— **不要照抄我这句话**。

**怎么验**：
1. 把 `SML_RUST_LIB` 指向一个**不存在的目录**跑一次 ⇒ 期望**整体 rc≠0**（修好之前是 0）。
2. 正常路径跑一次 ⇒ 期望 `RS-BRIDGE rc=0`，整体 rc=0。

## W9.2 新建 `.github/workflows/ci.yml`

**⚠️ 先读这条坑**：`sync-from-gitee.yml` 的文件头记着一件真事 ——
**workflow 的 `name:` 里不要用特殊字符（例如箭头 `→`），否则 GitHub 会静默拒绝解析
整个 workflow 文件**（`gh workflow run` 报 404）。所以新文件的名字与 `name:` 都用纯中文/ASCII。

**要求**（这是验收的核心，照抄结构，别自行扩范围）：

| job | 跑什么 | 关键细节 |
|---|---|---|
| `rust` | `cargo test --workspace`（在 `rust/` 里） | 工作目录 `rust/`；可选 `Swatinem/rust-cache@v2` |
| `rust-serde` | `cargo test --features serde --test serde_bridge` 与 `cargo test -p sml-value --features sml,serde`（在 `rust/`） | 这两条**不在** `--workspace` 里（默认不开 serde），必须单独列 —— HANDOFF §0 有记 |
| `non-rust` | C：**`working-directory: c`** 里跑 `python build_check.py --run`；C++：`python cpp/build_verify.py`（脚本内部自己 `cwd=HERE`，从哪跑都一样）；`node js/probe-error-codes.mjs`、`python tools/check_js_copies.py`（仓库根）；`sudo apt-get install -y luajit` 后 `python lua/run_check.py`（仓库根） | ⚠️ **C 的脚本必须在自己目录里跑**：实测从仓库根跑会 `cc1.exe: fatal error: sml.c: No such file or directory`（三行全红）。ubuntu 自带 gcc/g++/node/python3 |
| `guards` | `python tools/check_private_assets.py`；`python errors/gen_json.py` 与 `python errors/gen_codes.py` 后跟 `git diff --exit-code`（生成物幂等） | **`actions/checkout` 必须 `fetch-depth: 0`**（守卫要全历史），这条最容易漏 |
| `miri` | `rustup toolchain install nightly --component miri` → `cargo +nightly miri setup` → `python miri_check.py --test c_abi --timeout 300` | **工作目录必须是 `rust/`**（脚本内部用 `cwd="."`）；给 job `timeout-minutes: 60`；Miri 很慢，但**不许**设 `continue-on-error` |
| `osv` | `python osv_check.py`（在 `rust/`） | 依赖 W9.1 的退出码 |

**共用要求**：

- 每个 job 用 `runs-on: ubuntu-latest`，`python3` 直接可用（用 `python` 还是 `python3`
  取决于 runner 里的软链；**写 `python3` 更稳**，若脚本内部用 `sys.executable` 则无所谓）。
- **失败必须传导**：每个 `run:` 步骤的 `rc≠0` ⇒ job 失败 ⇒ 挡住合并。不要写 `|| true`、
  不要 `continue-on-error`。
- 触发：`push` 到 `main`/`master` + `pull_request` + `workflow_dispatch`（照抄
  `sync-from-gitee.yml` 里已有的触发写法）。

**怎么验（在本地，不许 push）**：

1. **YAML 语法**：`python -c "import yaml,io;print(yaml.safe_load(io.open('.github/workflows/ci.yml',encoding='utf-8').read())['jobs'].keys())"`
   （没有 pyyaml 就先 `pip install pyyaml`；**只做语法检查，不做真跑**）。
2. **每条命令在本地先单独跑通一遍**（Windows 上）：Miri 那条**只跑 1 个用例**
   证明脚本可跑（`python miri_check.py --test c_abi --skip` 或临时用 `--timeout 60`
   观察前几个），**不要**在本地跑全量（会很久）。
3. **明确写下你不能验证的部分**：CI 真跑要等用户推送到 GitHub；本地没有 Docker，
   `act` 大概率不可用 ⇒ **不要试图伪造 CI 结果**。报告里写「本地验到哪一步、剩下的等推送」。

## W9.3 别忘了「非 Rust 实现扫描」的语义

`TODO` 的 W9 行把「非 Rust 实现扫描」与 Miri / 安全门禁并列。**在本仓库的语境里它就是**
上面 `non-rust` job 那一组各端自检（C/C++/JS/Lua 的用例里含安全相关格子：C/C++ 的
`LIMITS`/`CODES`、JS 的探针、Lua 的检查）。
**不要**去引第三方扫描器（clang-tidy / cppcheck / semgrep 之类）—— 那会引入新的工具链与
误报面，**超出本轮范围**。若你认为必须加，**停下问人**。

## W9.4 🛑 决策点：让「失败能挡住合并」

workflow 只能让 job **变红**；「挡住合并」要 **GitHub 分支保护**（Settings → Branches →
Require status checks to pass）—— **这是用户在网页上做的事，你不许动**。

**你的动作**：报告里给出一张**给用户的清单**：新增了哪个 workflow、job 名各是什么
（分支保护要按 job 名勾选）、需要用户推送后到 Actions 页面确认哪几个 job 变绿、
以及「若某 job 在 CI 上因为环境差异红了，把日志贴回来」的请求。

## W9.5 文档收口

| 改哪 | 怎么改 |
|---|---|
| `CHANGELOG.md` | `## [未发布]` 加一段：新增 CI（列出 job 与各 job 的作用）；`osv_check.py` 行为变更（**从「永远 rc=0」改成「有洞即失败」**——这是**行为变更**，必须写进 CHANGELOG） |
| `TODO.md` | W9 行改 `✅ 已完成（日期）`；把「残余风险」描述换成实测结论；把「分支保护待用户配置」写进备注 |
| `HANDOFF.md` | §5（验证命令）补一节「CI」：本地怎么复现每个 job 的命令；并记下「CI 只在 GitHub 跑、Gitee 是权威源」 |
| `errors/README.md` | 不用改（与错误码无关） |

## W9.6 验收清单

- [ ] `rust/osv_check.py` 有洞时 `rc≠0`，且你**证明了**这一点（写出证明方法）
- [ ] `rust/osv_check.py` 查询失败不再静默当 OK（或明确豁免开关）
- [ ] `cpp/build_verify.py` **不再假绿**：`SML_RUST_LIB` 指向不存在的目录时**整体 rc≠0**（改前实测是 0）
- [ ] `cpp/build_verify.py` 的 `SML_RUST_LIB` 默认值不再是写死的盘符路径
- [ ] CI 里 RS-BRIDGE **真跑**（先在 `rust/` 构建 cdylib + 显式传 `SML_RUST_LIB`），不是被跳过
- [ ] `.github/workflows/ci.yml` 存在，`name:` 里**没有**特殊字符
- [ ] job 覆盖：`rust` / `rust-serde` / `non-rust` / `guards` / `miri` / `osv`
- [ ] `checkout` 用了 `fetch-depth: 0`（守卫要全历史）
- [ ] `miri` 与 `osv` 的工作目录是 `rust/`；`miri` 有 `timeout-minutes`
- [ ] 没有任何 `continue-on-error` / `|| true`
- [ ] YAML 语法检查通过（贴输出）
- [ ] 每条命令在**本地**单独跑过（Miri 只验到「脚本可跑」），并在报告里写清**哪些没能本地验证**
- [ ] 🛑 分支保护清单已写给用户，**没有**自己动仓库设置
- [ ] CHANGELOG / TODO / HANDOFF 三处收口
- [ ] 提交按主题一笔（`rust/osv_check.py` + `.github/workflows/ci.yml` + 文档），**没 push**

---

## 2. 什么时候必须停手问人

1. 本文档任何一条的**实测结果与文档不符**（行号、文件是否存在、输出不符）。
2. W8：`tree-sitter generate` 要求 `tree-sitter.json` 之类的额外布局文件。
3. W8：修 ERROR 需要**改动节点名契约**（会连带 `highlights.scm` 失效）。
4. W8：两处 🛑 决策点（`parser.c` 是否入库、`extension.toml` 填什么）。
5. W9：想引入第三方扫描器 / 想改 `cpp/build_verify.py` 里那句 msys PATH 之外的东西。
6. W9：任何需要**推送**、改 GitHub 设置、或要动 Secrets 的动作。
7. 任何一步改了 **3 次**还是达不到期望。

**停下来的报告格式**：`我在做 W?-.?` + `命令` + `完整输出` + `我卡在哪一步的判断上`。

---

## 3. 附：为什么这么规定（供你判断合理性）

- **W8 的核心风险不是「写不出语法」，而是「改坏了高亮」**：`highlights.scm` 与
  `grammar.js` 靠**节点名**耦合，改名不会报错、只会静默失效。所以 W8.2 明确写「不许改节点名」。
- **W8 的两处决策点是用户的**：`src/parser.c`（体积 ~1–3 MB，进 monorepo 还是拆仓库）与
  `extension.toml` 的取值（写死本机路径 = 公开仓库里的坏味道；正式发布要先拆仓库）。
  这两件事**信息不足**（用户没说要发布、也没说拆不拆），执行者不该替用户决定。
- **W9 的核心风险是「假绿」，而且已经抓到两个实例（都是实测，不是推测）**：
  ① `rust/osv_check.py` 永远 `rc=0`；
  ② `cpp/build_verify.py` 的 RS-BRIDGE 编译失败时只打印「跳过」然后 `sys.exit(0)` ——
  实测 `ld returned 1 exit status` 而整体返回码**仍是 0**。
  直接把它们塞进 CI，会得到一个「永远绿、什么也不挡」的门禁 —— 这正是 `TODO` 里
  W9 要解决的「残余风险」。**门禁的全部价值就在「失败真的会红」这一条上。**
- **W9 的第二个风险是「本地验不了」**：CI 要推送后才真跑。所以本文档要求
  「把能本地验的验到、把不能验的写清楚」，而不是编一个「CI 全绿」的结论。
