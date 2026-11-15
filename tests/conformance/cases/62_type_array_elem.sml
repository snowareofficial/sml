# conformance: expect-error E-CONTRACT-009
# 钉住：`[D]` 的**数组元素**也要按 D 的模式校验，且错误带下标（`v[1]`）。
# 判据照 Rust `check_type`：数组逐元素递归校验、错误直接上抛 —— 不能写成
# `.all(|it| ok)` 再回落成通用的「类型应为 [D]」，那样会丢掉下标。
# JS 早先只判「元素是字符串」，`"abc"` 直接放行（静默差异）。
@type name: D3 {
    序列: [ { 名: d, 类: 数字, 次: 3 } ]
}
@contract C { v: [D3] }
x { @is C v: [ "123", "abc" ] }
