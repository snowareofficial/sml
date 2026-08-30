# ============================================================================
# advanced.sml —— 高级示例：展示 SML 进阶能力（需显式 @feature 开启的部分）
#
# 覆盖特性：
#   · @feature enable 显式开启高级能力（glob-include / multi-include / namespace）
#   · $env 环境变量内联（env 特性，v3 默认含）
#   · include 多目标 + 命名空间内联（multi-include / namespace）
#   · include 通配（glob-include 引入整目录 widget）
#   · 片段定义 + 引用 + 引用后覆盖字段（继承）
#   · 契约组合 + @is 严格校验（未声明字段报错）
#   · 内联数组值（数组也可作顶层文件，需 top-level-array 特性且独占顶层）
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

# ---- 片段引用 + 字段覆盖（继承 + 改写）---
dbPrimary: &timeout_policy
dbReplica: &timeout_policy
dbReplica.read: 60                        # 引用后仅覆盖 read，其余继承

# ---- 契约定义：组合 + 默认值 + 枚举 + 数值区间 ----
@contract Endpoint {
    path:    str
    method:  enum [ GET POST PUT DELETE ] default GET
    timeout: int min 1 max 600 default 30
}

@contract Service {
    name:    str
    main:    Endpoint                     # 组合：字段值是块，递归校验
    replicas: int default 1
}

# ---- @is 严格校验：未声明字段会报错 ----
api {
    @is Service
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
include "common.sml", "secrets.sml" as sec

# ---- import 是 include 的别名，可互换使用 ----
# import 与 include 完全等价，仅关键字不同；下面两行等价：
import "advanced_inc/widget_a.sml" as wa    # import 同样支持 as 与 glob/regex
import "common.sml"                          # 等价 include "common.sml"（顶层内联）

# ---- 命名空间 include（namespace）：整段挂到 ui 键下 ----
include "common.sml" as ui.base

# ---- 部分引用（挑键，避免整文件 copy）----
# 只从 widgets.sml 挑 widget_login / widget_search 两个顶层键并入当前作用域，
# extra_secret 键不会被引入（对比上面 import "*.sml" 整目录内联）。
import "advanced_inc/widgets.sml" { widget_login, widget_search }

# 等价写法：键列表在前、目标文件用 in 指定，并通过 as 挂到命名空间隔离
import { widget_login } as w in "advanced_inc/widgets.sml"
# 上面等价于 import "advanced_inc/widgets.sml" as w { widget_login }
# 访问路径：w.widget_login.endpoint

# ---- 通配 include（glob-include）：引入整目录 widget ----
# 被引入的多个文件顶层键若同名会被聚合为数组；此处用唯一键
# widget_login / widget_search，故各自独立可见。
include "advanced_inc/*.sml"

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

# ---- 引用命名空间内联的内容做校验示例 ----
guard {
    @is Service
    name:    ui-guard
    main {
        path:    $env.GUARD_PATH          # 环境变量也能用在嵌套块里
        method:  GET
    }
}
