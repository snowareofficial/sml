# conformance: expect-error E-PARSE-008
# 钉住：**顶层裸标量必须显式报错**（W3 的验收口径）。
# SML 顶层需为容器；`42` 这类输入无法往返，五端一律 E-PARSE-008 —— 不许某端「静默成功」。
42
