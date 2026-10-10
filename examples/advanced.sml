# ============================================================================
# advanced.sml —— 高级示例：展示 SML 进阶能力（需显式 @feature 开启的部分）
#
# 覆盖特性：
#   · @feature enable 显式开启高级能力（glob-include / multi-include / namespace）
#   · $env 环境变量内联（env 特性，v3 默认含）
#   · include 多目标 + 命名空间内联（multi-include / namespace）
#   · include 通配（glob-include 引入整目录 widget）
#   · 片段定义 + 引用（块内可再写同名键；SML 的重复键**聚合为数组**，不是覆盖）
#   · 契约组合 + @is 严格校验（未声明字段报错）
#   · 数组值（键持有数组；「文件顶层就是数组」需 top-level-array 特性且独占顶层，见文末）
#   · 裸词 / 引号 / 转义 / Unicode 混用
#
# 解析方式：
#   Rust:  sml::parse_file("examples/advanced.sml")  （自动展开 include + @feature）
#   C-ABI: sml_load_file(path)  或  sml_loads(text, SML_F_ALL)
# ============================================================================

# ---- 显式开启高级能力（v3 基线之上裁剪）----
@feature enable glob-include, multi-include, namespace

# ---- 环境变量内联（env）----
meta {
    appName:    "SML 高级示例"
    builtBy:    $env.USER                 # 取环境变量 USER；缺失则为空串
    dataDir:    $env.APP_HOME             # 取环境变量 APP_HOME
    buildStamp: "2026-08-31"
}

# ---- 片段定义（fragment）----
@timeout_policy {
    connect: 5
    read:    30
    retries: 3
}

# ---- 片段引用（继承）+ 局部追加（SML 语义：重复键**聚合为数组**）----
dbPrimary: &timeout_policy                # 直接引用：connect / read / retries 全部继承
# ⚠️ `dbReplica.read: 60` 这种「点号路径」**不存在**（实测：SML 会把它当字面键 `dbReplica.read`，
#    既不覆盖也不报错 —— 正是最容易写错的一处）。要在引用后改写字段，得把引用放进块里
#    再写同名键；结果是 `read: [ 30, 60 ]`（SML 的重复键一律聚合为数组，与 YAML 的 merge 不同）。
dbReplica {
    &timeout_policy                       # 先引用片段（继承 connect / read / retries）
    read: 60                              # 再写同名键 ⇒ read: [ 30, 60 ]
}

# ---- 契约定义：组合 + 默认值 + 枚举 + 数值区间 ----
@contract Endpoint {
    path:    str
    method:  enum [ GET POST PUT DELETE ] default GET
    timeout: int min 1 max 600 default 30
}

# ⚠️ 契约名**不能叫 `Service`**：`examples/common.sml:18` 里也有一个 `@contract Service loose`，
#    同名会被它顶掉（实测后果：下面这几行**完全没生效** —— `api` / `guard` 里凭空多出
#    common.sml 那份契约的默认值 port: 8080 / tls: false / env: dev，而且**不报未声明字段**）。
#    改成独有名字后，「@is 严格校验」才真的是严格。
@contract ApiService {
    name:    str
    main:    Endpoint                     # 组合：字段值是块，递归校验
    replicas: int default 1
}

# ---- @is 严格校验：未声明字段会报错 ----
api {
    @is ApiService
    name: gateway
    main {
        path: /v1/route
        method: POST
    }
    replicas: 2
}

# ---- 多目标 include（multi-include）：一次引入多个文件 ----
# 注意各目标的归宿不同：
#   · "common.sml"      无 as  → 顶层内联（与本地键合并）
#   · "secrets.sml" as sec → 整段挂到 sec 命名空间下（sec.secrets.resendApiKey ...）
#
# ⚠️ 被 include 的 `common.sml` 里有一段 `_* … *_` 块注释：它同时是「注释写坏的后果」的
#    回归语料 —— 若把那段写成「块注释里再出现 `*/`」（`/* … /* … */ … */`），
#    SML 的块注释**不嵌套**（五端实测一致），第一个 `*/` 就闭合，剩下的字会被当成**数据**，
#    于是每个 include 它的文档里都会凭空多出一个脏键，而且**不报错**。
#    换句话说：**注释里的一个字符就能污染配置**，而这类错误没有任何提示 —— 别在块注释里写 `*/`。
include "common.sml", "secrets.sml" as sec

# ---- import 是 include 的别名，可互换使用 ----
# import 与 include 完全等价，仅关键字不同；`import` 同样支持 as / glob / regex：
import "advanced_inc/widget_a.sml" as wa

# ⚠️ 本行是「五端一致性」的**回归语料**：`import "x.sml"`（带扩展名、无 `as`）在 Rust
#    参考实现下曾被错误地自动挂成点分命名空间 ⇒ 多出一份 `common { sml { … } }`，而 JS
#    是顶层内联 —— 同一份文档两端得到**不同的树**。2026-09-19 已修：判据收窄为「只有
#    **点分模块名**（`import ui.buttons`）才自动挂命名空间」（`rust/sml-include` 的
#    `is_dotted_module_name`，该 crate 随之升到 0.1.0-alpha.3）。现在
#    `import "x.sml"` ≡ `include "x.sml"`（带扩展名 ⇒ 顶层内联）。
#
#    顺带一个「文本内联」的固有语义：common.sml 在本文件里被内联了**两次**
#    （上面 multi-include 一次 + 本行一次），两次都是**同一份内容** ⇒ 里面的同名键会按
#    「重复键聚合」变成数组；common.sml 顶层只有契约与片段、没有数据键，所以这里看不出差别。
import "common.sml"

# ---- 命名空间 include（namespace）：整段挂到 ui 键下 ----
include "common.sml" as ui.base

# ---- 部分引用（挑键，避免整文件 copy）----
# 只从 widgets.sml 挑 widget_login / widget_search 两个顶层键并入当前作用域；
# widgets.sml 里的 extra_secret **不会被这一次引入带出来**。
# ⚠️ 注意「这一次」：挑键只约束**本指令**，挡不住另一条指令去引同一文件 ——
#    所以下面 glob 段的模式必须收窄成 `widget_*.sml`，否则 extra_secret 照样进顶层。
import "advanced_inc/widgets.sml" { widget_login, widget_search }

# 等价写法：键列表在前、目标文件用 in 指定，并通过 as 挂到命名空间隔离
import { widget_login } as w in "advanced_inc/widgets.sml"
# 上面等价于 import "advanced_inc/widgets.sml" as w { widget_login }
# 访问路径：w.widget_login.endpoint

# ---- 通配 include（glob-include）：按模式引入一批文件 ----
# ⚠️ 模式**故意避开 widgets.sml**：它的文件名开头是 `widgets` 而不是 `widget_`，
#    因此不匹配 `widget_*.sml`。此前这里写的是 `advanced_inc/*.sml`，会把 widgets.sml
#    又整个引入一次，于是：
#      ① `extra_secret` 照样出现在顶层（`token: should-not-leak`）—— 与上面「不会被引入」的说明**自相矛盾**；
#      ② widget_login / widget_search 各被引入三次，按 SML 语义**聚合为 3 元素数组**。
#    现在只通配 widget_a / widget_b，顶层各聚合为 **2 元素**（实测）：
#      · widget_login  = 上面挑键那次 + widget_a.sml 这次；
#      · widget_search = 上面挑键那次 + widget_b.sml 这次；
#      · `import "…/widget_a.sml" as wa` 那次的 widget_login 挂在 wa 命名空间下，
#        **不参与**顶层聚合 —— 这正是「同名键一律聚合为数组，命名空间才能隔离」要演示的语义。
include "advanced_inc/widget_*.sml"

# ---- 内联数组（数组值）----
# 注：SML 顶层只能为单一容器形态——若要文件整体就是数组，需显式
# @feature enable top-level-array 且文件顶层不能混用键值对。此处演示
# 更常用的「键持有数组值」形式：
regions: [
    { region: cn-north-1 weight: 10 }
    { region: cn-east-1   weight: 5  }
]

# ---- 纯数组文件示意（需 top-level-array，且独占顶层，不可与键值块混用）----
# 若独立成文件 regions.sml 仅含下面这一行，并 @feature enable top-level-array
# 即可被 parse_file 直接读回（与 to_sml 的数组输出对称）：
# [ { region: cn-north-1 } { region: cn-east-1 } ]

# ---- 裸词 / 引号 / 转义 / Unicode 混用 ----
banner:   "SML \u{1F680} 高级特性演示 \n第二行\t缩进"
keywords: [ config markup declarative ]

# ---- 同一契约再用一次：环境变量也能出现在嵌套块里 ----
guard {
    @is ApiService
    name:    ui-guard
    main {
        path:    $env.GUARD_PATH          # 环境变量也能用在嵌套块里
        method:  GET
    }
}
