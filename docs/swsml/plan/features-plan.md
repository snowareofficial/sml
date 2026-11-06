# feature 域化与 v5 版本位 · 施工清单

> **产品线** swsml ｜ **状态** stale ｜ **最后核对** 2026-11-06
>
> ⚠️ **本文大部分已落地**：版本模型 `Edition` 已替换 `Version`（标识 + 功能集查表），
> `type.featag` 特性位已加。**规范结论已并入 `../spec/features.md`**；
> 逐条剩余进度以 `../ref/unfinished.md` 为准，不以本文的 ⬜ 为准。
> 保留本文只为查 §4.5 / §4.55 的逐条判据（谁定了「v5 严格字符串 == v4」、null 五条从哪来）。
> 元数据在 `docs/meta.sml`。

## 4. v5 版本位落地清单

**Rust（必须）**

1. `Version` 追加 `V5`（在 `V4` 之后，保持 `Ord` 语义）；`CURRENT = V5`；
2. `Version::from_word` 接受 `v5` / `5`；`name()` 返回 `"v5"`；
3. `FeatureSet::for_version(V5)` = baseline + 严格字符串（同 v2+）——
   **`type.featag` 不进 baseline**（opt-in）；
4. `errors/codes.sml` 的 `E-FEATURE-004` 文案（`v1..v4` → `v1..v5`）与 note（各端范围）重生成；
5. 回归：`@version v5` 在未开 `type.featag` 时，`!x` 仍按裸词处理（现状），
   开了才走新语法 —— 保证「v5 不等于全开」。

**JS（建议同批）**：`detectVersion` 接受 v5 + 文案；
**C / C++ / Lua**：**不在本批**，继续用 `E-FEATURE-004` 表达「本端不支持 v5」——
这与现状一致（C 到 v3、Lua 到 v1），不需要假装同步。

⚠️ 风险：`Version` 是 `pub enum`，追加变体对 `match` 是**破坏性**的
（所有 `match version { ... }` 都要补分支）⇒ 先全仓扫 `match.*Version` 再动，别边改边发现。

## 4.5 v5 到底改了哪些行为（逐条单列，不打包成「严格档」）

⚠️ 这里有个必须说清的：**v5 不是纯增量**。虽然字符串严格档与 v4 相同，但已有两条**收紧**，
若把它们统称「v5 严格档」，以后没人说得清这个档到底含什么。所以逐条单列：

| # | 行为 | v4 | v5 | 状态 |
|---|---|---|---|---|
| 1 | 字符串必须加引号（`strict_strings`） | 是 | 是（**不变**） | ✅ 已定 |
| 2 | `any` 类型 | 合法 | **需契约内 `!any`** | ✅ 已定 |
| 3 | 正文手写 `__*` | 原样保留（实测） | **报错** | ✅ 已定 |
| 4 | `!featag` 语法 | 不存在（`!x` 是裸词） | 存在（需 `type.featag`） | ✅ 已定 |
| 5 | 未注册 `@指令` | Rust 报错 / **JS 静默收成片段** | ？统一报错？ | 📌 **已登记，暂不动**（跨端同步单独一笔） |
| 6 | 符号式可选 `Endpoint?` | 已报 `E-PARSE-006`（2026-10-30 起） | 保持报错 | 📌 保持现状 |
| 7 | 契约数组 `[契约名]` | Rust/JS 支持、Lua 不支持 | ？五端对齐？ | 📌 **已登记，暂不动**（跨端同步单独一笔） |

第 5、7 条是「一次性把已知跨端分歧收进 v5」的机会窗口 —— v5 是唯一能合法改变既有行为的版本位，
错过了就又要等下一个大版本。但它们各自牵动五端，**建议单独定、不搭 featag 的车**。

## 4.55 v5 设计原则：**尽可能减少 null / none 思维**（用户 2026-11-03）

> 原话：「v5 会尽可能减少 null / none 思维，除非不使用 SML 能力。」

这条不只是 featag 的事，它会波及契约与值模型。已落地的第一处：**featag 的写入来源只做两档**
（默认 / `from-featag`），不引入 `none` / `readonly` 第三档 —— 「谁都写不了」用「必填但无人提供」
或 `optional` 表达（详见 `docs/swsml/spec/featag.md` §6.2）。

**需要逐个清点的地方（待展开，未做）**

| 处 | 现状 | 若要「减少 null 思维」 |
|---|---|---|
| `Value::Null` | 数据模型里有 | 保留（读外部数据需要），但**契约不鼓励用 null 表示「没有」** ⇒ 用「键缺失」 |
| `optional` | 键可不存在 | 与 null 的分工要写清：**optional = 键缺失，不是值为 null** |
| `default null` | 可能合法 | 是否禁掉？（待定） |
| `any` | 可含 null | 若契约开了 `!any`，是否仍允许 null？（待定） |
| emit 各后端 | 对 null 各有处理 | 需要一份「null 在五端的表现」清单，避免漂移 |

⚠️ 这条与 §4.5 第 2 条（`any` 需 `!any`）同源：二者都是「别让空值/任意值成为默认」。

## 4.6 v5 落地步骤清单（**只是清单，不含代码改动**）

实测足迹（`git grep "Version::V4\|v1..v4\|CURRENT"`），全部命中点如下：

| 步 | 改什么 | 位置 | 备注 |
|---|---|---|---|
| 1 | 枚举追加 `V5`（**末尾**，保 `Ord`）；`CURRENT = V5` | `rust/sml-feature/src/lib.rs:36-41` | 追加变体对**穷尽 `match`** 是破坏性的 —— 实测只有 `name()` 一处穷尽 match，必须补 |
| 2 | `from_word` 收 `"v5" \| "5"`；`name()` 返回 `"v5"` | 同文件 `:49-67` | |
| 3 | `for_version(V5)` = baseline + 严格字符串；**不加任何特性位** | 同文件 `:276-286` | 必须加一条守护用例：`for_version(V5) == for_version(V4)`（防止 v5 夹带特性） |
| 4 | CLI 版本字面量 match + 文案 `（v1..v4）` | `rust/smltools/src/main.rs:487-496`、`:291` 注释 | |
| 5 | 复核 `I-FEATURE-001` 提示是否要覆盖 v5 | `main.rs:605-613` | 该提示目前只针对 v3；v5 初期没有「未门控差异」⇒ 不需要加 |
| 6 | 码表文案 + 重生成 5 份产物 | `errors/codes.sml:270`（msg 里的「当前支持 v1..v4」与 note 的各端范围）⇒ 跑 `gen_json.py` / `gen_codes.py` / `gen_js_errors.py` | ⚠️ 不跑生成器会漂：`c/sml_codes.h`、`c/sml_codes_msg.h`、`lua/sml_errors.lua`、`js/sml-errors.mjs`、`rust/sml-codes/src/{codes,msgs}.rs` |
| 7 | 手写文档里的「v1..v4」 | `rust/src/lib.rs:30`、`rust/README.md:102/106`、`rust/sml-feature/README.md:31/68`、`c/sml_rs.h`、`cpp/sml_rs.hpp` | 后两个**不是生成物**，要人工改 |
| 8 | JS 侧版本白名单 | `js/sml.mjs:535-561`（`detectVersion` + 文案） | N3 已定「严格档同 v4」⇒ JS **只加版本号**，语义一行不改 |
| 9 | 其它端**不动** | C（到 v3）、Lua（仅 v1）、C++（不校验版本） | 继续用 `E-FEATURE-004` 表达「本端不支持」 |

**验证（每步都要跑）**

```
cargo test -j 1 --no-fail-fast          # 唯一允许红：本地探针 _stack_probe
node js/test-sml-errors.mjs             # 动了 i18n 产物就必须跑
python errors/gen_json.py --check && python errors/gen_codes.py --check && python errors/gen_js_errors.py --check
python tools/check_private_assets.py
cargo run -p smltools -- -i <v5 样例>.sml --to json   # 正例：v5 能解析
cargo run -p smltools -- -i <未开 featag 的 v5 样例>   # 反例：!x 仍按裸词（v5 不等于全开）
```

## 5. 顺序（与 featag 的关系）

```
① 定域清单（本文件 §3）
② 落 v5 版本位（Rust + JS）
③ 才动 type.featag（语法 + __featags + 三个新码）
④ 声明校验层（契约里 !name / 未声明即拒 / 类型校验）
⑤ 应用层参考实现（可选，不进核心）
```

理由：featag 是第一个「v5 新特性」，它的名字（`type.featag`）与版本位都由 ①② 决定；
倒过来做，`featag` 会变成下一个「不能改名的老名」。
