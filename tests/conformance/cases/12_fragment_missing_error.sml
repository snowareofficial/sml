# conformance: expect-error E-INCLUDE-006
# 钉住：**引用未定义的片段必须报错**，不许静默当空块/裸词。
# 历史：W16 之前 JS 会「静默给错串」；现在各端应同码失败（码是稳定契约，文案不是）。
w {
  x: &nodef
}
