@version v1
# ============================================================================
# secrets.sml —— 只演示环境变量注入（不含真实密钥）
# 等价 JSON 必须把明文写死；SML 让配置与密钥解耦。
# ============================================================================
secrets {
    resendApiKey: $env.RESEND_API_KEY
    dbPassword: $env.DB_PASSWORD
    optionalWebhook: $env.UNSET_WEBHOOK   # 未设置 -> 空串，不报错
}
