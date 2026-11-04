# weakbind 规范草案 v0.1（**语义未闭合 · 零实现**）

> 状态横幅：**第一刀已落地（Rust 单端）** —— 语法 / 作用域 / 旁路表 / `E-PARSE-029` 已实现并有用例；
> 内置表、`W-CONTRACT-001` 软校验、`--dump-builtins`、编辑器提示**均未实现**（见 §10）。
> `IDEAS.md` I-5 的定位与沿革以本文件为准。
> 决定来源：2026-11-04 与用户逐条确认（命名 `weakbind`、库可读取、warning 报在库里、
> 多端同步属发布前必做项、编辑器提示是主需求）。本文件只做归纳，不新增决定。
>
> ⚠️ 三条已定的**前提**，别再推翻：
> 1. `weak*` 一族**主要给编辑器提示**用，不是运行期契约豁免；库侧不参与解释是**目的**，不是副作用。
> 2. 多端同步**不是当前阻塞**，是发布前必做项 ⇒ 当前 **Rust 单端先行**（`E-CONTRACT-017` 那套模式）。
> 3. 同步未到位前，`@weakbind` **不得进共享语料**（`site/content`、`site/data/sml-lessons*`、
>    `examples/`、`tests/conformance/`）。

---

## 0. 一句话

**`weakbind` = 给编辑器看的一句「这块按内置契约 X 写」，库照旧不校验这块，但要能读出来、
并在与内置表对不上时给 warning。**

它解决的具体问题是**重复声明**：当契约集是**内置在宿主/库里**的时候，作者若用 `@contract`
在文档里再写一遍，等于同一个契约维护两份，且库会把文档那份当约束再校验一次。
`weakbind` 让文档那份只服务编辑器（补全 / 悬停 / 诊断），库**不重复消费**。

## 1. 目标 / 非目标

**目标**

1. 块内补全、悬停、诊断照常工作 —— **这是主目标**；
2. 「内置契约」只维护一份（在内置表里），文档侧只写**引用**，不写形状；
3. 库**能读到**弱绑定（可枚举、可获取），供 CLI / 编辑器 / 其它消费者使用；
4. 声明与内置表不符、**以及块内容与内置契约不符**，都给 warning ——
   不静默、也不升级为 error（**库侧软校验**，2026-11-04 用户裁决：做）。

**软校验的硬约束（不遵守就破判据 1）**

- **只读**：不填默认值、不删不改任何键，主树一个字节都不动；
- 只出诊断，不改变 `ParseOutput.value`；
- 内置表为空 ⇒ 整个软校验跳过（§4 空表护栏）。

**非目标（明确不做）**

| 不做 | 理由 |
|---|---|
| 用「应用契约」的完整路径去处理弱绑定 | 那会填默认值、做叠加、走 `strict_paths` ⇒ 改数据，等于 `@is` 改名。软校验只借必填 / 类型 / 区间 / 枚举这几档**只读**检查 |
| 用 `weakbind` 表达「跳过校验的 `@is`」 | 用户 2026-11-04 澄清：插件侧**仍然当正常 contract** 处理，削弱的是**库**的参与度，不是校验强度 |
| 允许弱绑定本身叠加/重复 | 与 `weakdoc` 的约定一致：同一个不能重复挂 |
| 把形状写进文档（强制字段体） | 那就是「写两遍」，与本项动机冲突 |

## 2. 语法

### 2.0 命名分层（三层别混）

| 层 | 名字 | 长什么样 | 落地成本 |
|---|---|---|---|
| 族名（**非语法**，只是称呼） | `weak*` | 文档里的统称 | 0 |
| **特性位（feature）** | **`type.weak`** | `@feature enable type.weak` | `FEATURES` 表加一行（`sml-feature/src/lib.rs:161`）+ 五端 + 编辑器 `FEATURE_NAMES`（`extension.js:318`） |
| **指令 / 关键字（keyword）** | `@weakbind` | `@weakbind tls` | 加进 `BUILTIN_DIRECTIVES`（`sml-parse/src/ext.rs:40`，现有 7 个：`contract` / `is` / `type` / `version` / `feature` / `when` / `for`） |

- `type.weak` 是**开关**：决定这族机制认不认；
- `weakbind` 是**语法**：决定文档里怎么写；
- 两者不在同一层，不冲突，也不互相替代。

两条实现注意：

1. **位序只能追加**：`FEATURES` 的顺序即 bit 序，`weak` 加在末尾（`lex.ident` 之后），不许插到中间。
2. **加进内置指令名单会挡掉一种既有写法**：`@when` 当初为此加了 `is_fragment_form` 判断
   （`parser.rs:1348–1353`），以免拦截「定义名为 `when` 的片段」。`weakbind` 是否也给同样的豁免，
   见 §7 待定。



```sml
@weakbind tls                      # 只指名字：形状全由内置表给 —— 真正"不写两遍"的形态
web { host: "a.com" port: 443 }

@weakbind tls { host: str }        # 可选字段体：写了才做字段级比对
web { host: "a.com" port: 443 }
```

- **形态**：`@weakbind <契约名> [<字段体>]`；字段体就是普通块，语法与 `@contract` 体一致。
- **门控**：必须 `@feature enable type.weak`（opt-in，与 `weak*` 整族一致）。未开 ⇒
  `E-FEATURE-001` + detail（照 `@when` 的做法，`parser.rs` 的 L1354–1356）。
  ⚠️ **名字必须带域**：`docs/feature-namespace-plan.md` §3 规则 1 —— 新增特性一律
  `<域>.<名>`，老名保持平铺。故是 `type.weak`，**不是**裸 `weak`（裸 `weak` 报 `E-FEATURE-003`）。
- ⚠️ **与 IDEAS 原设想的偏离**：原文写「落在外置指令侧最省事」。既然库要做对账、且五端必须同形，
  **建议做成内置指令**（与 `@is` / `@when` 同级）；外置指令留给宿主自定义扩展。

## 3. 作用域（照 `@when` 的先例）

- 只作用于**紧邻的下一个块**；
- 悬空（后面没有块）、连续出现两次、作用对象不是块 ⇒ 一律报错，**不静默**。

先例可抄：

| 行为 | `@when` 现有实现 |
|---|---|
| 记挂起状态 | `parser.rs` 的 `pending_when`（约 L1346–1364） |
| 连续出现 | `E-PARSE-016` |
| 悬空检查 | `finish_block` 里 `pending_when.is_some()` ⇒ `E-PARSE-017`（约 L1839–1849） |

`weakbind` 应新增 `pending_weakbind`，规则同上；码见 §4。

## 4. 码

| 码 | 级别 | 触发 | 归属 |
|---|---|---|---|
| `E-PARSE-029` | E | 弱绑定作用域非法（悬空 / 连续 / 对象不是块），三档靠 detail 区分 | 库 |
| `W-CONTRACT-001` | W | 弱声明与内置契约不匹配，三档靠 detail 区分 | 库（**不阻断解析**） |
| `E-FEATURE-001` | E | 未开 `weak` 特性（既有码，加 detail） | 库 |

`W-CONTRACT-001` 的 detail（**一个码、两族 detail** —— 沿用 `E-LEX-007` 的同因同码做法，
免得五端各记一遍文案）：

| detail | 检查对象 | 触发 |
|---|---|---|
| `decl.unknown_contract` | 声明 | 契约名不在内置表（含拼错） |
| `decl.unknown_field` | 声明体 | 字段体里的字段名不在内置契约 |
| `decl.type_mismatch` | 声明体 | 字段体里该字段类型与内置契约不同 |
| `block.missing_required` | 块内容 | 缺内置契约的 `required` 字段 |
| `block.type_mismatch` | 块内容 | 值的类型与内置契约不符 |
| `block.out_of_range` | 块内容 | 数值越 `min` / `max` |
| `block.enum_mismatch` | 块内容 | 值不在 `enum` 列表内 |
| `block.undeclared_field` | 块内容 | 出现了内置契约未声明的字段 —— **仅当该契约 `strictness = strict` 时报**；`warn` / `loose` 不报 |

- 块的**内容**能不能比，取决于契约名先在 `decl` 这一档对上；`decl.unknown_contract` 时
  不做块级检查（没有可比的契约）。

**不报**：内置契约有 `required` 字段而字段体没写 —— 体允许只描述关心的字段，
否则 `@weakbind tls`（连体都没有）第一条就违规，自相矛盾。

✅ 落地配套（已核）：`W-LINT-002`「契约定义从未被应用」**不会**误报弱声明
（弱绑定不是契约定义），钉子见 `smltools/tests/lint_weakbind.rs`。

✅ **实现（2026-11-04）**：八档全部落地。`Diagnostic` 没有 `detail` 字段 ⇒ 档位写在
**消息前缀**里：`sml: 弱声明与内置契约不匹配（decl.unknown_contract）：…`；
块内容的档位 `line` 为 `None`（块收尾没有单一源位置，宁缺勿错）。

⚠️ 空表护栏：**内置表为空时跳过全部对账**（否则一落地，所有名字都报 `unknown_contract`）。

## 5. 树形与旁路表

**主树不动**。弱绑定记录走**旁路表**，与 `ParseOutput.contract_map` 同类：

```rust
pub struct WeakBind {
    /// 被弱绑定的块在树里的位置（与 contract_map 的 key 同一套表达）
    pub path: Vec<String>,
    /// 契约名
    pub name: String,
    /// 可选字段体（未写体时为 None）
    pub body: Option<Vec<(String, TypeSpec)>>,
    /// 该块在原文里的行号（诊断用；拿不到时 None，与 Diagnostic 的取舍一致）
    pub line: Option<usize>,
}
// ParseOutput 新增字段
pub weak_binds: Vec<WeakBind>,
```

- **为什么不写进树**：写 `__weak` 会污染数据、且破坏「有没有 `weakbind`，主树完全一致」这条判据。
- ⚠️ `ParseOutput` 是公开结构体，加字段属破坏性变更 ⇒ 按仓库惯例记入 CHANGELOG「升级注意」。

**核心判据（可测）**：同一份文档，去掉全部 `@weakbind` 后，
**主树与 `contract_map` 必须逐字节相同**，差异只允许出现在 `weak_binds` 与诊断里。

## 6. 内置表与导出格式

### 6.1 内置表从哪来

仓库**目前没有**内置契约表（已核：全部契约来自文档解析，`*builtin*` 零命中）。

**结论（2026-11-04 按建议定）：库里不内置任何业务契约。**
「内置表」= **宿主注入的契约集**，库只提供两件事：**对账**（§4 的 warning）与**导出**。

```
宿主（smltools / 站点 / 编辑器桥）  ──注入──▶  ParseOptions.builtin_contracts
                                          │
        ┌─────────────────────────────────┴──────────────────┐
        ▼                                                    ▼
   弱绑定对账 → W-CONTRACT-001                    smltools -i builtin.sml --dump-builtins
                                                  → builtins.json（扩展打包时带上）
```

- **为什么库里不放内容**：与既有「外置类型」的哲学一致 ——
  `TypeSpec::Ext` 的注释原话是「名字与校验规则都留在下游仓库，**规范层不认识它**」。
  若把 `Service` / `User` 这类名字硬编进库，等于让规范层伪装成知道业务。
- **实测佐证**：语料里出现过的契约名共 29 个、118 次，全是教程示例
  （`Service` 24 · `Server` 12 · `Endpoint` 12 · `User` 8 · `ResenderConfig` 7 …）。
  这些一旦进了"内置表"，读者会误以为它们是官方内置契约 —— 必须避免。
- ✅ **读回已打通**：`ParseOptions::capture_contracts(true)` ⇒ `ParseOutput::contracts`
  （3 条用例，`rust/tests/builtin_contracts.rs`）。注入侧（`builtin_contracts`）与导出
  （`--dump-builtins`）仍未做 —— 这两步才是「内置表」真正完成。
- **文件格式**：内置集用 **SML** 写（与文档同语法、同一套校验），由 CLI 读入再注入；
  理由：`Contract` 含 `Arc<dyn TypeCheck / Modifier>`，**无法从 JSON 反序列化**。
- ⚠️ **落点限制**：解析 `builtin.sml` 的代码不能放 `sml-contract`（它不能依赖 `sml-parse`，
  见 `lib.rs` 关于反向依赖的注释），要放 `sml-parse` 或更上层 `sml` crate。

### 6.2 导出格式（草案）

```json
{
  "schema": 1,
  "source": "builtin.sml（宿主注入；CLI 用 `--builtin <path>` 指定）",
  "contracts": [
    {
      "name": "tls",
      "strictness": "strict",
      "requires": [],
      "allow_any": false,
      "strict_paths": false,
      "featags": [],
      "fields": [
        { "name": "host", "ty": { "kind": "str" }, "required": true,  "default": null,
          "min": null, "max": null, "from_featag": false, "ext_mods": [] },
        { "name": "port", "ty": { "kind": "int" }, "required": true,  "default": null,
          "min": 1, "max": 65535, "from_featag": false, "ext_mods": [] },
        { "name": "sni",  "ty": { "kind": "str" }, "required": false, "default": null,
          "min": null, "max": null, "from_featag": false, "ext_mods": [] }
      ]
    }
  ]
}
```

> **全量导出**（2026-11-04 定）：`strictness` / `requires` / `allow_any` / `strict_paths` /
> `featags` / `from_featag` / `ext_mods` 一并带上。编辑器当前用不到，但少一个字段就多一轮
> schema 变更（扩展要与库版本对齐），一次导全更省事。

`ty.kind` 取值（对应 `TypeSpec`）：

| kind | 附加字段 | 说明 |
|---|---|---|
| `any` / `str` / `int` / `num` / `bool` | — | 标量 |
| `array` | `of` | 元素类型，递归同构 |
| `enum` | `values` | 字符串数组 |
| `contract` | `name` | 契约引用（组合） |
| `pattern` | `name`, `pattern` | `@type` 声明的模式；`pattern` 是源串 |
| `ext` | `name` | 外置类型（下游注册，扩展侧只做提示、不校验） |

## 7. 已定 / 待定

**已定（2026-11-04）**

| # | 事项 | 结论 |
|---|---|---|
| 1 | 库对**块内容**做不做软校验 | **做** —— 只读、只出 warning、不填默认值（见 §1 硬约束） |
| 2 | 内置表内容 | **库里零内容**，宿主注入（§6.1） |
| 3 | 导出格式 | **全量字段**（§6.2） |

**待定**

| # | 事项 | 我的建议 |
|---|---|---|
| 1 | `@feature` 位叫 `weak` 还是 `weakbind` | `weak` —— 理由见 §7.1 |
| 2 | `@weakbind { }` / `@weakbind type: X { }` 这种「定义名为 `weakbind` 的片段」要不要留豁免（照 `@when` 的 `is_fragment_form`） | **不留** —— 常见度极低，留了会让「未注册指令」的判定多一个洞 |

### 7.1 为什么特性位叫 `type.weak`（而不是裸 `weak`）

**支持的理由**

1. **特性位是机制级的，不是指令级的**。现有位全是这一类：`typed-block` / `block-name` /
   `contract` / `when` / `type.featag`。`weak` 开启的是「弱声明这一族机制」，
   而不是「某一个关键字」。
2. **一族本来就要一起开**。三个成员的共同点只有一个 —— 库不参与、交编辑器/宿主。
   不存在「只开 bind 不开 data」的合理需求，一个位省掉将来
   `@feature enable weakbind,weakdata,weakdoc` 的三连写。
3. **同步成本**。每加一个特性名都要改五端 + 编辑器 `FEATURE_NAMES`
   （`editors/vscode/src/extension.js:318`）+ 文档；一个位顶三个。
4. 将来真需要细粒度，照 `type.featag` 的点号风格加 `weak.doc` 即可，不与 `weak` 冲突。

**反对的理由（也说清）**

- 单指令位的好处是「出问题能单独关」，共用一个位就关不掉单个；
- `weakdoc` 还要引入 `.smlex` + install 机制，行为差异比另外两个大 ——
  若担心被它拖累，`weak` 可以先只覆盖 `weakbind` / `weakdata`，`weakdoc` 落地时再要自己的位。

**命名不是偏好，是仓库已定的规则**：`docs/feature-namespace-plan.md` §3 规则 1 写死
「**只有新增特性带域**，老名保持平铺」⇒ 新增的只能是 `type.weak`，裸 `weak` 会报 `E-FEATURE-003`。
（§7.1 上半段的四条理由解释的是「为什么是**一个**特性位」，不是「为什么可以平铺」。）

**结论**：v1 用 `type.weak`；`weakdoc` 落地时若确认需要独立门控，再加 `type.weak.doc`？——
不行，`parse_name` 限两段，故届时是 `type.weakdoc` 之类的新位。

## 8. 多端同步清单（发布前必做，**不是当前阻塞**）

| 层 | 端 | 要做的三件事 |
|---|---|---|
| 语言层 | `rust` / `js` / `c` / `cpp` / `lua` | ① 识别 `@weakbind`（不报未注册指令）② 同码 warning ③ 有 `ParseOutput` 概念的端加同名旁路表 |
| 工具层 | `smltools` | `--dump-builtins --to json`；lint 放过弱声明 |
| 编辑器层 | `editors/vscode` / `editors/lsp` / `editors/zed` | `collectContractNames` 加 `@weakbind` 支路；读 `builtins.json` 做补全/悬停 |
| 站点 | `site/static/sml.mjs`（构建期同步自 js） | 随 js 自动同步 |
| 绑定层 | `bindings/{csharp,go,java,python,ruby}` | 随所绑定的实现走；需确认各自暴露的解析结果结构 |

## 9. 用例清单（Rust 单端先行时就要有）

| # | 用例 | 期望 |
|---|---|---|
| 1 | 有 / 无 `@weakbind` 的同一文档 | 主树 + `contract_map` 逐字节相同（**核心判据**） |
| 2 | 名字 + 空表 | 零 warning（空表护栏） |
| 3 | 名字不在内置表 | `W-CONTRACT-001` / `decl.unknown_contract` |
| 4 | 体里字段名不在内置契约 | `decl.unknown_field` |
| 5 | 体里类型不符 | `decl.type_mismatch` |
| 6 | 体缺 `required` 字段 | 零 warning（体是部分描述） |
| 7 | 块内容缺 `required` 字段 | `block.missing_required`，**且树不变** |
| 8 | 块内容类型不符 / 越界 / 枚举外 | `block.type_mismatch` / `block.out_of_range` / `block.enum_mismatch` |
| 9 | 块内有未声明字段，契约 `strictness` 分别为 strict / warn / loose | 仅 strict 报 `block.undeclared_field` |
| 10 | 块内容缺字段但契约有 `default` | **不填默认值**，只报 warning（判定「只读」是否守住） |
| 11 | 悬空 / 连续 / 对象不是块 | `E-PARSE-029` 三档 detail |
| 12 | 未开 `type.weak` 特性 | `E-FEATURE-001` + detail |
| 13 | 弱声明 + lint | `W-LINT-002` 不报 |
| 14 | `weak_binds` 内容 | 路径 / 契约名 / 行号正确 |

## 10. 实现进度（Rust 单端第一刀，2026-11-04）

| 项 | 状态 | 落点 |
|---|---|---|
| 特性位 `type.weak` | ✅ | `sml-feature/src/lib.rs`（`Feature::Weak` + `FEATURES` 末尾，bit 19） |
| 内置指令登记 | ✅ | `sml-parse/src/ext.rs` 的 `BUILTIN_DIRECTIVES` 加 `weakbind` |
| 语法 `@weakbind <名> [<体>]` | ✅ | `parser.rs::parse_at_directive` |
| 作用域（紧邻下一个块） | ✅ | `BlockFrame.pending_weakbind` + `value_step` |
| 码 `E-PARSE-029` | ✅ | `errors/codes.sml`（`impls: [rust]` / `status: partial`）；`W-CONTRACT-001` **未加** |
| 旁路表 `weak_binds` | ✅ | `ParseOutput.weak_binds` —— **恒收集的 `Vec`**，不是 `Option`（空 vec 与「没写过」对消费者等价） |
| 用例 | ✅ 8 条 | `rust/tests/weakbind.rs`（含核心判据的差分用例） |
| C-ABI 位名 | ✅ | `rust/src/c_abi.rs::sml_feature_name` bit 19 + `c/sml_rs.h` 的 `SML_F_WEAK` |
| 契约表回传 `capture_contracts` + `ParseOutput.contracts` | ✅ | 内置表可以「用 SML 写、再读回」（§6.1），3 条用例 |
| 内置表**注入** `ParseOptions.builtin_contracts` | ✅ | 空表 ⇒ 整段软校验跳过 |
| 导出 `smltools -i builtin.sml --dump-builtins [-o out.json]` | ✅ | 输出即 §6.2 的 schema（JSON 固定，不走 `--to`），4 条端到端用例 |
| `W-CONTRACT-001` 软校验 | ✅ | 八档（decl.* 3 + block.* 5），**只读**：不填默认值、不改树；10 条用例在 `rust/tests/weakbind_warn.rs` |
| 扩展提示（vscode / lsp / zed） | ⬜ | 属发布前同步那批（§8） |
| 其余四端 + smltools + 绑定层 | ⬜ | 同上 |

**已知边界（第一刀刻意没做，写下来免得被当成 bug）**

1. `@for` 子解析器里写 `@weakbind`：记录**不回传**父级 —— 与 `diags` / `contract_map`
   的既有取舍一致（回传要跨 `&mut` 借用）。
2. 被 `@when` 判假丢弃的块，若其上挂了 `@weakbind`，旁路表**仍会记一条**
   （记录发生在开帧时，早于 `@when` 的保留/丢弃裁决）。属边角，等有真实用例再收。
3. 片段引用（`&frag`）、标量、数组当目标 ⇒ 一律 `E-PARSE-029`，不静默。
