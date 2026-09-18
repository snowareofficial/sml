# SML 错误码总表 —— **唯一事实来源**
#
# 形状：`<级别>-<领域>-<序号>`，级别 E=错误 / W=告警 / I=提示，序号三位、只增不改。
# 沿用本仓库既有先例（qsm 的 lint 已在用 `E-ID-001` / `W-ID-100` / `I-ID-200`）。
#
# 三条纪律：
#   1. **码是稳定契约，文案不是**。各端 msg 允许措辞不同（甚至不同语言），只要**码相同**
#      就表示同一件事。本表的 msg 是**规范文案**（简短），不要求各端逐字一致。
#      码一旦发布就不再变含义，要改就发新码并把它标成 deprecated。
#   2. **同因同码**。同一触发条件在五端必须给同一个码 —— 这正是 W3（跨端统一）的抓手：
#      各端先对齐码，文案差异就不再是问题。
#   3. 序号**只增**，删掉的码留空位，不回收。
#
# `status` 字段的含义（**容易误读，这里明确定义**）：
#   - `done`    = 列在 `impls` 里的实现**都已经真的报这个错**（行为一致）；
#   - `partial` = 各端行为不一致，差异写在该条的 `note` 里。
#   它描述的是**行为是否已实现**，与「是否已经带上码」无关 —— 后者是 W10 的活。
#   码的落地进度：**五端（Rust / JS / C / C++ / Lua）与 smltools 均已全量带码**（W10 已完成）。
#   逐端怎么带、新端怎么加，见 errors/README.md 的「码的落地进度」一节。
#   （这一行曾长期停在「C / C++ / Lua 还只有文案」—— 落地过程中忘了同步，是"文档自己打自己脸"
#   的典型：本文件与 README 的说法相反时，**以 README 的进度表为准**，它每次落地都会改。）
#
# 分层（`impls` 里的名字按层看，这样 W10 能一眼圈定落地范围）：
#   语言层    —— W10 的落地对象，五端必须同码：
#                LEX / PARSE / CONTRACT / INCLUDE / LIMIT / FEATURE / EXT
#   宿主绑定层 —— Rust 派生宏与 serde 桥、C-ABI、Lua 绑定：IO / INTERNAL / DERIVE
#   工具层    —— smltools 的命令行、迁入格式、lint：MIGRATE / CLI / LINT
#   编辑器层  —— VSCode 扩展与 LSP 桥：EDITOR
#   三层的划分只影响**落地顺序**，不影响码的稳定性：任何一层的码都是对外契约。
#
# 覆盖度：**已全量**（2026-09-18 清点）。清点按「语义条件」去重，范围覆盖
#   rust（含 derive 宏与 serde 桥、C-ABI）/ js（含编辑器侧桥与站点副本）/ c / cpp / lua /
#   smltools（CLI、迁入格式、lint、highlight 定制）/ VSCode 扩展。
#   清点范围与「该报错却静默」清单见 `errors/README.md` 的「清点」一节。
#   某个码具体出现在哪些文件，请**在源码里按码检索** —— 各端用的是同一串码，
#   这正是「码兼作索引」的用意；本文件刻意不逐条维护行号（行号必烂，且没人会更新）。
#   新增错误点请照 README 的「加一条码」办理。
#
# 书写注意（本文件本身是 SML）：**所有文本值一律加引号**。因为 `{`、`[`、`"`、`:` 与
# `/`（连写成 `//`、`/*`）在 SML 里都是结构字符，裸写会把一个值切成好几段。
# 引号内还要避开三样东西：反斜杠、半角引号、`${...}` 形式的插值模板 —— 前两样是转义，
# 第三样在 SML 字符串里是**模板插值**，写进文案会被替换掉。半角冒号也一律改用全角「：」。
# 这四条不是洁癖：本表第一版就因为这些东西被工具链拒了三次（经过见 README）。
#
# 生成物（改完本文件必须重跑；两者都支持 `--check` 只校验不写，给 CI 用）：
#   1. `python errors/gen_json.py`  -> site/static/errors.json（官网查询工具读它）
#      生成走真实工具链 `smltools --to json`，所以本文件必须是合法 SML —— 顺便自检。
#   2. `python errors/gen_codes.py` -> rust/sml-codes/src/codes.rs、js/sml-codes.mjs、c/sml_codes.h
#      各端**不再手抄码**：Rust 引用常量、C/C++ 用宏。JS 与 Lua 因为要能单文件跑
#      （浏览器 / 宿主），仍是字符串字面量 —— 由 gen_codes.py **反向校验**：
#      源码里出现的码必须在表里，挡住「手打错一位数字」这类编译器不管的错。

version: 1
coverage: "全量（2026-09-18 清点：语言层 + 宿主绑定层 + 工具层 + 编辑器层）"

# 领域（domain）：码里那一段，一经确定不再增加别名
domains: {
    # ---- 语言层 ----
    LEX: "词法"
    PARSE: "语法"
    CONTRACT: "契约"
    INCLUDE: "片段与 include"
    LIMIT: "深度与预算上限"
    FEATURE: "特性、版本与环境变量"
    EXT: "扩展点（外置指令、类型、修饰符）"
    # ---- 宿主绑定层 ----
    IO: "输入输出"
    INTERNAL: "内部错误（不应发生）"
    DERIVE: "Rust 派生宏与 serde 桥"
    # ---- 工具层 ----
    MIGRATE: "迁入格式（JSON / TOML / YAML / XML）"
    CLI: "命令行与工具用法"
    LINT: "lint 诊断"
    # ---- 编辑器层 ----
    EDITOR: "编辑器扩展宿主层"
}

codes: [
    # ================= 词法（语言层） =================
    { id: E-LEX-001 domain: LEX severity: E title: "字符串未闭合"
      msg: "字符串未闭合（缺少结束引号）"
      impls: [ rust cpp js c ] status: partial
      note: "改前 JS、C、Lua **静默接受**未闭合字符串（把余下全文当串内容，整篇结构被吞）。**W16 起 JS（首批）与 C（C 批）都报此码**；C 侧同时收手 —— 词法错一旦记下，sml_parse 立即返回 NULL，不让后续语法错覆盖第一个错的码。Lua 待做" }
    { id: E-LEX-002 domain: LEX severity: E title: "未闭合块注释（斜杠星号）"
      msg: "未闭合的块注释，遇到文件结尾"
      impls: [ rust js c ] status: partial
      note: "改前 C 与 C++ 都静默：C 把注释一路吃到文件结尾、之后**整篇内容凭空消失**，C++ 还会丢掉后续内容（见 README 清点的静默清单）。**W16 起 JS（首批）与 C（C 批）报此码**；C++、Lua 待做" }
    { id: E-LEX-003 domain: LEX severity: E title: "未闭合块注释（下划线星号）"
      msg: "未闭合的块注释，遇到文件结尾"
      impls: [ rust js c ] status: partial
      note: "改前其余四端静默接受（C 同样会吃掉文件剩余部分）。**W16 起 JS（首批）与 C（C 批）报此码**；C++、Lua 待做" }
    { id: E-LEX-004 domain: LEX severity: E title: "字符串含未知转义符"
      msg: "字符串含未知转义符，转义集见规范"
      impls: [ rust cpp js c ] status: partial
      note: "严格策略：未知转义即失败，避免路径与正则被静默损坏。C 改前 default 分支把该字符原样收下、**连反斜杠一起丢掉**（Windows 路径那类值会被静默改坏），W16 起与 Rust/C++/JS 同码" }
    { id: E-LEX-005 domain: LEX severity: E title: "Unicode 转义非法"
      msg: "Unicode 转义非法（位数不足、非十六进制、或非法码点）"
      impls: [ rust cpp js c ] status: partial
      note: "含代理区码点；定长四位不足时一并报此码。C 改前不足四位照收（静默变成控制字符），W16 起报此码；JS 改前抛宿主运行时异常（不是码），W16 首批已改为带码" }
    { id: E-LEX-006 domain: LEX severity: E title: "转义符后遇文件结束"
      msg: "字符串中的转义符后遇到文件结束"
      impls: [ rust ] status: partial
      note: "其余端静默接受。⚠️ 同因不同码：C 的解析入口对该输入报 E-LEX-001（它没有 Rust 那种按行重建，转义符后遇 EOF 就是未闭合串）—— 两个实现口径不同，未合并" }

    # ================= 语法（语言层） =================
    { id: E-PARSE-001 domain: PARSE severity: E title: "未闭合的块或数组"
      msg: "未闭合的块或数组，遇到文件结尾"
      impls: [ rust cpp c js lua ] status: partial
      note: "Rust 另分契约体与 `@for` 循环体两种未闭合；JS 只在数组类型简写 `[T]` 缺 `]` 时报错，普通块或数组到文件结尾会静默返回。**C、C++ 与 Lua 原先也静默（声明与实现不符），W10 期间补齐**：C/C++ 覆盖块 / 数组 / 契约体三种，Lua 覆盖块 / 数组两种（Lua 原先没有契约体，W20 补上后也覆盖契约体）；三端顶层裸块到 EOF 收尾都算合法" }
    { id: E-PARSE-002 domain: PARSE severity: E title: "闭合符错配"
      msg: "块或数组未正确闭合：期望一个符号，却遇到另一个"
      impls: [ rust js c ] status: partial
      note: "改前 JS 在闭合符不匹配时直接结束当前块（静默），C 只报笼统的未匹配结束符。**W16 起 JS（首批）与 C（C 批）都报此码** —— C 侧 `a { ] }` 此前静默得到空对象（数据形状被悄悄改掉）；Lua 待做" }
    { id: E-PARSE-003 domain: PARSE severity: E title: "多余的结束符号"
      msg: "多余的结束符号，没有与之匹配的开始符号"
      impls: [ rust lua cpp js c ] status: partial
      note: "Lua 原先抛的是**不带码**的笼统文案「未匹配的右大括号/右方括号」，W10 期间加码。**W16 起 JS 与 C 都报此码**：覆盖顶层多余的 `}`/`]` 与数组里多余的 `}` 两类 —— C 改前这两格都静默（`m: [ } ]` 得空数组、键值块后多一个 `}` 直接通过）" }
    { id: E-PARSE-004 domain: PARSE severity: E title: "孤立的 at 符号"
      msg: "孤立的 at 符号不是合法指令（符号与名字之间不可有空白）"
      impls: [ rust ] status: partial
      note: "历史上孤立 at 会静默吞掉后续整块内容，故必须报错而非忽略；JS 对同类输入报 E-PARSE-011。⚠️ C 是第三种映射：它的词法器不区分「@ 与名字之间有空白」—— `@` 换行后跟 `k: 1` 会把它当片段名，于是落 E-PARSE-005（同因不同码，未合并；W16 的 C 批扫过全仓语料，无一受影响）" }
    { id: E-PARSE-005 domain: PARSE severity: E title: "不是合法指令且缺少片段体"
      msg: "该指令名不是合法指令且缺少片段体"
      impls: [ rust js c ] status: partial
      note: "只有 Rust 与 C 会提示排查方向（合法指令名单）。JS 在未注册指令**带**片段体时仍静默当作片段定义（W16 余额，未做）。⚠️ C 的粒度更粗：它没有指令注册表，`@feature` / `@when` / `@for` 这些**它不实现的指令**也一并落此码（与拼错的指令在 token 流上同形、无法区分）—— 方向是「响亮拒绝」而不是静默丢行，但用户看到的提示会指向「拼写」；C++/Lua 待做" }
    { id: E-PARSE-006 domain: PARSE severity: E title: "期望键或标识符"
      msg: "期望键或标识符，得其它记号"
      impls: [ rust js cpp lua ] status: partial
      note: "亦覆盖数组元素类型、契约字段名等位置；`default` 与 `min`/`max` 的字面量位置另有 E-PARSE-021、E-PARSE-022。**Lua 原先静默**（`:` 会被当成键名、多余的 `{` 会被当成键），W10 期间补齐（探针实测 `: 1` 解析出键名为 `:` 的树）" }
    { id: E-PARSE-007 domain: PARSE severity: E title: "裸词中含逗号"
      msg: "非预期的逗号：裸词中不可含逗号，请改用数组或引号包裹"
      impls: [ rust ] status: partial
      note: "其余四端把逗号当普通分隔符切分，无此报错" }
    { id: E-PARSE-008 domain: PARSE severity: E title: "顶层标量不可往返"
      msg: "顶层须为容器（键值块、对象块或数组），单独的标量无法往返"
      impls: [ rust js c ] status: partial
      note: "**这条码此前是「死码」（W16 查明并接线）**：全仓只有常量定义 + doctest/文档引用，**没有一处 `SmlError::new(E_PARSE_008, …)`** —— 而改前 `42` 会被静默当成「键即值」的裸键、解析成 `{\"42\": 42}`（**凭空造键**，重新序列化 ≠ 原文；与 W17 的 C 嵌套数组同族）。Rust 自 W16 起真的报它，判据 = **顶层恰好一个标量 token**（故 `hello world` 这类两 token 的裸键对**不算**，它值可往返）。⚠️ **已知边界**：带指令的顶层标量（`@version v1` + `42`）token 数 > 1，按此判据**不报** —— 有意保守，宁漏不误伤。**JS 与 C 已于 W16 对齐**（三端判据逐字一致，C 的 token 流末尾固定有一个 T_EOF，故「恰好一个标量」= n == 2）；C++、Lua 待做" }
    { id: E-PARSE-009 domain: PARSE severity: E title: "命名空间段非法"
      msg: "命名空间段不可使用该名字"
      impls: [ js ] status: partial
      note: "防原型污染：危险段名在 JS 侧必须拒绝" }
    { id: E-PARSE-010 domain: PARSE severity: E title: "键名非法"
      msg: "键名不可使用该名字"
      impls: [ js ] status: partial
      note: "防原型污染：危险键在 JS 侧必须拒绝" }
    { id: E-PARSE-011 domain: PARSE severity: E title: "at 符号后缺名字"
      msg: "at 符号之后须为名字（指令名或片段名）"
      impls: [ rust js ] status: partial
      note: "Rust 报「@ 后需片段名」、JS 报「@ 后需名称」；符号与名字之间有空白的情形另有 E-PARSE-004" }
    { id: E-PARSE-012 domain: PARSE severity: E title: "语法错误（兜底）"
      msg: "语法错误：未能给出更具体的原因"
      impls: [ rust c lua ] status: partial
      note: "Rust 的通用兜底、C 的解析失败兜底、Lua 的解析阶段包裹都归此码。出现此码说明该处上下文不足，应当单独改进，而不是靠扩大范围掩盖" }
    { id: E-PARSE-013 domain: PARSE severity: E title: "键名位置不支持插值"
      msg: "键名位置不支持 `$` 插值：循环变量只可用于值位置"
      impls: [ rust ] status: partial
      note: "`@for` 的循环变量只作用于值位置；动态键请改用数组表达" }
    { id: E-PARSE-014 domain: PARSE severity: E title: "when 条件非法"
      msg: "`@when` 的条件非法：缺少条件、左侧不是环境变量，或环境变量名缺失"
      impls: [ rust ] status: partial
      note: "只支持环境变量形式的左侧；文档内字段引用暂不支持" }
    { id: E-PARSE-015 domain: PARSE severity: E title: "when 缺少比较值"
      msg: "`@when` 的比较运算符后缺少比较值"
      impls: [ rust ] status: partial
      note: "写成了运算符后直接换行；正确形式见规范的条件一节" }
    { id: E-PARSE-016 domain: PARSE severity: E title: "when 连续出现"
      msg: "`@when` 连续出现：它只作用于紧邻的下一个字段或块"
      impls: [ rust ] status: partial }
    { id: E-PARSE-017 domain: PARSE severity: E title: "when 后无字段或块"
      msg: "`@when` 后未跟随任何字段或块"
      impls: [ rust ] status: partial
      note: "悬空的 `@when`：它只作用于紧邻的下一个字段或块" }
    { id: E-PARSE-018 domain: PARSE severity: E title: "for 语法非法"
      msg: "`@for` 语法非法：缺少循环变量、缺少 `in`、缺少枚举项，或缺少循环体"
      impls: [ rust ] status: partial
      note: "`in` 之后至少须有一个枚举项" }
    { id: E-PARSE-019 domain: PARSE severity: E title: "指令头语法非法"
      msg: "指令头语法非法：`@contract` / `@is` / `@type` 后缺少契约名、类型名或模式体"
      impls: [ rust js lua ] status: partial
      note: "JS 侧对应「@contract 后须契约体」「@type 后须类型名」「@is 后须契约名」等文案" }
    { id: E-PARSE-020 domain: PARSE severity: E title: "片段参数语法非法"
      msg: "片段参数非法：`type` 或 `name` 参数后缺少取值，或同一参数重复"
      impls: [ rust c ] status: partial
      note: "C 侧是 W16 补片段**显式参数**（`type: X` / `name: Y`）时一并接线的：此前 C 只认位置参数形式，显式形式既不被识别、也不报错（`@f type: Server { ... }` 会被静默丢掉整条定义）" }
    { id: E-PARSE-021 domain: PARSE severity: E title: "default 缺取值"
      msg: "`default` 修饰符后缺少取值"
      impls: [ rust cpp js lua ] status: partial
      note: "JS 侧报「期望字面量」（与 E-PARSE-022 共用同一处检查）；C++ 在 W10 期间补进 impls" }
    { id: E-PARSE-022 domain: PARSE severity: E title: "数值边界取值非数字"
      msg: "`min` 或 `max` 的边界取值不是数字"
      impls: [ rust c cpp js lua ] status: partial
      note: "取值非有限数的情形见 E-CONTRACT-010。C 与 C++ 在 W10 期间补齐；此前 C 更糟：`max abc` 把非法边界当 0，报的是 **E-CONTRACT-005 这个错码**（把合法值判成越界）" }
    { id: E-PARSE-023 domain: PARSE severity: E title: "enum 后不是数组"
      msg: "`enum` 后须为数组"
      impls: [ rust cpp lua ] status: partial
      note: "C++ 报「enum needs ...」；取值不在列表内是另一条码（E-CONTRACT-006）" }
    { id: E-PARSE-024 domain: PARSE severity: E title: "契约定义内字段规格语法非法"
      msg: "契约定义里的字段规格语法非法：类型为空、数组或枚举缺少闭合、字段名为空"
      impls: [ cpp ] status: partial
      note: "C++ 把契约定义解析与取值校验放在同一层，故这些语法错误目前只在该端出现" }

    # ================= 深度与预算上限（语言层） =================
    { id: E-LIMIT-001 domain: LIMIT severity: E title: "嵌套过深"
      msg: "嵌套过深，超过本实现的上限层数，疑似递归或恶意输入"
      impls: [ rust js c cpp lua smltools ] status: partial
      note: "上限 128 层。**口径已实测统一**（用闭合嵌套逐格扫过五端，块与数组两条入口都扫）：文档根不计层 ⇒ **128 层放行、第 129 层报此码**。改前是分裂的：块嵌套 Rust/C 只放行 127（守卫用 >=，而文案写「**超过** 128 层」，自相矛盾），C++ 的**数组**入口更是白送一层（`key: [ ]` 直接调 parse_array、绕过 depth 计数 ⇒ 129 层才报）—— W15 一并收敛：Rust/C 的守卫改 >，C++ 补了带守卫的 parse_array_nested。XML 迁入超限同报此码。**C 与 Lua 的 parse_array 不递归嵌套数组**，这两端没有可限的数组深度入口，那属数据正确性（C 见 W17，Lua 见 W20）" }
    { id: E-LIMIT-002 domain: LIMIT severity: E title: "模式匹配超步数预算"
      msg: "模式匹配超出步数预算，疑似病态规则或超长输入"
      impls: [ rust ] status: partial
      note: "JS 侧只有「源码长度」与「待校验值长度」两道闸，**没有步数预算**，病态正则仍可占满主线程" }
    { id: E-LIMIT-003 domain: LIMIT severity: E title: "include 展开次数超限"
      msg: "include 展开次数超过上限，疑似指数膨胀"
      impls: [ rust c cpp lua ] status: partial
      note: "三端上限数值都是 10000（差异用字段表达，不靠文案）；嵌套层数的上限另见 E-INCLUDE-004。此闸挡的是**菱形包含的 2^N 膨胀**，深度上限挡不住；C++ 原先完全没有这个闸（W18 一并补上，含 2^20 次读取的用例）；Lua 同为 10000（**全局**计数，菱形 2^20 用例已入套件）" }
    { id: E-LIMIT-004 domain: LIMIT severity: E title: "输出递归深度超过上限"
      msg: "递归深度超过上限（翻译后端）"
      impls: [ rust ] status: partial
      note: "各输出后端（markdown、xml、svg、slint、latex、html、custom、lvgl）共用此码；JS 的序列化无深度闸" }
    { id: E-LIMIT-005 domain: LIMIT severity: E title: "custom 输出长度超上限"
      msg: "custom 生成器输出超过长度上限（模板存在放大）"
      impls: [rust smltools ] status: done
      note: "提示检查被重复引用的嵌套片段；属工具层预算" }
    { id: E-LIMIT-006 domain: LIMIT severity: E title: "custom 数组循环次数超上限"
      msg: "custom 生成器数组超过循环上限"
      impls: [rust smltools ] status: done }
    { id: E-LIMIT-007 domain: LIMIT severity: E title: "模式源码长度超上限"
      msg: "模式（正则）源码超过长度上限，拒绝编译"
      impls: [ js ] status: partial
      note: "Rust 侧同样有长度闸，但当前是**静默不匹配**（不报错），见 README 清点的静默清单" }
    { id: E-LIMIT-008 domain: LIMIT severity: E title: "待校验值长度超上限"
      msg: "待校验字符串超过长度上限，拒绝校验"
      impls: [ rust js ] status: done
      note: "拒绝校验而非判为不匹配：避免超长输入把模式引擎拖死" }
    { id: E-LIMIT-009 domain: LIMIT severity: E title: "量词取值超上限"
      msg: "量词的重复次数超过上限"
      impls: [ js ] status: done
      note: "与「量词本身非法」（E-CONTRACT-015）区分：此处是数值过大，属预算问题" }
    { id: E-LIMIT-010 domain: LIMIT severity: E title: "内存分配失败"
      msg: "内存分配失败"
      impls: [ c cpp ] status: partial
      note: "纯 C / C++ 实现要自己管内存，malloc 失败是可报的错误条件（Rust 遇到 OOM 直接 abort，JS 由宿主抛 RangeError，都没有这个码）。缓冲不够与分配失败要分开：前者是调用方的错（E-IO-*），后者是资源耗尽" }

    # ================= 特性、版本与环境变量（语言层） =================
    { id: E-FEATURE-001 domain: FEATURE severity: E title: "特性未启用"
      msg: "该语法需要相应特性，请先启用该特性"
      impls: [ rust lua ] status: partial
      note: "Rust 覆盖 include/import、multi-include、glob-include、regex-include、namespace、contract、`@is`、`@when`、`@for`、fragment、top-level-array 等；**JS 只对 include 做了门控**，其余静默放行；用字段表达是哪个特性，码共用；**Lua 对 include 的高级写法（`as ns` 命名空间 / glob / regex / 多目标 / 部分引用）显式报此码**（W20 第二阶段：这些不做，但**绝不静默**）" }
    { id: E-FEATURE-002 domain: FEATURE severity: E title: "环境变量被禁用"
      msg: "当前特性集禁用了环境变量内联，裸词或字符串无法解析"
      impls: [ rust ] status: partial
      note: "`@when` 的条件用环境变量时同报此码；JS 无条件内插，无门控" }
    { id: E-FEATURE-003 domain: FEATURE severity: E title: "未知特性名"
      msg: "未知特性名"
      impls: [ rust ] status: partial
      note: "规范文案会附带可用特性列表；JS 把未知特性静默加入集合" }
    { id: E-FEATURE-004 domain: FEATURE severity: E title: "版本声明非法"
      msg: "不支持的版本声明，或版本声明互相冲突"
      impls: [ rust c cpp js lua smltools ] status: partial
      note: "各端支持的版本范围不同（Lua 仅 v1、C 到 v3）—— **码相同、支持的版本不同**，差异用字段表达，不再靠文案。C 的「超出接受范围」分支目前是死代码；`@version` 用作片段名、`@feature base` 参数非 v1..v4、命令行传入非法版本名都归此码" }
    { id: E-FEATURE-005 domain: FEATURE severity: E title: "裸词必须加引号"
      msg: "字符串必须加引号：当前特性集禁用了裸词字符串"
      impls: [ rust c ] status: partial
      note: "JS 无该检查" }
    { id: E-FEATURE-006 domain: FEATURE severity: E title: "feature 子命令未知"
      msg: "未知的 `@feature` 子命令"
      impls: [ rust ] status: done
      note: "可用子命令见文案附带的列表" }
    { id: E-FEATURE-007 domain: FEATURE severity: E title: "feature 指令缺少参数"
      msg: "`@feature` 指令缺少参数"
      impls: [ rust ] status: done }
    { id: E-FEATURE-008 domain: FEATURE severity: E title: "feature mode 参数非法"
      msg: "`@feature mode` 的参数须为白名单或黑名单之一"
      impls: [ rust ] status: done }
    { id: E-FEATURE-009 domain: FEATURE severity: E title: "请求特性与允许集无交集"
      msg: "文档请求的特性与调用方允许的特性没有交集"
      impls: [ rust ] status: done
      note: "宿主（如 C-ABI 调用方）用 allow 集收窄能力时的拒绝：不静默降级，直接失败" }
    { id: W-FEATURE-001 domain: FEATURE severity: W title: "位置参数写法已废弃"
      msg: "位置参数写法已废弃，推荐带参数名的写法"
      impls: [ rust js ] status: partial
      note: "告警而非错误：兼容既有文档，同时提示迁移。注意 Rust 的常规入口会丢弃诊断，只有走带诊断的入口才拿得到 —— 见 README 清点的静默清单" }
    { id: I-FEATURE-001 domain: FEATURE severity: I title: "命令行特性与解析器版本不一致"
      msg: "解析器按其固定版本模式工作，命令行声明的特性版本仅作提示"
      impls: [ smltools ] status: done }

    # ================= 契约（语言层） =================
    { id: E-CONTRACT-001 domain: CONTRACT severity: E title: "未定义的契约"
      msg: "引用了未定义的契约"
      impls: [ rust c cpp js lua ] status: partial
      note: "含字段引用了未定义契约的嵌套情形；C++ 报「unknown contract」" }
    { id: E-CONTRACT-002 domain: CONTRACT severity: E title: "字段类型不符"
      msg: "字段类型应为期望类型，实际为其它类型"
      impls: [ rust c cpp js lua ] status: partial
      note: "JS 报「类型错误：期望某类型，实得某类型」；C++ 按期望类型分别报" }
    { id: E-CONTRACT-003 domain: CONTRACT severity: E title: "必填字段缺失"
      msg: "字段必填但缺失"
      impls: [ rust c cpp js lua ] status: partial }
    { id: E-CONTRACT-004 domain: CONTRACT severity: E title: "未声明字段（严格模式）"
      msg: "字段未在契约中声明；确需放宽请在契约名后写 loose"
      impls: [ rust c cpp js lua ] status: partial }
    { id: E-CONTRACT-005 domain: CONTRACT severity: E title: "数值越界"
      msg: "字段值小于下界或大于上界"
      impls: [ rust c cpp js lua ] status: partial
      note: "取值非有限数的情形是 E-CONTRACT-010" }
    { id: E-CONTRACT-006 domain: CONTRACT severity: E title: "枚举取值非法"
      msg: "取值不在枚举列表内"
      impls: [ rust c cpp js lua ] status: partial
      note: "C 侧注释里专门论证了「必须与 E-CONTRACT-002 区分」；W10 期间核实 C 确实在报，故补进 impls" }
    { id: E-CONTRACT-007 domain: CONTRACT severity: E title: "外置类型校验失败"
      msg: "字段不符合扩展类型的要求"
      impls: [ rust js ] status: partial
      note: "失败原因由注册方提供，随消息一并返回" }
    { id: E-CONTRACT-008 domain: CONTRACT severity: E title: "组合字段应为块"
      msg: "字段应为块并按该契约校验，实际不是块"
      impls: [ rust c cpp js lua ] status: partial
      note: "C++ 报「contract applied to non-object」；C 侧 W10 期间核实确实在报，补进 impls" }
    { id: E-CONTRACT-009 domain: CONTRACT severity: E title: "自定义类型格式不符"
      msg: "字段的值不符合该类型的格式要求"
      impls: [ rust js ] status: partial
      note: "含要求字符串却给了数字（号码、编号、身份证需引号）与值过长拒绝校验；模式编译或匹配失败也归此码" }
    { id: E-CONTRACT-010 domain: CONTRACT severity: E title: "数值约束取值为非有限数"
      msg: "字段的值为非有限数，不能作为数值约束的取值"
      impls: [ rust c cpp lua ] status: partial
      note: "JS 侧未见对应检查。C 与 C++ 在 W10 期间补齐：min/max 边界取到 nan/1e400 时同报此码（此前 C 静默、C++ 把边界吞掉）" }
    { id: E-CONTRACT-011 domain: CONTRACT severity: E title: "外置修饰符校验失败"
      msg: "字段不符合扩展修饰符的要求"
      impls: [ rust ] status: partial
      note: "与 E-CONTRACT-007（外置**类型**）区分：修饰符与类型是两套注册点" }
    { id: E-CONTRACT-012 domain: CONTRACT severity: E title: "未知数组元素类型"
      msg: "数组元素类型名未知，且不是已注册的扩展类型"
      impls: [ rust lua ] status: partial }
    { id: E-CONTRACT-013 domain: CONTRACT severity: E title: "模式定义非法"
      msg: "模式定义非法：未知字符类、未知量词、不支持的元素，或无法识别的元素"
      impls: [ rust js ] status: partial
      note: "JS 逐项报「未知字符类」「未知量词」「无法识别的模式元素」，并明确不支持懒惰量词；Rust 报「模式编译失败」" }
    { id: E-CONTRACT-014 domain: CONTRACT severity: E title: "模式规则引用非法"
      msg: "模式规则引用非法：引用了未定义的规则，或规则循环引用"
      impls: [ js ] status: partial
      note: "Rust 侧同类问题由模式引擎在编译期归入 E-CONTRACT-013" }
    { id: E-CONTRACT-015 domain: CONTRACT severity: E title: "量词取值非法"
      msg: "量词的取值非法：不是整数、为负数，或上界小于下界"
      impls: [ js ] status: partial
      note: "数值过大属预算问题，见 E-LIMIT-009" }

    # ================= 片段与 include（语言层） =================
    { id: E-INCLUDE-001 domain: INCLUDE severity: E title: "include 文件缺失或读取失败"
      msg: "include 无法定位或读取目标文件"
      impls: [ rust c cpp js lua ] status: partial
      note: "C 侧「路径无法规范化解析」同报此码。⚠️ 本条曾把 lua 从 impls **移除**（当时 Lua 根本没有 include 语法，`include \"x\"` 与 `@include \"x\"` 都被静默当普通键）；**W20 第二阶段补上 include 后 lua 已回到 impls**；此前写的「Lua 的宿主入口报文件不存在」是**归类错误** —— 宿主入口读的是文档本身，归 E-IO-001（与 C 的 sml_parse_file 同格）" }
    { id: E-INCLUDE-002 domain: INCLUDE severity: E title: "include 循环引用"
      msg: "include 循环引用"
      impls: [ rust c cpp lua ] status: partial
      note: "JS 无环检测，自包含会耗尽调用栈（抛宿主 RangeError），不是此码；C++ 原先声明了此端却**不可能触发**（环检测的栈 push 完立刻 pop、永远为空），W18 已修好并给出反向用例（菱形包含必须合法）；Lua 由 W20 第二阶段实现，同用**链栈**（只判「根→当前」），故菱形包含同样合法、自包含/互包含报此码" }
    { id: E-INCLUDE-003 domain: INCLUDE severity: E title: "include 越界拒绝"
      msg: "include 目标不在基准目录内，已拒绝"
      impls: [ rust c cpp lua ] status: partial
      note: "安全边界：阻止 include 逃出工程目录；JS 用虚拟文件表，无基准目录概念；Lua 按**路径分量**比前缀（不是字符串前缀），越界即拒绝（W20 第二阶段）" }
    { id: E-INCLUDE-004 domain: INCLUDE severity: E title: "include 嵌套超过上限"
      msg: "include 嵌套超过上限层数"
      impls: [ rust c cpp lua ] status: partial
      note: "三端上限数值都是 32（Rust MAX_INCLUDE_DEPTH / C MAX_INC_DEPTH / C++ SML_MAX_INCLUDE_DEPTH）；smltools 自带的 include 展开上限也归此码（数值不同）—— 差异用字段表达；C++ 原先既无此码也无上限：超深包含是**静默跳过**（字段凭空消失），W18 一并修好；Lua 也取 32，且**文档根不计层**（31 层放行 / 32 层报，边界本身有用例钉住）" }
    { id: E-INCLUDE-005 domain: INCLUDE severity: E title: "键列表语法非法"
      msg: "键列表语法非法：期望键列表、或键列表为空、或缺少闭合"
      impls: [ rust js ] status: partial
      note: "**W14 已修**：JS 在空键列表这条分支上原抛宿主 ReferenceError（报告函数不在其作用域内），走 parseSafe 更被静默吞成 ok=false 且无码；现两路都给本码。⚠️ 两端**入口不同**：JS 在解析器内部处理 include（`include \"x.sml\" as w { }` 走全量 parse），Rust 在 sml-include 的指令解析里（要直接调 parse_include_line）—— 故 probe 与 rust/tests/error_codes.rs 各有一条同条件用例，见后者的 include_key_list_codes" }
    { id: E-INCLUDE-006 domain: INCLUDE severity: E title: "未定义的片段引用"
      msg: "未定义的片段引用"
      impls: [ rust c ] status: partial
      note: "含命名空间逐级回退后仍未命中的情形。C 改前把未命中的引用 `return sml_new_str(w)` 静默退化成字符串（下游取值取不到、还查不出原因），W16 起报此码；用户已裁决这类必须报错（「这种不应该出现，堪比 void」）。JS 仍把未定义引用静默当普通键处理（W16 余额，未做）" }
    { id: E-INCLUDE-007 domain: INCLUDE severity: E title: "片段展开结果不是对象"
      msg: "片段展开结果不是对象，无法与所在块合并"
      impls: [ rust ] status: done
      note: "块内引用片段时期望得到对象；得到标量或数组即报此码" }
    { id: E-INCLUDE-008 domain: INCLUDE severity: E title: "部分引用缺少目标文件"
      msg: "部分引用的写法必须接上目标文件"
      impls: [ rust ] status: done }
    { id: E-INCLUDE-009 domain: INCLUDE severity: E title: "部分引用不能配通配"
      msg: "部分引用不能配合 glob 或 regex 通配，请指定单个文件"
      impls: [ rust ] status: done
      note: "部分引用只取键，通配会命中多个文件 —— 组合语义未定义，故直接拒绝" }
    { id: E-INCLUDE-010 domain: INCLUDE severity: E title: "基准目录不可解析"
      msg: "include 基准目录不可解析，无法做越界校验，已拒绝继续"
      impls: [ rust c cpp lua ] status: done
      note: "fail-closed：宁可拒绝也不放行；C 的文案是「已拒绝」。C++ 原先用 weakly_canonical（只做词法规范化，不存在的目录也会\"成功\"规范化），于是这一格被降级成「目录里没这个文件」而报出 E-INCLUDE-001（错码）—— W18 改用严格 canonical；Lua 同理 fail-closed（`base` 为空串亦算不可解析 → 报此码）" }
    { id: E-INCLUDE-011 domain: INCLUDE severity: E title: "include 预处理词法失败"
      msg: "include 预处理阶段的词法失败"
      impls: [ rust cpp ] status: done
      note: "与文档正文的词法错误（E-LEX-*）区分：此处指 include 行在展开前的词法阶段就失败。C++ 原先**丢弃**了子文件的词法错误、把残缺 token 段插进去（未闭合字符串会变成静默截断的文档），W18 改为报此码。⚠️ **Lua 不在 impls，是设计差异不是漏做**：Lua 把 include 展开成文本后**整体词法**一次，子文件里的未闭合字符串/注释走正文的**静默清单**（已实测：子文件与正文同一表现，都静默），故不单独报此码" }

    { id: E-INCLUDE-012 domain: INCLUDE severity: E title: "include 路径写法非法"
      msg: "include 路径写法非法（未加引号或含非法字符）"
      impls: [ lua smltools ] status: done
      note: "smltools 的 include 展开要求路径加引号；未加引号时原先**暂归 E-INCLUDE-001** —— 那是**已知错码**（用户拿 E-INCLUDE-001 去查会看到「文件缺失或读取失败」，被误导），W21 立此码归位。⚠️ 语言层（`sml-include::parse_include_line`）对「未加引号」的判定归它自己的实现，**Lua 也已落地**（W20 第二阶段：未加引号 / 引号未闭合 / 多余字符都报此码，`include` 与 `@include` 两种写法一致）" }

    # ================= 扩展点（语言层） =================
    { id: E-EXT-001 domain: EXT severity: E title: "未注册的指令、类型或修饰符"
      msg: "该指令、类型或修饰符未在本环境注册"
      impls: [ rust js ] status: partial
      note: "扩展机制的固有代价：带方言的文档在未注册的环境里读不了 —— 这条码是有意为之，不是缺陷。JS 在未注册指令**带**片段体时会静默当作片段定义" }
    { id: E-EXT-002 domain: EXT severity: E title: "扩展注册名冲突"
      msg: "不可注册与内置同名的扩展；同名重复注册同样报错"
      impls: [ rust ] status: done
      note: "三条注册点（指令、类型、修饰符）共用此码" }
    { id: E-EXT-003 domain: EXT severity: E title: "外置指令缺少取值"
      msg: "外置指令的参数名后缺少取值"
      impls: [ rust js ] status: done }
    { id: E-EXT-004 domain: EXT severity: E title: "外置指令返回非对象"
      msg: "外置指令的合并结果须为对象"
      impls: [ rust js ] status: done
      note: "该返回值用于合并进所在块，非对象无法合并" }
    { id: E-EXT-005 domain: EXT severity: E title: "外置修饰符缺少取值"
      msg: "外置修饰符期望取值"
      impls: [ rust ] status: done }
    { id: E-EXT-006 domain: EXT severity: E title: "custom 规则文档非法"
      msg: "自定义生成规则文档非法：缺少 rules、rules 为空，或某条规则缺少模板"
      impls: [rust smltools ] status: done }
    { id: E-EXT-007 domain: EXT severity: E title: "编辑器定制文档非法"
      msg: "编辑器定制文档非法：作用域名、规则锚点、颜色取值不符规范"
      impls: [ smltools ] status: done
      note: "覆盖 `--to highlight` 与 `--to tmlanguage` 的输入校验；规则里同时给出同侧两个锚点也算非法（位置无法确定）" }
    { id: E-EXT-008 domain: EXT severity: E title: "外置指令执行失败"
      msg: "外置指令处理该输入时失败"
      impls: [ rust ] status: done
      note: "失败原因由注册方（`Directive::call` 的返回值）给出，随消息一并返回；与「未注册」（E-EXT-001）、「取值为空」（E-EXT-003）、「返回非对象」（E-EXT-004）区分" }

    # ================= 输入输出（宿主绑定层） =================
    { id: E-IO-001 domain: IO severity: E title: "读取失败"
      msg: "读取文件失败"
      impls: [ rust c lua ] status: partial
      note: "Rust 侧同时覆盖文档入口与 include 展开两条读文件路径；Lua 侧是宿主入口 `lua/main.lua` 读不到输入文件（与 C 的 sml_parse_file fopen 失败同格）" }
    { id: E-IO-002 domain: IO severity: E title: "输入为空"
      msg: "输入为空"
      impls: [ c ] status: partial
      note: "⚠️ **声明与实现不符**（W10 期间实测）：本条 impls 只写了 c，但 C 对空输入 / 空文件 / 仅空白文件**都返回空容器、不报错**，与其余端一致 —— 也就是说目前**没有任何实现报这个码**。「空输入算不算错」需要五端统一口径，属 W16 的判定对象，W10 未动" }
    { id: E-IO-003 domain: IO severity: E title: "写入或建目录失败"
      msg: "写入文件或创建目录失败"
      impls: [ smltools ] status: done }
    { id: E-IO-004 domain: IO severity: E title: "读取目录失败"
      msg: "读取目录失败"
      impls: [ smltools ] status: done
      note: "目录批量模式的入口；与单个文件的读取失败（E-IO-001）区分" }
    { id: E-IO-005 domain: IO severity: E title: "未检测到输入"
      msg: "未检测到输入：既没有指定输入文件，也没有可读的管道输入"
      impls: [ smltools ] status: done
      note: "用法提示而非读文件失败：标准输入是终端且未给输入文件时给出，附完整用法入口" }
    { id: E-IO-006 domain: IO severity: E title: "标准输入读取失败"
      msg: "读取标准输入失败或通道中断"
      impls: [ smltools ] status: done }
    { id: E-IO-007 domain: IO severity: E title: "外部工具不可用或失败"
      msg: "外部工具不可用（未安装或无法启动），或其构建失败"
      impls: [ smltools ] status: done
      note: "当前用于站点构建器调用；错误里带退出码，便于区分「没装」与「跑了但失败」" }

    # ================= 内部错误（宿主绑定层） =================
    { id: E-INTERNAL-001 domain: INTERNAL severity: E title: "内部错误"
      msg: "内部错误：走到了不应到达的分支"
      impls: [ rust c lua smltools ] status: partial
      note: "Rust 的不可达断言、C-ABI 与 C 的空指针入参、Lua 的入参类型检查、smltools 的不可达分支都归此码。出现即 bug，请带最小复现报 issue；对外文案统一，细节只进日志与诊断。**C++ 不在其中**（W10 期间核实：它的 API 用 std::string* 而非裸缓冲，没有空指针入参面）" }
    { id: E-INTERNAL-002 domain: INTERNAL severity: E title: "内置资源损坏"
      msg: "内置资源损坏（打包或构建事故）"
      impls: [ smltools ] status: done
      note: "例如内置高亮基线不是合法 JSON、缺少必需字段。用户无解，只能升级或重装" }

    # ================= Rust 派生宏与 serde 桥（宿主绑定层） =================
    { id: E-DERIVE-001 domain: DERIVE severity: E title: "值与目标类型不符"
      msg: "SML 值的类型与目标宿主类型不符"
      impls: [ rust ] status: done
      note: "涵盖派生宏与 serde 桥两侧的「期望某类型，实际为某类型」" }
    { id: E-DERIVE-002 domain: DERIVE severity: E title: "数值超出目标类型范围"
      msg: "数值超出目标宿主类型的取值范围"
      impls: [ rust ] status: done
      note: "含无符号类型收到负数、整型收到小数" }
    { id: E-DERIVE-003 domain: DERIVE severity: E title: "未知的枚举值或变体"
      msg: "未知的枚举值或枚举变体"
      impls: [ rust ] status: done }
    { id: E-DERIVE-004 domain: DERIVE severity: E title: "枚举变体形态不符"
      msg: "枚举变体的形态与目标定义不符"
      impls: [ rust ] status: done
      note: "含类型标记不是字符串、无法判断变体形态、变体携带数据与形态不匹配，以及内部结构约定字段缺失" }
    { id: E-DERIVE-005 domain: DERIVE severity: E title: "序列化时键不是字符串"
      msg: "序列化时对象的键必须是字符串"
      impls: [ rust ] status: done }
    { id: E-DERIVE-006 domain: DERIVE severity: E title: "宿主特性未启用"
      msg: "该能力需要启用对应的宿主构建特性"
      impls: [ rust ] status: done
      note: "构建配置问题，不是文档问题；属宿主绑定层的对外提示" }
    { id: E-DERIVE-007 domain: DERIVE severity: E title: "派生属性或类型形状非法"
      msg: "派生宏的属性或类型形状非法（编译期报错）"
      impls: [ rust ] status: done
      note: "含未知属性、容器级只允许重命名策略、联合类型不支持。用户看到的是编译错误，此码用于文档化与检索" }
    { id: E-DERIVE-008 domain: DERIVE severity: E title: "派生入口的解析失败"
      msg: "派生反序列化时 SML 解析失败（内层原因见原始错误）"
      impls: [ rust ] status: done
      note: "外壳只标记「经由派生宏入口」，内层应是 E-LEX-* 或 E-PARSE-*；两者都要给出" }

    # ================= 迁入格式（工具层） =================
    { id: E-MIGRATE-001 domain: MIGRATE severity: E title: "顶层出现文本内容"
      msg: "迁入文档在顶层出现了文本内容（只允许空白）"
      impls: [ smltools ] status: done }
    { id: E-MIGRATE-002 domain: MIGRATE severity: E title: "标签未闭合"
      msg: "标签未闭合：文件在标签内提前结束"
      impls: [ smltools ] status: done
      note: "含属性区结束、标签名后立即结束两种形态" }
    { id: E-MIGRATE-003 domain: MIGRATE severity: E title: "标签名为空"
      msg: "标签名为空"
      impls: [ smltools ] status: done }
    { id: E-MIGRATE-004 domain: MIGRATE severity: E title: "结束标签不匹配"
      msg: "结束标签与开始标签不匹配，或结束标签缺少闭合符号"
      impls: [ smltools ] status: done }
    { id: E-MIGRATE-005 domain: MIGRATE severity: E title: "多余的结束标签"
      msg: "多余的结束标签：没有与之匹配的开始标签"
      impls: [ smltools ] status: done }
    { id: E-MIGRATE-006 domain: MIGRATE severity: E title: "属性区语法非法"
      msg: "属性区语法非法：出现非法字符、缺少等号、取值未加引号或引号未闭合"
      impls: [ smltools ] status: done
      note: "含取值里出现尖括号，以及自闭合写成单个斜杠的情形" }
    { id: E-MIGRATE-007 domain: MIGRATE severity: E title: "注释或声明段未闭合"
      msg: "注释、处理指令、文档类型声明或 CDATA 段未闭合"
      impls: [ smltools ] status: done
      note: "四类段共用此码：它们都是「开到文件结尾还没关」" }
    { id: E-MIGRATE-008 domain: MIGRATE severity: E title: "不支持的声明或处理指令"
      msg: "此处不支持该声明或处理指令"
      impls: [ smltools ] status: done
      note: "只认注释与文档类型声明；其余声明、以及元素内容里的处理指令都拒绝，不静默丢弃" }
    { id: E-MIGRATE-009 domain: MIGRATE severity: E title: "实体未闭合或未知"
      msg: "实体未闭合，或使用了未支持的实体名"
      impls: [ smltools ] status: done
      note: "只认五个具名实体与数字实体；其余报错而不猜。探测窗口按**字符**取，避免切在多字节字符中间" }
    { id: E-MIGRATE-010 domain: MIGRATE severity: E title: "实体数字非法"
      msg: "实体的数字部分非法（非十六进制，或不是合法码点）"
      impls: [ smltools ] status: done
      note: "含代理区码点" }
    { id: E-MIGRATE-011 domain: MIGRATE severity: E title: "TOML 语法非法"
      msg: "TOML 语法非法：不是键值形式、键名为空、缺少取值，或内联表里缺少等号"
      impls: [ smltools ] status: done }
    { id: E-MIGRATE-012 domain: MIGRATE severity: E title: "TOML 表定义冲突"
      msg: "TOML 表定义冲突：同名已存在且不是表，或不是表数组"
      impls: [ smltools ] status: done
      note: "与语法错误分开：这类是「语法没问题，但语义上不能这样重复定义」" }
    { id: E-MIGRATE-013 domain: MIGRATE severity: E title: "YAML 结构非法"
      msg: "YAML 结构非法：出现意外内容、不是键值形式、缩进过深，或流式结构后有多余字符"
      impls: [ smltools ] status: partial
      note: "未知转义序列目前**宽松保留**（原样写入反斜杠加字符）而不报错 —— 与 YAML 规范不一致，属已知取舍" }
    { id: E-MIGRATE-014 domain: MIGRATE severity: E title: "YAML 流式结构未闭合"
      msg: "YAML 流式结构未闭合"
      impls: [ smltools ] status: done }
    { id: E-MIGRATE-015 domain: MIGRATE severity: E title: "YAML 别名未定义"
      msg: "YAML 别名引用了未定义的锚点"
      impls: [ smltools ] status: done }
    { id: E-MIGRATE-016 domain: MIGRATE severity: E title: "YAML 非法 UTF-8"
      msg: "YAML 流式结构里出现非法 UTF-8"
      impls: [ smltools ] status: done }
    { id: E-MIGRATE-017 domain: MIGRATE severity: E title: "不是合法 JSON"
      msg: "迁入文本不是合法 JSON（或嵌套过深）"
      impls: [ smltools ] status: partial
      note: "底层对「非法」与「过深」返回同一个空值，故合为一码；嵌套过深另见 E-LIMIT-001" }

    # ================= 命令行与工具用法（工具层） =================
    { id: E-CLI-001 domain: CLI severity: E title: "输入格式未知"
      msg: "输入格式取值未知"
      impls: [ smltools ] status: done
      note: "显式指定时校验；按扩展名推断不会走到这里" }
    { id: E-CLI-002 domain: CLI severity: E title: "输出格式未知"
      msg: "输出格式取值未知"
      impls: [ smltools ] status: done
      note: "文案附带全部可用取值" }
    { id: E-CLI-003 domain: CLI severity: E title: "参数组合非法"
      msg: "命令行参数组合非法：互斥项同时给出，或依赖项缺失"
      impls: [ smltools ] status: done }
    { id: E-CLI-004 domain: CLI severity: E title: "lint 只适用于 SML 文档"
      msg: "lint 只能检查 SML 文档"
      impls: [ smltools ] status: done }
    { id: E-CLI-005 domain: CLI severity: E title: "目录模式必须指定输出目录"
      msg: "目录批量模式必须指定输出目录"
      impls: [ smltools ] status: done
      note: "缺省输出会污染源目录，故直接拒绝而不是猜" }
    { id: E-CLI-006 domain: CLI severity: E title: "规则文档解析或构建失败"
      msg: "自定义生成规则文档解析或构建失败"
      impls: [ smltools ] status: done
      note: "规则文档本身是 SML，解析失败的内层原因是 E-LEX-* 或 E-PARSE-*" }
    { id: E-CLI-007 domain: CLI severity: E title: "输出后端报错"
      msg: "输出后端报错（内层原因见原始错误）"
      impls: [ rust smltools ] status: done
      note: "**W21 已根治**：码现在由 `swsml` 的 emit 后端**自己带**（`rust/src/emit/mod.rs` 的 `backend_error()`），不再是 smltools 按**文案前缀**猜出来的 —— 那套猜法有个隐蔽的静默失效：它的单测喂的是**自己手写的字面量**，上游一改文案，测试照样绿、码静默退化成更粗的东西。⚠️ **候选拆分**（本轮不拆：拆码=改对外契约，且这些条件彼此异质，该先想清按什么维度拆）：markdown 的 HTML 透传拒绝危险标签、latex 的危险控制序列、latex 任务勾选不支持、md/latex 的 table 缺 header、slint 的 property 声明缺合法类型" }

    { id: E-CLI-008 domain: CLI severity: E title: "命令行用法错误"
      msg: "命令行用法错误：未知参数、缺少取值或取值非法"
      impls: [ smltools ] status: done
      note: "clap 自身的用法错误（未知参数 / 缺取值 / 非法枚举）原先由 clap 直接打印并 exit 2、**不带码**；W21 立此码，让 smltools 接管这层输出（在其消息上补码，不重写 clap 的用法提示）。与 E-CLI-001..007 的区别：那七条是**我们自己**的校验，本条是**解析器/框架**报的用法错误" }

    # ================= lint 诊断（工具层） =================
    { id: E-LINT-001 domain: LINT severity: E title: "缩进里出现制表符"
      msg: "缩进里出现制表符：SML 缩进敏感，请统一用空格"
      impls: [ smltools ] status: done
      note: "制表符宽度因编辑器而异，会让缩进敏感的文档在不同环境里结构不同 —— 故按错误处理" }
    { id: W-LINT-001 domain: LINT severity: W title: "片段定义从未被引用"
      msg: "片段定义后从未被引用"
      impls: [ smltools ] status: done }
    { id: W-LINT-002 domain: LINT severity: W title: "契约定义从未被应用"
      msg: "契约定义后从未被应用"
      impls: [ smltools ] status: done }
    { id: W-LINT-003 domain: LINT severity: W title: "同一块内键名重复"
      msg: "键在同一块内重复，后者会覆盖前者"
      impls: [ smltools ] status: done
      note: "静默数据丢失的常见来源；文案会指出上一次出现的行号" }
    { id: W-LINT-004 domain: LINT severity: W title: "字段是空值"
      msg: "字段是空值"
      impls: [ smltools ] status: done }
    { id: W-LINT-005 domain: LINT severity: W title: "嵌套超过建议阈值"
      msg: "嵌套深度已超过建议阈值"
      impls: [ smltools ] status: done
      note: "建议阈值低于解析上限（E-LIMIT-001）：这条只提醒可读性，不是拒绝" }

    # ================= 编辑器扩展宿主层 =================
    { id: E-EDITOR-001 domain: EDITOR severity: E title: "解析器加载失败"
      msg: "编辑器侧的解析器加载失败，补全与诊断不可用"
      impls: [ vscode lsp ] status: done
      note: "宿主桥接层的问题，与文档内容无关" }
    { id: E-EDITOR-002 domain: EDITOR severity: E title: "高亮配置加载失败"
      msg: "高亮配置加载或解析失败"
      impls: [ vscode ] status: done
      note: "配置文件本身是 SML，解析失败的内层原因是 E-LEX-* 或 E-PARSE-*" }
    { id: E-EDITOR-003 domain: EDITOR severity: E title: "无法格式化"
      msg: "无法格式化：文档存在解析错误"
      impls: [ vscode ] status: done
      note: "内层原因是 E-PARSE-*；此码只表示「编辑器侧动作失败」，避免把它当成新的语言错误" }
    { id: E-EDITOR-004 domain: EDITOR severity: E title: "未打开工作区"
      msg: "请先打开一个工作区文件夹"
      impls: [ vscode ] status: done
      note: "需要写入工程内文件的操作（如生成高亮配置）缺少落点时给出" }
    { id: W-EDITOR-001 domain: EDITOR severity: W title: "自定义词与官方关键字冲突"
      msg: "自定义词与官方关键字冲突，已忽略"
      impls: [ vscode ] status: done
      note: "告警而非错误：冲突项被跳过，其余自定义词照常生效" }
]
