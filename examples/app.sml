@version v1
# ============================================================================
# app.sml —— 主配置（演示 include / 片段展开 / 契约 / 环境变量 / 注释）
# ============================================================================

# 1) include 文本内联（路径带引号）：把 common.sml 原样贴进来
include "common.sml"

-- 2) 片段展开：把 @net 的字段注入到 network（值复用，不是约束）
network: &net

# 3) 契约校验：api 块必须符合 Service，缺失字段补默认值
api {
    @is Service
    name: gateway
    port: 443
    tls: true
    env: prod
    # ratio 缺省且 optional -> 不报错
}

// 4) 环境变量注入：敏感值不落盘，运行时由 $env 读取（缺失则为空串）
auth {
    clientId: $env.OAUTH_CLIENT_ID
    clientSecret: $env.OAUTH_CLIENT_SECRET
    issuer: "https://sso.swebase.cn"   # 引号串，冒号后空格可省
}
