# 钉住：契约的**默认值回填**（`default` 修饰符）与 `optional` 字段不出现。
# 期望：a = { city: Beijing, country: CN }（zip 是 optional 且无默认 ⇒ 不出现）。
# 判据必须是**解析结果里**真的有 country —— 只断言「解析成功」抓不到「默认没填」。
@contract Address {
    city: str
    country: str default CN
    zip: str optional
}

a { @is Address  city: Beijing }
