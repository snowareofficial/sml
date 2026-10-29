# conformance: expect-error E-PARSE-006
# 仅含遗留修饰符 `=`（与类型名之间带空格，等价写法 `p: int default 5432`）。
# 撕开 33 的「用例掩盖」：JS 端此前把 `=` 并入 word，致拒绝分支对 `=` 是死代码、
# 整条契约被静默接受成垃圾字段；本例只验证 `=` 这一条，确保 JS 也能报 E-PARSE-006。
@contract S {
    p: int = 5432
}
a { @is S }
