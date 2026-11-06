# weakdata 规范草案 v0.1（**Rust 单端已实现**）

> 状态横幅：**Rust 侧已实现**（语法 / 旁路表 / 判别用例）；其它端与编辑器**未做**，
> 属「发布前统一同步」那批（同 `weakbind`）。
> 决定来源：2026-11-04 与用户逐条确认（形态 A、同名后者覆盖、只允许块、可写在任意块内并带路径）。
>
> 同族文档：`docs/swsml/spec/weakbind.md`（共享的族级规则在那里，本文不重复）。

---

## 0. 一句话

**`weakdata` = 一份「关于这份文档的说明」，解析时按名收进旁路表，**永远不进主树**。**

它是给**拓展/编辑器**读的元数据（高亮配置、自定义提示…），不是文档数据本身。

## 1. 与 `weakbind` 的边界（`weak*` 族内部的唯一真差别）

| | 数据在树里？ | 库校验？ | 谁消费 |
|---|---|---|---|
| `weakbind` | **在**（普通数据块） | 不校验（软校验只出 warning） | 编辑器/插件按契约解释 |
| `weakdata` | **不在**（只在 `ParseOutput.weak_data`） | 不适用（无契约） | 拓展按名读取 |

一句话记法：**`weakbind` 管「别校验」，`weakdata` 管「别进树」。**

## 2. 语法

```sml
@feature enable type.weak        # 族共用开关（与 weakbind 同一个位）

@weakdata highlight {
    keyword: "#569cd6"
    comment: "#6a9955"
}

@weakdata hint {
    message: "这里建议用 include 拆分"
}
```

- **形态（A，已定）**：`@weakdata <名> { 任意键值 }`。名字是拓展的**取用键**。
- **载荷只允许块**（已定）：标量 / 数组不接收 —— 真需要单值就写 `k: v` 一个字段。
- **位置**：任意块内都可以；记录**带 `path`**（块内声明能定位，顶层为 `[]`）。
- **门控**：`type.weak`；不开 ⇒ `E-FEATURE-001`（detail 指明指令名）。
- **错误形态**：缺名字 / 缺载荷块 ⇒ `E-PARSE-019`（指令头语法非法）—— 不为它单开码。

## 3. 记录与读取

```rust
pub struct WeakData {
    pub path: Vec<PathSeg>,   // 声明所在块的位置；顶层为 []
    pub name: String,         // 拓展按名取用
    pub value: Value,         // 载荷：任意 SML 数据
    pub line: Option<usize>,  // `@weakdata` 行号
}
// ParseOutput
pub weak_data: Vec<WeakData>,
```

- **同名覆盖（已定）**：同一 `(path, name)` 只留**最后一条**。理由：它是提示通道，
  按名取值天然「最后一次生效」；报错会把编辑期便利变成负担。
- 不同名字可共存；不同块下的同名互不干扰（路径不同）。

## 4. 核心判据（可测）

1. **主树零污染**：`value` 里查不到 `@weakdata` 的任何键；
2. **运行期语义不变**：有 / 无 `@weakdata`，`value` 与 `contract_map` **逐字节相同**，
   差异只出现在 `weak_data`；
3. **不做隐式合并**：库只按名收集，**从不**把载荷并进任何块（与 `flatten` 的区别）。

## 5. 为什么它是内置指令，而不是「宿主各自注册一条外置指令」

外置指令 + `Outcome::Discard` 能做到「不进主树」，但那是**每个宿主各自命名**：
同一份文档换个宿主、换个编辑器就不认。

`weakdata` 把形态固定下来 —— 文档**可移植**、工具**无关**，一次覆盖所有宿主与编辑器
（这也是 `TODO.md` 里「weak\* 三兄弟落地必然改 vsix」的由来）。

**库仍然不解释内容**：`highlight` 里有什么键、`hint` 是什么意思，全在拓展侧。
这是「预留读取通道，不预置业务语义」的分界。

## 6. 实现进度（Rust 单端，2026-11-04）

| 项 | 状态 | 落点 |
|---|---|---|
| 指令登记 | ✅ | `sml-parse/src/ext.rs` 的 `BUILTIN_DIRECTIVES` 加 `weakdata` |
| 语法 + 门控 | ✅ | `parser.rs::parse_at_directive`（复用 `Feature::Weak`） |
| 旁路表 | ✅ | `ParseOutput.weak_data: Vec<WeakData>` |
| 同名覆盖 | ✅ | 记录时按 `(path, name)` 就地替换 |
| 用例 | ✅ 7 条 | `rust/tests/weakdata.rs`（含核心判据的差分用例） |
| 其余四端 + 编辑器 + 绑定层 | ⬜ | 发布前统一同步 |

**已知边界**

1. `@for` 子解析器里写 `@weakdata`：记录**不回传**父级（与 `diags` / `contract_map` 同取舍）。
2. 载荷块会被**完整解析**（该有的语法/契约错误照报），只是**不进主树** ——
   「不进解析」的准确说法是「不进结果树」，不是「不解析」。
