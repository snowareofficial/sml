# ============================================================================
# common.sml —— 公共片段，被 app.sml 通过 include 文本内联
# 本文件覆盖：块注释、行注释、片段定义、契约定义
# ============================================================================

/* 多行块注释：
   这里可以写任意说明，
   跨越多行也不会影响解析。 */

/* 另一种块注释风格（Soup 系习惯）：
   与 /* */ 等价，仅书写风格不同 */

// C 风格单行注释

-- Soup/Lua 风格单行注释

-- 通用契约：所有对外服务必须满足（loose 允许未声明字段）
@contract Service loose {
    name: str                      # 必填：服务名（裸词 str 即字符串类型）
    port: int default 8080        # 带默认值：未提供则用 8080
    tls: bool default false
    env: enum [ dev staging prod ] default dev   -- 枚举：取值须在此列表
    ratio: num min 0 max 1 optional             // 数值区间 + 可选
}

# 公共基础片段：内网服务通用网络设置（用 @name 定义，&name 引用）
@net {
    region: cn-north-1
    dns: internal.swebase.cn
    timeout: 30
}
