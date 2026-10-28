# 钉住：**裸词 `$env.VAR`** 内联 —— 五端一致的那一档。
# driver 会为子进程设 SML_CONF_TOKEN=conf-token-1（占位值，不含任何真实凭据）。
# 未定义变量按既定口径**静默取空串**（不是报错）—— 这条也是口径的一部分。
token: $env.SML_CONF_TOKEN
missing: $env.SML_CONF_UNSET
