# weakdoc 规范草案 v0.1（**库侧与 CLI 发现已实现，编辑器侧未做**）

> 状态横幅：**Rust 库侧已实现**（语法 / 旁路表 / 判重 / 判别用例）；
> `.smlex` 的三命名空间发现已由 `smltools --resolve-profiles` 实现（§3、§8）。
> **未做**：编辑器侧的发现与提示、其余四端与绑定层。
> 决定来源：2026-11-04 与用户确认（三命名空间；项目级目录用 `.smlex/profiles`）。
>
> 同族文档：`docs/swsml/spec/weakbind.md`、`docs/swsml/spec/weakdata.md`。

---

## 0. 一句话

**`weakdoc` = 在文档里写一句「本文件按某个 profile 来」；库只记住这句话，不读文件、不改变解析结果。**

profile 是**工具链**的特化支持（某类文档的校验规则、补全提示、生成参数），
不是文档数据，也**不允许注入**（I-17 硬性约束 1）。

## 1. 语法

```sml
@feature enable type.weak        # 族共用开关

@weakdoc api-docs                # 只写名字，不写载荷
@weakdoc changelog               # 可以引用多个不同 profile
```

- **形态**：`@weakdoc <名>`，**无载荷块**。写了 `{ … }` 会被显式拒绝（`E-PARSE-019`）
  并提示「要内联数据请用 `@weakdata`」—— 两者形态相近但语义完全不同。
- **不能重复挂**：同一 profile 在一份文档里出现两次 ⇒ `E-PARSE-030`。
  判重是**文档级**（不看块路径）：profile 作用于整份文档，重复写就是笔误。
- **可以有多个**：不同 profile 可同时引用（I-17 硬性约束 3）。
- **门控**：`type.weak`；不开 ⇒ `E-FEATURE-001`。

## 2. 记录

```rust
pub struct WeakDoc {
    pub path: Vec<PathSeg>,   // 写在哪个块里（顶层为 []）；判重不看它，只做定位
    pub name: String,
    pub line: Option<usize>,
}
// ParseOutput
pub weak_docs: Vec<WeakDoc>,
```

## 3. 三命名空间（**工具侧的规则，库不参与**）

| 命名空间 | Linux / macOS | Windows | 优先级 |
|---|---|---|---|
| **项目** | `<proj>/.smlex/profiles/<名>.smlex` | 同左（随仓库提交，团队共享） | 最高 |
| **用户全局** | `$XDG_CONFIG_HOME/sml/profiles/<名>.smlex`（默认 `~/.config/sml/profiles/`） | `%APPDATA%\sml\profiles\<名>.smlex` | 中 |
| **系统** | `/usr/share/sml/profiles/<名>.smlex` | `%ProgramData%\sml\profiles\<名>.smlex` | 最低 |

规则：

1. **按名拼路径**：文件名即 profile 名 ⇒ 同层不可能重名，也就没有「同层冲突」这条分支；
2. 查找顺序 **项目 → 用户 → 系统**，**先命中者胜**（高层覆盖低层，不报错）；
3. 三处都没有 ⇒ **工具侧**决定提示方式（建议 warning，不阻断 —— profile 是增强项）；
4. 工具链要提供 `--profile-dir` / `--no-profiles` 之类的显式开关，便于测试与 CI 复现。

**为什么这些规则不放进库**：库一旦读文件系统，「解析结果对有无 `.smlex` 完全一致」
（I-17 硬性约束 1 的可测判据）就得靠额外机制保证；而现在库**连文件都没碰**，判据天然成立。

**用法（`smltools`，已实现）**

```console
$ smltools -i doc.sml --resolve-profiles
api-docs      project   ~/proj/.smlex/profiles/api-docs.smlex
missing-one   -         (未找到)
$ echo $?
1
```

- 输出 TAB 分隔三列：**名字 / 作用域 / 路径**；作用域取值 `explicit` / `project` / `user` / `system`；
- 未命中记 `-`，且退出码为 **1**（不能静默当“有”）；
- `--profile-dir <dir>` 追加显式根（优先级最高，便于测试与 CI 复现）；
- `--no-profiles` 关掉三命名空间发现（只剩 `--profile-dir`）。

## 4. 核心判据（可测）

1. **不能注入**：有 / 无 `@weakdoc`，`value` / `contract_map` / `weak_binds` / `weak_data`
   **逐字节相同**，差异只在 `weak_docs`；
2. **不读文件**：库内没有任何路径查找/读取代码路径 —— 判据 1 由构造保证，不靠测试兜底；
3. **判重文档级**：重复引用报错，不静默去重。

## 5. 实现进度

| 项 | 状态 | 落点 |
|---|---|---|
| 指令登记 | ✅ | `sml-parse/src/ext.rs` 的 `BUILTIN_DIRECTIVES` 加 `weakdoc` |
| 语法 + 门控 | ✅ | `parser.rs::parse_at_directive`（复用 `Feature::Weak`） |
| 旁路表 | ✅ | `ParseOutput.weak_docs: Vec<WeakDoc>` |
| 判重 | ✅ | `E-PARSE-030`（新码，`impls: [rust]` / `status: partial`） |
| 用例 | ✅ 6 条 | `rust/tests/weakdoc.rs` |
| 三命名空间发现（`smltools`） | ✅ | `--resolve-profiles`：TAB 三列输出，未命中退出码 1；4 条端到端用例 |
| `--profile-dir` / `--no-profiles` | ✅ | 显式目录优先级最高；`--no-profiles` 关掉三命名空间 |
| 编辑器侧的发现与提示 | ⬜ | 属「发布前同步」那批 |
| profile 内容格式 | ✅ 定稿 v1.0 | `docs/swsml/spec/smlex.md`（形状契约在 `rust/smltools/profile.sml`） |
| 其余四端 + 绑定层 | ⬜ | 发布前统一同步 |

**已知边界**

1. `@for` 子解析器里写 `@weakdoc`：记录会**克隆一份**带进子解析器，但子解析器的结果
   不回传父级 —— 与 `contract_map` 同取舍（`@for` 体里挂 profile 属边角）。
2. 库不校验 profile 是否存在，所以「写错名字」在本层**完全静默** —— 这是刻意的：
   profile 由工具侧装载，库没有判断依据。
