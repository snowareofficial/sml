# feature 命名空间与版本位

> **产品线** swsml ｜ **状态** current ｜ **最后核对** 2026-11-06
>
> 特性名的命名规则：17 个特性归 4 个域（`lex` / `mod` / `type` / `flow`），新增特性必须带域前缀；
> v5 版本位的语义约束。施工清单与 v5 落地步骤见 `../plan/features-plan.md`；元数据在 `docs/meta.sml`。

## 1. 现状（实测，`rust/sml-feature/src/lib.rs`）

**17 个特性**，名字与位序同表（`FEATURES`），位序 = 枚举判别值，**只追加不重排**
（C-ABI `sml_feature_name(bit)` 与位序、名字都绑死）。

| # | 名字 | 位 | 默认 | 归属（提议） |
|---|---|---|---|---|
| 0 | `bareword-string` | 0 | v1 开 / v2+ 关 | `lex` |
| 1 | `include` | 1 | ✅ baseline | `mod` |
| 2 | `env` | 2 | ✅ baseline | `lex` |
| 3 | `contract` | 3 | ✅ baseline | `type` |
| 4 | `fragment` | 4 | ✅ baseline | `mod` |
| 5 | `top-level-array` | 5 | ✅ baseline | `flow` |
| 6 | `namespace` | 6 | ✅ baseline | `mod` |
| 7 | `implicit-ns` | 7 | ✅ baseline | `mod` |
| 8 | `multi-include` | 8 | ❌ opt-in | `mod` |
| 9 | `glob-include` | 9 | ❌ opt-in | `mod` |
| 10 | `regex-include` | 10 | ❌ opt-in | `mod` |
| 11 | `ext-rewrite` | 11 | ❌ opt-in | `mod` |
| 12 | `when` | 12 | ❌ opt-in | `flow` |
| 13 | `for` | 13 | ❌ opt-in | `flow` |
| 14 | `typed-block` | 14 | ❌ opt-in | `type` |
| 15 | `case-insensitive` | 15 | ❌ opt-in | `type` |
| 16 | `block-name` | 16 | ❌ opt-in | `type` |

**版本支持矩阵（实测，见 `errors/codes.sml` 的 `E-FEATURE-004` note）**：

| 端 | 支持的版本 |
|---|---|
| Rust | v1–v4（`Version::CURRENT = V4`） |
| JS | v1–v4 |
| C | 到 v3 |
| C++ | **不校验**：`cpp/sml.cpp:1030` 遇到 `@version` 直接跳过其值 ⇒ v5 对它「透明」（既不拒绝、也不支持新语法） |
| Lua | 仅 v1 |

## 2. 硬约束（决定了「整理」不能怎么做）

1. **老特性名一个都不能改**：`sml_feature_name(bit)` 把名字暴露给 C 调用方，
   改名 = 改 C-ABI 行为；位序同理（只能追加）。
2. **`@feature enable <名字>` 是文档语法**：改老名 = 既有文档失效。
3. ⇒ 所以「整理」只能是 **分类 + 规则 + 新名带域**，**不是重命名**。

## 3. 四个域与两段上限

**名字形态**：`<域>.<名>`，**禁止三段**（用户要求「层数不能过多」）。

| 域 | 管什么 | 现有成员（仅归类，**字符串不变**） | 新增成员示例 |
|---|---|---|---|
| `lex` | 词法、字符串、环境 | `bareword-string` `env` | —— |
| `mod` | 模块化、复用 | `include` `namespace` `implicit-ns` `multi-include` `glob-include` `regex-include` `ext-rewrite` `fragment` | —— |
| `type` | 类型与契约 | `contract` `typed-block` `case-insensitive` `block-name` | **`type.featag`** |
| `flow` | 求值与顶层形状 | `when` `for` `top-level-array` | —— |

规则：

1. **只有新增特性带域**；老名保持平铺（`typed-block` 不改名）。
   ⇒ 判据变得很便宜：**名字里有点 = 新特性**。
2. **域是封闭白名单**：新增域必须改本表，不许随手分层。
3. **不做别名**：不为老特性提供 `type.contract` 这种第二名字 ——
   一个特性两个名字会让诊断、`@feature disable`、C-ABI 回显全部出现「同一件事两种写法」。
4. **单一事实来源**：域归类的唯一落点是 `FEATURES` 表旁的一张注释表（或 `docs/`），
   生成物（C 头 / JS / Lua）不手抄。
5. `!any` **不占特性位**（契约级开关，见 `docs/swsml/spec/featag.md` §10 Q7）。

---

## 6. 已定（2026-11-03 用户逐条确认，**本节已无未决**）

| # | 问题 | 决定 |
|---|---|---|
| N1 | 域清单 | ✅ **四个域**：`lex` / `mod` / `type` / `flow` |
| N2 | 老特性是否补域前缀别名 | ✅ **不补**（同一特性两个名字是债务） |
| N3 | v5 的严格字符串是否等于 v4 | ✅ **等于 v4**；v5 的其它收紧逐条单列（§4.5），**不引入「v5 严格档」概念** |
| N4 | `@feature enable` 是否接受带点名 | ✅ 接受 `type.featag`；**只在第一个点切一次**，`a.b.c` 直接拒 |
| — | 未注册指令 / 契约数组（§4.5 第 5、7 条） | 📌 **只登记，暂不动** —— 跨端同步会单独做 |
