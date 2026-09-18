# 变更日志

本文件从 **0.6.1** 起开始记录；更早的变更见 Gitee 提交历史与各次 release 说明。

格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)。
版本号遵循 Cargo 的语义化版本解释：**0.x 阶段 MINOR 变化视为不兼容**（0.6 → 0.7 会让下游无法自动更新），
PATCH 为兼容新增 —— 因此「新增后端 / 新增 API」走 PATCH（0.6.0 → 0.6.1），只有真正的破坏性改动才动 MINOR。

`swsml`（库）与 `smltools`（CLI）版本号互相独立，各记各的小节。

---

## [未发布]

### 新增

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
  - **C 与 C++ 的原生实现也已全量带码**（C 23 个码 / C++ 22 个码；码作**消息前缀**写进同一个
    `err` 缓冲，形如 `E-LEX-001 文案`，取码 `sscanf(err, "%15s", code)`）。
    四端各有一份「触发条件 → 期望码」用例：`rust/tests/error_codes.rs`、
    `js/probe-error-codes.mjs`、`c/test_codes.c`、`cpp/test_codes.cpp`，**交集部分逐一同码**。
  - 仍缺：**Lua**（`lua/lib/sml.soup` 是编译产物、源码不在本仓库，须回 Soup 工程重编）
    与 `smltools` 的部分输出。进度见 `errors/README.md` 的「码的落地进度」。
- **错误码体系（`E-<领域>-<序号>`）+ 官网查询工具**：`errors/codes.sml` 是唯一事实来源
  （用 SML 写，因此 `smltools` 自己就能校验它），`errors/gen_json.py` 走**真实工具链**
  `smltools --to json` 生成 `site/static/errors.json`，官网新增 `/errors` 查询页
  （按码/关键词/领域过滤，显示级别、规范文案、哪些实现已给此码）。
  **已全量：137 条**，覆盖 14 个领域，分四层（语言层 / 宿主绑定层 / 工具层 / 编辑器层）——
  逐条清点范围、以及「该报错却静默通过」的清单见 `errors/README.md` 的「清点」一节。
  形状沿用仓库既有先例（qsm 的 `E-ID-001`）。**关键约定：码是稳定契约，文案不是** ——
  各端措辞可不同（甚至不同语言），码相同即同一件事；这也是跨端统一（W3）的抓手。
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

### 变更

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
