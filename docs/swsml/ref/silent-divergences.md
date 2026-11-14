# 兼容检查检不出的差异（判据的四类盲区）

> **产品线** swsml ｜ **状态** current ｜ **最后核对** 2026-11-14
>
> 探针：`tools/check/probe_matrix.py`（四组：`errors` / `gates` / `values` / `warnings`）。
> 用例级一致性看 `tests/conformance/known_divergences.md`；语法特性维度看
> `docs/swsml/ref/grammar-divergences.md`。本文只回答一个问题：
> **套件判 AGREE 的格子里，哪些其实是差异。**

## 0. 现状

`python tools/check/probe_matrix.py`：**80 行里 5 行各端不一致**，全部是数字载体：

| 用例 | rust | c | cpp | js | lua | 性质 |
|---|---|---|---|---|---|---|
| `int-big` / `int-i64max` | 整数 | 整数 | 整数 | 字符串 | 字符串 | JS `Number` / LuaJIT `double` 表示不了 i64 精度 |
| `num-one-dot-zero` | `1.0` | `1.0` | `1.0` | `1` | `1` | 只有一个数字类型，浮点身份丢失（`19` 放行） |
| `num-sci` | `1e3` | `1000.0` | `1000.0` | `1000` | `1000` | C/C++ 仅写法差（值同为浮点 1000）；JS/Lua 丢浮点身份 |
| `num-negzero` | `-0.0` | `-0.0` | `-0.0` | `0` | `-0` | JS 丢符号 |

其余 **75 行全部一致**：错误码 43 条、特性门控 11 条、警告 6 条、值身份 15 条。
最初查出的 24 行差异已收敛到这 5 行，且这 5 行无法在解析器层消除（见 §5 R-2）。

## 1. 为什么套件看不见

`tests/conformance/run_conformance.py` 的判据是「同值树」：各端解析 → 各自 dump → 交回 Rust
归一化 → 比 JSON 值与 token 流。四个结构性缺口：

| 盲区 | 套件为什么看不见 | 本文怎么测 |
|---|---|---|
| 错误码 | 负向用例只有十来条；其余错误条件从不被触发，也就从不比较码 | `--suite errors`：43 条最小坏文档 |
| 特性门控 | `@feature disable X` 之后再用 X，值树常与「没写 `@feature`」一模一样；或该端根本不认那段语法（报别的码），与「门控生效」在结果上不可区分 | `--suite gates`：11 条 |
| 值身份 | 各端 dump 交回 Rust 回读 —— 只要**文本**相同，`1` 与 `1.0`、`"42"` 与 `42` 的差别就被抹平 | `--suite values`：20 条 + `typeof` / `type` 直接取证 |
| 警告 | 警告既不影响值树也不影响退出码 | `--suite warnings`：6 条，直接调各端诊断 API |

## 2. 错误码矩阵：43 条五端同码

```bat
python tools/check/probe_matrix.py --suite errors
```

包含 `E-LEX` / `E-PARSE` / `E-CONTRACT` / `E-FEATURE` / `E-INCLUDE` 的四十三种触发条件。
三类曾经漂移的码现已统一：

- 值位置的游离 `:` / 孤立 `@`：C / C++ 原报 `004` / `006`，现与 Rust 同为兜底 `012`；
  **行首**孤立的 `@` 仍是 `004`（五端一致）。
- `@is C bogus`（第二个名字不是已注册契约）：四端原报 `017`（多契约未实现），现与 Rust 同为
  `001`（未定义的契约）。名字都注册过时仍报 `017` —— 那才是真·多契约能力缺口。
- `E-LEX-006`（转义符后 EOF）：C 原报 `004`，现与其余四端同为 `006`。

## 3. 特性门控：11 条全部一致

`contract` / `fragment` / `env`（裸词与引号内）/ `bareword-string` / `top-level-array` / `when` /
`for` / `type.weak` 的门控五端都生效；`typed-block` / `block-name` 关掉后 Rust 也不报错，
因为特性只决定「块头首词是否按契约解释」，不决定合法性。

## 4. 值身份：20 条里 5 条各端不同

```bat
python tools/check/probe_matrix.py --suite values
```

已对齐的部分（不再复现）：数字样字符串（`"42"` / `"true"` / `"null"` / `"1.5"` / `"007"`）
五端都带引号；`007` 保留字符串；`a: null` 五端都留键；`"x\0y"` 五端都写 `\0` 转义；
超 i64 的整数五端都保留字符串。

剩下 5 行见 §0。JS / Lua 那几格**无法在解析器层修**：除非给值模型加盒子（BigInt / 定点），
否则 `1.0` 与 `1` 在 `Number` / `double` 里就是同一个值。套件对 `19` / `20` 两个用例以 `*`
放行，抹平范围写在 `run_conformance.py` 的 `WAIVERS` / `WAIVER_SIG` 里。

> ⚠️ **JS BigInt 试过，已回退**：`BigInt` 能精确表示 (2^53, i64]，但 `js/ambr.mjs` 的
> `intValue()` 对超出 2^53 的整数一律返回**字符串**，向量测试（`test-ambr-*.mjs`）据此对拍。
> 让解析器单独返回 BigInt 会把 `.ambr` 产品线拆成两套口径 —— 除非连解码器、写入器与
> `js/ambr-vectors.mjs` 一起改，并同步 C/C++/Lua 的解码器。属值层议题，不做局部补丁。

## 5. 逐条：已对齐与残留

### 已对齐（修复后不复现）

| 编号 | 原差异 | 现状 |
|---|---|---|
| S-01 | C 解析 `@contract C { a: str default }` **段错误**（0xC0000005） | 报 `E-PARSE-021`；根因是标点 token 的 `v` 为 NULL 被交给 `strcmp` |
| S-02 | C / C++ 把 `007` 解析成整数 7 | 保留字符串（前导零判据与 Rust 同源） |
| S-03 | Lua 丢掉对象字段的 `null`（`a: null` → 空文档） | 与数组元素位置共用 `NULL` 哨兵，dump 输出 `null` |
| S-05 | 四端 dump 把 `"42"` 写成 `42`（自己读回来类型就变了） | 四端 `needs_quote` 全部按 Rust `text_rules` 重写 |
| S-06 | NUL：C 截断；C++ / JS / Lua 写裸 0x00 | 五端一致：`sml_value` 增加 `slen`、token 增加 `len`，引号串按**字节长度**入队与序列化，dump 转义 `\0` |
| S-07 | 未加引号的 `include x.sml`：C++ 当字段、JS 凭空造 `x.sml` 键 | 两端报 `E-INCLUDE-012`（JS 只保留 `import a.b` 与 `re:` 两个豁免） |
| S-08 | BOM 会改变 C 的「行首指令」判定 | `resolve_includes` 在 depth 0 剥 BOM |
| S-09 | `@feature bogus` / 缺参 / `mode xyz`：JS 静默、C / C++ / Lua 码错 | 五端统一 006 / 007 / 008；合法但未实现的子命令仍报 006 |
| S-10 | `@feature disable env` 对**裸词** `$env.X` 不生效（C / C++ / Lua） | 三端在裸词 `$env` 分支补门控 |
| S-11 | 契约定义的形态错误各端乱报（`enum x`、`min abc`、`[enum …]`） | 统一到 023 / 022 / 012；JS 的 `min abc` 此前静默穿透区间约束 |
| S-12 | 值位置孤立 `@` / 游离 `:`：Lua **接受**（还把内部标记 `@bare` 写进数据）、C/C++ 码错 | 五端同为 012 |
| S-13 | Lua 把超 i64 整数降级成浮点（Rust 是字符串） | 按字典序判 i64 上界，超界保留字符串 |
| S-14 | 警告层没有任何对照组 | `--suite warnings`：五端 6 条全同（`W-PARSE-001` 等），`W-LINT-*` / `W-CLI-*` 属 Rust CLI lint 规则，不参与解析期诊断对比 |
| S-15 | C / C++ / Lua 的 typed-block **不应用契约**（只认显式 `@is`），契约的 default / 必填 / 类型校验在块头写法下静默失效；旧切片因契约没有默认值而看不出来 | 三端补上契约应用；新增切片 `typed-block-default`（契约带 `default`）钉住 |
| S-16 | featag（块头 `!` 区 / `__featags` / 契约内 `!name`）只有 Rust 实现，四端报三种不同的拒码 | 四端实现 S2–S4，`featag` / `featag-decl` 两切片五端 AGREE；S5–S8（校验 / 落点 / from-featag / `!any`）仍是缺口 |
| S-17 | JS 的 `dumpInline` 对嵌套容器写 `[..]` / `{..}` 占位 ⇒ 回读时 `..` 变成字符串（`!obey[4]` 的 args 丢值） | 改为递归展开 |
| S-18 | 契约校验跳过的元数据键集合各端不同（C/JS 只跳 `__type`/`__name`，Lua/C++ 只跳两个） | 统一为「跳 `__type` / `__featags`，`__name` / `__args` 参与校验」；且块头契约的校验发生在 `__name` 落位之前（与 Rust 两条实测行为一致） |

### 残留

- **R-2 数字载体**：JS / Lua 的 i64 大整数与浮点身份（§0 五行）。JS 的 `BigInt` 与 LuaJIT 的
  定点盒子都会牵动 `.ambr` 值模型与五端解码器，需作为值层项目单独立项；当前由 `19` / `20`
  两个放行项明确登记，不是假装一致。
- **`@type` / featag / 多契约**：属「未实现特性」，不是静默差异，见
  `docs/swsml/ref/grammar-divergences.md`。featag 按 `docs/swsml/plan/featag-slices.md` 的
  S9 策略四端刻意不实现。

## 6. 未判区（还没有对照组）

| 层 | 现状 |
|---|---|
| 旁路表 | `@weakbind` / `@weakdata` / `@weakdoc` 不进主树，dump 里看不到；`sml_parse_file_full` 与 Rust 的 `ParseOutput` 都能取，但探针没取。 |
| include 挑选 / glob 顺序 | `include x { a b }`、`include "*.sml"` 的命中顺序与挑选结果没有用例；顺序差异会被 dump 的键序差异掩盖。 |
| `@feature mode` 语义 | 参数校验五端一致（008），但 `mode whitelist/blacklist` 的**语义**只有 Rust 实现，四端报 006（明确不支持）。 |

## 7. 回写位置

| 落点 | 内容 |
|---|---|
| `tests/conformance/cases/` | 建议补：`default` 缺值（C 曾崩溃，现已 021）、`a: null`（Lua 曾丢键）、带 BOM 的行首指令、未加引号的 `include`、数字样字符串、NUL 往返 |
| `tests/conformance/known_divergences.md` | 本轮修复状态 + `19` / `20` 放行的理由（现在只剩 Lua 一侧） |
| `errors/codes.sml` | `E-FEATURE-003` / `006` / `007` / `008` / `E-INCLUDE-012` 的 `impls` 与注记已随修复更新 |
| `c/sml.h` / `c/sml.c` | `sml_value.slen` 与 `token.len`：字符串按字节长度存取（C-ABI 结构体加字段，调用方只经 API 分配，不受影响） |
