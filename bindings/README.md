# SML 宿主绑定（多语言）

SML v3 的**宿主语言绑定**，统一经 **C-ABI**（`c/sml_rs.h` 为宪法，Rust cdylib `sml`/`libsml`
为单一实现源）暴露能力。**不重写 SML 解析器**。

## 对齐宪法

所有绑定逐符号对齐 `c/sml_rs.h` 的 v3 全集：`sml_loads` / `sml_load_file` / `sml_free` /
`sml_free_str` / `sml_typeof` / `sml_get` / `sml_at` / `sml_size` / `sml_key_at`（本任务为
「不经 JSON 完整物化对象」补的极小键迭代符号，加法兼容）/ `sml_str_dup` / `sml_int_value` /
`sml_real_value` / `sml_bool_value` / `sml_dumps` / `sml_dump` / `sml_dump_err` /
`sml_version` / `sml_features_mask` / `sml_feature_name`。

## 统一实现路径（§2.3-A：值树直访）

- **读**（`loads`/`load_file`/取值）：直接遍历 v3 值树，**不经 JSON**——int/float/bool 类型保真、
  富错误（`sml_error`：code_str/code/line/column/position/source/text）直接到手。
- **写**（`dumps`）：ABI 无值树构建器，按文档走官方桥 `宿主对象 → JSON → sml_dump`。
  这是文档规定的唯一 ABI 序列化出口；**读路径零 JSON 依赖**。

## 内存纪律（§0.3，五门一致）

1. `sml_loads`/`sml_load_file` 返回的根：物化后 `sml_free`。
2. `sml_str_dup`/`sml_key_at`/`sml_dumps`/`sml_dump`/`sml_version_str` 返回的 `char*`：读后 `sml_free_str`
   （`sml_key_at` 的键是堆副本，必须 free）。
3. `sml_get`/`sml_at`/`sml_get_path` 返回**借用**指针：不释放、不跨根释放使用。
4. `sml_version` 返回**静态**串：只读，不释放。

## 语言清单

| 语言 | 目录 | FFI 机制 | 验证状态 |
|---|---|---|---|
| Python | `python/` | `ctypes` | ✅ 跑绿（§4 八断言 + 内存循环） |
| Ruby | `ruby/` | `fiddle`（零三方依赖） | ✅ 跑绿 |
| Java | `java/` | JDK Panama FFM | ✅ 跑绿 |
| Go | `go/` | `cgo` | ✅ 跑绿 |
| C# | `csharp/` | `P/Invoke` | ⚠️ 本机无 `dotnet`，未实地验证（逻辑与其余四门严格对齐） |

## 已知事实（实现行为，非 bug）

- **行列上报**：`line`/`column` 只对「带源码位置的错误」（契约 / `@feature` / `@version`）填充；
  通用语法与 EOF 收尾错误（`E-PARSE-001/003/012`）一律 `line=0, col=0, position=0`。
  故 §4 断言 4 用第 2 行的 `@is NoSuchContract` 真验证行列到位。
- 对象键序：底层 `BTreeMap`，键按字典序稳定（物化后宿主顺序无关）。
- 构建 cdylib：见 `rust/`（产物名 `sml`/`libsml`）。绑定加载用 `SML_LIB` 环境变量或 `PATH`。
