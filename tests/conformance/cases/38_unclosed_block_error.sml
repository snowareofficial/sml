# conformance: expect-error E-PARSE-001
# 钉住：**未闭合的块**必须报错，不许某端静默收尾。
# 判据来自 codes.sml 的 E-PARSE-001（Rust 另分契约体与 @for 循环体两种，C/C++/Lua 覆盖块/数组/契约体）。
a {
  b: 1
