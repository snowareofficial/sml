# 08 / 09 的 include 目标（文件名不以 `_` 开头也无妨：driver 只扫 cases/ 顶层，
# 子目录里的文件不会被当成用例，见 run_conformance.py::discover）。
sub: 2
lib {
  deep: 3
}
