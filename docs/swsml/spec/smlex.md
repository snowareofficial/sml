# `.smlex` 内容格式 v1.0（**格式已定稿 · 解释器未实现**）

> 状态横幅：**格式定稿**（2026-11-04）。三条待定已全部裁决，见 §8。
> 已实现：`smltools --resolve-profiles` 定位文件（`docs/swsml/spec/weakdoc.md` §3）、
> `rust/smltools/profile.sml` 导出形状（§9）、**`checks` 三项执行**（`--lint`，§10）。
> **未实现**：`applies_to` 匹配、`hints` 接编辑器、`emit` 接生成器。
> 决定来源：2026-11-04 与用户确认「先把 `.smlex` 的内容格式定下来」。

---

## 0. 一句话

**`.smlex` 就是一份普通 SML 文档**，顶层字段固定为 5 类；解释权在**工具侧**，库不认识它。

```sml
name: api-docs
version: 1
description: "API 参考文档的特化支持"

applies_to { glob: ["docs/api/**/*.sml"] }
checks     { require_top_level: [title version] }
hints      { keys: [title authors date] }
emit       { html { toc_depth: 3 } }
```

## 1. 为什么是 SML，而不是 JSON / YAML / TOML

1. **自举**：同一套解析器、同一套错误码、同一套编辑器高亮 —— 不为 profile 再造一种格式；
2. **可用现有机制校验**：profile 的形状可以写成一份 `@contract profile`，
   配合 `smltools --dump-builtins` 直接喂给编辑器做补全（`docs/swsml/spec/weakbind.md` §6）；
3. **可直接当测试语料**：`.smlex` 与普通文档同构，仓里所有解析器测试手法都能复用。

## 2. 顶层字段（**必填 2 个，其余可选**）

| 字段 | 类型 | 必填 | 谁消费 | 语义 |
|---|---|---|---|---|
| `name` | `str` | ✅ | 工具侧 | 必须与**文件名 stem 一致**（不一致 ⇒ 工具侧报错，避免"拷错文件"） |
| `version` | `int` | ✅ | 工具侧 | **profile 格式版本**，当前 `1` |
| `description` | `str` | — | 人 | 一句话说明，编辑器悬停时显示 |
| `applies_to` | 块 | — | 工具侧 | 什么样的文档该用它（见 §3.1） |
| `checks` | 块 | — | lint / 工具 | **只读**检查清单（见 §3.2） |
| `hints` | 块 | — | 编辑器 | 补全 / 悬停用的词表（见 §3.3） |
| `emit` | 块 | — | 生成器 | 按后端分块的生成参数（见 §3.4） |

**未知顶层字段一律 warning + 忽略**（不是 error）—— 见 §5。

## 3. 四个子块

### 3.1 `applies_to` —— 适用范围（工具据此建议启用）

```sml
applies_to {
    glob: ["docs/api/**/*.sml"]      # 路径匹配
    requires: [Endpoint]             # 文档里已定义/已应用这些契约才匹配
}
```

- 纯**匹配条件**，不改变任何解析行为；
- 工具可以据此提示「这份文档看起来该挂 `api-docs`」，但**不自动挂**（挂不挂写在文档的 `@weakdoc` 里）。

### 3.2 `checks` —— 只读检查清单

```sml
checks {
    require_top_level: [title version]   # 顶层必须有这些键
    forbid_contracts: [Draft]            # 文档里不许出现这些契约
    forbid_keys: [tmp debug]             # 这些键名不许出现（任意层级）
}
```

- **只读**：只产出诊断，**绝不修改文档**（这是「不能注入」在 profile 侧的落点）；
- 每项都是**声明式清单**，执行交给 lint —— 不引入脚本、不引入表达式求值；
- v1 只定这三项；新增项走"未知项 warning"路径，等真需要再加。

### 3.3 `hints` —— 编辑器词表

```sml
hints {
    keys: [title authors date]       # 顶层键的补全候选
    values {                         # 指定键的值候选
        layout: [post, page]
    }
}
```

- 只影响**编辑器**，与解析结果无关；
- 与 `weakbind` 的分工：`weakbind` 给的是**契约形状**（类型/必填），`hints` 给的是**自由键名的候选**。

### 3.4 `emit` —— 生成器参数（按后端分块）

```sml
emit {
    html { toc_depth: 3 }
    latex { paper: a4 }
}
```

- 块名 = 后端名（与 `smltools --to` 的取值同名）；
- 工具不认识的块 ⇒ warning + 忽略（同一份 profile 可能服务多个工具）。

## 4. 三条不变量

1. **不能注入**：profile 的任何字段都不得影响 `ParseOutput.value` ——
   判据仍是「有 / 无 `.smlex`，解析结果逐字节相同」；
2. **不认识就忽略**（前向兼容）：未知字段 / 未知 `checks` 项 / 未知 `emit` 后端 ⇒ warning，不报错；
3. **name == 文件名**：工具侧校验，不一致直接报错（防止拷贝后忘记改名）。

## 5. 版本与兼容

- `version: 1` 是当前唯一版本；将来不兼容的改动抬到 `2`，工具按版本分派；
- **新增可选字段不抬版本**（靠不变量 2 保证老工具能读）；
- 老工具遇到 `version > 1` ⇒ **warning 并按最高已知版本尽力解释**（不拒绝，profile 是增强项）。

## 6. 多个 profile 同时挂（I-17 待定项的落地）

| 块 | 合并规则 |
|---|---|
| `checks` | **并集**（越多越严；同一项重复出现去重） |
| `hints` | **并集**（`keys` 去重；同名 `values` 的候选合并） |
| `emit` | 按后端合并；**同名参数取高优先级**（显式 `--profile-dir` > 项目 > 用户 > 系统；同层按 `@weakdoc` 出现顺序，后者胜）并出 warning |
| `name` / `version` / `description` | 不合并（各自独立） |

⚠️ 冲突**不阻断**：profile 是增强项，冲突只出 warning（与「不认识就忽略」同一取舍）。

## 7. 完整示例

```sml
# .smlex/profiles/api-docs.smlex
name: api-docs
version: 1
description: "API 参考文档：必须有 title，禁止 Draft 契约"

applies_to {
    glob: ["docs/api/**/*.sml"]
}

checks {
    require_top_level: [title version]
    forbid_contracts: [Draft]
}

hints {
    keys: [title version authors]
    values { layout: [post, page] }
}

emit {
    html { toc_depth: 3 }
}
```

## 8. 已裁决（原待定项）

| # | 事项 | 结论 |
|---|---|---|
| 1 | `extends`（profile 继承） | **v1 不做** —— 多 profile 已能叠加，继承会引入钻石依赖与合并顺序问题 |
| 2 | `checks` 是否带参数（如 `severity: warn`） | **v1 不带** —— 严重度统一由工具的 lint 决定 |
| 3 | profile 自身的形状契约放哪 | `rust/smltools/profile.sml`，用 `--dump-builtins` 导出（见 §9） |

## 9. 形状契约在哪（自举闭环）

```console
$ smltools -i rust/smltools/profile.sml --dump-builtins > profile-builtins.json
```

- 契约集：`profile` / `applies_to` / `checks` / `hints` / `emit`；
- **全部 `loose`**：格式要求「未知字段 ⇒ warning + 忽略」，若用 `strict`，
  一个更新版本的 profile 在老工具里会被直接判错 —— 那就违反不变量 2 了；
- `hints.values` 与 `emit` 的键名是开放的，字段语法表达不了 ⇒ 不声明（`loose` 下照常放行），
  编辑器这两处只能补到块名；
- 钉子：`smltools/tests/dump_builtins.rs::profile_shape_dumps`
  （同时钉住可选字段必须导成 `required: false`，否则编辑器会提示"必填"，误导）。

## 10. 实现进度（`checks` 已在 `--lint` 里执行）

| 项 | 状态 | 落点 |
|---|---|---|
| 格式定稿 v1.0 | ✅ | 本文件 |
| 文件定位（三命名空间） | ✅ | `smltools --resolve-profiles`（`weakdoc` 草案 §3） |
| 形状契约 + 导出 | ✅ | `rust/smltools/profile.sml` + `--dump-builtins` |
| **`checks` 三项执行** | ✅ | `smltools/src/profile.rs`，接在 `--lint` 之后 |
| `applies_to` 匹配（**建议**启用） | ✅ | `--lint` 里给 `I-LINT-001` 提示；**只提示、不自动挂** |
| `hints` 接编辑器 | ⬜ | 发布前同步那批 |
| `emit` 接生成器 | ⬜ | 需要与各 `--to` 后端对齐参数名 |

**执行侧的新码**（`errors/codes.sml`，`impls: [smltools]`）：

| 码 | 含义 |
|---|---|
| `E-LINT-002` | `checks.require_top_level` 缺字段 |
| `E-LINT-003` | `checks.forbid_contracts` 命中文档里定义过的契约 |
| `E-LINT-004` | `checks.forbid_keys` 命中任意层级的键 |
| `E-LINT-005` | profile **不可用**：找不到 ⇒ warning；读不出/解析失败/`name` 不符 ⇒ error |
| `I-LINT-001` | `applies_to` 命中了但文档没引用它 —— **提示**（I 级，不影响退出码） |

`applies_to` 的判据：`requires` 里的契约都已定义 **且** `glob` 至少一条命中文档路径；
相对模式允许从任一目录边界起匹配（`docs/api/**/*.sml` 能命中绝对路径的尾部）。
**没写 `applies_to` 的 profile 一律不猜** —— 宁缺勿滥。

三档严重度的取舍：**用户显式挂了 profile**（文档里写了 `@weakdoc`），规则不生效不能静默 ⇒ error；
但三处命名空间都没命中时可能只是"没装"，故只 warning。
