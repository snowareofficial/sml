@version v1
@feature enable type.featag, typed-block, block-name, when, for, type.weak
# SPDX-License-Identifier: MulanPSL-2.0
# =============================================================================
#  grammar.sml —— SML（SNOWARE Markup Language）形式化文法 · 自描述 · 全集
# =============================================================================
#  标准编号 : SOrg Tech STD 20262510.a
#  本文件   : 用 SML 写 SML 的文法（dogfooding）。它既是 EBNF 规范，也是一份
#             能被解析器直接读的 .sml 文档 —— 规范与工件是同一份东西。
#
#  全集声明 : 本文档刻意把**所有已实现的 SML 元素**写成活代码（含 featag、@type、
#             @when、@for、weak*、@feature 特性位），因此它**以 Rust 参照实现为准**：
#             其余四端会响亮拒绝自己不认识的特性位（JS: E-FEATURE-003、Lua: @feature
#             本身非法、C / C++ 未实现 featag / @type / weak*）。要被五端共读的核心
#             子集看 showcase.sml 与 showcase_contract.sml。
#
#  记法约定（本文件内部）：
#    · 产生式一律写成字符串，故串内的 { } [ ] : # @ & 都是普通字符，无需转义；
#    · ::= 定义，| 选一，{ X } 零或多次，[ X ] 零或一次，大写名是占位符不是终端；
#    · 串内不出现双引号与反斜杠，故本文件一个转义序列都没有；
#    · 环境变量前缀在产生式里写作「美元符 env . NAME」—— 因为真的写出来，它会被
#      解析器当成插值替换掉（这条限制本身也是文法的一部分，见 lexical 第 25 行）。
#
#  权威来源 : Rust 端（rust/sml-lex、sml-parse、sml-contract、sml-include）
#  五端状态 : 见 implementations 段；单文档无法自证的项（顶层数组 / include /
#             外置扩展点）在 conformance 段逐条列出理由。
#  验收命令 : cargo run -p smltools -- -i grammar.sml --to json
# =============================================================================


# -----------------------------------------------------------------------------
#  0. 契约与模式类型：本文件自己也遵守契约（解析期校验 + 默认值回填）
# -----------------------------------------------------------------------------
#  DocMeta 是严格的（默认 strict）：meta 块里多写一个未声明字段就报 E-CONTRACT-004。
@contract DocMeta {
    name: str
    standard: str
    ends: [ str ]
    status: enum [ stable draft deprecated ] default stable
    weight: num min 0 max 100 optional
}

# Note 是宽松的（声明处写 loose）：未声明字段放行，已声明字段照样校验。
@contract Note loose {
    text: str
}

# 组合而非继承：字段的类型就是另一个契约，可多层嵌套、递归回填默认值。
@contract Address {
    city: str
    country: str default CN
}

@contract Node {
    host: str
    address: Address
}

meta {
    @is DocMeta
    name: SML
    standard: "SOrg Tech STD 20262510.a"
    ends: [ rust c cpp js lua ]
}

about {
    @is Note
    text: 一份文法，两种用途
    author: snoware
    site: "https://sml.swebase.cn"
}

node {
    @is Node
    host: db1.internal
    address { city: Beijing }        # country 缺失 -> 回填 CN
}


# -----------------------------------------------------------------------------
#  1. 分层：L0 字符 → token；L1 token → 树；L2 指令；L3 契约；L4 扩展；L5 元数据
# -----------------------------------------------------------------------------
layers: [
    { id: L0 name: 词法 level: "字符序列 → token 流" }
    { id: L1 name: 结构 level: "token 流 → 数据树" }
    { id: L2 name: 指令 level: "解析期改写：include / 裁剪 / 展开 / 定义" }
    { id: L3 name: 契约 level: "结构约束与默认值回填（可选 schema 层）" }
    { id: L4 name: 扩展 level: "下游注册的指令 / 类型 / 修饰符" }
    { id: L5 name: 元数据 level: "__type / __name / __args / __featags" }
]


# -----------------------------------------------------------------------------
#  2. L0 词法
# -----------------------------------------------------------------------------
lexical {
    rules: [
        "document      ::= [ BOM ] { trivia } root { trivia } EOF"
        "trivia        ::= ws | comment"
        "ws            ::= SP | TAB | CR | LF        (* 换行不是语法，只是 token 定界 *)"
        "BOM           ::= U+FEFF                    (* 仅文件首一个，等长换成 3 个空格 *)"
        "comment       ::= line-comment | block-comment"
        "line-comment  ::= ( # | -- | // ) { ANY-EXCEPT-LF } [ LF ]"
        "block-comment ::= /* … */ | _* … *_          (* 不嵌套；未闭合 ⇒ E-LEX-002 / E-LEX-003 *)"
        "token         ::= { | } | [ | ] | , | : | AT | STRING | WORD    (* 只有这 9 类 *)"
        "AT            ::= @ 紧邻非空白字符            (* 词首且后接空白 ⇒ E-PARSE-004 *)"
        "STRING        ::= 双引号 { 字符 | ESCAPE } 双引号   (* 未闭合 ⇒ E-LEX-001 *)"
        "ESCAPE        ::= BACKSLASH ( n | t | r | 0 | 双引号 | BACKSLASH | u HEX{4} | u{ HEX+ } )"
        "              (* 上表之外一律 E-LEX-004：不静默、不猜测；单引号不是定界符 *)"
        "WORD          ::= word-char { word-char }"
        "word-char     ::= ANY-EXCEPT ( { } [ ] , : SP TAB CR LF 双引号 )"
        "word-char     ::= @  仅当不在词首              (* a@b.c 是一个 WORD，邮箱免引号 *)"
        "注释起始      ::= # | -- | // | /* | _*        (* 出现即结束 WORD；引号内不算 *)"
        "括号          ::= ( ) 既不是 token 也不是分隔符：a: (x) 的 WORD 就是 (x)"
        "scalar        ::= STRING | WORD                (* WORD 再按形态落成下面几种 *)"
        "BOOL          ::= true | false                 (* 小写、整词 *)"
        "NULL          ::= null                        (* 小写、整词 *)"
        "INT           ::= [ + | - ] DIGIT { DIGIT }    (* 无前导零，且落在 i64 内 *)"
        "FLOAT         ::= [ + | - ] [ DIGIT { DIGIT } ] [ . DIGIT { DIGIT } ] [ ( e | E ) [ + | - ] DIGIT { DIGIT } ]"
        "STR-FALLBACK  ::= 007 | 0x20 | 0b101 | 超 i64 的整数 | 其余 WORD   (* 一律字符串，不猜 *)"
        "ENV-TERM      ::= 美元符 env . NAME            (* 必须占满整个 WORD；未定义 ⇒ 空串 *)"
        "ENV-IN-STRING ::= STRING 内出现 ENV-TERM 时作文本替换      (* 需特性 env *)"
        "LOOP-VAR      ::= STRING 内的 美元符 左花 NAME 右花        (* 仅 @for 体内；未绑定原样保留 *)"
        "美元符        ::= ASCII 0x24                   (* 不是 token，只是 WORD 的首字符之一 *)"
    ]
    notes: [
        "键名与值共用同一套 WORD 规则：能当键的就能当值，反之亦然"
        "含这些字符的标量必须加引号：空白 { } [ ] , : 双引号 以及词首 @"
        "看起来像数字的字符串必须显式加引号，否则回读时被重新归类（保真提醒）"
    ]
}


# -----------------------------------------------------------------------------
#  3. L1 结构
# -----------------------------------------------------------------------------
syntax {
    rules: [
        "root          ::= kv-block | object | array"
        "kv-block      ::= { trivia | member }          (* 顶层键值块：没有外层花括号 *)"
        "object        ::= { { trivia | member } }"
        "array         ::= [ { trivia | element } ]     (* 顶层数组需特性 top-level-array *)"
        "member        ::= directive | field | fragment-expand"
        "field         ::= key [ : ] value              (* 冒号可省：name { } 等价 name : { } *)"
        "key           ::= WORD | STRING"
        "value         ::= scalar | object | array | for-clause | fragment-value"
        "fragment-value::= & NAME                       (* 值引用：把片段的值复制到这里 *)"
        "fragment-expand::= & NAME                      (* 键位置：把片段字段并入当前块 *)"
        "for-clause    ::= @for VAR in ITEM { ITEM } { member }   (* 展开为数组 *)"
        "element       ::= scalar | object | array | bare-block | for-clause"
        "bare-block    ::= TYPE { ARG } { member }      (* 块级类型标注：X 块名 { } *)"
        "              (* TYPE 命中已定义契约 ⇒ 自动应用该契约，等价块内首行 @is（特性 typed-block）*)"
    ]
    rules_separator: [
        "字段分隔      ::= 无需分隔符：a: 1 b: 2 合法，边界由 key : 形态决定"
        "块内逗号      ::= 仅当其后是 key : 时合法，否则 E-PARSE-007（尾逗号也不许）"
        "数组逗号      ::= 任意位置可选：含尾逗号与连续逗号"
        "空值          ::= key : 后紧跟 } ] , 或 EOF ⇒ null"
        "重复键        ::= 同块内同名键提升为数组：k: 1 k: 2 ⇒ k: [ 1 2 ]"
        "顶层标量      ::= 单独一个 42 ⇒ E-PARSE-008（顶层必须是容器，格式固有限制）"
        "裸冒号        ::= 键或值位置游离 : ⇒ E-PARSE-006 / E-PARSE-012（A::B、http://x 须加引号）"
        "块级类型+冒号 ::= k: T { ... } 非法；带冒号时 T 是标量值 ⇒ E-PARSE-006"
    ]
}


# -----------------------------------------------------------------------------
#  4. L2 指令：一律不进结果树，除非标注为产出
# -----------------------------------------------------------------------------
directives: [
    { name: version   output: none   status: impl
      form: "@version LIT [ CODENAME ] [ dev ]"
      note: "版本 = 功能集；可多次声明但须一致；整行等长剥离" }

    { name: feature   output: none   status: impl
      form: "@feature base vN | enable NAMES | disable NAMES | mode whitelist|blacklist | whitelist NAMES | blacklist NAMES"
      note: "特性位开关；未知名 ⇒ E-FEATURE-003，不得越权开启 ⇒ E-FEATURE-009" }

    { name: type      output: none   status: impl
      form: "@type name: TYPE { 模式 }"
      note: "注册自定义类型；模式本身也是 SML 数据（规则即数据）" }

    { name: contract  output: none   status: impl
      form: "@contract NAME [ loose | warn | strict ] { CONTRACT-BODY }"
      note: "定义契约；须在 @is 之前定义（与片段同为顺序依赖）" }

    { name: is        output: none   status: impl
      form: "@is NAME [ loose | warn | strict ] { , NAME ... }"
      note: "在当前块内应用契约，可叠多个；严格度可就地覆盖" }

    { name: when      output: prune  status: impl
      form: "@when ENV-TERM [ ( == | = | != ) LIT ]"
      note: "解析期裁剪紧随其后的字段或块；闭集，没有表达式；需特性 when" }

    { name: for       output: array  status: impl
      form: "KEY : @for VAR in ITEM { ITEM } { ... }"
      note: "有界列表展开为数组；只在值位置；需特性 for" }

    { name: requires  output: none   status: impl_rust
      form: "@requires NAME { NAME }"
      note: "契约体内的契约级依赖声明；JS 端明确拒绝 ⇒ E-CONTRACT-018" }

    { name: weakbind  output: side   status: impl
      form: "@weakbind NAME [ { KEY : SPEC } ]"
      note: "弱绑定：进旁路表，不进主树" }

    { name: weakdata  output: side   status: impl
      form: "@weakdata NAME { ... }"
      note: "同上" }

    { name: weakdoc   output: side   status: impl
      form: "@weakdoc NAME"
      note: "同上，不接受载荷块" }

    { name: fragment  output: none   status: impl
      form: "@NAME [ type: T ] [ name: N ] { ... }"
      note: "片段定义；缺体 ⇒ E-PARSE-005；位置参数 v4 起废弃" }

    { name: include   output: inline status: impl
      form: "[ @ ] include TARGET { , TARGET }"
      note: "文本内联而非对象合并，因此可出现在块内部；上限 32 层" }

    { name: import    output: inline status: impl
      form: "[ @ ] import TARGET { , TARGET }"
      note: "include 的别名；import a.b 自动 as a.b（点当目录层级）" }

    { name: namespace output: wrap   status: impl
      form: "TARGET 后接 as NS"
      note: "以块包裹实现：as db ⇒ db { ... }；db.host 是普通 WORD，核心解析不做点号解引用" }

    { name: glob      output: inline status: impl
      form: "include STRING-含-*"
      note: "需特性 glob-include；命中 0 个静默展开为空" }

    { name: regex     output: inline status: impl
      form: "include re:STRING"
      note: "需特性 regex-include；非法模式 ⇒ E-PARSE-025（不再静默判不匹配）" }
]


# -----------------------------------------------------------------------------
#  5. include / import 的精确形态
# -----------------------------------------------------------------------------
include_directive {
    rules: [
        "INCLUDE-LINE ::= [ @ ] ( include | import ) TARGET { , TARGET } [ 行尾注释 ]"
        "TARGET       ::= PATH [ as NS ] [ { KEY { , KEY } } ]"
        "TARGET       ::= { KEY { , KEY } } [ as NS ] in PATH"
        "PATH         ::= STRING | IMPORT-DOTTED-NAME | re:STRING | STRING-含-*"
        "IMPORT-DOTTED-NAME ::= a.b.c                  (* 仅 import；自动挂命名空间 a.b.c *)"
        "挑键两写法    ::= include x { a b } 等价 include { a b } in x"
    ]
    limits: [
        "相对路径按被包含文件自身目录解析（同 C 预处理器）"
        "循环引用 ⇒ E-INCLUDE-002；目标缺失 ⇒ E-INCLUDE-001；都不静默跳过"
        "include 的路径必须加引号 ⇒ E-INCLUDE-012；例外是 import a.b 与 re: 前缀"
        "引号内的 # 不是注释；挑键不能配合 glob / regex ⇒ E-INCLUDE-009"
        "parse() 是纯函数、不做 IO；include 由 parse_file() 或 resolve_includes() 展开"
    ]
}


# -----------------------------------------------------------------------------
#  6. L3 契约
# -----------------------------------------------------------------------------
contract_system {
    rules: [
        "CONTRACT-BODY ::= { , | CONTRACT-ITEM }"
        "CONTRACT-ITEM ::= @requires NAME { NAME }"
        "CONTRACT-ITEM ::= ! any | ! strict-paths         (* 契约级开关；不收 : 与 [ ] *)"
        "CONTRACT-ITEM ::= ! NAME [ : FIELD-SPEC ] [ -> PATH ]"
        "CONTRACT-ITEM ::= KEY : FIELD-SPEC"
        "FIELD-SPEC    ::= TYPE { MODIFIER }"
        "TYPE          ::= str | int | num | bool | any"
        "TYPE          ::= enum [ WORD { WORD } ] | enum( WORD { , WORD } )"
        "TYPE          ::= [ TYPE ]                        (* 数组；元素位置不许 enum ⇒ E-CONTRACT-012 *)"
        "TYPE          ::= TYPE-NAME | EXT-TYPE | CONTRACT-NAME"
        "              (* 没有 float：num 覆盖 int 与浮点；写 float 会被当成契约名 *)"
        "MODIFIER      ::= required | optional | default VALUE | min NUM | max NUM | from-featag"
        "EXT-MODIFIER  ::= one_of | exactly_one | any_of | depends_on | items_min | items_max | x_only"
        "              (* 须经 ParseOptions::with_modifier 注册；不注册等同禁用 *)"
        "PATH          ::= WORD { [ WORD ] | . WORD }      (* featag 落点：rules[0].name *)"
    ]
    strictness: [
        "strict   默认：未声明字段 ⇒ E-CONTRACT-004"
        "warn     放行 + Warning 诊断"
        "loose    静默放行（声明处或 @is 处可就地放宽）"
        "优先级   strict > warn > loose"
        "边界     三态只管「形状宽容」；类型 / 必填 / 区间 / 枚举 / 互斥恒为错误"
    ]
    semantics: [
        "应用时回填 default，并校验必填、类型、枚举、数值区间、数组元素类型"
        "校验发生在解析期：违反契约直接返回带码错误，而非留到应用侧"
        "跨字段修饰符只看显式写出的键，default 回填的值不算出现"
        "x_only 与 strict 互斥 ⇒ E-CONTRACT-011；from-featag 键被直写 ⇒ E-CONTRACT-024"
    ]
}


# -----------------------------------------------------------------------------
#  6b. 契约与模式类型的活示例
# -----------------------------------------------------------------------------
#  @type：值的字面格式由解析器按「模式数据」匹配，与契约正交（Rust / JS 实现）
@type name: 时刻 {
    序列: [
        { 名: 时, 类: 数字, 次: 2 }
        { 字面: ":" }
        { 名: 分, 类: 数字, 次: 2 }
    ]
}

@contract Window {
    start: 时刻
    end: 时刻 default "23:59"
}

window {
    @is Window
    start: "09:30"
}

# 多契约叠加：@is 后可跟多个契约，各自可带严格度（C 端对多契约报 E-CONTRACT-017）
@contract Left { l: int }
@contract Right { r: int }

pair {
    @is Left loose Right loose
    l: 1
    r: 2
}

# 契约级 @requires：声明该契约依赖哪些特性位（JS 端明确拒绝 ⇒ E-CONTRACT-018）
@contract Gated {
    @requires when
    flag: bool default true
}

gated {
    @is Gated
}


# -----------------------------------------------------------------------------
#  7. L4 扩展点（核心语法保持小而稳，领域差异交给下游注册）
# -----------------------------------------------------------------------------
extension {
    rules: [
        "EXT-DIRECTIVE ::= @ NAME [ name: VALUE ] [ { MEMBER } ]"
        "              (* 没有括号参数：@x(a) 的指令名就是字面 x(a) *)"
        "OUTCOME       ::= Discard | Emit( OBJECT )      (* Emit 的字段并入所在块 *)"
        "EX-TYPE       ::= 注册后即可作为 TYPE 使用"
        "EX-MODIFIER   ::= 注册后即可作为 MODIFIER 使用，可带跨字段校验"
    ]
    builtin_names: [ contract is type version feature when for weakbind weakdata weakdoc ]
    notes: [
        "内置指令名不可被外置占用：注册即报错；同名重复注册同样报错，不静默覆盖"
        "不注册任何扩展时，解析行为与无扩展逐字一致（有单元测试钉着）"
        "位置参数 @x X { }：v4 起废弃，仅当注册方声明 positional 才接受并产出弃用诊断"
        "C-ABI 不支持扩展（trait 对象跨不了 C 边界）；方言文档在不认该指令的环境里读不了"
    ]
}


# -----------------------------------------------------------------------------
#  8. v5 草案（表 A）：已成文、部分已实现，但尚未定稿
# -----------------------------------------------------------------------------
#  featag 示范用的契约（活代码）：!name 在契约体内声明，块头以 !name 使用
@contract Service {
    name: str
    port: int default 8080
    !critical
    !obey: [ int ]
}

# 落点 -> PATH：参数既进 __featags，也写进数据树；父对象自动创建
@contract Deploy loose {
    image: str
    !tls
    !replicas: int -> spec.replicas
    !zone: str -> spec.placement.zone
}

# auto 占位：参数写 auto 表示「取声明里的 default」
@contract Menu loose {
    title: str
    !density: str default compact -> layout.density
}

# from-featag：该字段只允许 featag 落点写入，正文直写会报 E-CONTRACT-024
@contract Seal loose {
    mark: str from-featag
    !mark: str -> mark
}

# 无落点、无参：只留一条记录；块名可省（省了就没有 __name）
@contract Edge {
    region: str default cn-north-1
    !probe
}

v5_draft {
    note: "当前 @version 仍是 v4；下列形态按 docs/swsml/spec/v5.md 表 A 登记，未定稿；featag 子块是活代码（Rust 端可解析）"
    rules: [
        "FEATAG-DECL   ::= ! NAME | ! NAME : FIELD-SPEC | ! NAME : FIELD-SPEC -> PATH"
        "BLOCK-HEAD    ::= TYPE FEATAG-REGION [ , ] [ NAME ] { MEMBER }"
        "FEATAG-REGION ::= { FEATAG-ITEM }              (* 区内靠空白，不写逗号 *)"
        "FEATAG-ITEM   ::= ! NAME | ! NAME [ VALUE { , VALUE } ] | NAME [ VALUE { , VALUE } ]"
        "META-KEY      ::= __featags                    (* 保序数组：[ { name: ... args: [...] } ] *)"
        "PLACEHOLDER   ::= auto                         (* 参数槽占位：取声明里的 default *)"
        "歧义三连      ::= 值位置 a: !b ⇒ E-PARSE-027；块名以 ! 开头 ⇒ E-PARSE-028；空 featag 区 ⇒ E-PARSE-026"
        "兜底          ::= ! 只在「块头契约名之后」是记号，其它位置出现即歧义即报错"
    ]
    items: [
        { id: A1  what: "featag 声明（契约体内 !name）"                 state: draft }
        { id: A2  what: "featag 使用（块头 wea !critical obey[1,2]）"   state: draft }
        { id: A3  what: "__featags 元数据键（保序数组）"                 state: draft }
        { id: A4  what: "from-featag 字段修饰符"                        state: draft }
        { id: A5  what: "!any 契约级开关（any 默认禁用）"                state: draft }
        { id: A6  what: "!strict-paths 契约级开关"                      state: draft }
        { id: A8  what: "正文手写 __* ⇒ 报错"                            state: draft }
        { id: A9  what: "! 歧义一律报错"                                 state: draft }
        { id: A10 what: "auto 占位值"                                    state: draft }
        { id: A11 what: "@feature enable 域.名（如 type.featag）"         state: draft }
        { id: A12 what: "default null 禁用"                              state: decided }
        { id: A13 what: "optional 与 null 的分工明文化"                  state: decided }
    ]

    # -------------------------------------------------------------------------
    #  featag 使用示例（活代码；Rust 已实现 S0–S8，其余四端按设计不实现）
    # -------------------------------------------------------------------------
    featag {
        status: impl_rust
        spec: "docs/swsml/spec/featag.md"
        enable: "@feature enable type.featag, typed-block, block-name"
        enable_note: "三个特性缺一不可：type.featag 开语法、typed-block 让块头首词命中契约、block-name 允许契约词后跟块名"

        # 例一：声明 + 记录（无落点）—— 只多出 __featags，字段照旧按契约校验
        record {
            # 块头三段：契约词 Service / featag 区 !critical !obey[4] / 逗号后的块名 prod
            Service !critical !obey[4], prod {
                name: gateway
            }
            reading: [
                "__type 记契约词、__name 记块名；元数据不参与契约字段校验"
                "__featags 是保序数组：无参项只有 name，带参项多一个 args 数组；重复与顺序都保留"
                "!critical 与 !obey 必须先在契约里声明，否则未声明即拒"
                "port 来自契约 default 回填 —— 与 featag 无关，但同块出现，可见两层正交"
            ]
        }

        # 例二：带落点 -> —— 参数既进 __featags，也直接写进指定路径
        target {
            Deploy !tls replicas[3] zone[cn-north-1], api {
                image: nginx-1.25
            }
            reading: [
                "声明侧 !replicas: int -> spec.replicas：类型是 int，落点是 spec.replicas"
                "父对象 spec.placement 不存在时自动创建（!strict-paths 可关掉这条）"
                "落点造出的 spec 是契约未声明字段 ⇒ 严格契约直接 E-CONTRACT-004，故 Deploy 写成 loose"
                "数组不会被凭空创建：落点路径上有不存在的数组 ⇒ E-CONTRACT-023（长度与元素无解）"
            ]
        }

        # 例三：auto 占位 —— 参数写 auto 表示「取声明里的 default」
        auto {
            Menu density[auto], main {
                title: 主菜单
            }
            reading: [
                "结果里 layout.density 是 compact：来自 !density: str default compact"
                "auto 只在参数槽有特殊含义；没有落点时它只是普通字符串"
            ]
        }

        # 例四：from-featag —— 该字段只允许 featag 落点写入
        from_featag {
            Seal mark[stamped], box {
            }
            reading: [
                "mark 声明为 str from-featag：正文里直接写 mark: xxx 会报 E-CONTRACT-024"
                "可见 featag 既是「标签记录」，也是「唯一可信来源」的写入通道"
            ]
        }

        # 例五：块名可省 —— 逗号后直接跟 { }，结果里就没有 __name
        anonymous {
            Edge !probe, {
            }
            reading: [
                "Edge 用 region 的 default 回填，说明 featag 不干扰契约默认值"
            ]
        }

        gotchas: [
            "未开 type.featag 时块头的逗号触发 E-PARSE-007（实测：块名靠逗号与 featag 区断开）"
            "值位置裸写 a: !b ⇒ E-PARSE-027；要字面量就给 !b 加引号"
            "块名以 ! 开头 ⇒ E-PARSE-028；空 featag 区 Edge ! , ⇒ E-PARSE-026"
            "! 只在「块头契约词之后」是记号，别处出现即歧义即报错（A9）"
            "上面的 __type / __name / __featags 是解析器产物，不是手写字段；v5 起正文手写 __* 报错（A8）"
            "auto 只能作参数槽占位；裸词 auto 与加引号的 auto 当前不可区分"
        ]
    }
}


# -----------------------------------------------------------------------------
#  9. 已废弃 / 已拒绝的写法（逐条给码）
# -----------------------------------------------------------------------------
rejected: [
    { form: "x: str?"               code: E-PARSE-006  note: "契约符号修饰符 ? 已废弃，用 optional" }
    { form: "p: int = 5"            code: E-PARSE-006  note: "同理，用 default" }
    { form: "a: B::C"               code: E-PARSE-006  note: "裸冒号：含 : 的标量必须加引号" }
    { form: "a: :"                  code: E-PARSE-012  note: "值位置的游离 :" }
    { form: "@f Server [ prod ] { }"  code: E-PARSE-005  note: "片段位置参数 v4 起废弃，改 type: / name: 参数" }
    { form: "k: T { ... }"          code: E-PARSE-006  note: "块级类型标注不能带冒号，写 T 块名 { }" }
    { form: "obey(1, 2)"            code: E-PARSE-007  note: "() 不是 token；逗号把词切开 ⇒ 裸词含逗号" }
    { form: "default null"          code: 未实现        note: "v5 起禁用；要「可能没有」写 optional" }
    { form: "__type 手写"           code: 未实现        note: "v5 起报错；现状可伪造元数据键" }
    { form: "a: x,y"                code: E-PARSE-007  note: "裸词含逗号，须加引号或改用数组" }
]


# -----------------------------------------------------------------------------
# 10. 五端实现与跨端约定
# -----------------------------------------------------------------------------
implementations: [
    { end: rust dir: "rust/" status: reference  note: "swsml crate，rlib + cdylib + C-ABI" }
    { end: c    dir: "c/"    status: incubating note: "链接同一套 C-ABI，或纯 C 实现" }
    { end: cpp  dir: "cpp/"  status: incubating note: "封装同一套能力" }
    { end: js   dir: "js/"   status: incubating note: "零依赖 ESM；VS Code 扩展复用同一解析器" }
    { end: lua  dir: "lua/"  status: native     note: "Soup 原生实现" }
]

cross_end_notes: [
    "对象键序不保证：对象是映射，Rust 按键排序输出，其余保书写顺序 ⇒ 需要序列时用数组"
    "引号策略不同：同一份数据两端可写出不同文本（都能读回）；像数字的字符串须显式加引号"
    "裸冒号三端口径：Rust 拒绝、JS 造出 : 键、C 丢字段 ⇒ 含 : 的标量一律加引号"
    "typed-block 多契约叠加：Rust 支持；JS 只认单个 ⇒ E-CONTRACT-017"
    "错误码是稳定契约（同一条件同一个码），文案不是 ⇒ 判断种类请用 code，别匹配文案"
]

diagnostics: [
    { family: E-LEX      stage: 词法  sample: "未闭合字符串 / 未闭合块注释 / 非法转义 / 非法标识符" }
    { family: E-PARSE    stage: 语法  sample: "闭合符错配 / 游离 : / 裸词含逗号 / 顶层标量 / 歧义 !" }
    { family: E-CONTRACT stage: 契约  sample: "未声明字段 / 缺必填 / 越界 / 枚举不匹配 / 修饰符冲突" }
    { family: E-FEATURE  stage: 特性  sample: "特性未开 / 未知名 / 版本越界 / 越权开启" }
    { family: E-INCLUDE  stage: 引入  sample: "目标缺失 / 循环引用 / 路径未加引号 / 挑键配通配" }
    { family: E-LIMIT    stage: 限额  sample: "嵌套过深 / 正则过长" }
]


# -----------------------------------------------------------------------------
# 11. 生态（不是文法，但同属「成果」，一并记录）
# -----------------------------------------------------------------------------
ecosystem {
    crates: [ sml-lex sml-parse sml-value sml-contract sml-include sml-feature sml-pattern sml-regex amberice smltools smlkit-easy swsml ]
    editors: [
        { name: vscode extent: "高亮 诊断 补全 格式化 跳转 悬浮 自检 颜色标记" }
        { name: lsp    extent: "诊断 补全 跳转 悬浮" }
        { name: zed    extent: "Tree-sitter 高亮 括号匹配 注释切换" }
    ]
    tooling: [
        "smltools：to json / yaml / toml / tmlanguage / format / check 等子命令"
        "serde 可选 feature：Value 手写 Serialize/Deserialize，保持数据形状自然"
        "amberice / smldb：二进制布局与数据库（规范已成文，实现为零）"
    ]
}


# -----------------------------------------------------------------------------
# 12. 自证：本节全部是**活代码**，用文件自己演示自己能描述的东西
# -----------------------------------------------------------------------------
@base {
    owner: snoware
    license: MulanPSL-2.0
}

self_demo {
    # 引号可选：裸词即字符串；含空格才加引号
    bareword: NY
    quoted: "21 2nd Street"

    # 块冒号可省：address { } 等价 address: { }
    address { city: Beijing state: NY }

    # 数组：逗号可选、可换行、元素可为块
    tags: [ urgent internal q3 ]
    ports: [ 80, 443, 8080 ]
    empty: []
    rows: [
        { k: a v: 1 }
        { k: b v: 2 }
    ]

    # 类型自识别：true/false/null 整词，数字按形态落点
    t: true
    f: false
    n: null
    i: 42
    neg: -7
    fl: 1.5
    sci: 1.5e3
    looksNumeric: "10021"

    # 行内多字段：无需分隔符
    a: 1 b: 2

    # 词中 @ 是普通字符，邮箱免引号
    mail: ops@example.com

    # 环境变量内联：未定义 ⇒ 空串，不报错
    apiKey: $env.SML_GRAMMAR_DEMO_KEY

    # 片段以值的形式引用
    copy: &base
}


# -----------------------------------------------------------------------------
# 13. 解析期指令的活示例：@when / @for / weak*
# -----------------------------------------------------------------------------
live_parsing {
    # @when：按环境变量在解析期裁剪紧随其后的字段；未定义 ⇒ 空串 ⇒ 条件为假 ⇒ 整字段消失
    @when $env.SML_GRAMMAR_ENV == prod
    prodOnly: true

    # @for：有界列表展开为数组；循环变量在引号串里用 ${名} 引用（未绑定则原样保留）
    hosts: @for h in web api {
        name: "${h}"
        port: 8080
    }

    # weak*：只进旁路表，主树零污染（需特性 type.weak）
    @weakdata fmt {
        theme: snow
    }
    @weakdoc api-docs
    @weakbind Service
    soft { name: x }
}


# -----------------------------------------------------------------------------
# 14. 自证清单：本文件演示了哪些、哪些在一个文档里无法自证、为什么
# -----------------------------------------------------------------------------
conformance {
    live: [
        "@version / @feature / @type / @contract / @is / @when / @for / @requires"
        "weakbind / weakdata / weakdoc"
        "featag：声明 / 带参 / 落点 / auto / from-featag / 块名省略"
        "注释四种 / 引号可选 / 块冒号可省 / 数组免逗号 / 对象数组 / 行内多字段"
        "片段值引用 / 环境变量 / 类型自识别 / 空数组"
        "契约默认值回填 / 契约组合 / 契约宽松 / 多契约叠加"
    ]
    not_in_one_document: [
        "顶层数组：文档根只能有一个形态，本文件根是键值块 ⇒ 数组写法见 examples/full.sml"
        "include / import / glob / 正则引入 / 命名空间：需要外部文件，单文档无法自证"
        "外置扩展点（ext::Directive / TypeCheck / Modifier）：需要宿主注册，文档侧无法自证"
        "@version 的代号与 dev 槽：会改变版本语义，本文只写最小形态"
    ]
    why: "全集活示例必然用到只有 Rust 实现的特性位（featag / @type / weak*），故本文件以 Rust 端为验收标准：cargo run -p smltools -- -i grammar.sml --to json；要被多端共读的核心子集看 showcase.sml"
}
