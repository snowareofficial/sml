@version v1
# ============================================================================
# full.sml —— 综合示例：覆盖全部语言特性
# 特性清单（Rust / C / JS / C++ 四端已对齐）：
#   · 注释：#  --  //  /* */（注：/_* *_/ 风格块注释 Rust 端暂不支持，统一用 /* */）
#   · 裸词字符串 vs 引号字符串
#   · 块冒号可省：addr { } ≡ addr: { }
#   · 数组：逗号可选、可换行、元素可为块
#   · 转义：\u{} 写 Unicode，\n \t 控制字符
#   · 嵌套块（不依赖缩进，花括号定界）
#   · 片段定义/引用：@frag {}  +  &frag
#   · 契约：@contract + @is + default + optional + enum + min/max + 组合
# ============================================================================

// ---- 注释全覆盖 ----
# 井号行注释
-- 双横线行注释（Soup/Lua 风格）
// 斜杠行注释（C 风格）
/* 斜杠星 块注释
   可跨行 */
/* 星下划线 块注释 */
/* 另一种 块注释 */
_* hi *_

-- ---- 标量 ----
name: "SML 配置"            # 引号串
tagline: 声明式配置格式       # 裸词串（含中文无需引号）
enabled: true               # bool
maxConn: 1024               # int
ratio: 0.75                 # float
nothing: null               # None

-- ---- 字符串转义 ----
banner: "SML \u{1F680} 上线 \n第二行\t制表"   // \u{1F680}=火箭 emoji

-- ---- 数组（逗号可选 / 可换行）----
tags: [ urgent internal q3 ]                 # 裸词数组
ports: [ 80, 443, 8080 ]                     # 带逗号也行
empty: []

-- ---- 对象数组（免逗号、可换行）----
endpoints: [
    { path: /health method: GET }
    { path: /api/v1 method: POST }
]

-- ---- 块冒号可省 ----
address {
    street: "21 2nd Street"
    state: NY
}

-- ---- 嵌套块（不依赖缩进）----
database {
    primary { host: db1.internal port: 5432 }
    replica { host: db2.internal port: 5432 }
}

-- ---- 片段定义与引用 ----
@endpoint_base {
    timeout: 30
    retries: 3
}
serviceA: &endpoint_base     # 引用片段：serviceA 获得 timeout/retries
serviceB: &endpoint_base

-- ---- 契约：组合 + optional + enum + min/max ----
@contract Credential {
    user: str
    token: str optional          # 可缺省
}
@contract Worker {               # 默认 strict：未声明字段报错（loose 需显式写）
    name: str
    load: num min 0 max 100
    status: enum [ idle busy ]
    cred: Credential             # 组合：字段值是块，递归按 Credential 校验
}

worker {
    @is Worker
    name: importer
    load: 42
    status: busy
    cred {
        user: etl                # token 省略（optional）通过
    }
}
