# conformance: expect-error E-PARSE-006
# 仅含粘连式遗留修饰符 `?`（与类型名之间无空格）。
# 撕开 33 的「用例掩盖」：C++ 端此前把 `str?` 当整体类型串静默放过
# （实测报 E-CONTRACT-003 missing field），本例只验证 `?` 这一条，
# 确保 C++ 也能报 E-PARSE-006，而非依赖 JS 端的 `?` 拦截。
@contract S {
    x: str?
}
a { @is S }
