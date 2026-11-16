# 钉住：`@type` 的 `正则:` 逃生舱**五端同树**。
# Rust 原生把 regex 子集编译进无回溯 IR；C / C++ / Lua 移植同构子集；JS 走 RegExp。
# 覆盖两件事：① 纯正则（`名: 前缀` + 字符类 + 计数量词）；② 正则与结构化写法混用。
@type name: Phone {
    正则: "^1[3-9][0-9]{9}$"
}
@type name: Mix {
    序列: [
        { 名: 前缀, 正则: "^[A-Z]{2}" }
        { 字面: "-" }
        { 名: 编号, 类: 数字, 次: 4 }
    ]
}
@contract C {
  p: Phone
  code: Mix
}
x { @is C p: "13800138000" code: "AB-1234" }
