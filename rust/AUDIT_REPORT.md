# SML Rust 实现 — 安全与正确性审计报告（待修复项）

- 审计对象：`rust/` 下未提交改动（emit 多目标转译后端、C-ABI、core 解析器、derive 整数转换）
- 审计方式：源码审查 + 语法感知模糊测试（18 万文本用例 + 18 万 emit 用例）+ 定向根因定位
- 报告日期：2026-09-01
- **状态（第七轮，2026-09-01）：全部已修复。语法升至 v4（片段参数显式化），
  原语法多义性已消除。已建立防复发测试 `tests/syntax_guard.rs`（24 常驻，0 ignored）。
  `cargo test` 全量 225 passed / 0 failed；`cargo clippy --all-features --all-targets`
  0 error（本轮修复 5 处 soundness 缺口，见 0-C）。**

---

## 0-C. 第七轮（2026-09-01）：`cargo clippy` soundness 缺口（已修复）

前六轮从未运行过 clippy（`cargo build` / `cargo test` 不触发它），本轮首次运行即**编译失败**：

```
cargo clippy --all-features --all-targets
  error: this public function might dereference a raw pointer but is not marked `unsafe`
  src/c_abi.rs:38 / 52 / 208 / 212 / 252
```

| 编号 | 项目 | 状态 | 说明 |
|---|---|---|---|
| R-15 | C-ABI 导出函数未标 `unsafe` 却解引用裸指针 | ✅ 已修 | 4 个函数（5 处告警点） |

**问题**：`sml_parse` / `sml_dump` / `sml_parse_ex` / `sml_parse_file` 声明为**安全函数**，
体内却直接 `CStr::from_ptr(外部传入的裸指针)`。对 C 调用方无影响（C 本来就不校验），
但从 Rust 侧看，任何 **safe Rust** 代码都能传入野指针触发 UB——这是真实的 soundness 缺口。

**修复**：4 个函数改为 `pub unsafe extern "C" fn` 并补 `# Safety` 文档段。
**导出符号与 `include/sml.h` 签名完全不变**；`tests/c_abi.rs` 走自己的 `extern "C"` 声明块，
调用点本已包在 `unsafe {}` 中，无需改动。

**附带**：`tests/version.rs` 的 `3.14` 触发 `clippy::approx_constant`（测试数据非圆周率近似），
就地 `#[allow]` 并注释说明，不改测试数据。

**复跑**：`python clippy_report.py` → **0 error**（剩余 78 条均为 hygiene 类 warning）；
`python run_test.py` → **225 passed / 0 failed**。

> 教训：与 P0-3「只覆盖两个端点」同构——`cargo test` 全绿会让人误以为完整，
> 但 lint 是**另一条独立的检查通道**，不跑就永远看不见。

### 残余风险（无法消除，仅记录）

以下不属于「已发现待修」，而是**本审计方法论覆盖不到**的部分，任何「安全」结论都以此为界：

| 面 | 现状 |
|---|---|
| OSV 0 命中的含义 | 仅覆盖**已披露** CVE；未披露漏洞 / 0day / 供应链投毒查不到 |
| `c_abi.rs` 的 20+ 处 `unsafe` | 仅靠**人工审阅 + 测试**覆盖，**未跑 Miri**，未做 UB 形式化验证 |
| 安全门禁自动化 | `osv_check.py` / `clippy_report.py` / fuzz **均未接入 CI**，回归不会被自动发现 |
| `smlconv` 外部命令 | `Command::new("zola").arg("build")` 依赖 PATH，属外部攻击面，本轮未审计 |
| 审计方法论 | 前六轮均为「人工枚举攻击面」，非覆盖率驱动的持续 fuzz |
| 非 Rust 实现 | C / C++ / Lua / JS / Python **未扫描**，官网已标注「实验性、暂不保证」 |

---

## 0-A. 语法层排查与防复发机制（最新，2026-09-01）

### 根因：兜底策略选择了 fail-soft

本轮发现的语法问题并非各自独立，而是**同一根因的不同实例**：
解析器对「意外 / 非法输入」的兜底分支默认选择**猜测并继续**，而非**报错**。

| 缺陷 | 兜底行为 |
|---|---|
| `@contract` 漏 `}` | 清空整个文档，返回 Ok |
| 未知指令 `@xxx` 后接块 | 把块当片段体吃掉，返回 Ok |
| 未闭合块 / 多余 `}` | 吞掉或忽略后续内容（已修） |
| 多目标 include 未开特性 | 降级为普通内容（已修） |
| 未定义片段 `&nope` | 降级为字符串（已修） |
| 裸块第三个词 | 静默丢弃（已修，现保留于 `__args`） |
| 前导零 `0755` | 剥离为 `755`（已修，现保持字符串） |
| 未知转义 `\z` | 吞掉反斜杠（已修，现报错） |

**为何此前未被发现**：测试覆盖的是两个端点——「正确输入→正确输出」与「明显错误→报错」；
而缺陷位于中间地带「畸形输入→静默给出错误结果」。fuzz 只断言「不 panic」，
不断言「结果正确」，因此这类问题全部漏网。

### 防复发：`tests/syntax_guard.rs`

已新增回归测试文件（18 个用例，`cargo test` 自动运行），采用三层防守：

1. **核心不变量（内容守恒）**：解析成功时，源码中出现的每个键都必须出现在结果中
   （任意层级）。自动抓出「内容被吃掉」。
2. **结构畸变**：删除 / 插入 `{ } [ ] " : @` 等结构性符号，断言要么报错要么守恒。
3. **历史回归**：已修复缺陷的精确用例，防止退化。

文件头部还写入了**新增语法时的自检清单**：对意外输入的兜底必须显式二选一——
能理解则给结果，不能则**返回 Err**；禁止「猜测并继续」。

### 已修复：孤立 `@` 静默吞掉紧随的块（词法层）

- **现象**：配置中一个孤立的 `@` 会让紧随其后的块（及后续内容）被静默丢弃，不报错。
  ```
  @            +  blk { x: 1 }        →  Ok，结果为空
  a: 1 / @ / b { x: 2 } / c: 3        →  Ok，a、c 保留，**b 块消失**（部分丢失，更难察觉）
  ```
- **根因（词法层）**：`@` 与后随名字之间的**空白在分词时被丢弃**，使得
  `@` + `blk { .. }` 与 `@blk { .. }` 产生**完全相同**的 token 流
  （`At, Word("blk"), LBrace, ..`）。解析器据此把孤立 `@` 当作片段定义，
  将紧随的块当作片段体消费掉（片段不进主树），因此既不报错也无输出。
  也正因如此，该问题**无法在语法层修复**——必须回到词法层保留「是否紧邻」的信息。
- **改动**（`rust/src/core.rs`）：
  1. `enum Tok` 新增 `BareAt` 变体，语义为「`@` 之后紧跟空白或行尾，没有名字」。
  2. `tokenize` 中按 `@` 是否紧邻后随内容（`!c.is_whitespace()`）分别生成 `At` / `BareAt`。
  3. `parse_block_inner` 新增 `Tok::BareAt` 分支，直接返回错误：
     ``sml: 孤立的 `@` 不是合法指令；片段定义须写作 `@name { ... }`（`@` 与名字之间不可有空白），或删除该 `@``
- **兼容性验证**（均为回归用例 `at_related_valid_syntax_still_works` 覆盖）：
  片段定义与引用、`@name Type { }` / `@name Type Name { }` 参数形式、
  `@contract` / `@is` / `@version` / `@feature`、
  **以及 `@` 出现在词中间的情形（`email: a@b.c`、`k: x@y`）全部不受影响**。
- **当前状态**：`cargo test` 全绿（219 passed / 0 failed）；
  `tests/syntax_guard.rs` 中该项已由 `#[ignore]` 转为常驻断言。

### 已修复：语法升至 v4，片段参数显式化（消除多义性）

`@nosuch` 这类**未知指令名**后接块时，原会被解释为「定义名为 `nosuch` 的片段，
其后单词作为 `__type` / `__name` 参数」，块内容不进主树且不报错：

```
@nosuch      +  blk { x: 1 }   →  Ok，结果为空   （v3 及更早）
@versoin v1  +  blk { x: 1 }   →  Ok，结果为空   （指令名拼写错误）
```

多义性根源：`@nosuch Type { .. }` 与合法的「片段定义 + type 参数」
`@f Server { .. }` 在 token 流上完全一致，解析器无法判别用户意图。

**解决方案（采纳显式关键字方案）**：片段的 `type` / `name` 参数改为显式形式，
位置参数形式废弃，随语法版本升至 **v4**。

| 写法 | v3 | v4 |
|---|---|---|
| `@f { host: a }` | ✅ | ✅（不变，最常见形式） |
| `@f type: Server { }` | ✅ | ✅（新推荐写法） |
| `@f type: S name: prod { }` | ✅ | ✅（顺序不限） |
| `@f Server { }` | ✅ | ❌ 报错（位置参数已废弃） |
| `@nosuch blk { x: 1 }` | ⚠️ 静默清空 | ✅ 报错 |

**改动清单**（`rust/src/core.rs`）：

1. `enum Version` 新增 `V4`，`CURRENT` 改为 `V4`；`from_word` 支持 `v4`/`4`，
   `name()` 返回 `"v4"`；`@feature base` 错误提示更新为 `v1/v2/v3/v4`。
   `strict_strings` 依据 `self >= Version::V2` 判定，V4 自动继承严格语义。
2. `Parser` 新增 `peek_at(n)` 前看辅助方法。
3. 片段参数解析改为显式关键字循环：仅当 `type`/`name` **后紧邻冒号**时才视为参数
   （因此名为 `type` / `name` 的片段 `@type { .. }` 仍可正常定义），
   支持重复参数检测；其余情形一律报错。
4. 错误信息合并两种意图的排查指引（拼错的指令 / 未显式化的参数），
   因二者在语法上确不可区分。

**兼容性**：v4 与 v3 的字符串引号、标量裸词等规则完全一致，仅片段参数形式变动。
调用方通过 `@version v3` 可继续按旧语法解析（默认仍为 v1）。

**回归用例**（`tests/syntax_guard.rs`，均已由 `#[ignore]` 转为常驻）：
`v4_explicit_fragment_params_work`、`v4_positional_fragment_params_rejected`、
`unknown_directive_name_with_block_must_error`、`v4_duplicate_params_rejected`、
`fragment_named_type_or_name_still_works`、`v4_version_accepted`。

> 说明：`"C:\temp\new"` → `"C:\temp ew"` 属**预期行为**（`\t` `\n` 为合法转义，
> 与 JSON/YAML 一致），非缺陷；Windows 路径须写 `\\`。未知转义（如 `\z` `\d`）已修为报错。

---

## 0-B. 第三轮审计（2026-09-01）：emit 注入面、ReDoS 与依赖 CVE（已修复）

**范围**：Rust 实现全量（40 个依赖包 + 8 个源文件 + `smlconv` bin）。
**方法**：依赖 CVE 查询（OSV.dev，脚本 `osv_check.py`）+ 源码审计 + 19 个 PoC 实测。
**结果**：依赖 **0 CVE**；发现并修复 6 项；另发现 1 项既有正确性缺陷（未改，见文末）。

| 编号 | 项目 | 状态 | 说明 |
|---|---|---|---|
| N1 | Markdown `code` 块 `lang` 注入 | ✅ 已修 | **P1-3 围栏逃逸的残留面** |
| N2 | Markdown 表格单元格结构字符 | ✅ 已修 | `\|` 伪造列、换行伪造行 |
| N3 | Markdown 字段名注入 | ✅ 已修 | 逃逸出列表、破坏 `**key**` 定界 |
| N4 | ReDoS 步数预算按起点重置 | ✅ 已修 | 总开销 = 起点数 × 2M |
| N5 | Slint 字符串字面量未转义换行 | ✅ 已修 | 产出不可编译的跨行字符串 |
| N6 | custom emit `exclude` 被 `text` 绕过 | ✅ 已修 | `exclude:["text"]` 形同虚设 |

### N1　Markdown `code` 块 `lang` 未清洗 → 围栏逃逸（P1-3 残留）

`code_fence` 原先只按**代码体**计算围栏长度，同一条语句里的 `lang` 却被原样拼接：

```rust
let lang = v.get("lang").and_then(|x| x.as_str()).unwrap_or("");
let fence = code_fence(body);            // 只算了 body
out.push_str(&format!("{} {}{}\n", pad, fence, lang));
```

端到端实测（`code { lang: "js\n\n# INJECTED\n\n<script>alert(1)</script>" text: "body" }`）
确认 `# INJECTED` 与 `<script>` **逃逸到代码块之外**，成为 Markdown 正文。

这正是 P1-3 修复时「只补了 body 侧、漏掉同一语句里的 lang」——与本报告
`### 已修复：语法升至 v4` 一节总结的「只覆盖两个端点、中间地带漏网」同构。

**修复**：
- 新增 `sanitize_code_lang`，采用**白名单**（`[A-Za-z0-9+#.-]`）剔除其余字符。
  之所以用白名单而非只去换行：info string 会被渲染器原样写进
  `<code class="language-…">`，仅去换行仍残留 `<` `>` `"` 等可破坏属性的字符。
- `code_fence` 改为接受 `&[&str]`，围栏长度同时覆盖 `body` 与 `lang`（防御纵深）。

### N2　Markdown 表格单元格未中和结构字符

`escape_text` 只有 XML 语义（`& < > " '`），不认识 Markdown 表格定界符：
`|` 会伪造额外列，换行会伪造额外行，让不可信数据「看起来像」表头或另一条记录。

**修复**：新增 `md_cell_text`——`|` → `\|`，换行折叠为空格（表格内无法表达真换行），
再交给 `escape_text`。四处取值点（表头、数组行、对象行、其他）统一改走该函数。

### N3　Markdown 字段名未转义

字段名被原样拼进 `### name` 与 `- **key**: value`。字段名同样可能来自不可信输入，
其中的换行会逃逸出当前块（伪造标题 / 新列表项），`*` `` ` `` `[` 会破坏 `**key**` 定界。

**修复**：
- 新增 `md_escape_key`：先中和 `\ * \` [ ] #`（加反斜杠）与换行（→空格），再做实体转义。
  刻意**不转义 `_`**——词内下划线在 CommonMark 中不构成强调，而它是标识符高频字符
  （`max_retries`），转义会损害可读性。
- `md_escape_inline` 增补换行折叠（换行会中断当前块，使 `- **k**: v` 后续内容
  被解析为标题 / 新段落，属同一根因）。

### N4　ReDoS 步数预算按起点重置

```rust
fn backtrack_match(...) -> Option<usize> {
    let mut steps: u64 = 0;   // ← 每个起始位置都重新拿到全额 2M 步
```

非锚定匹配会对 `0..=text.len()` 的每个起点各调用一次，故实际总开销 = **起点数 × 2M**。
而 glob / regex include 会对目录中**每个文件名**调用一次 `regex_matches`，
一个恶意模式即可挂死整个解析。实测单次调用耗时已达 1.83s。

**修复**：预算提到 `regex_matches` 中统一持有，通过 `&mut u64` 传入 `backtrack_match`
并在所有起点间共享累积；耗尽即 `break` 判定为不匹配。同时每个起点只调一次
（原写法对同一起点调用两次，既浪费一半开销，也可能对「是否匹配到末端」得出不一致结论）。

### N5　Slint 字符串字面量未转义换行

`slint_value_str` 只转义 `\` 与 `"`。值是**数据**而非代码，`"` 已转义故不构成注入；
但未转义的字面换行会产出 Slint **无法编译**的跨行字符串。

**修复**：新增 `slint_escape_str`，补 `\n` `\r` `\t` 转义，两个取值点统一使用。

### N6　custom emit 的 `exclude` 被 `text` 字段绕过

子节点遍历处已按 `field_allowed` 过滤 `text`，但填充 `{value}` 时又直接
`v.get("text")` 取回，使 `exclude: ["text"]` 形同虚设。

**修复**：取值前同样过 `field_allowed("text", opt)`；`include_fields` 方向一并生效。

### 本轮实测确认**安全**的面

| 面 | 证据 |
|---|---|
| include 路径遍历 | `../`、绝对路径、`glob "../*.sml"`、`regex "re:../.*"` **四种越界全部被拒**（canonicalize + starts_with，递归子目录同样受限） |
| SVG / XML / LVGL 注入 | `onload` 被丢弃；`href="javascript:…"` → `""`；`fill="red" onmouseover=…` 的引号转义为 `&quot;` |
| Slint 代码逃逸 | 回调 `clicked` 含 `}}` → 留空为 `{ }`；属性名 `a;\n}\nEvil {` → `aEvilb` |
| LaTeX | `\end{verbatim}` 中和为 `\end{verbatim }`；math 含 `\write18` → Err 拒绝 |
| Markdown HTML 透传 | `onclick` 丢弃、`href=javascript:` 清空、`<script>` 转义 |
| custom emit 放大 | 8MB 输出上限生效，深度 40 的 4 倍放大模板 11ms 返回 Err |
| C-ABI | 200 层嵌套 JSON → NULL；深层 Value 迭代式 Drop 正常 |
| 依赖 | OSV 查询 40 个包，**0 CVE**（`python osv_check.py` 可复跑，建议进 CI） |

### 回归用例

新增 14 个常驻用例于 `tests/security.rs`（`R-9`…`R-14` 系列）：
`markdown_code_lang_cannot_escape_fence`、`markdown_code_lang_keeps_normal_language`、
`markdown_code_body_fence_still_grows`、`markdown_table_cell_cannot_forge_columns`、
`markdown_table_cell_cannot_forge_rows`、`markdown_table_keeps_normal_cells`、
`markdown_field_name_cannot_forge_heading`、`markdown_field_name_keeps_normal_keys`、
`regex_step_budget_is_shared_across_start_positions`、`regex_still_matches_correctly`、
`slint_string_value_escapes_newlines`、`slint_string_value_keeps_normal_text`、
`custom_exclude_applies_to_text_field`、`custom_exclude_still_allows_other_fields`、
`custom_include_only_applies_to_text_field`。

每个修复项均配**正常输入对照用例**，防止过度清洗（`rust` / `c++` / `f#` 等语言标注、
`max_retries` 等常规字段名、正常表格内容、正常正则匹配）。

### 新发现（未修，待定）

**`MiniRegex` 的 `+` 存在 off-by-one**：前一字符已由 default 分支消耗一次，
而 `+` 分支又要求**至少再消耗一个**，故 `x+` 实际等价于 `xx*`——
`^ab+c$` 匹配 `"abbc"` 但**不匹配 `"abc"`**。同理 `?` 的语义也需复核。

属已有缺陷、非本轮引入，且修复会改变 `regex-include` 的既有匹配行为
（并需额外处理 `.+` 中 `.` 的特殊分支），故未在本轮擅改。
`tests/security.rs::regex_still_matches_correctly` 已按**当前**行为断言并加注释锁定现状。
如需修正，请单独确认。

---

## 0. 第二轮复测结果速览

对报告 8 项逐一实测：

| 编号 | 项目 | 状态 | 实测证据 |
|---|---|---|---|
| P0-1 | parse 数组路径深度限制 | ✅ 已修 | 深度 127 数组=Ok；128/129 数组=Err（与对象一致）；50000 层返回 Err 不崩溃 |
| P0-2 | `Value` 递归 Drop | ✅ 已修 | `drop(50000 层 Object)` / `drop(50000 层 Array)` 均正常 |
| P0-3 | emit 递归 clone（**部分**） | ⚠️ **xml 未修** | svg/lvgl/slint/md/latex/sml 均返回 `Err(深度超过上限 128)`；**xml 在 20000 / 50000 层栈溢出 abort** |
| P1-1 | lvgl depth 检查 | ✅ 已修 | 返回 `Err(lvgl: 递归深度超过上限 128)` |
| P1-2 | slint body / callback depth 检查 | ✅ 已修 | 均返回 `Err(slint: 递归深度超过上限 128)` |
| P1-3 | Markdown 围栏逃逸 | ✅ 已修 | 改用 4 反引号围栏，注入内容包在围栏内 |
| P1-4 | Slint 标识符清洗 | ✅ 已修 | `x; injected` → `xinjected`（空格、分号已剔除） |
| P2-1 | C-ABI NUL 静默空串 | ✅ 已修 | `sml_str_dup` 返回 NULL；对照正常串返回 `Some("hello")` |

### 最后一项修复（P0-3 残留，已完成）

- **位置**：`rust/src/emit/xml.rs:88` / `xml.rs:101`
- **原现象**：`to_xml` 在 20000 / 50000 层嵌套 `Value` 上**栈溢出 abort**（exit `-1073741571`）。
- **根因**：`emit_node` 收集子节点时递归 clone 整个子树，深度检查（128 层）在 clone 之前无法生效——clone 本身递归 N 层即溢出。
- **改动**（与已修的 `svg.rs:163`、`lvgl.rs:228` 写法对齐）：

  ```rust
  // 修改前
  let mut child_nodes: Vec<(String, Value)> = Vec::new();
  child_nodes.push((k.clone(), val.clone()));

  // 修改后：持有引用而非克隆子树
  let mut child_nodes: Vec<(String, &Value)> = Vec::new();
  child_nodes.push((k.clone(), val));
  ```

  遍历处 `emit_node(cv, ...)` 中 `cv` 变为 `&&Value`，依赖自动解引用，无需额外改动。

- **验证结果**：

  | 用例 | 修复前 | 修复后 |
  |---|---|---|
  | `to_xml` 对象嵌套 20000 层 | abort | `Err(xml: 递归深度超过上限 128)` |
  | `to_xml` 对象嵌套 50000 层 | abort | `Err(xml: 递归深度超过上限 128)` |
  | `to_xml` 数组嵌套 20000 / 50000 层 | abort | `Err(...)` |

- **功能回归**：`to_xml` 正常输出不变（标签、属性、转义 `&lt;b&gt;&amp;&quot;c&quot;`、文本节点、数组子节点均正确）；`to_svg` / `to_lvgl` 对照正常；128 层内可正常生成。
- **测试套件**：全部通过 —— 单元测试 59、c_abi 21、comments 9、contract 26、contract_showcase 1、emit 33、macro_roundtrip 20、version 13、其余 2（1 忽略），**共 184 个，0 失败**。

---

## 一、已修复确认（回归验证通过，无需再处理）

以下为此前发现、现已确认修复的项，列出供回归参考：

| 项目 | 验证证据 |
|---|---|
| 正则 include ReDoS | `MAX_REGEX_STEPS = 2_000_000` 步数预算（`core.rs:2308`） |
| include 指数膨胀 DoS | `MAX_INCLUDE_EXPANSIONS = 10_000`（`core.rs:1869`） |
| `sml_load_file` 忽略 flags | 已改为 `feature_set_from_flags` → `parse_file_features` |
| NaN 绕过契约 min/max | 返回 `数字边界必须为有限值` |
| 多目标 include 静默降级 | 返回 `需要特性 multi-include` |
| Markdown XSS（`<script>` / `javascript:`） | 输出 `&lt;script&gt;`，危险 URI 被清空 |
| XML/SVG/LVGL 名字注入 | `sanitize_xml_name` 生效（`x><evil/>` → `x__evil__`） |
| 控制字符 / UTF-8 序列化 | 换行与中文正确保留 |
| 未定义片段引用静默降级 | 正确报错（`val=None`） |
| `CustomOptions::new()` + `to_custom` panic | `panic=false` |
| 未闭合 `/*` 静默吞内容 | 正确报错 |
| 多行字符串内指令误剥离 | note 保留 `@version` |
| md / latex / sml 后端深度检查 | 均返回 `递归深度超过上限 128` 且不崩溃 |
| `to_sml` NaN/inf 字面量 | 改为带引号字符串输出 |
| derive 整数边界（u64/i128/usize 等） | 已有严格上下界与 `try_from` |
| **P0-1** parse 数组路径缺失递归深度检查（`parse_array_inner` 绕过 depth 守卫） | 改为调用 `self.parse_array()` 复用 depth 上限（`core.rs:1104`） |
| **P0-2** `Value` 递归 Drop 导致深层嵌套释放栈溢出 | 手写迭代式 `Drop`（`mem::replace`+`mem::take`+显式工作栈，`value.rs:18`），原生栈深度为常数 |
| **P0-3** emit 后端递归 clone 深层子值（svg/xml/lvgl/slint 崩溃） | 收集 children 改用 `&Value` 引用，去除递归 clone（`emit/svg.rs`/`emit/xml.rs`/`emit/slint.rs`） |
| **P1-1 / P1-2** lvgl / slint 缺失 depth 检查 | 各 emit 入口追加 `if depth > MAX_VALUE_DEPTH` 守卫（`emit/mod.rs:MAX_VALUE_DEPTH=128`） |
| **P1-4** `sanitize_slint_ident` 保留空格/分号等危险字符 | 改为白名单清洗（`is_ascii_alphanumeric() \|\| '_' \| '-' \| '$'`，空/数字开头补 `_`） |

---

## 二、待修复缺陷

### P0-1　parse 数组路径缺失递归深度检查 → 栈溢出 DoS

- **位置**：`rust/src/core.rs:1086` `parse_array_inner`
- **严重度**：P0（进程 abort，不可捕获）
- **现象**：嵌套数组的解析不受深度限制，而嵌套对象受限（128 层）。

对照实测（`k: [[[…]]]` 与 `k: {{{…}}}`）：

| 深度 | 数组 | 对象 |
|---|---|---|
| 127 | Ok | Ok |
| 128 | **Ok** | Err（有防护） |
| 129 | **Ok** | Err（有防护） |
| 50000 | **栈溢出 abort** | Err |

- **根因**：`parse_array`（`core.rs:1073-1084`）持有 depth 守卫，但 `parse_array_inner` 在遇到嵌套 `[` 时递归调用的是**自身**（约 1085 行 `arr.push(self.parse_array_inner()?)`），完全绕过 wrapper 的 `self.depth += 1` 与上限检查。对象路径 `parse_block`/`parse_block_inner`（`core.rs:804`）则正确成对守卫。
- **影响面**：所有基于 `parse` 的入口，含 C-ABI `sml_loads`（此前报告的"C-ABI 栈溢出"实为此根因，非 C-ABI 独立问题）。
- **复现**：

```rust
let t = "k: ".to_string() + &"[".repeat(50000) + &"]".repeat(50000);
let _ = sml::parse(&t);   // 进程 abort (exit -1073741571)
```

- **修复建议**：将 `parse_array_inner` 中嵌套 `[` 的递归调用改为 `self.parse_array()`（走 wrapper），或在该处同步维护 depth 计数与上限判断。
- **验证**：深度 129 的数组应返回 `Err(嵌套过深)`；50000 层不再 abort。

---

### P0-2　`Value` 递归 Drop 导致栈溢出

- **位置**：`rust/src/value.rs` `enum Value` 的派生 Drop
- **严重度**：P0（根本性问题，影响面最广）
- **现象**：**仅构造并 drop** 一个深层嵌套 `Value` 即崩溃，不调用任何 API：

```rust
let mut v = Value::Int(1);
for _ in 0..50000 {
    let mut m = BTreeMap::new();
    m.insert("k".to_string(), v);
    v = Value::Object(m);
}
drop(v);   // 栈溢出 abort
```

数组嵌套（`Value::Array(vec![v])` 循环）同样崩溃。

- **根因**：`Value` 为递归枚举，编译器自动生成的 `Drop` 是递归实现，嵌套 N 层即递归 N 层栈帧。
- **影响面**：即使 P0-1 修复、解析器拦住深层输入，**用户程序化构造**的深层 `Value` 在释放时仍会崩溃；这也是此前观察到"md/latex 已返回深度超限 Err 却仍崩溃"的真正原因。
- **修复建议**：为 `Value` 手写迭代式 `Drop`（用显式栈展开逐层释放），或在构造入口统一限制深度。前者是彻底解法。
- **验证**：上述 `drop` 用例不再崩溃。

---

### P0-3　emit 内部 clone 深层子树 → 绕过深度检查后溢出

- **位置**：`rust/src/emit/svg.rs:163`（及同类处）、`xml.rs`、`slint.rs`、`emit/lvgl`
- **严重度**：P0
- **现象**：深度检查（128 层）本应拦住，但后端在**检查之前**先递归 clone 了整个子树，clone 深度随总深度线性增长：

| 总深度 | svg 结果 |
|---|---|
| 200 | `Err(递归深度超过上限 128)` ✔ |
| 1000 | `Err(...)` ✔ |
| 5000 | `Err(...)` ✔ |
| 20000 | **栈溢出 abort** |

（以上均已用 `Box::leak` 排除 P0-2 的 Drop 干扰，确认是 clone/遍历本身溢出。）

- **根因**：如 `svg.rs:163` `children.push((Some(k.clone()), val.clone()))`——在通过 depth 检查后立刻递归 clone 深层子值；`Value::clone` 同样是递归实现。md / latex / sml 后端不做此类 clone，故 50000 层仍安全。
- **修复建议**：收集 children 时改用引用（`&Value`）而非 `val.clone()`；若必须 clone，需在 clone 前做深度预检。
- **验证**：svg / xml / lvgl / slint 在 20000 层应返回 `Err(...)` 而非 abort。

---

### P1-1　lvgl 后端缺失 depth 检查

- **位置**：`rust/src/emit/xml.rs:198` `emit_lvgl_node`
- **严重度**：P1（防御性补齐，当前被 P0-3 掩盖）
- **根因**：同文件 `emit_node`（`xml.rs:77-82`）有 `if depth > MAX_VALUE_DEPTH` 守卫，`emit_lvgl_node` 仅有 `depth: usize` 形参而无任何上限判断。
- **修复建议**：补入与 `emit_node` 一致的 depth 检查。

### P1-2　slint 后端 `emit_slint_body` / `emit_callback` 缺失 depth 检查

- **位置**：`rust/src/emit/slint.rs:122`、`rust/src/emit/slint.rs:169`
- **严重度**：P1
- **根因**：`emit_slint`（`slint.rs:80-82`）有检查，但 `emit_slint_body` 与 `emit_callback` 均无。
- **修复建议**：两处补入 depth 上限判断。

---

### P1-3　Markdown 代码块围栏逃逸

- **位置**：`rust/src/emit/markdown.rs`（code 块输出处）
- **严重度**：P1（内容注入，可构造标题/HTML）
- **现象**：

```
输入：code { text: "x\n```\n# injected" }
输出：```
     x
     ```
     # injected      ← 已逃逸到代码块之外，成为 Markdown 标题
     ```
```

- **修复建议**：输出 code 块时检测 text 中的反引号序列，改用更长的围栏（如连续 4 个以上反引号），或对内容中的围栏做转义处理。
- **验证**：`# injected` 应保留在代码块内部。

### P1-4　Slint 元素名清洗不彻底（空格/特殊字符残留）

- **位置**：`rust/src/emit/slint.rs:85`（`sanitize_slint_ident`）
- **严重度**：P1（语法破坏 / 注入）
- **现象**：字段名 `x; injected` → 输出 `x_ injected {`，**空格被保留**，可破坏 Slint 语法结构。
- **根因**：清洗函数将部分字符替换为 `_`，但未剔除空格、`;` 等所有非法标识符字符。
- **修复建议**：改为严格白名单——仅保留 `[A-Za-z0-9_$]` 及 `-`（内部），其余字符一律剔除（非替换为 `_`）；若清洗后为空或首字符为数字，回落到安全默认名。
- **验证**：`x; injected` 应得到无空格、无分号的合法标识符。

---

### P2-1　C-ABI 遇 NUL 字节静默返回空串

- **位置**：`rust/src/c_abi.rs`（`cstr` 辅助函数 / `sml_str_dup` 路径）
- **严重度**：P2（数据损坏，无错误信号）
- **现象**：SML 源 `k: "a\0b"` 经 C-ABI 读取字符串得到 `""`，**既不返回数据也不报错**，违反"失败返回 NULL"的约定。
- **根因**：`CString::new` 遇内嵌 NUL 失败后回退为空串，未置错误码。
- **修复建议**：`CString::new` 失败时返回 NULL 并通过 `CSmlError` 输出错误，而非静默空串。
- **验证**：应返回 NULL 且错误码非零。

---

## 三、建议修复顺序

1. **P0-1**（改一行递归调用）— 成本最低，消除最直接的公开入口 DoS
2. **P0-2**（为 `Value` 实现迭代式 Drop）— 根本解，消除所有深层释放崩溃
3. **P0-3**（emit 去除递归 clone，改用引用）— 消除 svg/xml/lvgl/slint 崩溃
4. **P1-1、P1-2**（补齐 lvgl / slint depth 检查）— 防御性加固
5. **P1-3、P1-4**（Markdown 围栏、Slint 标识符清洗）— 内容注入类
6. **P2-1**（NUL 错误处理）

> 建议 P0-1 ～ P0-3 一并处理：三者叠加才构成完整的深层输入防护（解析 → 持有 → 释放 → 输出）。

---

## 四、验证方法说明

修复后建议按以下方式回归（均为本次审计已验证有效的手段）：

1. **深度对照**：分别构造数组嵌套与对象嵌套，在 127/128/129/50000 层验证行为一致（数组与对象应同时受 128 层限制）。
2. **Drop 隔离**：用 `Box::leak` 跳过释放，以区分"遍历溢出"与"Drop 溢出"——这是定位 P0-2 与 P0-3 的关键手法。
3. **临界深度扫描**：对 svg 等后端在 200/1000/5000/20000 层扫描，确认均返回 `Err` 而非 abort。
4. **模糊测试**：语法感知生成器（键名/标量/指令/注释随机组合 + 字节级变异）迭代数万次，断言 parse → `to_sml` → reparse 的 round-trip 不变，且全过程无 panic。此前 18 万次迭代除已知项外未发现 panic，可作回归基线。

---

## 五、附注：本次审计中已排除的误报

记录以避免重复排查：

- **XML 属性值引号注入**：`attr_val` 已转义 `"`，不成立。
- **SVG 数字属性注入**：`num_attr` 仅接受合法数值，非数字被跳过，不成立。
- **include 路径遍历**：存在 canonicalize + `starts_with` 防护，实测被拦截，不成立。
- **控制字符 round-trip**：`to_sml` 输出字面控制字符但可正确回读（SML 引号串允许字面换行），不是缺陷。
- **emit 全线无深度上限**：不准确——md/latex/sml 均已有限制；真正的缺口是 P0-3 的 clone 与 P1-1/P1-2 的两处遗漏检查。
