# 移交文档

> 面向下一个会话。读完即可接手，不必翻聊天记录。末次提交 `07b180a`（2026-09-18）。

## 0. 现状

`smlconv` 已更名 **`smltools` 0.2.0**，从单一转换器扩为工具箱：
迁移（JSON/TOML/YAML ⇄ SML）、`--strip`、`--lint`、目录批量、**用 SML 定制编辑器高亮**。
`smltools` 39 个单元测试全绿。

## 1. 已完成（提交顺序）

| 提交 | 内容 |
|---|---|
| `3977b42` | 清仓库欠账：warning 19→0、修 `mod tests` 缺 `#[cfg(test)]`、YAML/测试栈溢出等 |
| `a42de59` | `smlconv` → `smltools`（69 文件引用、站点 URL alias、0.2.0） |
| `5a324f5` | `--from json/yaml` + `--strip` + `--lint` |
| `380c7ad` | TOML 双向（`--to/--from toml`） |
| `a7442ca` | 目录批量（`-i dir -o dir`） |
| `8701b02` | 用 SML 定制高亮（`--to tmlanguage`） |
| `07b180a` | 编辑器定制包（`--to highlight`）：颜色 / 项目内生效 / 扩展编译 / Zed |

**能力矩阵**：输入 `sml|json|toml|yaml`（按扩展名自动推断）+ 目录；
输出 `md|json|toml|tmlanguage|highlight|xml|svg|latex|slint|lvgl|html|custom|sml`。

## 2. 待办（优先级）

1. **`--from xml`** ← 下一步，用户要拿超大 XML 试效果（规格见 §3）
2. **Zed 扩展骨架**：`editors/zed/` 需 `extension.toml` + `languages/sml/config.toml`
   + `grammars/sml/grammar.js`（给 grammar.js 而非手工 parser.c，避免与语法演进脱节）
3. **README 中英**：中文加 QQ 群 `589281320` + 邮箱 `dev@mail.swebase.cn`；
   新建 `README.en.md` 完整对照（QQ 群注明"中文社区"）+ 顶部语言切换
4. **「标准即教科书」+ 扩展机制**写进 README（官网 `ch13-extension.md` 中英已有）
5. **迁移文档**：smltools README + 站点「从 JSON/YAML/TOML/XML 迁到 SML」指南

## 3. `xml.rs` 规格（尚未落盘，需重写）

⚠️ 上一会话由 `code-explorer` 子代理产出，但**该子代理是只读的、无法落盘**，
主代理只见到尾部片段 —— 所以这份文件需要**重新实现**，规格如下。

文件：`rust/smltools/src/xml.rs`，接口 `pub fn parse(text: &str) -> Result<sml::Value, String>`。

**映射约定（必须严格遵守）**
- 根元素 → 顶层对象的一个键，值为对象
- 子元素 → 键；**同名兄弟合并为数组**（保序）
- 元素属性 → 该元素的 `_attrs` 对象；文本 → `_text`（trim 后非空才写）
- 空元素 → `{}`；**所有叶子值一律字符串**（XML 无类型，不猜数字/布尔）
- 忽略 `<?xml?>`、`<!-- -->`、`<!DOCTYPE>`；CDATA 原样入 `_text`
- 实体：`&lt; &gt; &amp; &quot; &apos;` + `&#65;` / `&#x41;`
- 命名空间前缀保留（`ns:tag`）

**错误**：`第 N 行：说明`，用 `Result::Err`，非测试代码不得 panic/unwrap。

**最新需求（超大 XML，必须一并实现）**
1. **深度上限**：递归下降在超深嵌套会爆栈 → 超过上限（与 `sml::MAX_VALUE_DEPTH` 对齐）
   返回 Err，**绝不崩溃**；用大栈线程跑相关测试（见 §4-6）
2. **性能**：避免 O(n²)（不要在大字符串上反复 `find`/拼接）；实体解码只在含 `&` 时才走解码路径
3. **超长单行**：XML 常整篇一行，行号会退化成"第 1 行" → **同时报字符偏移**
4. **内存预期**：`_attrs`/`_text` 包装会让 Value 树显著大于源文件；超大文件先跑小样本看
   内存与耗时，再上全量；`--strip` 可减小产物体积
5. 需要时考虑迭代式解析或分批（若实测暴露瓶颈，先量再改）

接线：`InputFormat::Xml`（`.xml` 自动推断）+ `Format::Xml` 已有（`--to xml`），
`load_input` 加分支，`InputFormat::name` / `ALL` 同步。

## 4. 关键坑（本次踩过，别重复）

1. `sml::Value` **实现了 `Drop`** → 不能按值 match 移出内部字段（**E0509**）；用引用或 clone。
2. `Value` 只有 `as_str()` / `as_float()` —— **没有** `as_array()` / `len()`。
3. `jsonify` 输出**键按字典序**（`Value::Object` 基于 `BTreeMap`）→ 迁移往返键序会变，
   已记入 CHANGELOG 作为已知限制。
4. `mod tests` **必须**带 `#[cfg(test)]`，否则非测试构建下测试专用 import/辅助函数被误报
   unused/dead_code，`cargo fix` 会**删掉它们**（本次真的删了 `tmpdir`，`cargo test` 直接编译失败）。
5. 深嵌套测试要放进**显式大栈线程**（`std::thread::Builder::stack_size`）——
   Rust 测试线程默认仅 2MB，5 万层嵌套会**测试自己**先溢出。
6. **PowerShell 的 AMSI 会崩**（`System.AccessViolationException`）→ 一律用
   `python -c "..."` 或写 `.py` 脚本文件执行命令。
7. 非 ASCII 处理：`sanitize_filename` / `sanitize_section` 要保留 Unicode 字母数字，
   否则中文标题会变成 `______`。
8. **`code-explorer` 子代理是只读的**（只有 lsp/search/read 工具）—— 需要写盘的活别派给它。

## 5. 高亮定制机制（`--to highlight` / `--to tmlanguage`）

- 定制文件 schema：`directives` / `elements` / `types` / `colors` / `rules`（含 `after`/`before` 锚点）
- 基线：`rust/smltools/assets/baseline.tmLanguage.json`（`include_str!` 内嵌；
  同步方式见 `assets/README.md`）；**锚点名从基线动态读取**，不硬编码
- TextMate `patterns` **顺序敏感**：锚点写错必须**报错并列可用锚点**，绝不静默追加到末尾
  （末尾恰是"被前面规则吃掉、永不生效"的位置）
- `--to highlight` 输出 5 份产物：`syntaxes/`、`vscode/settings.fragment.json`（项目内就地生效）、
  `themes/`、`zed/highlights.scm`、`zed/themes/sml.json`
- Zed 用 Tree-sitter（不吃 TextMate scope）：`.scm` 里方言指令用 `(#match? ...)` 谓词限定；
  Zed 主题按语义键映射。**`.scm` 依赖 grammar 节点名**，已在文件头标注

## 6. 验证命令

```bash
cd rust
cargo test -p smltools          # 39 个测试
cargo check --workspace         # 应为 0 warning（只剩 "hard linking" 环境噪声）
```

```bash
# 迁移
smltools --from json --to sml -i app.json            # 也支持 toml / yaml
smltools --from json --to sml -i conf.d -o out/      # 目录批量
# 检查与剥离
smltools --lint -i doc.sml
smltools --to json --strip -i doc.sml
# 编辑器定制
smltools --from sml --to highlight -i my-dialect.sml -o out/
```

## 7. 下个会话的第一件事

用户会提供一个**超大 XML**。建议顺序：

1. 按 §3 实现并落盘 `xml.rs`（**含深度上限**）+ 接线 `--from xml`
2. 先跑**小样本**验证映射正确（嵌套 / 属性 / 重复兄弟 / 实体 / CDATA）
3. 再上超大文件：观察**耗时与内存**；必要时用 `--strip` 减小产物
4. 若暴露性能瓶颈 → 先量化（时间/内存 profile）再决定迭代式解析或分批
5. 把结果（尤其性能数据与任何精度损失）记进本文件或 CHANGELOG
