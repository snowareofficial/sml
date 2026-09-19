# 变更日志

本文件从 **0.6.1** 起开始记录；更早的变更见 Gitee 提交历史与各次 release 说明。

格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)。
版本号遵循 Cargo 的语义化版本解释：**0.x 阶段 MINOR 变化视为不兼容**（0.6 → 0.7 会让下游无法自动更新），
PATCH 为兼容新增 —— 因此「新增后端 / 新增 API」走 PATCH（0.6.0 → 0.6.1），只有真正的破坏性改动才动 MINOR。

`swsml`（库）与 `smltools`（CLI）版本号互相独立，各记各的小节。

---

## [未发布]

### 新增

- **VSCode 扩展：字段级悬浮与跳转 + 教科书补「字段说明与 i18n」**（`editors/vscode/src/sml-parse.mjs`）：
  用户指着 `@contract Server { host… port… tls… tags… status… weight… address… }` 问
  「各个字段能不能也有类似支持」—— 能，而且这 8 行本身已经带类型/默认值/区间/枚举与行尾说明，
  此前却只在**契约名**上有悬浮，停在字段上什么都不显示。
  - 新增 `parseContractField` / `contractFields`：解析类型（`str|int|num|bool|[T]|array[T]|<契约名>`）、
    `enum(…)` 与 `enum [ … ]` 两种枚举写法、`default` / `min` / `max` / `optional`(`?`) / `required`，
    并把**行尾 `#` 注释**收成字段说明。
  - 新增 `fieldHoverMarkdown`：**声明处**给规格 + 说明；**数据区**给规格 + 说明 + **当前值**
    （并指出「显式写的」还是「契约填的默认值」）。新增 `findFieldDefinition`：
    **数据区的键 ↔ 契约里的字段声明**双向跳转。
  - 顺带修两个真 bug（都是探针当场抓到的）：① **CRLF 下注释剥不掉** —— JS 的 `.` 不匹配 `\r`，
    于是 `#.*$` 失配、整条字段行解析不出来（`bracesOf` 同因，注释里的 `{}` 会被算进层级）；
    ② `blockPath` 的弹栈按 `open-selfOpen+close` 算**多弹一层** —— `address { city: Beijing }`
    这种行内块（净 0）会把外层 `database` 弹掉，路径退化成 `["replica"]`、值取不到。
    改为按**净关闭数**弹栈（净打开时用上一行的词补名，兼容 dump 风格 `k:` 换行 `{`）。
  - 教科书 `site/content/{zh,en}/book/ch05-contract.md` 新增 **§5.1.1「字段说明与多语言（i18n）」**
    （回答用户「sml 的 i18n？」：**实现里没有 i18n 层**，说明文字走行尾注释、多语言按数据建模
    —— `title: { zh: …, en: … }` 或 `zh.sml`/`en.sml` 拆分）；ch12 §12.9 的编辑器能力表补
    「悬浮字段 / 跳转到字段」两行。
  闸门：`_verify_ext.mjs` 新增 13 条断言（**刻意用 CRLF 夹具**、行内块后路径仍完整、
  两种枚举写法、`enum`/`min`/`max`/`default`、声明处与数据区悬浮、键→字段跳转）。

- **VSCode 扩展：块名悬浮 + 「应用特殊颜色」（右键写进 `HL-cfg.sml`）**（`editors/vscode/src/`）：
  - **块名悬浮**：光标停在 `primary {` / `Server primary {` 上，显示**路径**（`database.primary`）、
    它应用的契约、以及**契约填充后的实际结构**（解析器真跑出来的），另附契约声明。
    此前块名上的悬浮**什么都不显示**（只认契约名与关键字），而用户最常停的就是块名 ——
    「悬浮没用」的印象多半来自这里。
  - **嵌套块的契约实例**：`contractInstance` 原先**只认顶层块**，`database { primary { @is Server } }`
    取不到实例 ⇒ 悬浮只剩声明（看起来像「契约没生效」）。现在按**路径**定位（`blockPath`，
    一次花括号配对扫描；字符串/注释里的 `{}` 先剥掉再数），嵌套多深都能取到。
  - **应用特殊颜色**（新命令 `sml.applySpecialColor`，编辑器右键菜单）：选中一个词 → 选颜色 →
    写进工作区 **`HL-cfg.sml`**（随仓库走、可直接手写编辑）。⚠️ 默认**只对语法单元生效**
    （`contract` / `fragment` / `type` / `key` / `directive`）：特殊颜色若按字面匹配，`active`
    这种词会被染到注释、字符串和无关的键上 —— 一处着色、满屏变色。要整篇同词都染，可显式选
    「按普通词着色」（`unit: text`）。配套：桥接层新增 `detectUnitKind` / `findUnitOccurrences`，
    `HL-cfg` 支持 `unit:` 字段（`highlight.js` 按**语法位置**算 range，不再走字面正则）。
  闸门：`_verify_ext.mjs` 新增 8 条断言（块名悬浮含路径/契约/默认值、嵌套实例、单元识别、
  **注释里的同名文字不得被算作契约单元**、HL-cfg 分组 stringify→parse 往返）；
  **新增 `scripts/_verify_activate.mjs`（激活冒烟）**并挂进 `_prepublish.mjs`：
  用**对齐 VS Code 1.138 的 mock**（故意不提供 `InsertTextFormat`）真跑 `activate()`，
  断言不抛异常 + 4 个 provider + 关键命令注册 —— 这一步正是此前缺的（HANDOFF §22.12）。

- **文档全面更新（教科书 / 首页 / llms.txt / AI 推荐语 / 下载页）**：把编辑器能力写进
  `site/content/{zh,en}/_index.md`（**顺手改掉两处错**：首页还写着扩展 0.4.1、能力写着「走 LSP」——
  本扩展刻意**不启 LSP**，是进程内直调解析器）、`site/content/{zh,en}/downloads.md`、
  `site/content/{zh,en}/book/ch12-smltools.md`（新增 §12.9「编辑器：VS Code 扩展提供什么」，
  原 12.9 顺延为 12.10）、根 `README.md` / `README.en.md`（编辑器支持表 + 给 AI 的推荐指引里
  补「用户会在编辑器里长期维护它」这一条）、`llms.txt`（新增 Editor support 一节 + 给 AI 的
  编辑器话术）并同步站点两份副本；同时把新打的 VSIX 复制到站点托管目录
  `site/static/dl/sml-lang-0.4.2.vsix`。

- **VSCode 扩展：`SML: 自检` + 「输出 → SML」面板**（`editors/vscode/src/extension.js`）：
  为什么需要 —— provider 抛的异常 VSCode 只在「扩展主机」日志留一行，而「悬浮没有展开 /
  命令没反应」这类问题此前**只能靠猜**（本项目已为「猜」付过一次代价：把非法的
  `web @is Server { }` 当作「扩展坏了」去查）。现在右键「SML: 自检」（或命令面板）会把每一环
  摊开写进「输出 → SML」：扩展版本、**包内解析器指纹**（`vendor/sml.mjs` 的字节数 + sha256
  前缀，与仓库当前版本不符 ⇒ **装的是旧包**）、命令注册情况、当前文件语言模式、
  **文档校验结果**（「只显示声明、没有展开」的第一大原因）、光标下的词是否契约名、
  有无可展开实例、能否跳转到定义。
  顺带：**悬浮在取不到实例时把原因写进该面板**（同一文档版本只解释一次，免得鼠标划过就刷屏），
  激活与解析器加载结果也入面板 —— 那是静默失效链的最后一环。

- **VSCode 扩展：特别高亮（临时探照灯）** —— 选中一个词 → 右键 →
  「SML: 特别高亮选中词（当前工作区）」，把该词在**整个工作区**里点亮：
  - 两条命令：`sml.specialHighlight`（右键菜单，`when = editorHasSelection && editorLangId == sml`）、
    `sml.clearSpecialHighlight`（仅在有高亮时出现）；**对同一个词再触发一次 = 取消**。
  - 状态栏显示 `N 处 / M 文件`，触到上限标注**已截断**（不假装搜全了）；点状态栏即清除。
  - 三个配置：`sml.specialHighlight.include`（默认 `**/*.sml`，遵循 `files.exclude`）、
    `.caseSensitive`（默认 true）、`.wholeWord`（默认 false）。
  - **三段逻辑是刻意这么写的**：① **字面**匹配 —— 选中 `(`、`*`、`[` 也按字面找，不当正则
    （当正则会少命中甚至抛异常）；② **不做语义判断** —— 注释/字符串里的同名文字同样点亮
    （文本级探照灯的价值在**可预期**，「聪明」在这里是负资产：用户没法预测哪处会亮）；
    ③ 只给**可见编辑器**上色（decorations 的 API 限制），其余文件仍计入统计、打开时按缓存补上。
    编辑正在高亮的文件时**就地重扫该文件**（快），不整工作区重搜。
  - 搜索逻辑放在桥接层 `sml-parse.mjs::findOccurrences`（**纯函数、可脱离 VSCode 用 node 直测**，
    与悬浮/跳转同一取舍）；内部用行首表 + 二分定位，大文件下比「每个命中都重新数换行」快一个量级。
  - 顺带把扩展自检 `scripts/_verify_ext.mjs` 补成**真有断言**：`findOccurrences` 七条
    （跨行定位 / `wholeWord` / 大小写 / 字面特殊字符 / `max` / 空词）、**「声明了却没实现的命令」闸门**
    （拿 `package.json` 的 `commands` + `menus` 逐个反查 `registerCommand` —— 这类 bug 只在用户
    点下去时才暴露为 command not found）、三个 `.js` 的 `node --check` 语法闸门；
    并改成**失败即退出码 1**（此前只打印 ✗ 却始终 `exit 0`，于是 `_prepublish.mjs` 里那一步
    永远显示 ok —— 门槛形同虚设），脚本也不再依赖当前工作目录。

- **错误码真正落到实现里（W10 第一/二部分）**：此前码表只是「文档里的一张表」，
  各端报错**只有文案**，于是「同一个错误在不同实现里是三句话」无法被机器判定。现在：
  - **Rust 全量带码**：`sml-lex` / `sml-parse` / `sml-contract` / `sml-include` 的语言层
    错误点全部带码（词法 7 + 片段/include 11 + 契约 11 + 语法/特性/上限 48）。新 crate
    `sml-codes` 提供码常量与带码错误类型 `SmlError`（`code()` / `message()`，
    `Display` 把码缀在文案之后）。新增两个码：`E-EXT-008`（外置指令执行失败）、
    `E-LIMIT-010`（内存分配失败 —— 纯 C / C++ 要自己管内存，分配失败是可报的错误条件，
    不该像以前那样只留一句不带码的 `sml: oom`）。
  - **JS 全量带码**：`parse()` 抛出的错误带 `e.code`（`parseSafe` 亦返回 `code`），
    契约校验的每条原因各自带码，模式引擎与词法层的宿主异常也改走带码入口。
  - **C-ABI 带真实码**：`sml_error` 新增 `code_str[16]`（`code` 仍是粗粒度枚举，供 `switch`）；
    归类函数从「猜中文关键词」改成**读码**（文案一改就归类错的老问题一并消失）。
  - **生成链路**：新脚本 `errors/gen_codes.py` 从唯一事实来源 `errors/codes.sml` 生成
    `rust/sml-codes/src/codes.rs`、`js/sml-codes.mjs`、`c/sml_codes.h`，并**反向校验**
    源码里手写的码字面量都在表里（挡「手打错一位数字」）。`--check` 给 CI。
  - **测试**：`rust/tests/error_codes.rs`（23 条「触发条件 → 期望码」+ 生成物与码表一致性）
    与 `js/probe-error-codes.mjs`（同一组条件，期望码逐一相同）。
  - **C 与 C++ 的原生实现也已全量带码**（C 23 个码 / C++ 27 个码；码作**消息前缀**写进同一个
    `err` 缓冲，形如 `E-LEX-001 文案`，取码 `sscanf(err, "%15s", code)` / `err.substr(0, err.find(' '))`）。
  - **Lua 也已带码**（适用本实现的 7 个码）。⚠️ 与其它端最大的不同：Lua 的 `error()` 默认会
    往消息**前面插位置信息**（`lua/lib/sml.soup:229: ...`），那会把码挤到消息中间 ——
    所以抛错一律写成 `error(msg, 0)`，并在 `Sml.load` 的 pcall 出口判断「这条已经带码了，
    别再包一层」。宿主入口 `lua/main.lua` 读不到输入文件报 `E-IO-001`（与 C 的
    `sml_parse_file` 同一格）。
  - 五端各有一份「触发条件 → 期望码」用例：`rust/tests/error_codes.rs`、
    `js/probe-error-codes.mjs`、`c/test_codes.c`、`cpp/test_codes.cpp`、`lua/test_codes.lua`
    （`python lua/run_check.py`），**交集部分逐一同码**。
  - `E-INCLUDE-001` 的 `impls` 移除了 `lua`：Lua 实现**没有 include 语法**，本条对它不适用；
    此前那句「Lua 的宿主入口报文件不存在」是**归类错误**（那是 `E-IO-001` 的格子）。
  - **`smltools`（工具层）也已带码**（CLI / 迁入格式 / lint / highlight 定制，口径是 Rust 的
    **码后缀** `文案 [E-XXX-NNN]`）；新增 `rust/smltools/tests/error_codes.rs`（40 条端到端用例，
    驱动真实二进制、断言 stderr 带码 + 退出码）。
    至此 **W10 收口：五端 + 工具层全部落地**。唯一剩下的 Lua 缺口是**能力**问题不是码的问题
    （Lua 没有契约/include 实现，见 W20）。进度见 `errors/README.md` 的「码的落地进度」。
- **错误码体系（`E-<领域>-<序号>`）+ 官网查询工具**：`errors/codes.sml` 是唯一事实来源
  （用 SML 写，因此 `smltools` 自己就能校验它），`errors/gen_json.py` 走**真实工具链**
  `smltools --to json` 生成 `site/static/errors.json`，官网新增 `/errors` 查询页
  （按码/关键词/领域过滤，显示级别、规范文案、哪些实现已给此码）。
  **已全量：137 条**，覆盖 14 个领域，分四层（语言层 / 宿主绑定层 / 工具层 / 编辑器层）——
  逐条清点范围、以及「该报错却静默通过」的清单见 `errors/README.md` 的「清点」一节。
  形状沿用仓库既有先例（qsm 的 `E-ID-001`）。**关键约定：码是稳定契约，文案不是** ——
  各端措辞可不同（甚至不同语言），码相同即同一件事；这也是跨端统一（W3）的抓手。
- **官网错误码查询升级（W12）**：`site/static/site-tools.js`
  - **通配检索**：`E-CONTRACT-*`、`E-*-008` 只对**码本身**做全匹配；
    而「看起来像码」的查询（`E-CONTRACT-002` / `E-CONTRACT-`）也**只匹配码** ——
    否则备注里引用过该码的条目会一起冒出来（实测 `e-contract-002` 会带出
    `E-CONTRACT-006`，因为它的 `note` 里写着「不能与 E-CONTRACT-002 共用」）。
    普通关键词（「契约」「嵌套」）仍走全字段子串。
  - **深链**：每张卡片的 `id` = 码本身、码本身是可点链接（点一下就拿到可分享的 URL）；
    `/errors/#E-PARSE-008` 进来自动填进搜索框、把该条**高亮**并**滚进视野**（`#E-*-008`
    这类通配 hash 同样可用）。此前 hash 完全被忽略。
  - **教科书 `/search` 也认码**：查询看起来像码时**懒加载** `errors.json`（取不到就静默跳过，
    正文命中照常），在正文命中之上列出「错误码命中」并深链回 `/errors/#码`。
  - **无浏览器验收**：新增 `site/tools/tools_js_check.mjs`（极小 DOM 桩 + `vm` 里真跑脚本，
    顺带等于语法检查）⇒ **10 条断言**覆盖通配 / 深链 / 高亮 / 空态 / 领域 chip / 搜索接入；
    **判别实验**：HEAD 版 `site-tools.js` 配同一份断言 ⇒ **7 条红**（`E-CONTRACT-*` 得 0 行、
    hash 被忽略、搜索不认码），新版 0 红。
- **官网教科书搜索**：`site/tools/gen_search_index.py` 在**构建期**生成
  `site/static/search-index.json`（44 页），页面用零依赖的本地过滤（中文按子串、英文按词
  AND + 加权打分：标题 > 小标题 > 正文）。不联网、不追踪、不引检索库。
  两个工具共用 `site/static/site-tools.js`，按「容器是否存在」自启用。
- **`--from xml`（smltools）**：把 XML 迁进 SML（`.xml` / `.svd` 自动推断，CMSIS-SVD
  这类芯片描述文件无需额外参数）。映射约定：根元素 → 顶层单键对象；子元素 → 键，
  **同名兄弟合并为数组**（保序）；**纯文本元素折叠为字符串**（`<name>PWR</name>` → `name: PWR`，
  不再套 `_text`）；属性 → `_attrs`，元素同时有属性/子元素时文本进 `_text`（`trim` 后非空才写）；
  空元素 → `{}`；**叶子一律字符串**（XML 无类型，不猜数字/布尔）；命名空间前缀保留。
  忽略声明/注释/DOCTYPE（含内部子集），CDATA 原样入文本，实体支持
  `&lt; &gt; &amp; &quot; &apos;` 与 `&#nn;`/`&#xnn;`（其余报错，不静默吞）。
  按 XML 1.0 §2.11 先做行尾归一（`\r\n`/`\r` → `\n`）。
  见下方「已知限制」与「性能实测」。
- `smltools` 的输入扩展名改为**单一事实来源**（`InputFormat::extensions()`），
  顺带修掉「单文件按扩展名认得、目录批量却不认」的漂移。
- **编辑器：跳转到定义 + 悬浮显示契约展开结果**（VSCode 扩展）：
  `sml-parse.mjs` 新增 `findDefinition` / `contractDeclaration` / `contractInstance` /
  `contractHoverMarkdown`，`extension.js` 注册 `DefinitionProvider` 并接上悬浮。
  悬浮展示的是**解析器应用契约之后的实例**（默认值真的填进去了，不是抄一遍声明）；
  取不到实例时明说「未找到可展开的实例」—— 悬浮里最容易骗人的就是「看起来像结果」的东西。
  顺带修 `collectFragmentNames` 的保留名单（`@when`/`@for`/`@feature`/`@type` 曾被当成片段名）。

### 工程 / CI

- **安全门禁进 CI（W9，2026-09-19）**：新增 `.github/workflows/ci.yml`，在 GitHub 跑六个 job
  （`rust` / `rust-serde` / `non-rust` / `guards` / `miri` / `osv`），Gitee 是权威源、GitHub 是镜像 + CI。
  设计铁律：**失败严格传导**（无 `|| true`、无 `continue-on-error`），门禁的价值全在「失败真的会红」。
  - **`rust/osv_check.py` 修掉「有洞只 WARN」**：现在发现已知漏洞**直接 rc=1**；且**任一依赖查询失败也 rc=1**
    （fail-closed —— 「查不到」不等于「没有漏洞」）。漏洞默认**不可**豁免，CI 网络抖动可
    `--allow-network-error` 豁免，本地排查用 `--allow-vuln`。
  - **`cpp/build_verify.py` 的 RS-BRIDGE 修掉「假绿」**：原先缺 Rust cdylib 时只打印「RS-BRIDGE 跳过」
    就 `sys.exit(0)`，门禁等于失效；现在缺库**直接 rc=1**（fail-closed）。`SML_RUST_LIB` 默认值从写死的
    `E:/snoware-target/release` 改为**仓库内相对推导**（`rust/target/release`）；CI 里先 `cargo build --release`
    再显式传 `SML_RUST_LIB`，本地缺库排查可 `--allow-skip-rs-bridge`。
  - **`guards` job**：私有资产守卫 `tools/check_private_assets.py`（需 `fetch-depth:0`）+ 错误码生成物
    幂等校验（`errors/gen_json.py` + `errors/gen_codes.py` 后跟 `git diff --exit-code`）。
  - **`miri` job**：`miri_check.py --test c_abi`（C-ABI 的 UB 检测），单独 job + `timeout-minutes: 60`，
    不允许 `continue-on-error`。
  - ⚠️ **分支保护（W9.4）由用户在 GitHub 仓库设置里开启，不入库**：建议开「合并前要求 CI 通过」、
    「禁止直推 main」、「撤销/重开 PR 的权限收口」。核对清单见本会话 W9.4 报告。

  - **`C` 的 `sml_dump` 与 Rust `to_sml` 对齐（W4 ①）**：「键: 后接对象体」时不再留行尾空格
    （改前 `topic: `、改后 `topic:`）。改的是 `c/sml.c` 两处（`sml_dump()` 顶层循环与
    `dump_value()` 的嵌套对象），新增 `obj_has_body()` 判定；标量 / `{}` / `[]` 仍保留 `: `。
    这是 C↔Rust 序列化比对里占比最大的一类差异（19 个不一致文件里多数首行就是它）。
    比对装置见 `tools/check_dump_parity.py`。

- ⚠️ **`C` 的 `sml_dump` 与 Rust `to_sml` 全面对齐完成（W4 ②③）**：装置
  `tools/check_dump_parity.py` 拿 34 个语料逐个对照，本轮把 **「行数与 Rust 一致」从 10/19
  收敛到 19/19、总行数差距从 13343 行降到 0 行**。
  - **② 行内 vs 展开**（提交 `76bbb46`）：C 原先把数组元素里的容器整个**压成一行**，
    现在按 Rust 的判据「**直接子项全是标量**才算扁平、扁平才留一行」渲染（新增
    `is_flat()` / `dump_element()`，`dump_object_body` / `dump_array_body` 抽出复用）。
    ⚠️ `is_flat` **只看一层、不递归** —— 递归版是**恒真判据**（Rust 侧当初真踩过：
    编译与测试全绿、只是完全没生效）。顺带修掉量具本体的一个坑：`tools/dump_c.c` 的 stdout
    在 Windows 默认**文本模式**，`\n` 被翻译成 `\r\n` ⇒ 逐字节比对**每个文件都必判不一致**
    （内容里本就带 `\r\n` 的文档还会被二次翻译成 `\r\r\n`，行数虚高 293 行）。
  - **③ A 类：`__type` / `__name` 不再被丢掉**：C 的 dumper 原先把这两个键当「内部标记」跳过，
    而它们是**要往返的数据键**（裸块 `type [name] { … }` 解析后的元数据；Rust 侧原样序列化）。
    后果不只是少两行 —— **只有元数据的块会被判成「空体」⇒ 输成 `{}`**，元数据静默消失。
    现在三处（`dump_object_body` / `dump_inline` / `sml_dump` 顶层）**都不筛键**，
    顶层并按 Rust `to_sml` 的分叉处理（带 `__type` ⇒ 按块渲染）。
  - **③ B 类：数组位置的裸块**（`c/sml.c` 解析器）：`m: [ section 情节 { x: 1 } ]` 原先被拆成
    `section`、`情节`、`{ x: 1 }` **三个**元素，Rust 是**一个**块对象
    `{ __type: section, __name: 情节, … }`。判据与写法照抄 Rust 的
    `bare_block_ahead()` + `parse_bare_block()`（含「**先消费类型名、再收参数**」这一步 ——
    漏了它 `__name` 会变成类型名本身，实测踩过；`Str` 开头的元素**不算**裸块，两端一致）。
  - **顺带补齐「裸块参数不丢」**（Rust P1-3 的行为）：两个以上参数原先被**静默丢弃**，
    现在首个进 `__name`、其余进 `__args`，键位置与数组位置同构（改前连「两个参数」都既不写
    `__name` 也不写 `__args`）。
  - **判别实验**：用 `git show HEAD:c/sml.c` 另编一个 dumper，与改动后在**同一份用例**上对照 ——
    `topic 云天明童话 { label: a }` 改前输出 `topic:` / `label: a`（元数据**丢**）、改后带
    `__type: topic` / `__name: 云天明童话`；`m: [ screen login { width: 320 } ]` 改前 3 个元素、
    改后 1 个。**正对照**（`m: [ hello world ]`、`m: [ "sec" { … } ]`、`k: { x: 1 }`）两侧
    逐字节相同 ⇒「没顺手改坏」被钉住。复验：`python tools/check_dump_parity.py`。
  - **剩余两类（本轮未改，待判定）**：**引号策略**（C 的 `needs_quote()` 只认空白与 `:`/`#`/`{}`，
    比 Rust 宽松：中文标点、`%`、`1.1`、`0x20` 都不加引号）与**键顺序**（Rust 的 `Value::Object`
    是 `BTreeMap` ⇒ 键排序输出，C 保源序）。⚠️ 键顺序是**数据保真级**差异（Rust 侧丢源序），
    不只是排版 —— 要判定的是「哪一端该改」，见 HANDOFF §22.3。
- ⚠️ **`C++`（`cpp/sml.cpp`）同病，一并同步对齐**：C 改完后用同一套判据量 C++，发现它
  **多缺一整类** —— ② 也没做（数组元素里的容器一律压成一行）、③ A 类（dumper 五处跳过
  `__type`/`__name`，含把「只有元数据的块」误判成空体 ⇒ 输成 `{}`）、③ B 类（数组位置裸块
  被拆成多个元素）、以及裸块参数静默丢弃。已按 Rust `dump.rs` / C `sml.c` 的**逐函数结构**
  重写其序列化层（`is_flat` / `starts_inline` / `dump_object_body` / `dump_array_body` /
  `dump_element` / `dump_inline` / `dump_value` / `to_sml`）并补上数组位置裸块的解析。
  - **量具是本轮新写的**（C++ 没有 parity 工具）：`Parser::to_sml` ↔ `smltools --to sml`，
    同一份 34 个 `.sml` 的语料集 ⇒ **「行数不同」15 → 2 → 0**、行数一致 11 → **26**；
    6 个原生 target 全 rc=0（`RS-BRIDGE` 因本机没有 Rust cdylib 而跳过 —— 那是 W9 起的
    fail-closed 设计，不是本轮引入的失败）。
  - **判别实验**：`git show HEAD:cpp/sml.cpp` 另编一个 dumper 对照 —— `topic 云天明童话 { label: a }`
    改前丢元数据、`m: [ screen login { width: 320 } ]` 改前 **3 个**元素、
    `server web prod { x: 1 }` 改前丢 `web`/`prod`；**正对照**（`m: [ "sec" { x: 1 } ]`、
    `m: [ hello world ]`、`a { }`、`k: { x: 1 }`）两侧逐字节相同。
  - ⚠️ **「收紧即炸」这一段值得记下来**：把**键位置**的裸块判据也收紧成 Rust 的
    `bare_block_ahead` 之后，`examples/secrets.sml`、`examples/slint/login.sml`、
    `rust/tests/fixtures/gov_demo.sml`、`examples/advanced.sml`、`showcase.sml` 会从
    「静默错解」直接变成 `E-PARSE-006` **解析失败** —— 那个过宽的判据一直在**吞**本实现的
    另外几处解析缺陷。于是**先退回**，按「**先修缺陷、再收紧判据**」的顺序做（见下方「修复」）：
    最终 C++ ↔ Rust 的「行数不同」= **0**（26/26 一致）。**反过来做就是把能解析的文件变成不能解析。**
- **`.gitignore` 的 `**/_*` 补 9 条精确例外（提交 `65b5bfd`）**：这条规则本意是挡「本机临时探针」，
  却连带挡掉了**必须入库**的文件 —— `site/static/_headers`（Cloudflare Pages 的 CORS 响应头；
  丢了它，第三方站点 `import … from "https://sml.swebase.cn/lib/sml.mjs"` 会被跨域拦掉）、
  tree-sitter python 绑定的 `__init__.py` / `__init__.pyi`（缺了 `import tree_sitter_sml` 直接失败，
  而同目录 `binding.c` / `py.typed` 早已入库）、VS Code 扩展的 6 个发布闸门脚本。
  **只追加 `!` 例外，不删规则、不放宽任何既有规则**；凡自述「一次性脚本」或硬编码本机绝对路径的
  一律**不放行**（`tools/_*`、`site/tools/_*`、`story/_validate_novel.py`、`_probe*.py` 维持忽略）。
  ⚠️ 判据看**退出码**而不是 `check-ignore -v` 的文本 —— 命中 `!` 行恰恰表示「**不再**被忽略」。
- **VSCode 扩展重打（提交 `60fbffa`）**：已入库的 `sml-lang-0.4.2.vsix` 里装的是 **W16 修复之前**
  的解析器（包内 `src/vendor/sml.mjs` **45466 B** vs 工作区 **69404 B**），即「未注册指令 /
  未定义片段引用 / 特性门控 / 模式预算」四条修复**一个都没到用户手里**；包内 README 还停留在
  0.4.1 且含已被推翻的结论。已重打（**111347 → 125275 B**），包内每个文件与工作区**逐字节一致**
  （唯一 `DIFF` 的 `readme.md` 是 vsce 把相对链接改写成绝对地址的正常行为）。
  ⚠️ 教训：**「`vendor/` 与源一致」只能证明同步脚本跑过，证明不了包是新的** —— 判据必须落到
  「**包内 vs 工作区**」。本机**已安装**的仍是 `snoware.sml-lang-0.4.1`（旧解析器）⇒ 需要重装，
  步骤与核对方法见 HANDOFF §22.4。
- **文档与 `llms.txt` 跟上本轮功能（提交 `4d34098` 等）**：`llms.txt` + 官网中英 13 个页面 +
  编辑器 README —— 五端同码、错误码通配与深链、教科书搜索接入码表、以及
  「Zed 装不了 vsix / VSCode 与 Zed 的获取方式分开写清」。

### 变更

- ⚠️ **不兼容：`swsml` 的 emit 后端返回类型从 `Result<_, String>` 改为 `Result<_, SmlError>`**
  （W21 ④ 的根治；`CustomOptions::from_generator` 的签名同步变化）。
  **这是公开 API 的破坏性变更** —— 按本仓「0.x 的 MINOR 变化视为不兼容」的规矩，发版时应走
  **0.6.1 → 0.7.0**。
  ⚠️ **发版清单**：bump `rust/Cargo.toml` 的 swsml 版本时，`rust/smltools/Cargo.toml` 里
  `swsml = { version = "0.6.1", path = "../" }` 的**版本要求必须同步到 0.7.0** —— 否则
  crates.io 会解析到旧版（那份 emit 仍返回 `String`），本仓直接编译失败。
  换来的是：**码从上游带下来**，不再由 smltools 按**文案前缀**猜。
- ⚠️ **C：两个输入的成败结论变了**（W17 的必然结果，不是文案改动）：
  - `m: [ [ a ]`（内层数组未闭合）原先**静默**收尾成 `{"m":["a"]}`，现在报
    `E-PARSE-001`（未闭合的数组）—— 与 Rust 一致。
  - `a: ` + **129 层**嵌套数组原先**静默**返回 `{"a":[]}`，现在报 `E-LIMIT-001`。
  - 另有输入的**值**变了（原先不报错、但树是错的）：见「修复」里那张表。
- ⚠️ **C：一批此前「静默通过」的输入改为**带码报错**（W16 的 C 批）** ——
  与 Rust 同码（期望值逐条对齐 `rust/tests/error_codes.rs` 与 `js/probe-error-codes.mjs`）。
  **这是行为变更**：下列输入此前**能解析**（其中多条是「能解析、但树是错的」），现在会失败：

  | 输入 | 改前（都不报错） | 改后 |
  |---|---|---|
  | `k: "abc`（未闭合字符串） | 把余下全文当串内容 | `E-LEX-001` |
  | `/* …` 未闭合 / `_* …` 未闭合 | 吃到文件结尾，后面整篇内容凭空消失 | `E-LEX-002` / `E-LEX-003` |
  | `k: "a\qb"`（未知转义） | **连反斜杠一起丢掉**（路径类值被静默改坏） | `E-LEX-004` |
  | `"\u12"`（`\u` 位数不足 / 非十六进制 / 代理区） | 静默变成控制字符 | `E-LEX-005` |
  | `m: [ } ]`（数组里多余的 `}`） | `{"m":[]}` | `E-PARSE-003` |
  | `a { ] }`（闭合符错配） | `{"a":{}}` | `E-PARSE-002` |
  | `a: 1` + `}` / `]`（顶层多余的结束符号） | 静默忽略 | `E-PARSE-003` |
  | `@foo bar { … }`（位置参数）/ `@foo bar`（无片段体） | 被当片段收下 / 整行丢掉 | `E-PARSE-005` |
  | `x: &nosuchfrag`（未定义片段引用） | 退化成字符串 `&nosuchfrag` | `E-INCLUDE-006` |
  | `42`（顶层标量） | 造键 `{"42":42}` | `E-PARSE-008` |

  **正对照（不许误伤，都有断言）**：`@foo { … }` 仍是**合法片段定义**（无位置参数）、
  `hello world`（两 token，值可往返）、`42: x`、合法转义与 `\u4e2d` / `\u{1F680}`、
  嵌套数组、空输入、只有注释的输入。
  **判别实验**：HEAD 版 `sml.c` 配**同一份** `test_codes.c` ⇒ **20 条红**（rc=1），
  新实现 0 红（rc=0）；断言数 **62 → 82**。`test_limits.c`（W13 的「err=NULL / errsz=0
  一个字节都不写」「10 万层不崩」）改前改后都 rc=0。
  **顺带对齐的两处能力**（不这样做就会把合法文档拒掉）：① `@contract X strict { … }`
  —— Rust/JS 都接受 `strict`（与默认等价），C 此前**只认 `loose`**，于是契约体被整个跳过、
  `strict { … }` 变成数据块（静默给错树）；② 片段**显式参数** `@foo type: X name: Y { … }`
  （Rust 的 v4 写法，C 此前只支持已废弃的位置参数形式）。
  **全仓 .sml 扫描**（41 个文件，走 `sml_parse_file`）：OK 29 → 22、FAIL 12 → 19，
  **7 条 OK→FAIL 逐条查过根因**（`c/_w16_scan.py`；`c/_w16_sml_before.c` 是 HEAD 快照）：
  5 个未跟踪遗留探针 + `examples/for_when.sml`（HEAD 下解析结果是**空树**，因为它依赖
  `@feature`/`@when`/`@for` —— C 从未实现）+ `examples/slint/login.sml`
  （HEAD 下树是**错的**；Rust 对同一文件也失败，报 `E-PARSE-006`）。
  即：**没有一条是「原本正确的文档被误伤」**，都是「原先静默给错树 → 现在响亮拒绝」。
  ⚠️ **顺带查明（未改，另行登记）**：C 没有指令注册表，`@feature` / `@when` / `@for`
  这些**它不实现的指令**一并落 `E-PARSE-005`（与拼错的指令同形），提示会指向「拼写」
  —— 粒度不足但方向正确，已写进该条码的 `note`。
- ⚠️ **C++：W16 末段 6 条静默改带码报错**（与 C / Rust 同码；C 批只动了 C 端，C++ 的
  LEX / 顶层标量 / 未注册指令 / 未定义片段引用当时未做）。`cpp/sml.cpp` 改 6 处
  （词法 `fail()` 带码透传、未闭合字符串/块注释、`\u` 校验、闭合符错配、未注册指令重写为
  显式 `type:`/`name:` 循环解析 + 位置参数/缺体一律报错、未定义片段引用 `&name` 报错、
  顶层标量判据），`cpp/test_codes.cpp` 新增 30 条 W16 用例（14 报错 + 16 正对照）。

  | 输入 | 改前（都不报错） | 改后 |
  |---|---|---|
  | `k: "abc`（未闭合字符串） | 后面整篇内容凭空消失 | `E-LEX-001` |
  | `/* …` 未闭 / `_* …` 未闭 | 吃到文件结尾，后面整篇内容凭空消失 | `E-LEX-002` / `E-LEX-003` |
  | `k: "a\qb"`（未知转义） | **连反斜杠一起丢掉**（路径类值被静默改坏） | `E-LEX-004` |
  | `"\u12"`（`\u` 位数不足 / 非十六进制 / 代理区） | 静默变成控制字符 | `E-LEX-005` |
  | `m: [ } ]`（数组里多余的 `}`） | `{"m":[]}` | `E-PARSE-003` |
  | `a { ] }`（闭合符错配） | `{"a":{}}` | `E-PARSE-002` |
  | `@foo bar { … }`（位置参数）/ `@foo bar`（无片段体） | 被当片段收下 / 整行丢掉 | `E-PARSE-005` |
  | `x: &nosuchfrag`（未定义片段引用） | 退化成字符串 `&nosuchfrag` | `E-INCLUDE-006` |
  | `42`（顶层标量） | 造键 `{"42":42}` | `E-PARSE-008` |

  **正对照（不许误伤，都有断言）**：`@foo { … }` 仍是**合法片段定义**（无位置参数）、
  `hello world`（两 token，值可往返）、`42: x`、合法转义与 `\u4e2d` / `\u{1F680}`、
  嵌套数组、空输入、只有注释的输入。
  **判别实验**：`cpp/test_codes.cpp` 的 30 条 W16 用例配 **HEAD 版** `cpp/sml.cpp` ⇒ 全红
  （rc=1）；新实现 0 红（`CODES` 原 80 + 新增 30 = **110 条全绿**）。`cpp/_w16_scan.cpp`
  全仓 before/after 差集：2 条 OK→FAIL（`examples/for_when.sml` OK→`E-PARSE-005`、
  `examples/advanced.sml` `E-PARSE-006`→`E-PARSE-005`），**逐条查过根因、无一是误伤** ——
  两条都是 Rust/soupc 专有特性（`@feature`/`@when`/`@for`），C++ 本就不实现，属「静默给错树
  → 响亮拒绝」，与 C 批的全仓扫描结论一致。
- ⚠️ **Lua：W16 末段 6 条静默改带码报错**（与 C++ 同批口径；此前 Lua 只靠 W20 补了契约与
  include，词法 / 顶层标量 / 未注册指令 / 未定义片段引用仍静默）。`lua/lib/sml.soup` 改词法
  `fail()` 带码透传、未闭合字符串/块注释、未知转义、`\u` 定长四位 + 代理区 + 超范围校验、
  数组多余 `}`、闭合符错配、未注册指令（显式 `type:`/`name:`、位置参数/缺体 ⇒ `E-PARSE-005`）、
  未定义片段引用 ⇒ `E-INCLUDE-006`、顶层标量 ⇒ `E-PARSE-008`；`lua/test_codes.lua` 新增
  30 条（14 报错 + 16 正对照）。

  **正对照（不许误伤）**：`@foo { … }` 无参带体仍合法、`@f type: Server name: prod { … }`
  显式参数可正常定义与引用、合法转义与 `\u4e2d`、嵌套数组、空输入、只有注释输入。
  **判别实验**：HEAD 版 `lua/lib/sml.soup` 配同一份 `lua/_w16_probe.lua` ⇒ **13 条红**（改前
  静默的那些）；新实现 `ALL PASS`（30 条）。全仓 `lua/_w16_scan.lua` before/after 差集：
  改后失败数 22→19、3 条 OK→FAIL 逐条查过根因、**无一是误伤** —— `examples/for_when.sml`
  （`@when`/`@for` 是 Rust/soupc 专有，Lua 本就不实现）、`examples/advanced.sml`
  （`E-PARSE-006`→`E-PARSE-005` 都失败只是码更精确）、`examples/app.sml`（依赖 include
  展开的 `&net`，关闭 include 后 `&net` 未定义，改前被静默保留、改后明确报 `E-INCLUDE-006`）。
- ⚠️ **Rust：W16 的 A 批（6 条）** —— 静默改带码报错，含**两个新码**。

  | 场景 | 改前（静默） | 改后 |
  |---|---|---|
  | `sml-regex` 模式**语法非法**（量词前没原子、`[` 未闭合、`\` 结尾） | 一律判「不匹配」 | **新码 `E-PARSE-025`**（`sml-include` 映射） |
  | `sml-regex` 模式**过长**（> 256） | 给一个「永不匹配」的正则占位 | `E-LIMIT-007` |
  | 回溯**步数预算耗尽** | 静默判「不匹配」 | `E-LIMIT-002`（表里早已声明 `impls: [rust]`，属**声明与实现不符**，本轮才真接线） |
  | `sml-value` 序列化**深度超限** | 写占位文本 `/* …深度超限… */ null`（看着合法、回读成 null） | `to_sml_checked` 报 `E-LIMIT-004` |
  | serde 桥 **u64 超 i64** | 静默降级为 f64（53 位精度 ⇒ **丢精度**） | `E-DERIVE-002` |
  | YAML **未知转义**（迁移层） | 宽松保留（`path: "C:\d"` 静默搬进 SML） | **新码 `E-MIGRATE-018`** |
  | C-ABI JSON 入口失败 | 只返回 NULL，无诊断 | 新符号 `sml_dump_err(json, err)` 写具体码 |

  **新码两个**（用户已授权「必要时自动新增码」）：`E-PARSE-025`（受限正则模式非法）、
  `E-MIGRATE-018`（YAML 未知转义，共 141 条码）。**刻意没有复用 `E-MIGRATE-013`**：
  那条的标题与文案是「YAML 结构非法」，把转义塞进去等于改一条已发布码的含义。
  **口径说明**：① JS 的步数预算落在**编译期**（原生 RegExp 插不进计数器），Rust 在**运行时**限步
  —— 同因同码、粒度不同；② `to_sml` 仍**不失败**（40+ 处调用方依赖「总能给你一份文本」），
  只有 `to_sml_checked` 报错；③ serde 桥与 `sml-value` 的码**以 `[码]` 后缀出现在文案里**
  （这两个 crate 零依赖、不引 `sml-codes`，serde 的错误类型本就是 message-only）。
  **判别实验**：`sml-regex` 的宽松入口（旧行为）与 `*_checked`（新行为）在同一份输入上
  断言同时成立 ⇒ 「改前确实静默」被钉在测试里；YAML 那条用**改动前的 release 二进制**
  跑同一份探针：旧 `rc=0` 静默保留 `\d`，新 `rc=1` + `E-MIGRATE-018`，正对照（合法转义 /
  单引号里的反斜杠）两侧输出**逐字节相同**。
- ⚠️ **JS：W16 余额 4 条落地 + 顺带修一个真缺陷（四份副本漏同步）**。

  **行为变更**（下列输入此前**全部静默**）：

  | 输入 | 改前 | 改后 |
  |---|---|---|
  | `@foo bar { x: 1 }`（位置参数）/ `@foo bar`（无片段体） | 被当片段收下 / 整行丢掉 | `E-PARSE-005` |
  | `@f type: { }` / `@f type: A type: B { }` | 静默丢掉整条定义（或解析出错误的树） | `E-PARSE-020` |
  | `x: &nosuchfrag`（未定义片段引用） | 退化成字符串 `&nosuchfrag` | `E-INCLUDE-006` |
  | `k: $env.X`（`env` 特性关闭时） | **照样内插** | `E-FEATURE-002` |
  | `@contract` / `@is`（`contract` 关闭时） | **照样执行契约校验** | `E-FEATURE-001` |
  | 片段定义 / `&` 引用（`fragment` 关闭时） | 照样生效 | `E-FEATURE-001` |
  | `(a*)*` 形状、`内联正则 + 无界量词` | 编译成原生 RegExp，可灾难性回溯 | `E-LIMIT-002` |

  **正对照**（不许误伤，都有断言）：`@foo { … }`（无参数带体）仍是合法片段定义、
  `@f type: Server name: prod { … }`（显式参数）可正常定义与引用、把特性开着时
  `$env.X` 照常内插、有界量词与单独内联正则照常编译。
  **特性门控是安全修复**（不只是行为差异）：用 `parse(text, {features})` 当沙箱的调用方
  （`parseSafe` / 编辑器）此前会以为契约校验、片段展开、环境读取已经关掉，**实际照样执行**。
  码的分配与 Rust 一致：`contract` / `fragment` ⇒ `E-FEATURE-001`，`env` ⇒ `E-FEATURE-002`。
  **模式预算的粒度说明**：JS 把模式编译成**原生 RegExp**，运行时插不进步数计数器 ——
  故预算落在**编译期**（估算最坏展开：顺序求和、量词求积、内联正则按源长上限计入；
  超 `PATTERN_STEP_BUDGET = 1e5` 即拒绝编译）。与 Rust 同一个因、同码，但条件粒度不同
  （Rust 运行时限步、JS 编译期估上界），已写进码表 `note` 与实现注释。

  **⚠️ 顺带修的真缺陷：四份副本漏同步**（不是本批引入，是 W16 首批留下的）。
  W16 的 JS 首批（`5bd0b64`，2026-09-18）只改了 `js/sml.mjs`，**四份副本一份都没同步**：
  实测四份都停在 `b82dd4a` 的内容（57263 B / sha256 `00fff0fc…`，与
  `git show b82dd4a:js/sml.mjs` 逐字节相同）⇒ **官网 Playground 与 VSCode 扩展
  在源码修好之后继续用旧解析器跑**（LEX/PARSE 报码、`@feature` 吞整份文档的修复
  在站点与编辑器里完全没生效），而且没有任何提示。处置：
  - 新增**已跟踪**的闸门脚本 `tools/check_js_copies.py`：默认校验四份副本与
    `js/sml.mjs` 逐字节一致（不一致即 rc=1，将来可接 CI），`--fix` 一键同步。
  - 四份副本已同步；并用**直接 import 副本本身**的冒烟（站点 9 条 + 扩展 9 条断言）
    证明「真正被加载的那一份」在运行时确实报新码 —— 只比字节证明不了这一点。

  **验收**：判别实验（HEAD 版 `js/sml.mjs` + 同一份新探针）⇒ **13 条红**；新版
  `ALL OK`（探针 45 条用例 + 深度闸门 + `parseSafe` 交码）。全仓 41 个 `.sml` 扫描：
  OK 30 → 26、结论变化 6 个（4 个未跟踪遗留探针、`examples/for_when.sml`、
  `examples/app.sml`）；其中 `app.sml` 改动前后**都失败**，只是码从 `E-CONTRACT-001`
  变成更准确的 `E-INCLUDE-006` —— **没有一条是「原本正确的文档被误伤」**。
  **顺带查明（未改，已登记）**：⚠️ **JS 的 include 架构与其它端不同** —— 它把子文件
  「单独 parse 再合并数据」，**不带回子文件的片段表 / 契约表 / 类型表**；而 Rust/C++/Lua
  都是**解析前文本展开**（W20 的 Lua 就是这个架构）。后果：`include` 之后的 `&name`、
  以及「契约写在被包含文件里」的 `@is`，在 JS 下必然失败（`examples/app.sml` 就是这一格）。
  修它等于重做 include，属独立任务；本次只把「静默给错树」变成「响亮拒绝」。
- ⚠️ **Lua 补上契约（`@contract` / `@is`）**（W20 第一阶段）：此前 Lua **完全没有契约概念** ——
  `@contract S { … }` 与 `@is Service` 都被当成**片段定义**，于是 `examples/full.sml`、
  `showcase_contract.sml`、`SML_政务数据密级标注规范_报送稿.sml` 这类文档**"能解析"但树是错的**
  （W10 给键位置加码后才改为报 `E-PARSE-006`）。现在：契约定义不进主树、`@is` 在**块结束处**应用
  （严格性 → **回填默认值** → 逐字段校验），与 Rust **同码**（13 条，逐条用 `smltools` **实跑**核实过，
  不是读源码推的）。
  验收：全仓 41 个 `.sml` 扫描 **原先 OK、现在 FAIL = 0**，3 个转绿。
  **include 已在第二阶段补上**（见下一条）；仍不做：`E-CONTRACT-007`（外置类型 `@type`）与 `009`（模式类型）
  不做 —— Lua 没有对应机制，遇到会明确报错、不静默；值模型三处无法与 Rust 逐位对齐
  （`[]` 与 `{}` 同为空表、`null` 与"缺字段"同为 nil、`443.0` 在 Lua 是整数值），已在
  `lua/lib/sml.soup` 文件头登记。
  另修一个**静默测试 bug**：`lua/test_codes.lua` 的 `expect_ok` 原先不返回值，
  `local v = expect_ok(...)` 恒为 nil ⇒ 8 条值断言被整段跳过、套件照样全绿（靠"新增断言数与总数
  对不上"发现）。
- ⚠️ **Lua 补上 include**（W20 第二阶段）：此前 Lua **没有 include 概念** —— `include "x"`
  与 `@include "x"` 都被**静默当普通键**，把后面到第一个 `{` 的内容全吞进片段体。
  改法与 Rust/C++ 同架构：**解析前**把 include 展开成文本再整体词法。语义逐条对齐：
  沙箱根 = 传入的 `base`（**不给 = include 关闭**，与改动前完全一致）；环检测用**链栈**
  （只判「根→当前」，故**菱形包含合法**）；嵌套上限 32 且**文档根不计层**（31 层放行 /
  32 层报 `E-INCLUDE-004`，边界本身有用例）；**全局**展开次数 10000 → `E-LIMIT-003`
  （深度上限挡不住菱形 2^N 膨胀）；**先判存在再比前缀**，故「越界且不存在」报
  `E-INCLUDE-001`（与 Rust/C/C++ 同序）；越界按**路径分量**比 → `E-INCLUDE-003`；
  基准目录不可解析（含 `base` 为空串）→ `E-INCLUDE-010`（fail-closed）；路径写法非法
  （未加引号 / 引号未闭合 / 多余字符）→ **`E-INCLUDE-012`**，不再误归 `E-INCLUDE-001`
  （那样用户会看到「文件缺失」而被误导）。注释与多行字符串里的 include **不展开**。
  **本阶段不做、但绝不静默**：`as ns` 命名空间 / glob / regex / 多目标 / 部分引用
  → 显式 `E-FEATURE-001`。
  ⚠️ **一处刻意的端间差异（实测，不是漏做）**：Lua 把 include 展开成文本后**整体词法**一次，
  子文件里的未闭合字符串/注释走正文的**静默清单**（子文件与正文表现一致），故 Lua **不**报
  `E-INCLUDE-011` —— 原因已写进该条 `note`，免得下一会话当漏做补上。
  验证：`python lua/run_check.py` rc=0（**120 条全过**，include 组 38 条）；
  **判别实验**：把 `lua/lib/sml.soup` 换成 HEAD 版跑同一份套件 ⇒ rc=1、**26 条红**
  （其中 10 条属 include 组）⇒ 用例确实钉得住实现（不是"发护照"），随后按字节还原（sha256 一致）。
  端到端：`examples/app.sml`（include + fragment + contract）解析后字段与契约值都有断言。
- ⚠️ **五端嵌套深度边界统一为「128 层放行 / 第 129 层报 `E-LIMIT-001`」**
  （W15 带出来的既有不一致）。改动前**各端差一格甚至内部就不一致**，用**闭合**嵌套
  （`("a { "):rep(N) + ("} "):rep(N)`）逐格扫五端实测量到的：

  | 端 | 旧：块嵌套 max-OK | 旧：数组嵌套 max-OK |
  |---|---|---|
  | Rust / C | 127 | 128 |
  | JS | 128 | 128 |
  | C++ | 128 | **129** |
  | Lua | 128（W15 新增守卫） | 不递归（N/A） |

  两处根因：① Rust 与 C 的守卫用 `>=`，**128 层就报，而文案写的是「超过 128 层」——
  行为与自己的文案自相矛盾**；② C++ 的 `key: [ ... ]` 分支**直接调 `parse_array`**，
  绕过 `parse_value` 上的守卫，于是**第一层数组白送一层**（这与 W13 修的「块嵌套直接递归
  绕过守卫」是同一个洞，当时只补了块、漏了数组）。
  现统一为「文档根不计层」：Rust 两处 + C 一处守卫改 `>`，C++ 补 `parse_array_nested`。
  边界已**逐格钉住**在 `rust/tests/error_codes.rs`、`c/test_limits.c`、`cpp/test_limits.cpp`、
  `lua/test_codes.lua` 四处（只测 100 与 100000 是**测不出差一格**的）。
- ⚠️ **JS 空键列表曾抛宿主 `ReferenceError`**（W14 修）：`include "x.sml" as w { }` 这条分支
  调用了 `parse()` 作用域内的局部报告函数。尤其要留意 **`parseSafe()` 路径当时是"静默"的**
  —— 它把异常吞成 `{ok:false}` 且**不给 `code`**，用户既没拿到码、也看不到异常。
  现在两条 API 都给出 `E-INCLUDE-005`。5 份 `sml.mjs` 副本已同步（逐字节一致）。
- ⚠️ **Lua 的三处行为变更**（W10 落地 Lua 侧时一并发生；判别实验：同一份用例配 HEAD 版
  实现 **15 条红**，新实现 26 通过 / 0 失败）：
  - **未闭合的块 / 数组不再"能解析"**：原先 `a { b: 1`（文件到此结束）会**静默**返回
    `{a={b=1}}` —— 用户拿到被截断的文档却没有任何提示；现在报 `E-PARSE-001`。
    顶层**裸块**（`a: 1` + `b: 2` 这种没有外层花括号的）到 EOF 收尾仍然合法 ——
    这一格最容易误伤，已有反向用例钉住。
  - **键位置的结构记号不再被当成键名**：原先 `: 1` 会解析出键名为 `:` 的树（静默），
    `a { { x } }` 报的是「未匹配的右大括号」（**错误的码**）；现在都报 `E-PARSE-006`。
  - **副作用（要留意）**：用了 `@contract` / `@is` / `include` 的文档
    （如 `examples/app.sml`、`SML_政务数据密级标注规范_报送稿.sml`）在 Lua 里
    **以前是"能解析"的，但解析出的树是错的** —— `include "x.sml"` 被当成裸块键，
    把后面到第一个 `{` 的内容全吞进片段体。现在它们明确报 `E-PARSE-006`。
    从「静默给错树」变成「响亮地拒绝」是本意，但**这是用户可见的变化**：
    要不要在 Lua 里补契约 / include 支持是**产品决定**，已登记为 W20。
- ⚠️ **C++ include 的两处语义变更**（W18，与 Rust/C 对齐）：
  - **子文件的基准目录换成它自己的所在目录** —— 嵌套 include 现在相对**父文件**解析。
    原先一律相对根目录，子目录里的链式包含会找不到文件；Rust/C 一直是前一口径。
  - **超深 / 成环 / 读不到不再是静默跳过**，分别报 `E-INCLUDE-004` / `E-INCLUDE-002` /
    `E-INCLUDE-001`。其中 `E-INCLUDE-002` 原先虽然已在码表里声明了 `cpp`，
    却**不可能触发**（环检测的栈永远是空的）。
  - 「越界」的判定顺序也对齐 Rust/C：**先 canonicalize 再比前缀**，故目标不存在时报的是
    `E-INCLUDE-001` 而不是 `E-INCLUDE-003`（拿一个不存在的路径去测越界，测到的其实是
    「读不到」；本仓库的用例已改成用**真实存在**的越界目标）。
- ⚠️ **C 与 C++ 的一批对外行为变更**（W10 落地时顺手修的缺陷，每处都有「触发条件 → 期望码」
  用例，并用**判别实验**证明过：把修复还原成旧实现后，同一份用例会失败）。
  它们不是「文案改了」，是**取值或成败结论变了**：
  - **C++：`min`/`max` 边界原用 `std::stoll`**（而 `TypeModifiers` 是 `double`）——
    `stoll("0.5")` 按 strtoll 语义**返回 0 且不抛异常**，于是 `max 0.5` 把上界调成 0
    （**合法值被误判越界**）、`min 0.5` 把下界调成 0（**越界值被漏放**）；`"abc"` 才抛，
    又被 `catch(...){}` 吞掉（**边界静默丢失**）。改为 f64 解析 + 语法闸门 + 有限性检查，
    报 `E-PARSE-022` / `E-CONTRACT-010`（与 Rust 同码）。
  - **C++：`coerce_word` 进 `stoll` 前没清 `errno`** —— 同一线程里只要先前有过一次溢出，
    后面**每个普通整数**都被判成 `Float`，于是声明为 `int` 的字段被契约**误报
    `E-CONTRACT-002`**（合法数据被判类型错误）。纯 bug，同源修复。
  - **C++：`@include` 目标不存在**原先静默，现报 `E-INCLUDE-001`（其余端都报）。
    ⚠️ 该分支必须用 `inside` 设闸，否则会**覆盖**越界那条的 `E-INCLUDE-003`（把码报错）。
  - **C++：未知转义**原先静默接受，现报 `E-LEX-004`，接受集收窄到 Rust 的严格集
    （`\n \t \r \0 \" \\ \uXXXX`）；全仓 `.sml` 与 `cpp/*.cpp` 都没用到被去掉的 C 风格转义。
  - **C：超 i64 的整数字面量**原先被 `strtoll` **静默夹成 `LLONG_MAX/MIN`**（错值、形态却像
    正常值，还能通过 `int` 契约校验），现改为**保留为字符串**（对齐 Rust 的 `coerce_word`；
    `strtod` 那支不动 —— 它给的 `Float(inf)` 本来就和 Rust 一致）。
  - **C：契约 `min`/`max` 边界**原先 `atof` 且不看错误 —— `min abc` 静默，`max abc` 竟然报
    **`E-CONTRACT-005`（错码）**，把合法值判成越界。改为 `strtod` + `endptr` 尾随校验：
    非数字 → `E-PARSE-022`、非有限 → `E-CONTRACT-010`。
  - **C：未闭合的块 / 数组**原先静默返回残缺对象，现报 `E-PARSE-001`
    （块 / 数组 / 契约体三种；顶层块正常结束不算）。
  - **C++：`1e400` 原被归成 `Str`**（Rust 与 C 都是 `Float(inf)`），现对齐为 `Float(inf)`；
    超 i64 整数则保持 `Str`（这两格方向相反，别搞混）。
- ⚠️ **不兼容：语言层 crate 的错误类型由 `String` 改为带码错误**
  （`sml-lex` 0.1.0-alpha.3 / `sml-include` 0.1.0-alpha.2 / `sml-contract` 0.1.0-alpha.3 /
  `sml-parse` 0.1.0-alpha.3）。影响面被压到最小：
  - `impl From<SmlError> for String` 只取**文案**，所以既有那些 `Result<_, String>` 的
    调用方 `?` 一行都不用改，行为也不变；
  - `sml_parse::ParseError` 成了 `SmlError` 的别名，名字保留；
  - `Display` 现在会把码缀在文案之后（`sml: 字段 ... [E-CONTRACT-002]`）——
    这是**可观察的变化**，若有逐字比对错误文案的 golden 需同步；
  - 因此 `swsml` 下次发版需按 **MINOR** 处理（0.x 阶段 MINOR 视为不兼容），
    本文件所在版本号未动。
- `sml_error`（C-ABI）结构体新增 `code_str[16]`：**字段布局变了**，C/C++ 调用方
  按头文件重新编译即可（`c/sml_rs.h` 与 `cpp/sml_rs.hpp` 已同步，C++ 的 `Error` 亦加 `code_str`）。
- **`to_sml` 排版规则**（`sml-value/src/dump.rs`，**影响所有 SML 文本输出**）：
  数组元素与顶层非对象值改为「**扁平才留一行，含容器就展开多行**」——
  扁平 = 直接子项全是标量。于是 `phoneNumbers: [ { type: home } { type: office } ]`
  仍旧一行，而 `{ fields: { field: [ … ] } }` 这类真结构会分行缩进。
  动机：`--from xml` 迁移 CMSIS-SVD 时，一个 `peripheral` 元素被压成 **15 644 字符的单行**，
  既无法阅读也无法 diff/编辑。这是**可观察的行为变更**（下游若有逐字节 golden 需同步）。
- `--from xml` 的「纯文本元素折叠」+ `to_sml` 新排版，对 CMSIS-SVD（432 711 B）的净效果：
  **最长行 15 644 → 311 字符**、行宽中位 121 → 32、`>500` 字符的行 71 → **0**、
  行尾空白 3 行 → **0**；产物 277 440 → **276 669 B**（折叠省 84 658 B，展开的缩进花回
  83 887 B，基本抵消 —— 换来 544 → 6530 行的可读结构）；`_text` 归零，
  峰值内存 13.5 → 8.2 MB；`xml→sml→json` 仍与 `xml→json` **逐字节一致**。
- `to_sml` 不再在「`键:` 后接块」时于**行尾留空格**（旧输出每处都留一个，
  SVD 产物实测 3 行带尾随空白；新版面下该模式会成倍出现，故按值类型决定是否补空格）。

### 修复

- 🔴 **VSCode 扩展在 VS Code 1.138 上从来没能激活过（致命：顶层读了已被移除的 API）**
  （`editors/vscode/src/extension.js`）：`DIRECTIVES` 等模块级数组里写了
  `insertTextFormat: vscode.InsertTextFormat.Snippet`，而 **VS Code 1.138 的扩展宿主里
  `vscode.InsertTextFormat` 已被移除**（实测：宿主 bundle `extensionHostProcess.js` 里该名字
  出现 **0** 次；VS Code 自带的 `out/vscode-dts/vscode.d.ts` 里没有 `enum InsertTextFormat`）。
  顶层读它的成员 ⇒ **加载模块时即抛** `TypeError: Cannot read properties of undefined
  (reading 'Snippet')` ⇒ `activate` 从不执行 ⇒ provider / 命令 / 输出面板**全都不注册**；
  报错只落在「扩展主机」日志里，界面上就是「**悬停 / 右键菜单 / 特别高亮全都没反应**」。
  证据（本机日志）：`Activating extension snoware.sml-lang failed…`，从 **0.4.1（09-18 20:30）**
  到 0.4.2 每次激活都失败 —— 也就是说这个扩展此前**一次都没有真正跑起来过**。
  修法：`INSERT_SNIPPET = 2` / `INSERT_PLAIN = 1`（线上协议数值）+ `CompletionItemKind`
  改经 `CK(name, fallback)` 取值（枚举缺失时退化为 Text，仅图标不同）；顶层新增护栏注释。
  闸门：`scripts/_verify_ext.mjs` 新增「**已被移除的 API**」黑名单（现含 `InsertTextFormat`，
  先剥注释再匹配，避免注释里提到就误报）；假宿主**故意不提供** `InsertTextFormat`（对齐 1.138），
  只要顶层再读它，`activate` 就会当场抛异常。
  ⚠️ 教训：`devDependencies` 里的 `@types/vscode@^1.80` 是**编译期**的口径，运行期扩展宿主的
  API 演进只能拿**目标版本**的 `vscode.d.ts` / 宿主 bundle 去核（见 HANDOFF §22.12）。

- **语法高亮：同一行的第二个及以后的字段不着色（「字段组合」/ 内联对象全中）**
  （`editors/vscode/syntaxes/sml.tmLanguage.json` 的 `#key`）：该规则整个 match 被 `^\s*`
  **锚在行首**，于是只有行首那个词算键。真实 Oniguruma 引擎实测：
  `web { host: a, port: 8080 }` → `host✗ port✗`；`m: [ { a: 1, b: 2 } ]` → `a✗ b✗`；
  `a: 1 b: 2` → `b✗`；语料 `address { city: Shanghai  zip: "200120" }`、
  `{ type: home   number: "…" }` 同样中招。
  现改为三分支：① 行首键（原逻辑，允许缩进）；② 紧跟 `{` / `,` 且其后是 `:` 的词；
  ③ **空格分隔**的后续键（SML 分隔符可省）。两道防误伤：`(?<!:)` 挡住 `url: https://…`
  这类**值里的冒号**（`https` 不得着键色），`(?=:)` 挡住数组裸值 `[ a, b ]`。
  实测修复后全部着色、负向零误判（注释里的 `词:` 因注释规则优先仍然无色）。
  闸门：`scripts/_verify_tokenize.mjs` 新增 5 条「同行多字段」+ 3 条负向断言（真实引擎）；
  `scripts/_verify_grammar.mjs` 的 JS 侧镜像改为**多分支联合 + 全局扫描**（原来读
  `repo.key.match`，改成分支数组后它静默漏检 ⇒ 已修，并补 3 条用例）。

- **VSCode 扩展：「.sml 没被当成 SML」⇒ 悬浮 / 诊断 / 补全 / 右键菜单一起失效**
  （`editors/vscode/src/extension.js` 新增语言守卫 + `package.json` 加
  `workspaceContains:**/*.sml` 激活事件）：语言模式不是 `sml` 时，provider（按语言选择器
  注册）不被调用、右键菜单项（`when: editorLangId == sml`）不显示、TextMate 语法也不生效
  —— 与「扩展坏了」完全无法区分，且**不留任何痕迹**。现在：工作区里只要有 `.sml` 就会激活
  （否则语言模式不对时扩展压根不激活，检查根本跑不到 —— 鸡生蛋问题），发现 `.sml` 语言模式
  不是 SML 就弹警告并给「**设为 SML**」按钮（调 `languages.setTextDocumentLanguage`）；
  改不动时说明本窗口没注册 `sml` 语言 ⇒ **扩展没被加载**；自检面板新增
  `SML 语言已注册：✓/✗` 一行作为判据。

- **VSCode 扩展：6 条命令从未出现在命令面板（`package.json` 的贡献点写错了位置）**：命令原先
  声明在**顶层 `commands`** —— 那不是有效贡献点，VS Code **静默忽略**它 ⇒ **命令面板里搜不到
  任何 SML 命令**（右键菜单仍可用，因为它走 `contributes.menus`，与 commands 声明无关）。
  已整块搬进 **`contributes.commands`**（26 行纯位移，diff 干净）。
  ⚠️ 为什么一直没发现：**发布闸门也在读顶层 `pkg.commands`** —— 等于「校验了一个没人看的键」，
  于是一路绿灯。闸门已改成**双向断言**（声明↔实现各自互为子集），并新增一条回归闸门：
  **顶层 `commands` 必须不存在**。反向联调验证过：把顶层 `commands` 加回去 ⇒ 闸门报
  `✗ 命令声明在 contributes.commands（顶层 commands 是无效键）` 且 rc=1。

- ⚠️ **C++ 解析器四处静默错解（2026-09-19，与 W4 同批）**：
  1. **`$env.X` 在值位置被拆成两个 token** ⇒ 值退化成 `null`，而 `env.X` 掉到**键位置凭空造出一个键**
     （`examples/secrets.sml`：`resendApiKey: null` + `env.RESEND_API_KEY: env.RESEND_API_KEY`）。
     根因是词法器把 `$` 切成独立 token，而 Rust 里 `$` 就是**普通词字符**（`coerce_word` 里的
     `$env.` 分支一直没被走到）。现在不再切 `$` ⇒ 实测 `resendApiKey: ""` 等三个键全部正确、与 Rust 一致。
  2. **`@contract X strict { … }` 的 `strict` 不被消费** ⇒ 后面那个 `{` 不再是「紧跟契约名」，
     **整条契约声明被跳过**（`rust/tests/fixtures/gov_demo.sml`）。现在 `loose` 与 `strict` 都消费
     （`allow_extra` 分别 true / false）。
  3. **契约体未闭合静默通过**（`if (…) st.i++;` —— 有 `}` 才吃、没有就算了）⇒ 现在报
     `E-PARSE-001 契约体未闭合（缺少结束符号 }）`，与 Rust 同格。
  4. **键位置的裸块判据过宽**（只要「后继是个词」就试，参数收集还**贪心**地吃到 `{`/`}`/`,`）
     ⇒ 改用 Rust 的 `bare_block_ahead()`，参数只收词且经 `coerce`；`examples/common.sml`
     从 **14 行 → 1 行**（= Rust）。
     ⚠️ **顺序很重要**：先修 1–3 再收紧 4 —— 反过来做，`secrets.sml` / `slint/login.sml` /
     `gov_demo.sml` / `advanced.sml` / `showcase.sml` 会从「静默错解」直接变成 `E-PARSE-006` 硬失败
     （那个过宽判据一直在吞 1–3 的痕迹）。结果：C++ ↔ Rust 的「行数不同」**2 → 0**。
- **三处「测试自己坏了、却没人知道」**（W16 的 A 批顺带查出并修好）：
  1. `rust/tests/c_abi.rs` 里的 `CSmlError` **镜像结构缺 `code_str`**（W10 给真实 ABI
     结构加了这个字段，测试侧没跟着改）⇒ `CSmlError::fill` 会**写到测试这块结构之外
     16 字节**：栈上越界写，属 UB，只是恰好没炸。已补齐并加注释（凡是「与 C 头文件对齐」
     的镜像结构，改一边必须改另一边）。
  2. `rust/tests/serde_bridge.rs::from_str_enum_variants` 依赖**顶层裸词**，而 W16 的
     `E-PARSE-008` 已把顶层标量判为非法 ⇒ 该用例早就红了却没人发现：这条测试在
     `#![cfg(feature = "serde")]` 下，而 `cargo test --workspace` **不开** serde
     ⇒ 全量回归跑不到它。已改成等价的键值形式（`in-maintenance: in-maintenance`）。
  3. `sml-value/src/serde_bridge.rs` 的 doctest 引用 `sml::serde::from_str` ——
     而 `sml` 是**门面 crate**，`sml-value` 不可能引用它（循环依赖），加上 serde
     默认关闭 ⇒ 这条 doctest **从来没被编译过**。本轮在 `--features serde` 下真跑，
     当场红；已改成真测 `from_value`（文本入口的 doctest 留在门面 crate）。
- **测试夹具 `_gov_demo.sml` 从未进版本库**：它被 `rust/tests/gov_demo.rs` 真读，
  却在 `.gitignore` 的 `**/_*` 之下 ⇒ `cargo test` 只在「本机恰好有那个文件」时通过
  （换机器 / CI 必红，报的还是与测试意图无关的「读取失败」）。已改名挪到
  **已跟踪**的 `rust/tests/fixtures/gov_demo.sml`。
- **C 的嵌套数组被静默错解，还会凭空造键**（W17，P0 数据正确性）：
  `parse_array` 原先没有 `[` 分支，`[` 落到兜底 `else { next(ps); }` 被**丢掉**；后果不只是
  "少一层"，而是**内层的 `]` 被外层当成结束符**、剩下的 token 交给外层块解析**当成了键名**。
  改前实测（一个错都不报）：

  | 输入 | 改前得到 | 期望（Rust/JS 实测） |
  |---|---|---|
  | `m: [ [ a ] ]` | `{"m":["a"]}` | `{"m":[["a"]]}` |
  | `m: [ 1, [2, 3], 4 ]` | `{"m":[1,2,3],"4":4}` ← **凭空多一个键** | `{"m":[1,[2,3],4]}` |
  | `m: [ [a], [b] ]` | `{"m":["a"]}`（`[b]` 整个丢掉） | `{"m":[["a"],["b"]]}` |
  | `a: ` + 100 层 `[..]` | `{"a":[]}`（吞成空数组） | 正确嵌套 |

  假键比"少一层"更坏：**它会被下游当真实数据**（还能通过契约校验）。现在与 Rust
  （`parse_array_inner` 的 `Tok::LBrack` → `self.parse_array()`）和 JS（同源 bug 早先已修）
  **逐字一致**。
  ⚠️ **递归与守卫必须同时给**：`parse_array` 一并拆出了守卫 wrapper（与 `parse_block`
  共用 `ps->depth`，与 Rust 的 depth 计数器同口径）。只加递归不给闸 = 用**栈溢出**换静默
  错解 —— 栈溢出在 C 里是段错误，错误处理接不住。
  用例：`c/test_codes.c` 新增 `test_nested_arrays`（7 条形状断言，用 `sml_parse_json` 比对，
  期望值从 Rust/JS **实跑**抄来，不是读代码推的）；`c/test_limits.c` 补回数组嵌套的
  「128 放行 / 129 报」两格（修前写不了：129 层也会"成功"返回 `{"a":[]}`）。
  **判别实验**：HEAD 版 `sml.c` 配同一份新用例 ⇒ **8 条红**（7 条形状 + 129 层那格），
  新实现 0 红。⚠️ 其中 **128 层那格改前改后都通过**（它的作用是"守卫不能误伤"，**没有
  判别力**）—— 两种断言的作用已分别写在测试注释里。
- **`smltools` 不再按文案猜码**（W21 ④）：删掉 `backend_error` / `custom_rules_error` 两个
  文案前缀映射函数；`E-LIMIT-004`（8 个后端共 20 处递归深度）、`E-LIMIT-005`（custom 输出超长）、
  `E-LIMIT-006`（custom 数组循环）、`E-EXT-006`（custom 规则文档非法）、`E-CLI-007`（各后端
  自身失败）现在**都由 emit 后端自己带码**。新增 6 个**喂真实上游错误**的单元测试 ——
  不是手写字面量（那正是旧做法失效的原因）。
- **`toml.rs::descend` 的 `unreachable!()` 改为带码 `Err`**（W21 ③）：**panic 是不可接受的
  对用户失败方式**（一份畸形 TOML 就能让工具崩，而不是给出带码错误）。走真实解析路径**不可达**
  （`insert_keys` 的 `ensure_table` 已保证前置条件），故按**防御性**处理并写明；
  另有一条**故意违反前置条件**的用例证明「万一走到也只给带码错误、不 panic」。
- **clap 的用法错误现在带 `E-CLI-008`**（W21 ②）：`Cli::try_parse()` 接管那层输出 ——
  **保留** clap 的 Usage 提示与退出码 2，只在其后追加码；`--help` / `--version` **不受影响**
  （实测 rc=0、输出里不含码）。
- **`smltools` 的失败输出不再出现重复工具前缀**：`Err` 文案若自带 `smltools: `，外层统一
  `eprintln!("smltools: {e}")` 就叠成 `smltools: smltools: …`，让用户以为消息被**嵌了一层**。
  共删 6 处内层前缀（`E-INCLUDE-012` 未加引号 / `E-INCLUDE-001` 读取失败 /
  `E-INCLUDE-004` 嵌套超限 / `E-IO-007` ×3），并加**不变式断言**把它钉住 —— 断言放在
  `drive()`（全部 CLI 用例的唯一出口）里，于是**每一次** CLI 调用都过一遍（含
  `--help` / `--version` / `--lint`），另有一个具名用例把历史上中招的三条路径各走一遍，
  让规则有个可读的名字。判别实验：把内层前缀加回去 ⇒ 两条用例**同时变红**。
  **注释能提醒人，断言才拦得住人。**
- **C++ `@include` 会毁掉文档，且环检测名存实亡**（W18，P0 数据完整性）：
  `@include "b.sml"` 的目标文件字段**全部丢失**，includer 自己后面的字段也被吞掉 ——
  实测 a.sml = `@include "b.sml"` + `from_a: 1` 解析完只剩一个垃圾键 `include = "include"`。
  根因是「边解析边插 token」：把目标文件的 token 插到**路径 token 之前**、之后又 `st.i++`
  去"吃掉"路径，于是恰好跳过插入段的**首 token**，`include` 退化成裸块键、
  把后续字段全当参数吞掉。
  更麻烦的是**环检测的栈 push 完立刻 pop、永远为空** —— `E-INCLUDE-002` 实际不可能触发，
  自包含/互包含根本拦不住；而那个 off-by-one 恰好**压住了**无限展开（首 token 被跳过 ⇒
  嵌套的 `@include` 永远不被当指令），**所以只改索引会把它变成真死循环**。
  改法照 Rust/C 的架构：include 挪到**解析之前**的 `expand_includes` 里递归展开，
  链栈只装「根 → 当前」这一条路径（故**菱形包含仍合法**，并已加反向用例挡住
  「见过即拒」那种修法），同时补齐两道闸 —— 嵌套深度（`E-INCLUDE-004`，32 层）
  与**全局**展开次数（`E-LIMIT-003`，10000，挡菱形包含的 2^N 膨胀），后者 C++ **原本完全没有**。
  顺带修掉三处同源的静默：基准目录不可解析原被 `weakly_canonical`（只做词法规范化）
  消解成"目录里没这个文件"、报出 `E-INCLUDE-001`（错码）→ 改用严格 `canonical` 报
  `E-INCLUDE-010`；子文件的词法错误原被**丢弃**、残段照插（未闭合字符串变成静默截断的
  文档）→ 报 `E-INCLUDE-011`；超深包含原为静默跳过（字段凭空消失）→ 报 `E-INCLUDE-004`。
  用例 12 条，含**判别实验**：同一份用例跑 HEAD 版实现，**14 条红**
  （自包含 / 互包含 / 超深 / 膨胀四格在旧实现上全是"静默通过"）。
- **JS 侧嵌套数组被静默截断并产生伪键**（`js/sml.mjs` 的 `parseArray`）：数组元素是 `[` 时
  落到 `else break`，数组提前结束，剩下的 `[` 被外层块解析当成**键名** ——
  `m: [ [ a ] ]` 得到 `{"m":[],"[":"a"}`，**不报错但数据是错的**（Rust 侧同一输入得到
  `{"m":[["a"]]}`，Playground 与 VSCode 扩展都吃这个解析器，影响面不小）。
  已补递归分支，现在与 Rust 逐字节一致。5 份副本同步：扩展内置 `vendor/`、站点
  `static/{,lib/}`、`public/{,lib/}`。
- **JS 契约不支持 `[T]` 数组类型简写**（`js/sml.mjs`）：只认 `array [T]` 关键字形式，
  照 README/教程写的 `tags: [str] optional` 会抛「字段类型期望标识符」——
  **对合法 SML 的假报错**，而 VSCode 扩展直接复用该解析器，故一路显示到编辑器里。
  已补 `[T]`（含 `[ str ]` 空格形式），元素类型校验真实生效
  （`[str]` 拒绝数字：`字段 tags 类型错误：期望 array[str]，实得 array`）。
  顺带把修饰符解析抽成 `parseFieldModifiers`，两条路径共用一套规则。
  扩展内置副本已用 `scripts/sync-parser.py` 同步。
- **`sml-regex` 量词语义（`+` / `?` / `*` 全部 off-by-one）**：原子总被强制消费一次、
  量词只管「额外」次数，故 `x+` ≡ `xx*`（`^ab+c$` 匹配 `abbc` 却不匹配 `abc`）、
  `x*` ≡ `xx*`、`x?` ≡ `xx?`；且量词只认「前一个字符」，`[0-9]+` 这类「字符类 + 量词」
  整体失效。改为把「匹配一个原子」与「重复几次」拆开（新增 `Atom` / `parse_atom`），
  量词与其作用的原子在同一处处理（贪婪 + 回溯）。**会改变既有匹配结果**，
  `sml-regex` 按纪律升 `0.1.0-alpha.3`（`sml-include` 的 `^0.1.0-alpha.1` 可解析到本版）。
- **`sml-regex` 的 `^...$` 锚点松判**（同一次修复中翻出的第二个缺陷）：
  两端锚同时出现时只判「能匹配」而没判「匹配到结尾」，`^conf\.sml$` 会匹配
  `conf.sml.bak` —— 对「按文件名 include 过滤」是危险的松判（本不该包含的文件被包含）。
  已补齐 `anchored_end` 校验。独立量词 / 未闭合字符类 / 结尾悬空转义一律「不匹配」而非 panic。
- `--to highlight` 生成的 Zed 查询里，方言指令谓词写的是 `^(form|policy)$`，
  而 grammar 的 `directive` 节点文本是 `@form` —— **永远匹配不上**（不报错，高亮静默失效）。
  改为 `^@(form|policy)$`，并由 `zed_highlights_restrict_directives` 断言钉住；
  同时标点从 `(punctuation)`（grammar 里没有这个节点）改为匿名 token 列表
  `["{" "}" "[" "]" ":" ","]`。
- **C++ / C 的嵌套深度守卫可被绕过，深输入直接打穿栈**（`cpp/sml.cpp`、`c/sml.c`）：
  纯块嵌套（`a{a{a{…}}}`）在 C++ 侧是**直接递归** `parse_block`、绕过带守卫的入口，
  深度计数根本不增长；把「子块走受限入口」补齐后又暴露出**第二层问题** —— 守卫超限时把
  深度**复位为 0**，而复位不会让已压上的栈帧退回，外层循环随即又从 0 往下钻
  （每 128 层一轮地反复压栈），10 万层仍然崩。C 侧同样中招：它的 `parse_block` 明明是
  带守卫的 wrapper（审计据此判过「C 无此洞」），但同一个「复位不能收手」让它一样被打穿。
  改法：C++ 新增 `aborted` 中止标志、各层循环见到就 break；C 复用既有的 `ps->failed`
  同样 break —— 栈才会真正退掉（Rust 当年是靠 `Err` 的 `?` 向上传播解决的，思路一致）。
  两侧新增 `test_limits.cpp` / `test_limits.c` 并接进各自 runner：10 万层块 / 数组 /
  交替嵌套**报错返回而不崩**，100 层照常解析。
- **C 在 `err` 传 `NULL` 时越界写 / 空指针写**（`c/sml.c`；`sml.h` 明写 `err` 允许为 NULL）：
  ① 版本错误分支把 **1 字节**的复合字面量当缓冲，却拿调用方给的 `errsz` 作写入上限 ——
  `sml_parse(text, NULL, 256)` 就能往那 1 字节里写至多 255 字节，直接踩栈；
  ② 另有 21 处 `snprintf(errbuf, …)` 在 `errbuf` 为 `NULL` 时是空指针写。
  两类一起收敛到统一的 `set_err()` 助手：缓冲区为空（或大小为 0）时**一个字节都不写**。
  回归用例覆盖深嵌套 / 未知版本 / 未定义契约 / 空指针入参四条路径的 `err=NULL` 与 `errsz=0`。
  顺带的行为变更：C 的契约校验失败现在会**立即中止**解析（原先会把余下全文解析完再丢弃），
  对外仍是「返回 NULL + 错误文案」，只是文案以**第一条**错误为准。

### 已知限制

- **对象键序不保证（约定，2026-09-19 定）**：对象（`{ … }` 与键值块）是**映射**不是序列 ——
  参照实现 Rust 的 `Value::Object` 基于 `BTreeMap`（序列化**按键排序**），C / C++ / JS / Lua
  **保源序**。⇒ **不要依赖对象的键序**；需要有序序列时用**数组**（`[ … ]`，元素可写成裸块）。
  已写进 `README.md` / `README.en.md` 的「跨实现差异与约定」、`llms.txt`、`ch12-smltools.md`、
  `HANDOFF` §3.2。**Rust 侧不改**（换保序映射会牵动契约 / include / `@for` 一串按 BTreeMap 写的地方）。
- **引号策略是跨端差异（注明，2026-09-19 定）**：Rust 的 `to_sml` 对含特殊字符的裸键/值加引号
  （中文标点、`%`、看起来像数字的 `1.1`、`0x20`），C / C++ 只对含空白与 `:` `#` `{` `}` 的加；
  两种写法都能被各自读回。⚠️ **但有保真后果**：宽松策略下「看起来像数字的字符串」回读会
  **被重新归类**（`schemaVersion: 1.1` 回来是浮点）—— 需要严格保真的字符串请显式加引号。
  **不统一实现**，只写清（README 中英 + `HANDOFF` §3.2）。
- ⚠️ **两处「自己人」的问题（2026-09-19 定位，**未改**）**：
  ① **`examples/slint/login.sml` 是坏样例 —— 所有实现都拒（含 Rust）**：`` text: `root.busy ? "登录中…" : "登录"` ``
  里的**三目冒号**落到键位置 ⇒ `E-PARSE-006`（C 报 E-PARSE-003）。根因：**反引号不是字符串定界符**
  （Rust / C / C++ 都只当它是**普通裸词字符**，`` `#0f1117` `` 里的 `#` 还会起注释），而该文件头注释
  却宣称支持三目 `` `a ? b : c` `` ⇒ 样例与实现不符。**要么改样例，要么给语言补「反引号原始串」**。
  ② **行级 `&frag` 展开（splice）C / C++ 未实现**：Rust 把块内独立一行的 `&base` **展开进父块**
  （实测 ⇒ `{"w":{"a":1,"b":2,"c":3}}`），C / C++ 把它当键名 ⇒ 多一个 `&base` 键，**严格契约报
  `E-CONTRACT-004`**（`rust/tests/fixtures/gov_demo.sml`）。⚠️ 同批发现 **README 中英原先写反**
  （说「块内裸写 `&base` 不展开」）—— 已按参照实现的实际行为更正。详见 `HANDOFF` §22.7。
- XML 里名为 `_attrs` / `_text` 的**子元素**会与保留键同名并按同名兄弟规则并成数组
  （可预测，不静默覆盖）。
- DTD 内部子集自定义的实体不解析（DOCTYPE 整体跳过），用到时按未知实体报错。
- **内存放大**：有属性/子元素的节点要背 `_attrs`/`_text` 与 `BTreeMap` 每节点开销，
  16 MB XML 实测峰值 529 MB（约 33×）。时间线性（16 MB ≈ 2.0 s），故大文件可行，
  但内存需按 ~35× 预留。`--strip` 会把峰值再抬高一截（要在持有旧树的同时建新树）。
- **`@name` 片段无法压缩「同构但值不同」的重复结构**：SML 的片段是**无参数的值拷贝**
  （`@base { … }` 注册后 `&base` 取到的是同一份值），所以 DMA 那类「7 个通道、
  偏移与描述各不相同」的寄存器块**没法**用片段收敛 —— 自动合并会改数据形状、
  且平铺不出参数化差异。`--from xml` 不做这种变换，缩放只靠上面的折叠与排版。

### 性能实测（release，2026-09-18）

| 输入 | 输出 | 耗时（中位） | 峰值内存 |
|---|---|---|---|
| `CH32V103xx.svd` 432 711 B（CRLF） | `--to sml` 277 440 B（改造前基线） | 165 ms | 13.5 MB |
| 同上 | `--to sml` 276 669 B / 6530 行（折叠 + 新排版） | 166 ms | 8.2 MB |
| 同上 | `--to json` 194 243 B | 190 ms | 9.0 MB |
| 同上 | `--to json --strip`（strip 对 XML 输入是空操作） | 168 ms | 21.0 MB |
| 合成 1 MB XML | `--to sml` | 281 ms | 39.3 MB |
| 合成 4 MB XML | `--to sml` | 597 ms | 137.2 MB |
| 合成 16 MB XML | `--to sml` | 1993 ms | 529.2 MB |

- **往返闭合**：`xml→json` 与 `xml→sml→json` 逐字节相等（折叠与排版前后都成立）。
- **深度上限**：10 万层嵌套 → `rc=1`、`第 1 行（字符偏移 384）：嵌套深度超过上限 128`，
  129 ms / 6.4 MB，**不崩溃**（Rust 栈溢出是 abort，`catch_unwind` 接不住，故必须设界）。
- 单行超大 XML 报错会同时给出行号与**字符偏移**（行号此时恒为 1，没偏移等于没法定位）。
- `to_sml` 侧新增 4 个回归测试（`sml-value/src/dump.rs`）：扁平容器仍一行、
  含结构元素展开、**深层嵌套数组被守卫截住不溢栈**、空容器/标量输出不变。
  注意这类改动**只看测试是否通过是无效的** —— 判据写错成恒真时测试照样全绿、
  产物一个字节不变，故测试直接对**输出行**下断言（行宽、行尾空白、行内容）。

## [0.2.0] — 2026-09-18 · smltools（原 smlconv）

### 变更

- **crate 改名：`smlconv` → `smltools`。** 原 `smlconv` crate 不再更新，包名、二进制名与
  全部文档/脚本引用一并迁移。改名对下游是**不兼容变更**（`Cargo.toml`、脚本、CI 里的包名与
  命令名都要改），故按 0.x 纪律动 MINOR（0.1.9 → 0.2.0）。
  新名字也更贴合定位：它不只是「转换器」，还包含**迁移**（`--from json|yaml`）、
  **特征剥离**（`--strip`）与 **lint**。
- 站点章节 URL `/book/ch12-smlconv/` → `/book/ch12-smltools/`，旧地址用 Hugo
  `aliases` 保留跳转（避免已发布链接 404）。

### 新增

- `--to json`：SML → JSON。用于**对接既有工具链**（jq / 各类 JSON 库 / 只吃 JSON 的 API），
  而不是替代 SML；复用 crate 内既有的 `jsonify`，不另写序列化以免行为漂移。
  注意：键按**字典序**输出（`Value::Object` 基于 `BTreeMap`），原始书写顺序不保留。
- `--to toml` / `--from toml`：与 TOML **双向互转**（对接 Cargo / pyproject 等生态）。
  解析支持表 `[a.b]`、表数组 `[[a]]`、点号键、内联表、跨行数组、
  基本/字面量/多行字符串（含 `\uXXXX`）、整数（`_` / `0x` / `0o` / `0b`）、
  浮点（含 `inf`/`nan`）、布尔；**日期时间按字符串保留**（与 SML 裸词日期同为字符串，
  往返不改变形态）。序列化按 TOML 的唯一正解输出：标量键在前、子表 `[path]`、
  对象数组 `[[path]]`。
- `--from json|yaml`：把存量 JSON / YAML 迁进 SML。缺省按扩展名自动推断
  （`.json` → json，`.toml` → toml，`.yaml`/`.yml` → yaml，其余 → sml）。
  YAML 侧为**最小可用子集**（块/流式映射与序列、引号、数字、布尔、null、注释、
  块标量 `|`/`>`、锚点与别名），不追求完整 YAML 1.2；`yes/no/on/off` 一律当字符串
  （避免「挪威问题」）。不支持 `? 复杂键`、`!!` 标签、合并键 `<<` 与多文档。

## [0.6.1] — 2026-09-18 · swsml

### 新增

- **`emit-html` 后端**：`sml::emit::to_html` / `HtmlOptions`，把 SML 文档转成带内嵌排版 CSS 的语义 HTML5
  （`topic`→`<article class="book">`、`section`→`<section class="chapter">`、`para`→`<p>`、`quote`→`<blockquote>`、
  `img`→`<figure><figcaption>`）；`label` 生成锚点 `id`，供 `{ref: label}` 交叉引用；支持 `standalone` / `fragment` 两种形态。
- 新 feature：`emit-html`，并**已进入 default features**（与其余 `emit-*` 一致，兑现「默认全部开启」的文档承诺）。
- **外置扩展机制**（`sml::ext` / `sml::contract_ext`）：下游可注册自定义 `@指令` 与自定义契约类型，
  **无需改动 crate 源码**。用于把「方言」收编到扩展点上，而不是把它们写进 SML 规范层：
  - `ext::Directive` + `ParseOptions` + `parse_with()`：`@form` / `@policy` / `@flow` 这类
    **元数据块**（文档里写了、解析结果里不该出现）由下游自己声明语义。
    两种参数写法都收：推荐 `@xxx name: X { }`，兼容 `@xxx X { }`（后者产出弃用诊断）。
    内置指令名（contract/is/type/version/feature/when/for）不可被占用。
  - `contract_ext::TypeCheck` + `TypeSpec::Ext`：注册 `image` / `link` / `time` 这类**领域类型**，
    校验器随 `FieldSpec.ext` 传递，校验期无需全局查表。
  - `contract_ext::Modifier` + `FieldSpec.ext_data` / `mods`：注册自定义**字段修饰符**。
    解析期改写规格、校验期回调 `check()`（数组字段同样会跑）。
    典型用途 `items_max`（数组元素个数上限）—— 刻意不复用语义不同的 `max`。
  - 约束：**不注册任何扩展时，行为与既有实现完全一致**（已有单元测钉住）。
  - JS 侧同步（`js/sml.mjs` 及三处副本）：`parse(text, { directives, types, warnings })`，
    语义与 Rust 侧对齐（未注册不放行、位置参数给弃用警告、外置类型判定次序
    `@type` > 外置 > 契约引用）。
- `sml-parse` 0.1.0-alpha.1 → 0.1.0-alpha.2、`sml-contract` 0.1.0-alpha.1 → 0.1.0-alpha.2。
  后者**含不兼容改动**：`TypeSpec` 新增 `Ext` 变体、`FieldSpec` 新增 `ext` 字段。

### 修复

- `sml-parse`：移除 `parse_impl` / `parse_impl_tokens` 中从未被读取的 `version` 形参。
  版本的影响在调用层已由 `features_for(v, …)` 全部折算进特性集，挂着只会留一个 `unused` 警告，
  同时让调用方误以为「传错版本会有效果」。私有函数，公开 API 不变。

### 文档 / 工具链

- 教科书新增第 12 章「smltools 多目标翻译器」（中英），并补进目录页。
- VSCode 扩展 **0.4.2**：引号串内的环境变量引用 `$env.NAME` 现在同样着色（此前只有 `${...}` 着色）。
- `site/public/sml.mjs` 同步至源实现 `js/sml.mjs`（此前是落后副本）：
  括号 `(` `)` **不再是分隔符**（与 Rust 词法 `sml-lex` 对齐，`备注: (重要)` 不再被切成垃圾键）、
  `enum(...)` 写法在解析层兼容重组、量词支持 `{最小,最大}` 对象式与平铺 `最小/最大`、
  `@is type(契约名)` 等价形式、块级类型标注 `typed-block`（opt-in）。
- `site/public/llms.txt`：VSCode 扩展版本号更正。
- **Zed 扩展语法首次通过编译验证（W8）**：`editors/zed/grammars/sml/grammar.js` 用
  `tree-sitter-cli@0.22.6` 跑 `generate`，并用 `parse test/parse/{basic,advanced}.sml`
  验证，**两份语料均无 `ERROR`、无 `MISSING`**；生成的 `src/parser.c`（约 53 KB）、
  `src/grammar.json`、`src/node-types.json` 已提交进 monorepo。发布仓库定为独立镜像
  `snoware/tree-sitter-sml`，`extension.toml` 的 `[grammars.sml]` 已指向它
  （`rev` 待该仓库首次 push 后填真实 sha）。

---

## [0.1.9] — 2026-09-18 · smltools

### 新增

- `--to html`：原生产出独立 HTML5（此前长文档只能 `--to md` 再借 pandoc / Hugo 转，会丢 `label` 锚点语义）。
- 依赖 `swsml` 升至 0.6.1 并显式开启 `emit-html`。

---

## 其它（非代码）

- 新增长文档压测素材 `story/`：35 万字 SML 小说样本 `novel.sml`、生成脚本 `gen_novel.py`、
  七个后端（`sml/md/svg/slint/lvgl/latex/xml`）的产出，以及压测发现的功能缺口报告
  `ISSUE_smltools_缺失功能.md`。用于回归「超长文档解析 + 全后端 emit」的健康度。
