// SPDX-License-Identifier: MulanPSL-2.0
//! SML 语法分析器：递归下降，消费 [`Tok`] 序列产出 [`Value`] 树。

use std::collections::BTreeMap;
use std::sync::Arc;

use sml_codes::{
    E_CONTRACT_001, E_CONTRACT_010, E_CONTRACT_011, E_CONTRACT_012, E_CONTRACT_013, E_CONTRACT_018,
    E_CONTRACT_019, E_CONTRACT_020, E_CONTRACT_021, E_CONTRACT_022, E_CONTRACT_023, E_CONTRACT_025,
    E_EXT_003, E_EXT_004, E_EXT_005, E_EXT_008, E_FEATURE_001,
    E_FEATURE_002, E_FEATURE_003, E_INCLUDE_007, E_LEX_007, E_LIMIT_001, E_PARSE_001, E_PARSE_002,
    E_PARSE_003, E_PARSE_004, E_PARSE_005, E_PARSE_006, E_PARSE_007, E_PARSE_008, E_PARSE_011,
    E_PARSE_012,
    E_PARSE_013, E_PARSE_017, E_PARSE_019, E_PARSE_020, E_PARSE_021, E_PARSE_022, E_PARSE_023,
    E_PARSE_026, E_PARSE_028, E_PARSE_029, E_PARSE_030, SmlError,
};
// 这两个只在 `when` feature（@when/@for）下用到；关掉该 feature 时下游会报 unused_imports
#[cfg(feature = "when")]
use sml_codes::{E_PARSE_016, E_PARSE_018};
use sml_contract::{
    suggest_contract_name, Contract, ContractExt, ContractWarning, FeatagDecl, FieldSpec, Modifier,
    Strictness, TypeCheck, TypeSpec,
};
// `Script` 只用于「每契约单一语种」的判定
use sml_lex::identifiers::Script;
use sml_value::Seg;
use sml_feature::{Feature, FeatureSet};
use sml_lex::{Span, Tok, TokOwned, coerce_word, lookup_env};
// `Map` = `Value::Object` 的载体（默认 `BTreeMap`；`preserve_order` 档是保序 `Object`，
// 迭代序 == 源序）。解析器里凡是「正在收集的块字段表」都必须是 `Map` —— 它是
// 保序的**唯一真相源**；`fragments` / `types` / `contracts` 是普通表，仍是 `BTreeMap`。
use sml_value::{insert_or_merge_owned, Map, MAX_VALUE_DEPTH, Value};

use crate::contract_bridge::{apply_contract, apply_contracts};
use crate::contract_map::{ContractMap, PathSeg};
use crate::ext::{Diagnostic, DiagnosticKind, DirectiveTable, Outcome, WeakBind, WeakData, WeakDoc};

/// The parser. `pub` only so submodules like `cond` can reuse the cursor — **not** part of the public API ｜ 解析器（`pub` 仅为让 `cond` 等子模块复用游标）。
/// 一条挂起的 `@weakbind`：`(目标块路径, 契约名, 可选字段体)`。
/// 抽成别名只为压住 `clippy::type_complexity`，语义与 `weak_pending` 字段一致。
type WeakPending = (Vec<PathSeg>, String, Option<BTreeMap<String, FieldSpec>>);

pub struct Parser<'a> {
    pub(crate) toks: Vec<Tok<'a>>,
    /// 与 `toks` **一一对应**的原文区间（字节偏移），供错误定位。
    ///
    /// 可能为空：文件入口经 `resolve_includes` 拿到的是**跨文件合并**后的 token 流，
    /// 没有可映射的单一原文（此时 [`Self::err_at`] 会退回「不带位置」，宁缺勿错）。
    pub(crate) spans: Vec<Span>,
    /// 与 `toks` 一一对应的**行号**（0 起），供「`@is` 多词遇到换行即停」（D1）。
    ///
    /// 空表 = 行号未知（文件入口的 include 展开 / `@for` 体）⇒ 调用方必须退回
    /// **只收一个词**的旧行为 —— 宁可少收，不可把下一行的键吞进契约名。
    pub(crate) lines: Vec<usize>,
    pub(crate) i: usize,
    pub(crate) fragments: BTreeMap<String, Value>,
    /// 自定义类型表：名 -> 模式（SML 值）。由 `@type name: X { ... }` 填充。
    /// 供契约字段引用（`字段: X`）时编译为匹配器。
    pub(crate) types: BTreeMap<String, Value>,
    /// 契约表：名 -> 契约。由 `@contract Name { ... }` 填充
    pub(crate) contracts: BTreeMap<String, Contract>,
    /// 生效特性集（已与调用方允许范围交集）
    pub features: FeatureSet,
    /// 显式保序解析（见 `ext::ParseOptions::ordered`）。
    pub(crate) ordered: bool,
    /// 当前值嵌套深度（块 / 数组的递归层数）。
    /// 由 `parse_block` / `parse_array` 的 wrapper 维护，用于防止栈溢出。
    pub(crate) depth: usize,
    /// 命名空间栈：每个块（含 include `as ns` 产生的块）的名字依次入栈。
    /// 宏/契约注册与引用时，按栈路径加前缀（如 `ui.form.Button`），
    /// 使命名空间真正隔离宏，而非仅隔离数据键值。
    pub(crate) ns_stack: Vec<String>,
    /// 结果树路径栈（与 `ns_stack` **平行**、不替代）：`Key` = 对象键，`Index` = 数组下标。
    ///
    /// 只供契约映射记录用。为什么不动 `ns_stack`：它的字符串拼接语义
    /// （`ns_prefix()` / `qualify()` / 错误前缀）被十几个调用点依赖，
    /// 且裸块的 `Key(类型名)` 是**命名空间**产物、不是结果树位置 ——
    /// 两者本来就不该是同一个栈（数组里的裸块即是反例）。
    pub(crate) path_stack: Vec<PathSeg>,
    /// 契约映射（`ParseOptions::trace_contracts` 开启时才 `Some`；未开启恒 `None`，
    /// 解析行为与旧版逐字一致）。
    pub(crate) contract_map: Option<ContractMap>,
    /// 本次解析的环境变量覆盖表。`$env.X` 先在此查表，未命中才读进程环境。
    /// 用于 FFI 等「不修改进程环境」的注入场景（见 [`Self::env_var`]）。
    pub(crate) env: BTreeMap<String, String>,
    /// `@for var in ... { ... }` 的只读循环变量绑定。`${var}` 在字符串值里解析为
    /// 这里的值。作用域按 `@for` 嵌套自然叠加（内层覆盖外层同名），属于 cond 模块
    /// 的解析期指令家族，由 cargo feature `when` 门控整体带入。
    pub(crate) loop_vars: BTreeMap<String, String>,
    /// 外置 `@` 指令表（由下游注册，见 [`crate::ext`]）。
    /// 为空表时解析行为与既有实现**完全一致**，故零扩展 = 零影响。
    pub(crate) directives: Arc<DirectiveTable>,
    /// 非致命诊断（弃用提示等），随结果一并返回，不打断解析。
    pub(crate) diags: Vec<Diagnostic>,
    /// 是否收集**键冲突**告警（见 [`crate::ext::ParseOptions::warn_key_conflict`]）。
    ///
    /// 只影响 [`Self::diags`] 的内容，**不影响解析结果**（`Value` 与旧版逐字一致）。
    pub(crate) warn_key_conflict: bool,
    /// 是否在解析期应用文件内契约（`@is` / typed-block）。
    /// 默认 `true`；`false` 时跳过应用与校验（IDEAS I-2 的 opt-out）。
    pub(crate) apply_contracts: bool,
    /// 外置契约类型表（由下游注册，见 [`crate::ext`]）。
    pub(crate) contract_ext: Arc<ContractExt>,
    /// 是否收集多条错误（[`crate::ext::ParseOptions::collect_errors`]）。
    pub(crate) collect_errors: bool,
    /// 收集到的错误（仅 `collect_errors` 时非空）。
    pub(crate) errors: Vec<SmlError>,
    /// 弱绑定旁路表（`docs/swsml/spec/weakbind.md` §5）：`@weakbind` 的产物。
    ///
    /// **不进主树** —— 这是核心判据「有无 `@weakbind`，主树与 `contract_map` 逐字节相同」
    /// 的落点。未使用该指令时为空。
    pub(crate) weak_binds: Vec<WeakBind>,
    /// 内置契约表（宿主注入）：`@weakbind` 的软校验拿它比对。
    ///
    /// **空表 ⇒ 整段软校验跳过**（否则一落地，所有弱声明都会报 `decl.unknown_contract`）。
    pub(crate) builtin_contracts: BTreeMap<String, Contract>,
    /// 已登记的弱绑定，但**还没到目标块收尾** —— 块内容校验要等块填完才能做。
    ///
    /// 元组：`(目标块路径, 契约名, 可选字段体)`；收尾时按路径精确匹配取出。
    pub(crate) weak_pending: Vec<WeakPending>,
    /// 弱数据旁路表（`@weakdata` 的载荷）：**不进主树**，只给拓展按名读取。
    ///
    /// 同一 `(path, name)` 只留最后一条（同名后者覆盖，见 `docs/swsml/spec/weakdata.md` §3）。
    pub(crate) weak_data: Vec<WeakData>,
    /// 弱 profile 引用（`@weakdoc`）：只存名字，**库不读文件系统**。
    ///
    /// 判重是**文档级**（不看路径）：同一个 profile 不能重复挂，重复即 `E-PARSE-030`。
    pub(crate) weak_docs: Vec<WeakDoc>,
}

/// 单个块在迭代式解析中的可变状态（迭代式改造第一步：从 `parse_block_inner`
/// 原封不动搬出的局部量）。`node` 是正在收集的字段表；`applied_contracts` 记录
/// 块内声明的 `@is`（第二项为块级严格度覆盖）；`pending_when` 是作用于下一个
/// 键/块的 `@when`；`closing` 是本块期望的闭合符号（`None` = 顶层）；
/// `bare` 是裸块上下文（迭代式改造第二步 `Frame::Block` 拓展），`None` 表普通块。
struct BlockFrame {
    node: Map,
    applied_contracts: Vec<(String, Option<Strictness>)>,
    pending_when: Option<bool>,
    /// 作用于**紧邻的下一个块**的 `@weakbind`（照 `@when` 的先例）。
    ///
    /// 与 `pending_when` 一样是「挂起在父帧、由下一项消费」的状态；差别在于它在
    /// 目标块**开帧时**就落表（那时 `path_stack` 已压好该块的键），因此不必随
    /// 容器一路传到 `finish_block`。
    pending_weakbind: Option<PendingWeak>,
    closing: Option<TokOwned>,
    /// 裸块上下文：`Some` 时本块体是裸块 `type [name] [args…] { … }` 的体，
    /// 类型名 / 已收集参数存于此，块收尾（finish）时写入 `__type`/`__name`/`__args`
    /// 元数据并跑 typed-block 类型契约（迭代式改造第二步，作业书 §3）。
    bare: Option<BareCtx>,
}

/// 一条挂起的 `@weakbind` 声明（`docs/swsml/spec/weakbind.md` §2）。
///
/// 只活到「下一个块开帧」为止：那一刻写进 [`Parser::weak_binds`]，随即清空。
/// `line` 是 `@weakbind` 所在行（1 起），供宿主把诊断指回原文。
struct PendingWeak {
    name: String,
    body: Option<BTreeMap<String, FieldSpec>>,
    line: Option<usize>,
}

/// 裸块上下文（作业书 §3，`Frame::Block.bare`）：建帧时已把 args 收集齐并消费到 `{`，
/// `key` 是类型名（写入 `__type`）；块体闭合后据此补全裸块元数据。
struct BareCtx {
    key: String,
    args: Vec<Value>,
    /// 块头的 featag 项（**保序**）。空 = 没有 featag 区，或 `type.featag` 未开启 ——
    /// 两种情况对结果都没区别，故不必区分。
    featags: Vec<Value>,
}

/// 一个"待完成的容器"帧（作业书 §3）。`Block` 是一对 `{}`（含裸块体），`Array` 是一对 `[]`；
/// `cont` 记录本帧完成后把结果交回父帧的方式。迭代式改造第二步显式栈的栈元素。
// 解析器的热路径类型：每消费一个 token 就创建/匹配一次。`Box` 掉大字段能把枚举
// 压小，代价是**每 token 一次堆分配**，且所有 `match Frame::...` 的写法都要改。
// 这里刻意保留栈上布局 —— 省下的字节抵不上分配与改动面。
// ｜ Keep the stack layout: Box would trade enum size for a per-token allocation.
#[allow(clippy::large_enum_variant)]
enum Frame {
    Block { state: BlockFrame, cont: Cont },
    Array { arr: Vec<Value>, cont: Cont },
}

/// 本帧完成后把结果交回父帧/结果的方式（作业书 §3 `Cont`）。
enum Cont {
    /// 作为父块的一个键字段（键名、键下标供重复键诊断定位、`when` 供 `@when` 过滤）。
    Field { key: String, key_idx: usize, when: Option<bool> },
    /// 作为父数组的一个元素。
    Index,
    /// 顶层容器：直接作为结果返回。
    Root,
}

/// 显式栈驱动循环中，单帧单步推进的返回。`Continue` 表当前帧未结束、继续消费下一个
/// token；`Finish` 表当前帧已完成（对应的容器体已闭合）；`Open` 表遇到子容器，需压新帧。
// 同 `Frame`：`Open(Frame)` 让两个变体大小悬殊，但它返回后立刻被消费，不驻留。
#[allow(clippy::large_enum_variant)]
enum Step {
    Continue,
    Finish,
    Open(Frame),
}

/// 单步推进后的"值位置"结果（迭代式改造第二步）：`Leaf` 是立即得到的标量值/数组，
/// `Open` 是需要压栈继续解析的子容器帧。仅由加速循环把它们翻译回 `Step`。
// 同 `Frame`：仅由加速循环翻译回 `Step`，不长期持有。
#[allow(clippy::large_enum_variant)]
enum ValueStep {
    Leaf(Value),
    Open(Frame),
}

impl<'a> Parser<'a> {
    /// 按本次解析的显式选择新建对象载体。**所有**对象构造点都必须走这里。
    fn new_map(&self) -> Map {
        if self.ordered {
            Map::ordered()
        } else {
            // 必须直接落到 `Map::new()`：这里再调 self.new_map() 就是自递归。
            Map::new()
        }
    }

    /// Look up `$env.X`: the parse-local override table first, then the process environment ｜ 查 `$env.X`。
    pub fn env_var(&self, name: &str) -> String {
        lookup_env(Some(&self.env), name)
    }

    /// 解析 `@is` / 块级标注的一个契约词。
    ///
    /// `@is type(契约名)` 与 `@is 契约名` 等价 —— 括号形式让「类型标注」的意图
    /// 更显眼，且与块级标注写法 `type(契约名) 块名 { .. }` 保持同一形态。
    /// 括号在词法中不是分隔符，故 `type(办事人)` 整体是一个 Word；仅当契约表里
    /// **确实存在**名为 `type(办事人)` 的契约时（极端但合法的老文档），才按
    /// 原名解析 —— 向后兼容优先。
    ///
    /// 命名空间隔离：先按裸名查，再按当前 ns 前缀 qualify。
    /// 命中 ⇒ `Ok(resolve 后的名字)`；未命中 ⇒ `Err(给错误文案显示的名字)`。
    fn resolve_contract_word(&self, raw: &str) -> Result<String, String> {
        let cname = match Self::unwrap_type_call(raw) {
            Some(inner) if !self.contracts.contains_key(raw) => inner,
            _ => raw.to_string(),
        };
        if self.contracts.contains_key(&cname) {
            Ok(cname)
        } else {
            let q = self.qualify(&cname);
            if self.contracts.contains_key(&q) {
                Ok(q)
            } else if let Some(hit) = self.contract_by_first_letter(&cname) {
                // opt-in `case-insensitive`：精确与 ns 前缀都未命中时的兜底
                Ok(hit)
            } else {
                Err(q)
            }
        }
    }

    /// 首字母小写归一化：仅小写**第一个字符**，其余保持原样
    /// （即「首字母大小写不敏感」，不是整体忽略大小写）。
    fn first_letter_key(s: &str) -> String {
        let mut it = s.chars();
        match it.next() {
            Some(first) => first.to_lowercase().to_string() + it.as_str(),
            None => String::new(),
        }
    }

    /// 首字母大小写不敏感的**兜底**查找，仅在 `case-insensitive` 特性开启时生效。
    ///
    /// 优先级与精确查找保持一致：先按当前 ns 前缀（`qualify` 后）找，再按裸名找。
    /// 返回契约表里**真实存在的名字**（保留声明时的原样大小写），以便错误信息
    /// 与契约映射里显示的是声明名而非书写名。
    fn contract_by_first_letter(&self, w: &str) -> Option<String> {
        if !self.features.has(Feature::CaseInsensitive) {
            return None;
        }
        let qkey = Self::first_letter_key(&self.qualify(w));
        let bare = Self::first_letter_key(w);
        self.contracts
            .keys()
            .find(|k| Self::first_letter_key(k) == qkey)
            .or_else(|| self.contracts.keys().find(|k| Self::first_letter_key(k) == bare))
            .cloned()
    }

    /// 两个 token 是否在同一行（任一行号未知 ⇒ `false`，调用方按「不同行」保守处理）。
    pub(crate) fn same_line(&self, a: usize, b: usize) -> bool {
        match (self.lines.get(a), self.lines.get(b)) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        }
    }

    /// token 序号 `idx` 的起始字节偏移；**定位不到时返回 `None`**。
    ///
    /// 越界（读到文件结尾仍没等到某 token）时退化为最后一个 token 的结束偏移：
    /// 指向文末比指向第 1 行有用。
    pub(crate) fn tok_start(&self, idx: usize) -> Option<usize> {
        if self.spans.is_empty() {
            return None;
        }
        Some(match self.spans.get(idx) {
            Some(s) => s.start,
            None => self.spans[self.spans.len() - 1].end,
        })
    }

    /// 造一个**带位置**的错误：`idx` 处能定位就带上，定位不了就退回不带位置
    /// （宁可不给，也不给错的 —— 错误的位置比没有位置更糟）。
    pub(crate) fn err_at(
        &self,
        code: &'static str,
        msg: impl Into<String>,
        idx: usize,
    ) -> SmlError {
        match self.tok_start(idx) {
            Some(pos) => SmlError::new(code, msg).at(pos),
            None => SmlError::new(code, msg),
        }
    }

    /// token 序号 `idx` 所在行号（**1 起**）；行号未知（`lines` 为空）时返回 `None`。
    ///
    /// 与 [`Self::err_at`] 同一取舍：宁可不给，也不给错的。
    pub(crate) fn line_of(&self, idx: usize) -> Option<usize> {
        self.lines.get(idx).map(|l| l + 1)
    }

    /// 记一条**键冲突**告警（[`DiagnosticKind::DuplicateKey`] /
    /// [`DiagnosticKind::KeyOverwritten`]）。
    ///
    /// 收不收由 [`Self::warn_key_conflict`] 决定；行号拿不到时在文案末尾补一句
    /// 指引（文件入口的 token 流是跨文件合并的，没有可映射的单一原文）。
    fn push_key_diag(&mut self, kind: DiagnosticKind, idx: usize, msg: String) {
        if !self.warn_key_conflict {
            return;
        }
        let line = self.line_of(idx);
        let message = match line {
            Some(_) => msg,
            None => format!(
                "{msg}（本入口无行号信息；需要带行号的诊断请用 `smltools --lint`）"
            ),
        };
        self.diags.push(Diagnostic { kind, message, line });
    }

    /// Look up an `@for` loop variable `${var}` (read-only binding); `None` when unbound ｜ 查 `@for` 循环变量。
    pub fn loop_var(&self, name: &str) -> Option<&String> {
        self.loop_vars.get(name)
    }

    /// 字符串插值：把 `${var}`（循环变量）与 `$env.NAME` 替换为实际值。
    /// 这是 `@for` 把迭代值注入循环体的主要手段（只读、文本级）。
    ///
    /// - `${var}`：查 [`Self::loop_var`]，未绑定则原样保留（避免静默丢信息）；
    /// - `$env.NAME`：查环境变量（受 `Feature::Env` 约束，见下方调用点）；
    /// - 其它：`${`/`$env.` 以外的 `$` 视为普通字符。
    fn interp_str(&self, s: &str) -> String {
        // 先做 `${var}` 替换（循环变量优先于环境变量语义，且二者前缀不同不会冲突）。
        let mut out = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '$'
                && chars.peek() == Some(&'{') {
                    // 收集 `${ ... }` 中的名字
                    chars.next(); // 消费 '{'
                    let mut name = String::new();
                    for cc in chars.by_ref() {
                        if cc == '}' {
                            break;
                        }
                        name.push(cc);
                    }
                    if let Some(v) = self.loop_var(name.trim()) {
                        out.push_str(v);
                        continue;
                    }
                    // 未绑定：原样保留 `${name}`，让作者能发现笔误而非静默空串
                    out.push('$');
                    out.push('{');
                    out.push_str(&name);
                    out.push('}');
                    continue;
                }
                // 非 `${` 的 `$`（如孤立 `$env` 写错、或普通 `$`）：原样保留
            out.push(c);
        }
        // 再处理 `$env.NAME`（与裸词路径一致受 `Feature::Env` 约束）
        if out.contains("$env.") && self.features.has(Feature::Env) {
            // 仅当特性开启才替换，否则保留原串由上层 parse_value 报错
            let replaced = out.replace("$env.", "\u{0}ENV\u{0}");
            // 用临时哨兵拆分，避免与正常文本冲突
            let mut r = String::new();
            let mut parts = replaced.split("\u{0}ENV\u{0}");
            if let Some(head) = parts.next() {
                r.push_str(head);
            }
            for name in parts {
                // name 可能是 `FOO.xxx` 或 `FOO"` 等；取到第一个非标识符字符为止
                let end = name
                    .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '.'))
                    .unwrap_or(name.len());
                let (var, tail) = name.split_at(end);
                r.push_str(&self.env_var(var));
                r.push_str(tail);
            }
            out = r;
        }
        out
    }

    /// 从当前游标（应在 `{` 处）切出整个块的内部 token（不含两端括号），并推进游标越过 `}`。
    /// 用于 `@for` 对每个迭代值重放（重放语义：同份模板 + 不同循环变量 = 不同对象）。
    ///
    /// 仅 `@for` 使用：随 cargo feature `when` 一起编译，关闭时不参与编译
    /// （否则会留一条 dead_code 警告，掩盖真正的问题）。
    #[cfg(feature = "when")]
    fn slice_block_tokens(&mut self) -> Result<Vec<Tok<'a>>, SmlError> {
        if self.peek() != Some(&Tok::LBrace) {
            return Err(SmlError::from_code_title(E_PARSE_018).with_detail("`@for` 后须 `{ ... }` 循环体"));
        }
        self.next(); // 消费 '{'
        let mut depth = 1usize;
        let mut inner = Vec::new();
        loop {
            match self.next() {
                None => {
                    return Err(SmlError::from_code_msg(E_PARSE_001)
                        .with_detail("`@for` 循环体未闭合（缺少结束符号 }）"))
                }
                Some(Tok::LBrace) => {
                    depth += 1;
                    inner.push(Tok::LBrace);
                }
                Some(Tok::RBrace) => {
                    depth -= 1;
                    if depth == 0 {
                        break; // 不把最后的 '}' 纳入 inner
                    }
                    inner.push(Tok::RBrace);
                }
                Some(t) => inner.push(t),
            }
        }
        Ok(inner)
    }

    /// 以当前解析器为蓝本，生成一个**块级子解析器**，专用于重放 `@for` 的某一轮迭代。
    /// 继承片段表/契约表/特性集/命名空间/环境变量，但用独立的 `loop_vars`
    /// （叠加当前迭代的 `${var}=item`，以支持嵌套 `@for` 看到外层绑定）。
    /// 取外置指令的参数名。
    ///
    /// 两种写法都收，但位置参数形式会产出一条弃用诊断：
    /// - **显式（推荐）**：`@xxx name: X { .. }` —— 与片段定义的参数写法一致，
    ///   不与「拼错的指令」同形，v4 起是唯一不带歧义的形式；
    /// - **位置参数**：`@xxx X { .. }` —— v4 起废弃，仅在指令显式声明
    ///   `positional()` 时接受，为既有方言文档留迁移期通道。
    fn take_directive_arg(
        &mut self,
        fname: &str,
        positional: bool,
    ) -> Result<Option<String>, SmlError> {
        // 显式形式：紧邻的 `name` 后必须跟冒号，否则它是普通裸词（如块体首键）。
        let is_explicit = matches!(self.peek(), Some(Tok::Word(w)) if w.as_ref() == "name")
            && matches!(self.peek_at(1), Some(Tok::Colon));
        if is_explicit {
            self.next(); // `name`
            self.next(); // `:`
            return match self.next() {
                Some(Tok::Word(s)) | Some(Tok::Str(s)) => Ok(Some(s.into_owned())),
                other => Err(SmlError::from_code_title(E_EXT_003).with_detail(format!(
                    "扩展指令 `@{fname}` 的 `name:` 后须值，得 {other:?}"
                ))),
            };
        }
        // 位置参数：裸词后必须紧跟 `{`，否则这个裸词多半属于别的语法成分，
        // 不在此处消费（交给后续分支，避免把正常内容当参数吃掉）。
        if positional && matches!(self.peek(), Some(Tok::Word(_)))
            && matches!(self.peek_at(1), Some(Tok::LBrace))
        {
            let w = match self.next() {
                Some(Tok::Word(s)) => s,
                _ => unreachable!("前一步已确认是 Word"),
            };
            self.diags.push(Diagnostic {
                kind: DiagnosticKind::Deprecated,
                message: format!(
                    "sml: 扩展指令 `@{fname} {w} {{ .. }}` 使用了位置参数形式；\
                     自 v4 起推荐显式写作 `@{fname} name: {w} {{ .. }}`"
                ),
                line: self.line_of(self.i.saturating_sub(1)),
            });
            return Ok(Some(w.into_owned()));
        }
        Ok(None)
    }

    /// 同 `slice_block_tokens`：`@for` 专用，随 feature `when` 一起编译。
    #[cfg(feature = "when")]
    fn spawn_for_child(&self, inner: Vec<Tok<'a>>, var: &str, item: &str) -> Parser<'a> {
        let mut loop_vars = self.loop_vars.clone();
        loop_vars.insert(var.to_string(), item.to_string());
        Parser {
            toks: inner,
            // 循环体是父 token 流的**切片**，其 spans 无法按子序号对齐 ⇒ 不带位置。
            // 宁可不给（err_at 会退回无位置），也不要给出错位的位置。
            spans: Vec::new(),
            // 同上：行号未知 ⇒ `@is` 多词退回单词旧行为（见 `lines` 字段注释）
            lines: Vec::new(),
            i: 0,
            fragments: self.fragments.clone(),
            contracts: self.contracts.clone(),
            types: self.types.clone(),
            features: self.features,
            ordered: self.ordered,
            depth: self.depth + 1,
            ns_stack: self.ns_stack.clone(),
            path_stack: self.path_stack.clone(),
            // 子解析器的映射记录暂不回传父级（同 diags：回传要跨 &mut 借用，
            // 而 `@for` 体内应用契约的场景极少）。克隆只为路径上下文正确。
            contract_map: self.contract_map.clone(),
            // 弱绑定同 diags：子解析器的记录暂不回传父级（`@for` 体里写 `@weakbind` 极少）。
            weak_binds: Vec::new(),
            builtin_contracts: self.builtin_contracts.clone(),
            weak_pending: Vec::new(),
            weak_data: Vec::new(),
            // 判重是文档级：子解析器要看得到父级已有的引用，故克隆（同 contract_map）。
            weak_docs: self.weak_docs.clone(),
            env: self.env.clone(),
            loop_vars,
            directives: Arc::clone(&self.directives),
            // 子解析器独立收集：`@for` 体内触发的诊断暂不回传父级
            // （回传需要跨 &mut 借用，收益不抵复杂度；循环体通常不含弃用写法）。
            diags: Vec::new(),
            warn_key_conflict: self.warn_key_conflict,
            apply_contracts: self.apply_contracts,
            collect_errors: self.collect_errors,
            // 子解析器（`@for` 体）独立收集；暂不回传父级（与 diags 同）。
            errors: Vec::new(),
            contract_ext: Arc::clone(&self.contract_ext),
        }
    }

    /// 当前命名空间前缀（栈路径用 "." 连接，空栈返回空串）
    fn ns_prefix(&self) -> String {
        if self.ns_stack.is_empty() {
            String::new()
        } else {
            self.ns_stack.join(".")
        }
    }

    /// 把裸名套上当前命名空间前缀（若栈非空）
    fn qualify(&self, name: &str) -> String {
        let p = self.ns_prefix();
        if p.is_empty() {
            name.to_string()
        } else {
            format!("{p}.{name}")
        }
    }

    /// Peek at the current token (`pub` only for sibling submodules, not public API) ｜ 前看当前 token。
    pub fn peek(&self) -> Option<&Tok<'a>> {
        self.toks.get(self.i)
    }
    /// 前看 n 个 token（n=0 等价于 [`Self::peek`]）。
    /// 用于区分「关键字参数 `type:` `name:`」与「恰巧同名的片段名」。
    fn peek_at(&self, n: usize) -> Option<&Tok<'a>> {
        self.toks.get(self.i + n)
    }
    /// Consume and return the current token (`pub` only for sibling submodules, not public API) ｜ 消费并返回当前 token.
    /// 这是词法游标的一次推进，**不是** `Iterator`：解析器还要 `peek_at` / 回退 / 记偏移，
    /// 套 `Iterator` 会丢掉这些方法且让所有调用点改语义。
    /// ｜ A cursor step, not an Iterator; peek/rewind/offset are also required.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<Tok<'a>> {
        let t = self.toks.get(self.i).cloned();
        if t.is_some() {
            self.i += 1;
        }
        t
    }

    /// 解析契约体：逐条读 `field: <类型> [修饰符...]`，并收集 `@requires` 声明的特性。
    ///
    /// 契约体目前仅支持两类条目：
    /// - 字段规格 `field: <类型> ...`（主路径）；
    /// - 指令 `@requires feat1, feat2`（特性前置声明）。
    ///
    /// 其余 `@xxx` 一律报错而非静默退化成片段（这正是本项目反复踩的「静默失效」坑：
    /// 不认识的 `@xxx` 若走片段定义分支，会被悄无声息地当成名为 `xxx` 的片段消费掉）。
    fn parse_contract_body(
        &mut self,
        requires: &mut Vec<String>,
        featags: &mut Vec<FeatagDecl>,
        strict_paths: &mut bool,
        allow_any: &mut bool,
    ) -> Result<BTreeMap<String, FieldSpec>, SmlError> {
        let mut fields: BTreeMap<String, FieldSpec> = BTreeMap::new();
        loop {
            match self.peek().cloned() {
                Some(Tok::RBrace) => {
                    self.next();
                    break;
                }
                None => {
                    // 契约体未闭合（缺 `}` 即遇到文件结尾）：必须报错，否则后续顶层
                    // key 会被错位吞进契约体，最终主文档静默清空（P0-1）。
                    return Err(SmlError::from_code_msg(E_PARSE_001).with_detail("契约体未闭合（缺少结束符号 }）"));
                }
                Some(Tok::Comma) => {
                    self.next();
                }
                Some(Tok::At) => {
                    // —— 契约体指令：当前仅 `@requires` ——
                    self.next(); // 消费 @
                    let dir = match self.next() {
                        Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                        _ => return Err(SmlError::from_code_msg(E_PARSE_011)),
                    };
                    if dir == "requires" {
                        // 收集特性名：逗号 / 空格两种批量（与 `@feature enable` 一致，已实测均支持）。
                        // 单个特性名内若含逗号（如 `@requires a,b`）也按逗号拆开。
                        // 终止条件：遇到冒号（说明下一个是契约字段 `name:`）或 `}` / 文件末尾。
                        loop {
                            match self.peek().cloned() {
                                Some(Tok::Comma) => {
                                    self.next();
                                }
                                Some(Tok::Word(w)) | Some(Tok::Str(w)) => {
                                    // 该词后跟冒号 ⇒ 它是下一个字段名，`@requires` 列表到此结束
                                    if matches!(self.peek_at(1), Some(Tok::Colon)) {
                                        break;
                                    }
                                    self.next();
                                    for name in w.split(',') {
                                        let name = name.trim().to_string();
                                        if !name.is_empty() {
                                            requires.push(name);
                                        }
                                    }
                                }
                                _ => break,
                            }
                        }
                    } else {
                        return Err(SmlError::from_code_title(E_PARSE_005)
                            .with_detail(format!("契约体内不支持指令 `@{dir}`（仅 `@requires`）")));
                    }
                }
                // featag 声明（S4）：`!name` / `!name: 类型` / `!name: 类型 -> 落点`。
                // 记号与使用处一致（`!` 前缀），故词法零改动；`!` 只在**契约体内**与**块头**
                // 这两个位置是记号，别处一律报歧义（§2.3 B2/B3）。
                Some(Tok::Word(w)) if w.starts_with('!') => {
                    self.next();
                    let name = w[1..].to_string();
                    if name.is_empty() || name.starts_with('!') {
                        return Err(SmlError::from_code_msg(E_PARSE_026)
                            .with_detail(format!("`{w}`")));
                    }
                    // 契约级开关（不是 featag 声明）：
                    // - `!any`（S8）：允许本契约用 `any` 类型（默认禁用，§10 Q7）；
                    // - `!strict-paths`（S6）：落点父对象必须已存在（§6.1 规则 3）。
                    // 两者都**不占** featag 名，也不接受类型 / 参数 / 落点。
                    if name == "any" || name == "strict-paths" {
                        if self.peek() == Some(&Tok::Colon)
                            || self.peek_at(1) == Some(&Tok::LBrack)
                        {
                            return Err(SmlError::from_code_title(E_CONTRACT_021).with_detail(
                                format!("`!{name}` 是契约级开关，不接受类型 / 参数 / 落点"),
                            ));
                        }
                        if name == "any" {
                            *allow_any = true;
                        } else {
                            *strict_paths = true;
                        }
                        continue;
                    }
                    // 参数类型：复用字段规格解析（`[int]` / `str` / 契约引用 / 修饰符全支持）
                    let spec = if self.peek() == Some(&Tok::Colon) {
                        self.next();
                        Some(self.parse_field_spec()?)
                    } else {
                        None
                    };
                    // 落点：`-> 点路径`（不穿数组 —— 见 §10 Q12）
                    let target = if matches!(self.peek(), Some(Tok::Word(t)) if t == "->") {
                        self.next();
                        let raw = self.take_featag_target_text(&name)?;
                        let path = sml_value::Path::parse(&raw).map_err(|e| {
                            SmlError::from_code_title(E_CONTRACT_020)
                                .with_detail(format!("`{name}` 的落点 `{raw}`：{e}"))
                        })?;
                        Some(path)
                    } else {
                        None
                    };
                    // 声明处重名（B4）：只在此查重，使用处允许同名多次
                    if featags.iter().any(|d| d.name == name) {
                        return Err(SmlError::from_code_title(E_CONTRACT_019)
                            .with_detail(format!("`!{name}` 在本契约里出现了两次")));
                    }
                    // 标识符白名单（S1b，门控 `lex.ident`）：featag 名同样是标识符
                    if self.features.has(Feature::Ident) {
                        sml_lex::identifiers::check_ident_err(&name)?;
                    }
                    featags.push(FeatagDecl { name, spec, target });
                }
                _ => {
                    let key = match self.next() {
                        Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                        other => {
                            return Err(SmlError::from_code_title(E_PARSE_006)
                                .with_detail(format!("契约字段期望键，得 {other:?}")))
                        }
                    };
                    // 标识符白名单（S1b）：字段名同样是标识符
                    if self.features.has(Feature::Ident) {
                        sml_lex::identifiers::check_ident_err(&key)?;
                    }
                    if self.peek() == Some(&Tok::Colon) {
                        self.next();
                    } else {
                        return Err(SmlError::from_code_title(E_PARSE_006)
                            .with_detail(format!("契约字段 `{key}` 后须有冒号")));
                    }
                    let spec = self.parse_field_spec()?;
                    fields.insert(key.into_owned(), spec);
                }
            }
        }
        // 「每契约单一语种」（§2.2，门控 `lex.ident`）：同一契约内的标识符（字段名 + featag 名）
        // 必须同属一种文字系统。只比较「第一种脚本」；纯数字 / `_` / `-` 不参与。
        //
        // 为什么在**契约**这一层：同形字真正生效的地方是「同一个命名空间里两个看起来一样的
        // 名字」，而契约就是那个命名空间。不同契约可以用不同语种。
        if self.features.has(Feature::Ident) {
            let mut seen: Option<(Script, String)> = None;
            for id in fields
                .keys()
                .map(|s| s.as_str())
                .chain(featags.iter().map(|d| d.name.as_str()))
            {
                let Some(sc) = sml_lex::identifiers::script_of(id) else {
                    continue;
                };
                match &seen {
                    None => seen = Some((sc, id.to_string())),
                    Some((prev, first)) if *prev != sc => {
                        return Err(SmlError::from_code_msg(E_LEX_007).with_detail(format!(
                            "`{id}`（{sc}）与 `{first}`（{prev}）混用了文字系统：\
                             同一契约内的标识符只能有一种语种"
                        )));
                    }
                    _ => {}
                }
            }
        }
        Ok(fields)
    }

    /// 从 `type(契约名)` 这类「类型标注」形式中取出契约名。
    ///
    /// 括号在词法中不是分隔符，故 `type(办事人)` 整体是一个 Word，此处做
    /// 字符串层的解包即可，无需改动 tokenizer。
    /// 非该形式（或空名 `type()`）返回 `None`，交由调用方按原名处理。
    fn unwrap_type_call(s: &str) -> Option<String> {
        let inner = s.strip_prefix("type(")?.strip_suffix(')')?;
        if inner.is_empty() {
            None
        } else {
            Some(inner.to_string())
        }
    }

    /// 遗留符号修饰符 `?` / `=` 的**定点**拦截：契约字段的**类型位置**不许出现。
    ///
    /// 为什么不在词法层把它们切成分隔符：`?` 在 `@pattern` 里是量词（`sml-pattern`），
    /// 全局切分会把 `a?` 这类合法模式拆坏。故只在类型位置拦 —— 类型名本不该含
    /// 这两个字符，命中即说明作者写的是**遗留符号修饰符**（粘连式，如 `str?` / `int=5`）。
    ///
    /// 带空格的形式（`x: str ?`）在词法上已是独立 Word，由类型表的「未知类型」分支
    /// 与 `parse_contract_body` 的冒号检查拦住（同码） —— 两条路都要给 `E-PARSE-006`。
    fn reject_legacy_modifier_in_type(w: &str) -> Result<(), SmlError> {
        if let Some(sym) = w.chars().find(|c| *c == '?' || *c == '=') {
            return Err(SmlError::from_code_title(E_PARSE_006)
                .with_detail(format!("契约修饰符 `{sym}` 已废弃（用 `optional` / `default`）")));
        }
        Ok(())
    }

    /// 解析单个字段的类型与修饰符
    fn parse_field_spec(&mut self) -> Result<FieldSpec, SmlError> {
        // 命中外置类型时，把校验器带出去（塞进 `FieldSpec::ext`，校验期无需再查表）。
        let mut ext_ty: Option<Arc<dyn TypeCheck>> = None;
        let ty = match self.next() {
            Some(Tok::Word(w)) => {
                // 遗留符号修饰符 `?` / `=` 已废弃（2026-10-30），五端一律 E-PARSE-006。
                //
                // ⚠️ 为什么**不在词法层**把它们切成分隔符：`?` 在 `@pattern` 里是量词
                // （`sml-pattern`），全局切开会把 `a?` 这类合法模式拆坏。
                // 故在**契约字段的类型位置**定点拦截 —— 类型名本不该含这两个字符，
                // 命中即说明作者写的是遗留符号修饰符（粘连式，如 `str?` / `int=5`）。
                // 带空格的形式（`x: str ?`）在词法上已是独立 Word，由下面的
                // 「未知类型」分支与 `parse_contract_body` 的冒号检查拦住（同码）。
                Self::reject_legacy_modifier_in_type(&w)?;
                match w.as_ref() {
                "str" => TypeSpec::Str,
                "int" => TypeSpec::Int,
                "num" => TypeSpec::Num,
                "bool" => TypeSpec::Bool,
                "any" => TypeSpec::Any,
                "enum" => {
                    if self.peek() != Some(&Tok::LBrack) {
                        return Err(SmlError::from_code_msg(E_PARSE_023).with_detail("`enum` 后须为 [ ... ]"));
                    }
                    self.next();
                    let mut vals = Vec::new();
                    loop {
                        match self.peek().cloned() {
                            None | Some(Tok::RBrack) => {
                                self.next();
                                break;
                            }
                            Some(Tok::Comma) => {
                                self.next();
                            }
                            Some(Tok::Word(s)) | Some(Tok::Str(s)) => {
                                vals.push(s.into_owned());
                                self.next();
                            }
                            _ => {
                                self.next();
                            }
                        }
                    }
                    TypeSpec::Enum(vals)
                }
                // `enum(公开, 内部, 机密)` 的兼容重组。
                //
                // 括号 `(` `)` 在本语言的词法中**不是**分隔符（Tok 只有
                // { } [ ] , : @），故这类写法会被 `,` 切成
                // Word("enum(公开") / Word("内部") / Word("机密)")。
                // 此处按「起于 enum( 、止于以 ) 结尾的词」重组回成员列表。
                // 官网与教程大量使用 `enum(a, b, c)`，而 JS 侧此前因把括号
                // 列为分隔符而原生支持它 —— 本分支让两端行为统一。
                w if w.starts_with("enum(") => {
                    let mut vals = Vec::new();
                    let mut rest = w[5..].to_string(); // 去掉前缀 "enum("
                    loop {
                        if rest.ends_with(')') {
                            let last = &rest[..rest.len() - 1];
                            if !last.is_empty() {
                                vals.push(last.to_string());
                            }
                            break;
                        }
                        vals.push(rest.clone());
                        if self.peek() == Some(&Tok::Comma) {
                            self.next();
                        }
                        match self.peek().cloned() {
                            Some(Tok::Word(s)) | Some(Tok::Str(s)) => {
                                rest = s.into_owned();
                                self.next();
                            }
                            _ => break,
                        }
                    }
                    TypeSpec::Enum(vals)
                }
                // 非内置类型名，按此顺序判定：
                //   1. @type 声明的自定义类型（模式校验）
                //   2. **外置注册的类型**（下游定义，如 image / link / time）
                //   3. 回落为契约引用（组合）
                // 次序不可颠倒：@type 与外置/契约同名时，越靠前越"显式"。
                other => {
                    if let Some(pat) = self.types.get(other) {
                        let compiled = sml_pattern::compile_rule(pat, &self.types)
                            .map_err(|e| {
                                SmlError::from_code_title(E_CONTRACT_013)
                                    .with_detail(format!("类型 `{other}` 的模式编译失败：{e}"))
                            })?;
                        TypeSpec::Pattern {
                            name: other.to_string(),
                            pat: compiled,
                        }
                    } else if let Some(t) = self.contract_ext.type_arc(other) {
                        ext_ty = Some(t);
                        TypeSpec::Ext(other.to_string())
                    } else {
                        TypeSpec::ContractRef(other.to_string())
                    }
                }
                }
            },
            Some(Tok::LBrack) => {
                // 数组元素类型位置同样不许出现遗留符号修饰符（`[ str? ]`）。
                if let Some(Tok::Word(w)) = self.peek() {
                    Self::reject_legacy_modifier_in_type(w)?;
                }
                let inner = match self.next() {
                    Some(Tok::Word(w)) => match w.as_ref() {
                        "str" => TypeSpec::Str,
                        "int" => TypeSpec::Int,
                        "num" => TypeSpec::Num,
                        "bool" => TypeSpec::Bool,
                        "any" => TypeSpec::Any,
                        // `enum` 在**数组元素位置**仍不支持：JS 那边靠递归 `parseFieldSpec` 能认，
                        // 本实现只吃单个词，硬塞会把它当成名为 `enum` 的契约（静默错树）。
                        // 故显式拒绝并给准话（与 JS 的差异记在 E-CONTRACT-012 的 note 里）。
                        "enum" => {
                            return Err(SmlError::from_code_title(E_CONTRACT_012)
                                .with_detail("数组元素位置不支持 `enum`；请改写成 `[str]` 并在契约层约束"))
                        }
                        // 非内置名按单值位置的同一套次序判定（见上面 `other =>` 分支）：
                        //   1. `@type` 声明的模式类型
                        //   2. 外置注册的类型（如 `[image]`）
                        //   3. 回落为**契约引用** —— 即 `[契约名]`，数组元素逐项按该契约校验
                        other => {
                            if let Some(pat) = self.types.get(other) {
                                let compiled = sml_pattern::compile_rule(pat, &self.types).map_err(|e| {
                                    SmlError::from_code_title(E_CONTRACT_013)
                                        .with_detail(format!("类型 `{other}` 的模式编译失败：{e}"))
                                })?;
                                TypeSpec::Pattern {
                                    name: other.to_string(),
                                    pat: compiled,
                                }
                            } else if let Some(t) = self.contract_ext.type_arc(other) {
                                ext_ty = Some(t);
                                TypeSpec::Ext(other.to_string())
                            } else {
                                TypeSpec::ContractRef(other.to_string())
                            }
                        }
                    },
                    other => {
                        return Err(SmlError::from_code_title(E_PARSE_006).with_detail(format!("数组元素类型期望标识符，得 {other:?}")))
                    }
                };
                if self.peek() == Some(&Tok::RBrack) {
                    self.next();
                }
                TypeSpec::Array(Box::new(inner))
            }
            other => return Err(SmlError::from_code_title(E_PARSE_006).with_detail(format!("字段类型期望标识符，得 {other:?}"))),
        };

        // 修饰符：required / optional / default <值> / min <数> / max <数>
        let mut required = true;
        let mut default = None;
        let mut min = None;
        let mut max = None;
        // `from-featag`（S7）：该键只能由 featag 落点写入
        let mut from_featag = false;
        // 命中的**外置修饰符**：留到 FieldSpec 构造之后再 apply（它需要 &mut FieldSpec）。
        let mut pending_mods: Vec<(Arc<dyn Modifier>, Value)> = Vec::new();
        loop {
            // 若当前是 `标识符 :` 则视为下一个字段的开始，停止读修饰符
            let is_next_field = matches!(self.peek(), Some(Tok::Word(_)))
                && matches!(self.toks.get(self.i + 1), Some(Tok::Colon));
            if is_next_field {
                break;
            }
            match self.peek().cloned() {
                Some(Tok::Word(w)) => match w.as_ref() {
                    "optional" => {
                        required = false;
                        self.next();
                    }
                    "required" => {
                        required = true;
                        self.next();
                    }
                    "default" => {
                        self.next();
                        default = Some(match self.next() {
                            Some(Tok::Word(w2)) => coerce_word(
                                &w2,
                                &self.fragments,
                                self.features,
                                &self.ns_prefix(),
                                Some(&self.env),
                            )?,
                            Some(Tok::Str(s)) => Value::Str(s.into_owned()),
                            other => {
                                return Err(SmlError::from_code_title(E_PARSE_021).with_detail(format!("default 期望值，得 {other:?}")))
                            }
                        });
                    }
                    "min" => {
                        self.next();
                        min = Some(self.parse_spec_number()?);
                    }
                    "max" => {
                        self.next();
                        max = Some(self.parse_spec_number()?);
                    }
                    // 来源修饰符（S7）：该键只能由 featag 落点写入，文档直接赋值 ⇒ E-CONTRACT-024
                    "from-featag" => {
                        from_featag = true;
                        self.next();
                    }
                    // 未知词：先查**外置修饰符**表，未命中才按"下一个字段的开始"退出。
                    // 内置修饰符名在注册期就被拒绝，故不会与上面各分支抢。
                    other => match self.contract_ext.modifier_arc(other) {
                        Some(m) => {
                            self.next(); // 消费修饰符名
                            let v = self.parse_modifier_value()?;
                            pending_mods.push((m, v));
                        }
                        None => break,
                    },
                },
                _ => break,
            }
        }
        let mut spec = FieldSpec {
            ty,
            required,
            default,
            min,
            max,
            ext: ext_ty,
            ext_data: BTreeMap::new(),
            mods: Vec::new(),
            from_featag,
        };
        // 外置修饰符：解析期改写规格，并把值带进校验期（供 `Modifier::check` 使用）。
        for (m, v) in pending_mods {
            // 外置修饰符的解析期钩子：失败原因由注册方给出，这里补上码。
            m.apply(&mut spec, &v)
                .map_err(|e| SmlError::from_code_title(E_CONTRACT_011).with_detail(e))?;
            spec.ext_data.insert(m.name().to_string(), v);
            spec.mods.push(m);
        }
        Ok(spec)
    }

    /// 解析外置修饰符的取值：裸词 / 引号串 / 块 / 数组。
    /// Parse a **lone top-level scalar** — the read-back direction of `canonical_text`.
    /// 解析单个顶层标量 —— `canonical_text` 的读回方向。
    ///
    /// The caller guarantees the token stream is exactly one `Word` / `Str`
    /// (`ParseOptions::allow_top_scalar`). Uses the same coercion as a field value,
    /// so `42` → `Int`, `1.0` → `Float`, `true` / `null` / quoted strings all agree
    /// with what the emitter produces.
    /// ｜ 调用方保证恰好剩一个 `Word`/`Str`；与字段值走同一套 coerce，
    /// 保证与发射器逐字对称。
    pub(crate) fn parse_lone_scalar(&mut self) -> Result<Value, SmlError> {
        let v = match self.next() {
            Some(Tok::Word(w)) => coerce_word(
                &w,
                &self.fragments,
                self.features,
                &self.ns_prefix(),
                Some(&self.env),
            )?,
            Some(Tok::Str(s)) => {
                if s.contains("$env.") && !self.features.has(Feature::Env) {
                    return Err(SmlError::from_code_msg(E_FEATURE_002)
                        .with_detail(format!("字符串 `\"{s}\"` 无法解析")));
                }
                Value::Str(self.interp_str(&s))
            }
            // 调用方已判过是单标量；到这里说明判据与实现不一致，按原码报错。
            _ => return Err(SmlError::from_code_msg(E_PARSE_008)),
        };
        Ok(v)
    }

    fn parse_modifier_value(&mut self) -> Result<Value, SmlError> {
        match self.peek().cloned() {
            Some(Tok::Word(w)) => {
                self.next();
                coerce_word(
                    &w,
                    &self.fragments,
                    self.features,
                    &self.ns_prefix(),
                    Some(&self.env),
                )
            }
            Some(Tok::Str(s)) => {
                self.next();
                Ok(Value::Str(s.into_owned()))
            }
            Some(Tok::LBrace) => {
                self.next();
                self.parse_block(Some(Tok::RBrace))
            }
            Some(Tok::LBrack) => {
                self.next();
                self.parse_array()
            }
            other => Err(SmlError::from_code_title(E_EXT_005).with_detail(format!("修饰符期望取值，得 {other:?}"))),
        }
    }

    fn parse_spec_number(&mut self) -> Result<f64, SmlError> {
        match self.next() {
            Some(Tok::Word(w)) => {
                let f = w
                    .parse::<f64>()
                    .map_err(|_| SmlError::from_code_title(E_PARSE_022).with_detail(format!("期望数字，得 `{w}`")))?;
                // 拒绝 NaN/inf 作为边界：Rust 的 "nan".parse::<f64>() == Ok(NaN)，
                // 而 NaN 的所有比较均为 false，会让 min/max 校验被静默绕过
                // （审计 #2）。
                if !f.is_finite() {
                    return Err(SmlError::from_code_title(E_CONTRACT_010).with_detail(format!("数字边界必须为有限值，得 `{w}`")));
                }
                Ok(f)
            }
            other => Err(SmlError::from_code_title(E_PARSE_022).with_detail(format!("期望数字，得 {other:?}"))),
        }
    }

    /// 解析对象/块, 直到遇到 closing (None=顶层)。
    /// 迭代式改造第二步起，解析统一由显式栈驱动：这里建根 `Frame` 交给
    /// [`Parser::run_stack`] 跑到底。`run_stack` 内维护「显式栈容器层数」作为
    /// 深度守卫，任何嵌套深度的原生调用栈成本都是常数 —— 128 层放行 / 129 报
    /// 此码的执行口径与原递归实现逐字符一致。
    pub(crate) fn parse_block(&mut self, closing: Option<TokOwned>) -> Result<Value, SmlError> {
        let root = Frame::Block {
            state: BlockFrame {
                node: self.new_map(),
                applied_contracts: Vec::new(),
                pending_when: None,
                pending_weakbind: None,
                closing,
                bare: None,
            },
            cont: Cont::Root,
        };
        // 迭代式改造第二步：解析统一由显式栈驱动（深度守卫在 `run_stack` 内做，
        // 口径 128 放行 / 129 报此码不变 —— 见 `run_stack` 的 `Open` 分支）。
        self.run_stack(root)
    }


    /// `@` 指令与片段定义的处理体 —— 从 [`parse_block_inner`] 抽出。
    ///
    /// **为什么抽出来**：`parse_block_inner` 曾是一个 ~558 行的巨函数，debug 构建下
    /// 栈帧大到「递归到 `MAX_VALUE_DEPTH`(128) 层就撑爆默认 2 MiB 线程栈」。
    /// 本分支独占 ~350 行，是肥帧主因 —— 抽出后这些局部量不再占据**每一层递归**
    /// 的栈帧。详见 `TODO.md` 排雷区雷 1。
    ///
    /// 返回 `Ok(())` 表示本条已处理完毕，等价于原地的 `continue`。
    fn parse_at_directive(
        &mut self,
        node: &mut Map,
        applied_contracts: &mut Vec<(String, Option<Strictness>)>,
        pending_when: &mut Option<bool>,
        pending_weakbind: &mut Option<PendingWeak>,
    ) -> Result<(), SmlError> {
        // `when` feature 关闭时，`pending_when` 只被上面 `#[cfg(feature = "when")]`
        // 块使用；此处显式消费一次，消除 `unused_variables` 警告（TODO L289）。
        #[cfg(not(feature = "when"))]
        let _ = &*pending_when;
        // @name { ... } 片段定义 (不进主树)
        self.next();
        let fname = match self.next() {
            Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
            _ => return Err(SmlError::from_code_msg(E_PARSE_011)),
        };
        if self.peek() == Some(&Tok::Colon) {
            self.next();
        }
        // —— 外置扩展指令（下游注册，无需改动本 crate）——
        //
        // 命中条件：注册表里存在同名指令。内置指令名在注册阶段就被拒绝
        // （见 `DirectiveTable::register`），故这里绝不会拦截到
        // contract / is / type / version / feature / when / for。
        //
        // 这是「方言」的收编口：下游要挂元数据块（表单描述、处理流等）时，
        // 不必 fork 解析器，也不必把这些名字写进 SML 规范层 —— 方言定义
        // 留在下游仓库，SML 核心保持通用。
        if let Some(d) = self.directives.get_arc(&fname) {
            let arg = self.take_directive_arg(&fname, d.positional())?;
            let body = if self.peek() == Some(&Tok::LBrace) {
                self.next();
                self.parse_block(Some(Tok::RBrace))?
            } else {
                Value::Null
            };
            // 注：`Value` 实现了 `Drop`（浮点原始字面量需手工释放），
            // 因此不能按值 match 出内部字段（E0509），这里借引用再克隆。
            let outcome = d
                .call(arg.as_deref(), body)
                .map_err(|e| SmlError::from_code_title(E_EXT_008).with_detail(e))?;
            match &outcome {
                // 元数据块：文档里写了，解析结果里不出现
                Outcome::Discard => {}
                // 展开进主树：值必须是对象，其字段合并进指令所在的块
                Outcome::Emit(Value::Object(m)) => node.extend(m.clone()),
                Outcome::Emit(other) => {
                    return Err(SmlError::from_code_title(E_EXT_004).with_detail(format!(
                        "`@{fname}` 的 Emit 须返回对象（用于合并进所在块），得 {other:?}"
                    )))
                }
            }
            return Ok(());
        }
        // —— 契约定义：`@contract Name { ... }` ——
        if fname == "contract" {
            if !self.features.has(Feature::Contract) {
                return Err(SmlError::from_code_msg(E_FEATURE_001).with_detail("`contract`"));
            }
            let cname = match self.next() {
                Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                other => {
                    return Err(SmlError::from_code_title(E_PARSE_019)
                        .with_detail(format!("@contract 后须契约名，得 {other:?}")))
                }
            };
            // 可选修饰符 `loose` / `warn` / `strict`：显式声明三态严格度（D1）。
            // - loose  → 允许契约未声明的字段，且**静默**放行（旧档）
            // - warn   → 允许契约未声明的字段，但**产出一条 Warning 诊断**
            // - strict → 显式严格（与默认等价，写出仅为可读性/团队规范）
            // 均复用裸词，不引入新 token。对齐 JS 实现：js/sml.mjs 同样接受这三个词。
            let mut strictness = Strictness::Strict;
            if let Some(Tok::Word(w)) = self.peek().cloned() {
                match w.as_ref() {
                    "loose" => {
                        strictness = Strictness::Loose;
                        self.next();
                    }
                    "warn" => {
                        strictness = Strictness::Warn;
                        self.next();
                    }
                    "strict" => {
                        strictness = Strictness::Strict;
                        self.next();
                    }
                    _ => {}
                }
            }
            if self.peek() != Some(&Tok::LBrace) {
                return Err(SmlError::from_code_title(E_PARSE_019)
                    .with_detail(format!("@contract {cname} 后须 {{ ... }}")));
            }
            self.next();
            let mut requires: Vec<String> = Vec::new();
            // featag 声明（S4）：契约体内 `!name` / `!name: 类型` / `!name: 类型 -> 落点`
            let mut featags: Vec<FeatagDecl> = Vec::new();
            let mut strict_paths = false;
            let mut allow_any = false;
            let fields = self.parse_contract_body(
                &mut requires,
                &mut featags,
                &mut strict_paths,
                &mut allow_any,
            )?;
            // 定义时校验（不是应用时）：契约体里 `@requires` 声明的特性都必须已开启。
            // 特性集在解析前已由 `scan.rs` 把全文的 `@feature` 指令一并剥离并应用，
            // 因此此处校验的是「文档在任意位置声明过的特性」的并集，与书写顺序无关。
            for req in &requires {
                // 与 `@feature enable` 同一个判据（`lookup_feature` 是 `scan` 的私有函数，
                // 这里用同一套 `parse_name`）：带点名的形态规则不该因入口不同而不同。
                let f = Feature::parse_name(req).map_err(|e| {
                    SmlError::from_code_title(E_FEATURE_003).with_detail(format!(
                        "契约 `{cname}` 的 `@requires` 引用了无效特性 `{req}`：{e}"
                    ))
                })?;
                if !self.features.has(f) {
                    return Err(SmlError::from_code_title(E_CONTRACT_018).with_detail(
                        format!(
                            "契约 `{cname}` 要求特性 `{req}` 已开启（请用 `@feature enable {req}`）"
                        ),
                    ));
                }
            }
            // 命名空间前缀隔离：块内的契约按当前 ns 栈路径注册
            self.contracts.insert(
                self.qualify(&cname),
                Contract {
                    name: self.qualify(&cname),
                    fields,
                    strictness,
                    requires,
                    allow_any,
                    strict_paths,
                    featags,
                },
            );
            return Ok(());
        }
        // —— 自定义类型：`@type name: X { 模式 }` ——
        //
        // 类型体是普通 SML 数据（序列 / 类 / 次 / 任一 …），
        // 由 sml-pattern 编译为无回溯匹配器，供契约字段引用。
        // 参数须显式写作 `name: X`（v4 起位置参数形式已废弃）。
        if fname == "type" {
            // 仅 `@type name: X { ... }` 是自定义类型指令。
            //
            // `@type { ... }`（后无 `name:` 参数）仍须落回「名为 type 的
            // 片段定义」——syntax_guard 有专门的回归测试守着这条语义
            //（fragment_named_type_or_name_still_works），故不可抢占。
            let is_type_decl =
                matches!(self.peek(), Some(Tok::Word(w)) if w.as_ref() == "name")
                    && matches!(self.toks.get(self.i + 1), Some(Tok::Colon));
            if is_type_decl {
                self.next(); // 消费 name
                self.next(); // 消费 :
                let tname = match self.next() {
                    Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                    other => {
                        return Err(SmlError::from_code_title(E_PARSE_019)
                            .with_detail(format!("@type name: 后须类型名，得 {other:?}")))
                    }
                };
                if self.peek() != Some(&Tok::LBrace) {
                    return Err(SmlError::from_code_title(E_PARSE_019)
                        .with_detail(format!("@type {tname} 后须 {{ ... }} 模式体")));
                }
                self.next();
                let body = self.parse_block(Some(Tok::RBrace))?;
                // 命名空间前缀隔离：块内的类型按当前 ns 栈路径注册
                self.types.insert(self.qualify(&tname), body);
                return Ok(());
            }
            // 非指令形式：下落到片段定义逻辑（片段名 = "type"）
        }

        // —— 契约应用：`@is Name` 或 `@is a, b, c`（叠加）——
        if fname == "is" {
            if !self.features.has(Feature::Contract) {
                return Err(SmlError::from_code_msg(E_FEATURE_001).with_detail("`contract`"));
            }
            let raw = match self.next() {
                Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                other => {
                    return Err(SmlError::from_code_title(E_PARSE_019).with_detail(format!("@is 后须契约名，得 {other:?}")))
                }
            };
            // —— 多契约收集（D1，TASK-agent-w24 §2）——
            // `@is` 后**连续取裸词**：遇 `:` / `{` / `]` / **换行**即停；
            // 逗号作分隔（**可省**：`@is a, b` ≡ `@is a b`）。
            // ⚠️ 两条停机判据缺一不可：
            // - 「换行即停」：SML 一行可以只有 `@is X` 而把字段写在下一行，
            //   无脑收词会把下一行的键吞进契约名；
            // - 「裸词后跟 `:` 的是键」：`@is A host: h1` 的 host 是键值，
            //   不是契约名 —— 否则同行的键值永远写不了。
            // 行号未知（文件入口 / `@for` 体）⇒ **只收一个词**（旧行为），
            // 宁可少收，不可收错。
            let mut queue: Vec<(String, usize)> = vec![(raw.clone().into_owned(), self.i - 1)];
            // 块级严格度覆盖：契约原始名 → 覆盖档。严格度关键字紧跟在某个契约名之后，
            // 即绑定到那个契约（D3）：`@is X loose` / `@is X warn` / `@is X strict`。
            // 例：`@is A warn B` ⇒ A=Warn、B=契约级默认；`@is A B warn` ⇒ A=默认、B=Warn。
            let mut overrides: BTreeMap<String, Strictness> = BTreeMap::new();
            let mut last_name: Option<String> = Some(raw.clone().into_owned());
            let mut prev = self.i - 1;
            if !self.lines.is_empty() {
                loop {
                    let idx = self.i;
                    match self.peek().cloned() {
                        Some(Tok::Comma) => {
                            if !self.same_line(prev, idx) {
                                break;
                            }
                            self.next();
                            prev = idx;
                        }
                        Some(tok @ (Tok::Word(_) | Tok::Str(_))) => {
                            if !self.same_line(prev, idx) {
                                break;
                            }
                            let w = match tok {
                                Tok::Word(s) | Tok::Str(s) => s,
                                _ => unreachable!(),
                            };
                            // 紧跟契约名之后的严格度关键字 ⇒ 绑定到上一个契约（块级覆盖）
                            if matches!(w.as_ref(), "loose" | "warn" | "strict") {
                                if let Some(n) = last_name.take() {
                                    let s = match w.as_ref() {
                                        "loose" => Strictness::Loose,
                                        "warn" => Strictness::Warn,
                                        _ => Strictness::Strict,
                                    };
                                    overrides.insert(n, s);
                                    self.next();
                                    prev = idx;
                                    continue;
                                }
                            }
                            // 后跟 `:` 且又不是契约名 ⇒ 它是键，停
                            if matches!(self.peek_at(1), Some(Tok::Colon))
                                && self.resolve_contract_word(&w).is_err()
                            {
                                break;
                            }
                            self.next();
                            prev = idx;
                            queue.push((w.clone().into_owned(), idx));
                            last_name = Some(w.into_owned());
                        }
                        _ => break,
                    }
                }
            }
            // 逐词 resolve：`@is type(契约名)` 与 `@is 契约名` 等价（见
            // `resolve_contract_word` 的兼容说明）；每个词都必须命中契约表
            //（先裸名、再按当前 ns 前缀 qualify），否则 `E-CONTRACT-001`
            //（**带位置**，指向违规的那个词）。
            for (r, idx) in &queue {
                // opt-out（IDEAS I-2）：不校验契约是否存在，按原样记录 ——
                // 应用阶段已由 `self.apply_contracts` 整体跳过，故不会报错。
                if !self.apply_contracts {
                    let ov = overrides.get(r).copied();
                    if !applied_contracts.iter().any(|(en, _)| en == r) {
                        applied_contracts.push((r.clone(), ov));
                    }
                    continue;
                }
                match self.resolve_contract_word(r) {
                    Ok(n) => {
                        let ov = overrides.get(r).copied();
                        let entry = (n.clone(), ov);
                        if !applied_contracts.iter().any(|(en, _)| en == &n) {
                            applied_contracts.push(entry);
                        }
                    }
                    Err(display) => {
                        let hint = suggest_contract_name(&display, &self.contracts)
                            .map(|c| format!("（你是不是想写 `{c}`？）"))
                            .unwrap_or_default();
                        return Err(self.err_at(
                            E_CONTRACT_001,
                            format!("sml: 未定义的契约 `{}`{}", display, hint),
                            *idx,
                        ));
                    }
                }
            }
            return Ok(());
        }
        // —— 条件裁剪：`@when <条件>`（作用于紧邻的下一个字段/块）——
        //
        // 注意：`@when { .. }` / `@when type: X { .. }` 是**定义名为
        // when 的片段**，是既有合法语法，必须继续走下面的片段定义
        // 分支，不得被当指令拦截（否则会破坏已有文档）。
        // 故仅当后面不是片段形式时，才按条件指令处理。
        //
        // cargo feature `when` 关闭时整段不编译：此时 `@when <条件>`
        // 不是已知指令，会继续走片段定义分支并报出原有错误。
        #[cfg(feature = "when")]
        {
            let is_fragment_form = match self.peek() {
                Some(Tok::LBrace) => true,
                Some(Tok::Word(w)) => w == "type" || w == "name",
                _ => false,
            };
            if fname == "when" && !is_fragment_form {
                if !self.features.has(Feature::When) {
                    return Err(SmlError::from_code_msg(E_FEATURE_001)
                        .with_detail("`when`（请先写 `@feature enable when`）"));
                }
                if pending_when.is_some() {
                    return Err(SmlError::from_code_msg(E_PARSE_016));
                }
                let cond = crate::cond::eval_when_cond(self)?;
                *pending_when = Some(cond);
                return Ok(());
            }
        }
        // —— 弱绑定：`@weakbind <契约名> [<字段体>]`（作用于紧邻的下一个块）——
        //
        // 与 `@when` 同一套「挂起状态 + 下一个块消费」的机制，但**不碰契约层**：
        // 不 resolve、不校验、不填默认值，只把声明记进旁路表
        // （`docs/swsml/spec/weakbind.md` §5）。故这里只读 token，从不查 `self.contracts`。
        if fname == "weakbind" {
            if !self.features.has(Feature::Weak) {
                return Err(SmlError::from_code_msg(E_FEATURE_001)
                    .with_detail("`weakbind`（请先写 `@feature enable type.weak`）"));
            }
            if pending_weakbind.is_some() {
                return Err(SmlError::from_code_msg(E_PARSE_029)
                    .with_detail("连续两条 `@weakbind`：它只作用于紧邻的下一个块"));
            }
            let name_idx = self.i;
            let name = match self.next() {
                Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                other => {
                    return Err(SmlError::from_code_title(E_PARSE_019)
                        .with_detail(format!("@weakbind 后须契约名，得 {other:?}")))
                }
            };
            // 可选字段体：`{ 字段: 类型 … }`。只写名字时不消费任何块 ——
            // 形状完全由内置表提供，这正是「不写两遍」的落点。
            let body = if self.peek() == Some(&Tok::LBrace) {
                self.next();
                let mut fields = BTreeMap::new();
                loop {
                    match self.peek() {
                        Some(Tok::RBrace) => {
                            self.next();
                            break;
                        }
                        None => return Err(SmlError::from_code_msg(E_PARSE_001)),
                        _ => {}
                    }
                    let fkey = match self.next() {
                        Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                        other => {
                            return Err(SmlError::from_code_title(E_PARSE_006).with_detail(
                                format!("@weakbind 字段体期望字段名，得 {other:?}"),
                            ))
                        }
                    };
                    match self.next() {
                        Some(Tok::Colon) => {}
                        other => {
                            return Err(SmlError::from_code_title(E_PARSE_006).with_detail(
                                format!("`@weakbind` 的字段 `{fkey}` 后须冒号，得 {other:?}"),
                            ))
                        }
                    }
                    let spec = self.parse_field_spec()?;
                    fields.insert(fkey.into_owned(), spec);
                }
                Some(fields)
            } else {
                None
            };
            *pending_weakbind = Some(PendingWeak {
                name: name.into_owned(),
                body,
                line: self.line_of(name_idx),
            });
            return Ok(());
        }
        // —— 弱 profile 引用：`@weakdoc <名>`（只登记名字，库**不读文件系统**）——
        //
        // 项目 `.smlex` 目录 / 用户全局 / 系统三处命名空间的查找全在工具侧 ——
        // 「不能用于注入」的判据因此自动成立：库连文件都没碰、
        // 解析结果对有无 profile 必然一致（`docs/swsml/spec/weakdoc.md` §4）。
        if fname == "weakdoc" {
            if !self.features.has(Feature::Weak) {
                return Err(SmlError::from_code_msg(E_FEATURE_001)
                    .with_detail("`weakdoc`（请先写 `@feature enable type.weak`）"));
            }
            let name_idx = self.i;
            let name = match self.next() {
                Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                other => {
                    return Err(SmlError::from_code_title(E_PARSE_019)
                        .with_detail(format!("@weakdoc 后须 profile 名，得 {other:?}")))
                }
            };
            // 显式拒绝载荷块：那是 `@weakdata` 的形态，混淆了会让人以为 profile 能内联。
            if self.peek() == Some(&Tok::LBrace) {
                return Err(SmlError::from_code_title(E_PARSE_019).with_detail(
                    "@weakdoc 只写 profile 名，不接受载荷块（要内联数据请用 @weakdata）",
                ));
            }
            // 文档级判重：同一个 profile 不能重复挂（I-17 硬性约束 2）。
            if self.weak_docs.iter().any(|w| w.name == name) {
                return Err(SmlError::from_code_msg(E_PARSE_030)
                    .with_detail(format!("`{name}`（同一个 profile 不能重复挂）")));
            }
            self.weak_docs.push(WeakDoc {
                path: self.path_stack.clone(),
                name: name.into_owned(),
                line: self.line_of(name_idx),
            });
            return Ok(());
        }
        // —— 弱数据：`@weakdata <名> { … }`（载荷**不进主树**，只给拓展读）——
        //
        // 与 `@weakbind` 的分工：这里**没有契约**、内容**根本不进树**；
        // 库只负责「按名交出去」，不解释内容（`docs/swsml/spec/weakdata.md`）。
        if fname == "weakdata" {
            if !self.features.has(Feature::Weak) {
                return Err(SmlError::from_code_msg(E_FEATURE_001)
                    .with_detail("`weakdata`（请先写 `@feature enable type.weak`）"));
            }
            let name_idx = self.i;
            let name = match self.next() {
                Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                other => {
                    return Err(SmlError::from_code_title(E_PARSE_019)
                        .with_detail(format!("@weakdata 后须名字，得 {other:?}")))
                }
            };
            if self.peek() != Some(&Tok::LBrace) {
                return Err(SmlError::from_code_title(E_PARSE_019)
                    .with_detail("@weakdata 后须一个 `{ … }` 载荷块"));
            }
            self.next(); // `{`
            let value = self.parse_block(Some(Tok::RBrace))?;
            let entry = WeakData {
                path: self.path_stack.clone(),
                name: name.into_owned(),
                value,
                line: self.line_of(name_idx),
            };
            // 同名（同路径）后者覆盖：提示通道按名取值，天然「最后一次生效」。
            match self
                .weak_data
                .iter_mut()
                .find(|w| w.path == entry.path && w.name == entry.name)
            {
                Some(slot) => *slot = entry,
                None => self.weak_data.push(entry),
            }
            return Ok(());
        }
        // 可选显式参数：`type: X` 与 `name: Y`（顺序不限、均可省略）。
        //
        // v4 起废弃 v3 的位置参数形式（`@f Server [prod] { .. }`）。
        // 原因：位置参数使 `@nosuch Word { .. }`（拼错的指令）与
        // 「片段定义 + type 参数」在 token 流上完全同形，解析器无法判别，
        // 只能把块当片段体吃掉 —— 内容被静默丢弃且不报错。
        // 改为显式关键字后，任何位置参数形式都可安全地判为错误。
        let mut ftype: Option<String> = None;
        let mut farg: Option<String> = None;
        loop {
            // 仅当 `type`/`name` 后紧邻冒号时才视为参数，
            // 这样名为 `type`/`name` 的片段（`@type { .. }`）仍可正常定义。
            let is_param = matches!(self.peek(), Some(Tok::Word(w)) if w == "type" || w == "name")
                && matches!(self.peek_at(1), Some(Tok::Colon));
            if !is_param {
                break;
            }
            let kw = match self.next() {
                Some(Tok::Word(w)) => w,
                _ => unreachable!("is_param 已保证为 Word"),
            };
            self.next(); // 冒号
            let val = match self.next() {
                Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                other => {
                    return Err(SmlError::from_code_title(E_PARSE_020).with_detail(format!(
                        "片段 `@{fname}` 的参数 `{kw}:` 后须值，得 {other:?}"
                    )))
                }
            };
            if kw == "type" {
                if ftype.is_some() {
                    return Err(SmlError::from_code_title(E_PARSE_020)
                        .with_detail(format!("片段 `@{fname}` 的 `type:` 参数重复")));
                }
                ftype = Some(val.into_owned());
            } else {
                if farg.is_some() {
                    return Err(SmlError::from_code_title(E_PARSE_020)
                        .with_detail(format!("片段 `@{fname}` 的 `name:` 参数重复")));
                }
                farg = Some(val.into_owned());
            }
        }
        // 既非 `{` 也非流末尾：既可能是拼错的指令，也可能是旧的位置参数形式。
        // 两种意图无法区分，故错误信息同时给出两条排查指引。
        if !matches!(self.peek(), Some(Tok::LBrace) | None) {
            return Err(SmlError::from_code_title(E_PARSE_005).with_detail(format!(
                "`@{fname}` 缺少片段体 `{{ ... }}`；参数须显式写作 `type: X` 与 `name: Y`；\
                 位置参数形式自 v4 起已废弃；若是指令请检查拼写"
            )));
        }
        if self.peek() == Some(&Tok::LBrace) {
            self.next();
            let blk = self.parse_block(Some(Tok::RBrace))?;
            let mut sub = match &blk {
                Value::Object(m) => m.clone(),
                other => {
                    let mut m = self.new_map();
                    m.insert("_value".into(), other.clone());
                    m
                }
            };
            if let Some(t) = ftype {
                sub.insert("__type".into(), Value::Str(t));
            }
            if let Some(a) = farg {
                sub.insert("__name".into(), Value::Str(a));
            }
            if !self.features.has(Feature::Fragment) {
                return Err(SmlError::from_code_msg(E_FEATURE_001)
                    .with_detail("`fragment`"));
            }
            // 命名空间前缀隔离：片段定义按当前 ns 栈路径注册
            self.fragments.insert(self.qualify(&fname), Value::Object(sub));
        } else {
            // `@name` 既不是已知指令（contract/is）也不是片段定义（后无 `{`）：
            // 必须报错，而非静默吞掉后续行/块（P0-2：单独 `@` 会清空整个文档）。
            return Err(SmlError::from_code_title(E_PARSE_005).with_detail(format!(
                "`@{fname}` 不是合法指令且缺少片段体，无法解析"
            )));
        }
        Ok(())
    }
    /// 把 `sml-contract` 产出的契约告警转译为 `Diagnostic::Warning` 并收进 `diags`。
    ///
    /// `sml-contract` 不能依赖 `sml-parse`（否则循环依赖），故内部用独立的
    /// [`ContractWarning`](sml_contract::ContractWarning)；此处转译。
    /// warn 态告警只在 `*_with_warnings` 入口可见：`parse()` / `parse_file()`
    /// 在 `api.rs` 里直接丢掉 `diags`，故对既有调用方零影响。
    /// 落一条弱绑定：位置取当前 `path_stack`。
    ///
    /// 调用时机很关键 —— 必须在**目标块的键已压栈之后**、子帧开出来之时。
    /// 此时 `path_stack` 恰好指向那个块，与 [`ContractMap`] 的定位口径一致。
    fn record_weak_bind(&mut self, pending: PendingWeak) {
        let path = self.path_stack.clone();
        // 声明侧软校验（名字 / 字段名 / 字段类型）现在就能做；块内容留到收尾。
        self.check_weak_decl(&pending.name, pending.body.as_ref(), pending.line);
        self.weak_pending
            .push((path.clone(), pending.name.clone(), pending.body.clone()));
        self.weak_binds.push(WeakBind {
            path,
            name: pending.name,
            body: pending.body,
            line: pending.line,
        });
    }

    /// 记一条弱绑定软校验告警（`W-CONTRACT-001`）：**不阻断解析、不改主树**。
    ///
    /// 八档差异写在消息前缀里（`（decl.unknown_contract）` 这种），因为
    /// [`Diagnostic`] 没有 `detail` 字段 —— 见 `docs/swsml/spec/weakbind.md` §4。
    fn push_weak_warn(&mut self, line: Option<usize>, detail: &str, msg: String) {
        self.diags.push(Diagnostic {
            kind: DiagnosticKind::WeakBindMismatch,
            message: format!("sml: 弱声明与内置契约不匹配（{detail}）：{msg}"),
            line,
        });
    }

    /// 声明侧软校验：契约名在内置表里吗？体的字段名 / 类型对得上吗？
    ///
    /// **不查 `self.contracts`**：`@weakbind` 认的是**宿主内置**那份表，
    /// 与文档里 `@contract` 定义的契约互不相干（这正是本项的意义）。
    fn check_weak_decl(
        &mut self,
        name: &str,
        body: Option<&BTreeMap<String, FieldSpec>>,
        line: Option<usize>,
    ) {
        if self.builtin_contracts.is_empty() {
            return;
        }
        let mut msgs: Vec<(&'static str, String)> = Vec::new();
        match self.builtin_contracts.get(name) {
            None => msgs.push((
                "decl.unknown_contract",
                format!("内置契约表里没有 `{name}`"),
            )),
            Some(c) => {
                if let Some(body) = body {
                    for (fname, spec) in body {
                        match c.fields.get(fname) {
                            None => msgs.push((
                                "decl.unknown_field",
                                format!("契约 `{name}` 没有字段 `{fname}`"),
                            )),
                            Some(bi) if bi.ty != spec.ty => msgs.push((
                                "decl.type_mismatch",
                                format!(
                                    "字段 `{name}.{fname}` 的类型与内置契约不同（声明 `{:?}`，内置 `{:?}`）",
                                    spec.ty, bi.ty
                                ),
                            )),
                            _ => {}
                        }
                    }
                }
            }
        }
        for (detail, msg) in msgs {
            self.push_weak_warn(line, detail, msg);
        }
    }

    /// 块内容软校验（块收尾时做）：缺必填 / 类型 / 区间 / 枚举 / 未声明字段。
    ///
    /// **只读**：不填默认值、不删不改任何键 —— 「有无 `@weakbind` 主树逐字节相同」
    /// 这条核心判据全靠这里守住（§9 #1 / #10）。
    fn check_weak_block(&mut self, node: &Map) {
        if self.builtin_contracts.is_empty() || self.weak_pending.is_empty() {
            return;
        }
        let path = self.path_stack.clone();
        let pos = match self.weak_pending.iter().position(|(p, _, _)| *p == path) {
            Some(i) => i,
            None => return,
        };
        let (_, name, _) = self.weak_pending.remove(pos);
        let contract = match self.builtin_contracts.get(&name).cloned() {
            Some(c) => c,
            None => return,
        };

        let mut msgs: Vec<(&'static str, String)> = Vec::new();
        for (fname, spec) in &contract.fields {
            match node.get(fname) {
                None if spec.required => msgs.push((
                    "block.missing_required",
                    format!("块里缺必填字段 `{name}.{fname}`"),
                )),
                None => {}
                Some(v) => {
                    if let Err(e) =
                        sml_contract::check_value(&name, fname, spec, v, &self.builtin_contracts)
                    {
                        // 三段细分靠码映射：越界 / 枚举 / 其余都算类型不符。
                        let detail = match e.code() {
                            "E-CONTRACT-005" => "block.out_of_range",
                            "E-CONTRACT-006" => "block.enum_mismatch",
                            _ => "block.type_mismatch",
                        };
                        msgs.push((
                            detail,
                            format!("字段 `{name}.{fname}` 不合内置契约：{}", e.message()),
                        ));
                    }
                }
            }
        }
        // 未声明字段：只有**严格**契约才报 —— `warn` / `loose` 本来就是宽容档。
        if contract.strictness == Strictness::Strict {
            for k in node.keys() {
                if !contract.fields.contains_key(k) {
                    msgs.push((
                        "block.undeclared_field",
                        format!("契约 `{name}` 未声明字段 `{k}`"),
                    ));
                }
            }
        }
        // 收尾点没有单一源位置（块可能跨多行、可能来自 include）⇒ `line: None`，
        // 与 `Diagnostic::line` 的既有取舍一致：宁缺勿错。
        for (detail, msg) in msgs {
            self.push_weak_warn(None, detail, msg);
        }
    }

    fn record_contract_warnings(&mut self, ws: Vec<ContractWarning>) {
        for w in ws {
            self.diags.push(Diagnostic {
                kind: DiagnosticKind::Warning,
                message: w.message,
                line: None,
            });
        }
    }

    /// 单步推进块解析：消费一个 token 并更新 `frame`，返回当前应否结束本块。
    ///
    /// 迭代式改造第一步（纯重构、行为零变化）：原 `parse_block_inner` 的主循环体
    /// 原封不动搬到这里，`node`/`applied_contracts`/`pending_when`/`closing` 收进
    /// `frame`。`Ok(Finish)` 等价于原循环里的 `break`（本块完成），`Ok(Continue)`
    /// 等价于 `continue`（继续消费下一个 token），`Err` 原样冒泡。
    fn block_step(&mut self, frame: &mut BlockFrame) -> Result<Step, SmlError> {
        match self.peek() {
            None => {
                // 文件结尾：若本块期望闭合符号（嵌套块/数组），则未闭合，必须报错；
                // 否则为顶层正常结束。
                if frame.closing.is_some() {
                    let want = match frame.closing {
                        Some(Tok::RBrace) => "}",
                        Some(Tok::RBrack) => "]",
                        _ => unreachable!(),
                    };
                    return Err(SmlError::from_code_msg(E_PARSE_001)
                        .with_detail(format!("缺少结束符号 {want}")));
                }
                Ok(Step::Finish)
            }
            // 右符号：变体是单元变体，用 `&Tok::…` 模式即可借着匹配，无需克隆
            Some(&Tok::RBrace) | Some(&Tok::RBrack) => {
                let got = match self.peek() {
                    Some(&Tok::RBrace) => "}",
                    _ => "]",
                };
                if let Some(cl) = &frame.closing {
                    // 原判据是 `*cl == tok`（同符号即匹配）；不克隆 ⇒ 比判别式
                    let same = matches!(
                        (cl, self.peek().unwrap()),
                        (Tok::RBrace, Tok::RBrace) | (Tok::RBrack, Tok::RBrack)
                    );
                    if same {
                        self.next();
                        return Ok(Step::Finish);
                    }
                    // 嵌套块内遇到类型不匹配的右符号（如 `}` 与 `]` 混用）：
                    // 必须报错，而非静默吞掉，否则后续内容会被错误吞并。
                    let want = match cl {
                        Tok::RBrace => "}",
                        Tok::RBrack => "]",
                        _ => unreachable!(),
                    };
                    return Err(SmlError::from_code_title(E_PARSE_002)
                        .with_detail(format!("期望 {want}，却遇到 {got}")));
                }
                // 顶层遇到多余的右括号 `}` / `]`：必须报错（之前静默忽略，
                // 会掩盖作者漏写的 `key:`、错配括号等问题）。
                Err(SmlError::from_code_title(E_PARSE_003).with_detail(got))
            }
            Some(&Tok::Comma) => {
                // 逗号在 SML 中只用于对象字段分隔（`k: v, k2: v2`）与数组元素分隔
                // （由 `parse_array_inner` 处理，不会到此处）。
                // 此处遇到的逗号若属于「对象字段分隔」，其后应是 `key:` 形式；
                // 否则是裸词拆分（如 `a: x,y`）或非法孤立逗号，必须报错而非静默吞掉
                // （P2-7：裸词逗号会凭空拆出第二个键）。
                let looks_like_field_sep = matches!(
                    self.toks.get(self.i + 1),
                    Some(Tok::Word(_)) | Some(Tok::Str(_))
                ) && matches!(self.toks.get(self.i + 2), Some(Tok::Colon));
                if looks_like_field_sep {
                    self.next();
                    return Ok(Step::Continue);
                }
                Err(SmlError::from_code_msg(E_PARSE_007))
            }
            // 孤立 `@`：其后没有片段名/指令名。必须报错——否则其后紧跟的
            // 块会被当作片段体消费，导致内容被静默丢弃（不报错）。
            Some(&Tok::BareAt) => {
                Err(SmlError::from_code_msg(E_PARSE_004))
            }
            Some(&Tok::At) => {
                self.parse_at_directive(
                    &mut frame.node,
                    &mut frame.applied_contracts,
                    &mut frame.pending_when,
                    &mut frame.pending_weakbind,
                )?;
                Ok(Step::Continue)
            }
            _ => {
                // key
                let key = match self.next() {
                    Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                    other => {
                        // 位置指向**刚消费掉的那个 token**（游标已越过它）。
                        let pos = self.i.saturating_sub(1);
                        // 裸 `:` 绝大多数来自 `::`（如 `a: B::C`）或 URL（`a: http://x`）：
                        // 词法把 `::` 切成两个冒号，后一个落到键位置 ⇒「期望键」。
                        // 直接点名成因与修法（含 `:` 的标量请加引号），免得用户只能二分定位（TODO L162）。
                        let msg = if matches!(other, Some(Tok::Colon)) {
                            "sml: 键位置出现裸 `:`（常见于 `::` 或 `http://` 这类含冒号的未加引号标量）。\
                             含 `:` 的标量请加引号，例如 `a: \"B::C\"`"
                                .to_string()
                        } else {
                            format!("sml: 期望键, 得 {:?}", other)
                        };
                        return Err(self.err_at(E_PARSE_006, msg, pos));
                    }
                };
                // 防御：键名位置出现 `${var}` 插值语法时**显式报错**，而非静默生成
                // 错误结构。`${var}` 中的 `{`/`}` 会被 tokenize 当成块边界，导致键被拆成
                // `$` + 裸块，最终解析成垃圾对象。两种残留形态都要拦：
                //   - key == "$" 且下一个 token 是 `{`：原样 `${...}` 被拆开的残骸；
                //   - key 文本直接含 `${`：未触发块边界的其它情形。
                // 当前 `@for` 的 `${var}` 仅允许出现在**值位置**（`"${h}"`）；需要动态键名
                // 时请用数组表达（`hosts: @for h in a b { name: "${h}" }`）。
                // 键 token 的序号（游标已越过它）：供键冲突告警定位行号。
                let key_idx = self.i.saturating_sub(1);
                let looks_like_key_interp =
                    key.contains("${") || (key == "$" && self.peek() == Some(&Tok::LBrace));
                if looks_like_key_interp {
                    return Err(SmlError::from_code_msg(E_PARSE_013));
                }
                // 标识符白名单（S1b，门控 `lex.ident`）：键名不合规就在**这里**报，
                // 不等下游（下游只会把 `a.b` 当成路径切错，症状离病因很远）。
                // 片段合并键 `&name` 不是标识符，跳过 —— 它由片段表判对错。
                if self.features.has(Feature::Ident) && !key.starts_with('&') {
                    sml_lex::identifiers::check_ident_err(&key)?;
                }
                let colon = self.peek() == Some(&Tok::Colon);
                if colon {
                    self.next();
                }
                // 裸片段合并：块内 `&name`（键位置、无冒号）把片段字段合并进当前块。
                // 对齐教程（ch03/ch08）与 JS 引擎的实际行为：
                // - 片段字段插入 = 直接赋值（同名时以片段为准）
                // - 显式字段撞已有键 = 提升数组（走下方统一同名规则）
                // `key: &name`（带冒号）仍是值引用，互不影响。
                if !colon && key.starts_with('&') {
                    // `@weakbind` 只认块：片段引用（`&frag`）不是块 ⇒ 响亮拒绝，不静默忽略
                    if frame.pending_weakbind.take().is_some() {
                        return Err(SmlError::from_code_msg(E_PARSE_029)
                            .with_detail("`@weakbind` 后面必须紧跟一个块（这里是片段引用）"));
                    }
                    let resolved = coerce_word(
                        &key,
                        &self.fragments,
                        self.features,
                        &self.ns_prefix(),
                        Some(&self.env),
                    )?;
                    // `@when` 条件为假：片段已解析（token 已消费）但不写入
                    if let Some(false) = frame.pending_when.take() {
                        return Ok(Step::Continue);
                    }
                    match &resolved {
                        Value::Object(m) => {
                            for (fk, fv) in m {
                                // 静默改数据（雷 5 · A 组第 5 项）：片段合并是**直接赋值**，
                                // 不走下方「同名提升为数组」的统一规则 ⇒ 父块已有的同名键
                                // 会被**整个丢掉**，且不报错。语义保留（ch03/ch08 既定），
                                // 但必须让调用方看见 —— 故记一条 KeyOverwritten。
                                if frame.node.contains_key(fk) {
                                    self.push_key_diag(
                                        DiagnosticKind::KeyOverwritten,
                                        key_idx,
                                        format!(
                                            "sml: 父块原有的键 `{fk}` 被片段 `{}` 的展开值覆盖\
                                             （父块原值已丢失；片段合并不走重复键的数组提升）",
                                            key
                                        ),
                                    );
                                }
                                // §6.3：已存在的键**原位替换**（`Object::insert` 保证），
                                // 不存在的追加到尾部 —— 正是 `&frag` 拼接要的语义。
                                frame.node.insert(fk.to_string(), fv.clone());
                            }
                        }
                        other => {
                            return Err(SmlError::from_code_title(E_INCLUDE_007).with_detail(
                                format!("片段 `{key}` 展开结果不是对象，无法合并: {other:?}"),
                            ));
                        }
                    }
                    return Ok(Step::Continue);
                }
                // `@when` 条件：先取出 —— 容器值路径要把它翻译进 `Cont::Field`，
                // 在子容器闭帧归并时才做「保留/丢弃」裁决；标量路径在此直接判断。
                let pend_when = frame.pending_when.take();
                let pend_weak = frame.pending_weakbind.take();
                match self.value_step(&key, colon, key_idx, pend_when, pend_weak)? {
                    ValueStep::Leaf(val) => {
                        // `@when` 条件为假：值已解析（token 已消费），但不写入父节点。
                        // 必须先解析再丢弃，否则其 token 会被当成下一个键，导致误报。
                        if pend_when == Some(false) {
                            return Ok(Step::Continue);
                        }
                        // 同名冲突 -> 提升为数组（静默改数据：SML 既定语义，改告警）。
                        self.insert_field_val(&mut frame.node, key.into_owned(), key_idx, val);
                        Ok(Step::Continue)
                    }
                    // 子容器：交给显式栈（`Cont::Field` 已携带 key/key_idx/when）。
                    ValueStep::Open(child) => Ok(Step::Open(child)),
                }
        }
    }
    }

    // —— 显式栈驱动核心（迭代式改造第二步，作业书 §3）——
    //
    // 把原 `parse_block_inner` / `parse_bare_block` / `parse_value` / `parse_array(_inner)`
    // 的递归漏斗整体翻成「单 token 单步 + 显式栈」：每个容器（`{}` / `[]` / 裸块）是一帧
    // `Frame`，开帧（`Open`）压栈、闭帧（`Finish`）弹栈归并。原生调用栈深度恒为常数，
    // 深度守卫改由显式栈层数（`self.depth`）承担 —— `_stack_probe` 下 256KB 栈 128 层
    // 不崩的验收至此达成。
    //
    // `root` 是根帧（`Cont::Root`）。循环推进栈顶帧：`Continue` 继续消费，`Finish` 弹帧
    // 归并，`Open(child)` 守卫深度后压入子帧。栈空时根帧的 `Value` 即结果。
    fn run_stack(&mut self, root: Frame) -> Result<Value, SmlError> {
        // 口径（各端一致，**已实测钉住**）：`depth` 是「进入首帧**前**已进入的层数」
        // （根为 0），故允许 `depth == MAX_VALUE_DEPTH` —— 第 128 层放行、第 129 层报此码。
        if self.depth > MAX_VALUE_DEPTH {
            return Err(SmlError::from_code_msg(E_LIMIT_001)
                .with_detail(format!("超过 {MAX_VALUE_DEPTH} 层")));
        }
        self.depth += 1;
        let mut stack = vec![root];
        loop {
            let n = stack.len() - 1;
            let i_before = self.i;
            let step = match &mut stack[n] {
                Frame::Block { state, .. } => match self.block_step(state) {
                    Ok(s) => s,
                    Err(e) => {
                        if !self.collect_errors {
                            return Err(e);
                        }
                        self.push_error(e);
                        Self::recover_block(&self.toks, &mut self.i);
                        if self.i == i_before {
                            break; // 同步点没前移 ⇒ 停在此帧，避免死循环
                        }
                        Step::Continue
                    }
                },
                Frame::Array { arr, .. } => match self.array_step(arr) {
                    Ok(s) => s,
                    Err(e) => {
                        if !self.collect_errors {
                            return Err(e);
                        }
                        self.push_error(e);
                        Self::recover_array(&self.toks, &mut self.i);
                        if self.i == i_before {
                            break;
                        }
                        Step::Continue
                    }
                },
            };
            match step {
                Step::Continue => {}
                Step::Finish => {
                    self.depth -= 1;
                    if let Some(v) = self.pop_frame(&mut stack)? {
                        return Ok(v);
                    }
                }
                Step::Open(child) => {
                    if self.depth > MAX_VALUE_DEPTH {
                        return Err(SmlError::from_code_msg(E_LIMIT_001)
                            .with_detail(format!("超过 {MAX_VALUE_DEPTH} 层")));
                    }
                    self.depth += 1;
                    stack.push(child);
                }
            }
        }
        // 仅在 `collect_errors` 模式会走到这里（无进展保护触发）：
        // 返回根帧当前的**部分结果**，错误已收进 `self.errors`。
        let root = stack.into_iter().next();
        Ok(match root {
            Some(Frame::Block { state, .. }) => Value::Object(state.node),
            Some(Frame::Array { arr, .. }) => Value::Array(arr),
            None => Value::Null,
        })
    }

    /// 记录一条错误（`collect_errors` 模式），设上限防跑飞。
    fn push_error(&mut self, e: SmlError) {
        const MAX_ERRORS: usize = 64;
        if self.errors.len() < MAX_ERRORS {
            self.errors.push(e);
        }
    }

    /// 块内错误恢复：跳到「同层的新字段（`Word/Str` 紧跟 `:`）」/ `@` 指令 /
    /// 本帧闭合 `}` / EOF；同步点本身**不消费**，交给下一轮正常处理。
    fn recover_block(toks: &[Tok<'_>], i: &mut usize) {
        let mut depth: i32 = 0;
        let mut k = *i;
        while let Some(tok) = toks.get(k) {
            match tok {
                Tok::LBrace | Tok::LBrack => {
                    depth += 1;
                    k += 1;
                }
                Tok::RBrace | Tok::RBrack => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                    k += 1;
                }
                Tok::Word(_) | Tok::Str(_) if depth == 0 => {
                    if matches!(toks.get(k + 1), Some(Tok::Colon)) {
                        break; // 新字段的起点
                    }
                    k += 1;
                }
                Tok::At if depth == 0 => {
                    k += 1;
                    break; // 指令起点
                }
                _ => k += 1,
            }
        }
        *i = k;
    }

    /// 数组内错误恢复：跳到本帧闭合 `]` 或同层 `,`（消费逗号，作为下一元素起点）。
    fn recover_array(toks: &[Tok<'_>], i: &mut usize) {
        let mut depth: i32 = 0;
        let mut k = *i;
        while let Some(tok) = toks.get(k) {
            match tok {
                Tok::LBrace | Tok::LBrack => {
                    depth += 1;
                    k += 1;
                }
                Tok::RBrace | Tok::RBrack => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                    k += 1;
                }
                Tok::Comma if depth == 0 => {
                    k += 1;
                    break;
                }
                _ => k += 1,
            }
        }
        *i = k;
    }

    /// 弹出一帧并应用收尾（块：悬空 `@when` + `@is` 契约 + 契约映射 + 裸块元数据/类型
    /// 契约；数组：清路径），再依 `cont` 把结果交回父帧。`Some(v)` = 根帧已结束，`v` 即结果。
    fn pop_frame(&mut self, stack: &mut Vec<Frame>) -> Result<Option<Value>, SmlError> {
        let frame = stack.pop().expect("run_stack: Finish 时栈不应为空");
        Ok(match frame {
            Frame::Block { state, cont } => {
                let is_root = matches!(cont, Cont::Root);
                let v = self.finish_block(state, is_root)?;
                match cont {
                    Cont::Root => Some(v),
                    c => {
                        self.merge_value(c, v, stack);
                        None
                    }
                }
            }
            Frame::Array { arr, cont } => {
                if !matches!(cont, Cont::Root) {
                    self.path_stack.pop();
                }
                let v = Value::Array(arr);
                match cont {
                    Cont::Root => Some(v),
                    c => {
                        self.merge_value(c, v, stack);
                        None
                    }
                }
            }
        })
    }

    /// 块帧收尾：原 `parse_block_inner` 尾部的悬空 `@when` 检查 + `@is` 契约 + 契约映射，
    /// 以及 `parse_bare_block` 的裸块元数据 / typed-block 类型契约。
    /// `is_root` 表根帧：根帧建帧时未压 path/ns，故跳过对应弹栈（避免弹到外层栈顶）。
    fn finish_block(&mut self, state: BlockFrame, is_root: bool) -> Result<Value, SmlError> {
        // 悬空的 `@when`：条件后没有跟随任何字段/块，几乎可以肯定是笔误
        //（例如把 `@when` 写在了块末尾，或条件本想作用于块内却被写到了块外）。
        // 静默忽略会让作者以为条件生效了，故显式报错。
        if state.pending_when.is_some() {
            let e = SmlError::from_code_msg(E_PARSE_017);
            if self.collect_errors {
                self.push_error(e);
            } else {
                return Err(e);
            }
        }
        // 同理：`@weakbind` 后面没等到块（写在了块末尾，或后面只有标量）。
        // 静默放过会让作者以为这条声明生效了 —— 而它连目标都没有。
        if state.pending_weakbind.is_some() {
            let e = SmlError::from_code_msg(E_PARSE_029)
                .with_detail("`@weakbind` 后未跟随任何块（它只作用于紧邻的下一个块）");
            if self.collect_errors {
                self.push_error(e);
            } else {
                return Err(e);
            }
        }
        // 弱绑定的**块内容**软校验必须在这里做：块已填完，而 `path_stack` 还没弹
        // （弹栈在下面的 `is_root` 分支里）。只读：不填默认值、不改树。
        self.check_weak_block(&state.node);
        let BlockFrame { mut node, applied_contracts, bare, .. } = state;
        // 块结束：若声明了 `@is`，应用契约（填默认值 + 校验 + 严格性检查）。
        // `apply_contracts(false)` 时整段跳过（IDEAS I-2 的 opt-out）。
        if self.apply_contracts {
            // featag S8：`@is` 这条路径也要过「`any` 默认禁用」——与 typed-block 同一判据，
            // 否则同一个契约换个应用方式（`@is` vs 块头）就得到不同结果。
            if self.features.has(Feature::Featag) && !applied_contracts.is_empty() {
                let names: Vec<String> = applied_contracts.iter().map(|(n, _)| n.clone()).collect();
                self.check_any_allowed(&names)?;
            }
            if applied_contracts.len() == 1 {
                // 单契约：**逐字走原路径**（错误文案 / 行为与旧版完全一致）
                let (cname, ov) = &applied_contracts[0];
                let hint = suggest_contract_name(cname, &self.contracts)
                    .map(|c| format!("（你是不是想写 `{c}`？）"))
                    .unwrap_or_default();
                match self.contracts.get(cname).cloned() {
                    Some(mut c) => {
                        // D4①：块级显式覆盖 > 契约级声明（本块把有效严格度降到 `ov`）
                        if let Some(s) = ov {
                            c.strictness = *s;
                        }
                        let mut ws = Vec::new();
                        match apply_contract(&c, &mut node, &self.contracts, &self.ns_prefix(), &mut ws) {
                            Ok(()) => self.record_contract_warnings(ws),
                            Err(e) if self.collect_errors => self.push_error(e),
                            Err(e) => return Err(e),
                        }
                    }
                    None => {
                        let e = SmlError::from_code_title(E_CONTRACT_001)
                            .with_detail(format!("`{cname}`{hint}"));
                        if self.collect_errors {
                            self.push_error(e);
                        } else {
                            return Err(e);
                        }
                    }
                }
            } else if applied_contracts.len() > 1 {
                // 多契约：叠加（D3/D4②：strict 优先、顺序无关、字段并集、冲突 ⇒ E-CONTRACT-016）
                let mut ws = Vec::new();
                match apply_contracts(&applied_contracts, &mut node, &self.contracts, &self.ns_prefix(), &mut ws) {
                    Ok(()) => self.record_contract_warnings(ws),
                    Err(e) if self.collect_errors => self.push_error(e),
                    Err(e) => return Err(e),
                }
            }
        }
        // 契约映射：本块（路径 = path_stack，开帧时已把键/下标压好）应用了哪些契约。
        if let Some(map) = &mut self.contract_map {
            for (n, _) in &applied_contracts {
                map.record(self.path_stack.clone(), n.clone());
            }
        }
        // 裸块：先弹其命名空间（typed-block 契约解析需用外层 ns，与原实现时序一致），
        // 再补全 `__type`/`__name`/`__args` 元数据并应用块级类型契约。
        if let Some(bare) = &bare {
            if !is_root {
                self.ns_stack.pop();
            }
            match self.finish_bare_block(&mut node, bare) {
                Ok(()) => {}
                Err(e) if self.collect_errors => self.push_error(e),
                Err(e) => return Err(e),
            }
        }
        if !is_root {
            self.path_stack.pop();
        }
        Ok(Value::Object(node))
    }

    /// 裸块元数据补全 + typed-block 类型契约（从原 `parse_bare_block` 逐字搬出；
    /// 此刻 `bare` 帧已闭合、其命名空间已弹，故契约解析用外层 ns）。
    fn finish_bare_block(&mut self, m: &mut Map, bare: &BareCtx) -> Result<(), SmlError> {
        let key = &bare.key;
        let args = &bare.args;
        // —— 块级类型标注：`<契约名>… <块名> { .. }`（D2，TASK-agent-w24 §2）——
        //
        // 裸块首词若命中契约表，自动把该契约应用到本块，等价于块内首行
        // `@is 契约名`。与既有裸块 `type [name...] { }` 完全同形，故**零新语法**：
        // 未命中契约表时行为与旧版一致（首词仅作 `__type` 元数据）。
        // 由 opt-in 特性 `typed-block` 门控，未开启时既有文档不受任何影响。
        //
        // **多契约**：从首词起，**连续命中契约表**的词都是契约；第一个未命中
        // 的词是块名。仅当**契约词 ≥ 2** 才走叠加；只有 1 个契约词时逐字保持
        // 旧行为（回归红线：`server web prod { }` ⇒ `__type: server` +
        // `__name: web` + `__args: [prod]`）。
        //
        // 刻意**先于** `__type` / `__name` 的写入：这两个键是语法元数据，
        // 不应作为数据字段参与 strict 契约的「未声明字段」检查
        // （JS 侧以跳过这两个键达到同样效果）。
        let mut names: Vec<String> = Vec::new();
        let mut consumed_args = 0usize; // args 里被算作契约词的数量
        if self.apply_contracts && self.features.has(Feature::TypedBlock) && self.features.has(Feature::Contract) {
            // 命名空间隔离：先按裸名查，再按当前 ns 前缀查（与 `@is` 一致）
            let resolve = |w: &str| -> Option<String> {
                if self.contracts.contains_key(w) {
                    Some(w.to_string())
                } else {
                    let q = self.qualify(w);
                    if self.contracts.contains_key(&q) {
                        Some(q)
                    } else {
                        self.contract_by_first_letter(w)
                    }
                }
            };
            if let Some(r) = resolve(key) {
                names.push(r);
                for a in args {
                    match a {
                        Value::Str(s) => match resolve(s) {
                            Some(x) => {
                                names.push(x);
                                consumed_args += 1;
                            }
                            None => break, // 首个非契约词 = 块名，其后照旧进 args
                        },
                        _ => break,
                    }
                }
            }
            // —— 拆雷：typed-block 模式下，前缀里"认不出来"的词**不再**静默当块名 ——
            //
            // 旧行为：`Rule Securty ssh { .. }` 里拼错的 `Securty` 会被静默当成块名，
            // 真正的块名 `ssh` 被挤进 `__args`，全程不报错 ⇒ 契约写错＝静默失效。
            // 现在：未开启 `block-name` 时直接报错，拼错的契约名立刻暴露。
            //
            // ⚠️ 只在**首词命中了契约表**（typed-block 模式）时生效；普通裸块
            // `server web prod { }` 的既有行为不受影响（回归红线）。
            if !names.is_empty() && args.len() > consumed_args && !self.features.has(Feature::BlockName) {
                if let Value::Str(w) = &args[consumed_args] {
                    let hint = suggest_contract_name(w, &self.contracts)
                        .map(|c| format!("（你是不是想写 `{c}`？）"))
                        .unwrap_or_default();
                    return Err(SmlError::from_code_title(E_CONTRACT_001).with_detail(
                        format!(
                            "块级标注 `{key}` 后的 `{w}` 不是已定义的契约；\
                             若本意是把它作为块名，请先 `@feature enable block-name`{hint}"
                        ),
                    ));
                }
            }
            // —— featag S8：`any` 默认禁用，写了 `!any` 的契约才放行 ——
            if self.features.has(Feature::Featag) {
                self.check_any_allowed(&names)?;
            }

            // —— featag S7：`from-featag` 的键不许文档手写 ——
            //
            // ⚠️ **必须在填充之前**：填充之后 featag 自己写的键与文档手写的键长得一样，
            // 就分不出「来源合法」还是「伪造」了 —— 而来源正是这条规则的**全部意义**。
            if self.features.has(Feature::Featag) {
                for n in &names {
                    if let Some(c) = self.contracts.get(n) {
                        sml_contract::check_from_featag(c, m, &self.contracts, "")?;
                    }
                }
            }

            // —— featag S5（校验）+ S6（按落点写值）——
            //
            // ⚠️ **必须都在契约应用之前**：`from-featag` 那类「只能由 featag 提供」的必填字段，
            // 一旦让契约先跑，就会被判成「必填字段缺失」（E-CONTRACT-003），
            // 而它的值本来就应该在下一步由 featag 落点写进来（S7 直接依赖这个次序）。
            if self.features.has(Feature::Featag) && !bare.featags.is_empty() {
                self.check_featags(bare, &names)?;
                self.fill_featag_targets(m, bare, &names)?;
            }
            let mut applied = false;
            if names.len() == 1 {
                // 单契约：**逐字走原路径**（错误文案 / 行为与旧版完全一致）。
                // typed-block 不支持块级覆盖（D3 仅 `@is X warn|loose` 支持），
                // 故有效严格度 = 契约级默认。
                let cname = &names[0];
                let hint = suggest_contract_name(cname, &self.contracts)
                    .map(|c| format!("（你是不是想写 `{c}`？）"))
                    .unwrap_or_default();
                let c = self.contracts.get(cname).cloned().ok_or_else(|| {
                    SmlError::from_code_title(E_CONTRACT_001).with_detail(format!("`{cname}`{hint}"))
                })?;
                let mut ws = Vec::new();
                apply_contract(&c, m, &self.contracts, &self.ns_prefix(), &mut ws)?;
                self.record_contract_warnings(ws);
                applied = true;
            } else if names.len() > 1 {
                // 多契约：叠加（D3/D4②：strict 优先 / 字段并集 / 冲突 ⇒ E-CONTRACT-016）
                let entries: Vec<(String, Option<Strictness>)> =
                    names.iter().cloned().map(|n| (n, None)).collect();
                let mut ws = Vec::new();
                apply_contracts(&entries, m, &self.contracts, &self.ns_prefix(), &mut ws)?;
                self.record_contract_warnings(ws);
                applied = true;
            }
            if applied {
                // 契约映射：path_stack 此刻仍含本块的键/下标（调用方在闭帧后才弹），
                // 正是本块在结果树里的位置。
                if let Some(map) = &mut self.contract_map {
                    for n in &names {
                        map.record(self.path_stack.clone(), n.clone());
                    }
                }
            }
        }
        // __name 的起点：多契约时跳过被算作契约词的 args（首个非契约词是块名）；
        // 单契约 / 无契约与旧版完全一致（args[0]）
        // —— featag S5 的兜底：没有应用任何契约时也要报「无人声明」——
        //
        // 正常路径已在上面（契约解析块内）校验过；只有 `typed-block` / `contract` 特性关掉时
        // 才轮到这里。门控与契约校验**同一个开关**（`apply_contracts(false)` 时整段跳过）。
        if self.apply_contracts
            && self.features.has(Feature::Featag)
            && !bare.featags.is_empty()
            && names.is_empty()
        {
            self.check_featags(bare, &names)?;
        }
        let name_args: &[Value] = if names.len() > 1 { &args[consumed_args..] } else { &args[..] };
        // featag（S3）：块头的 `!` 区落成**保序数组**。写在契约应用**之后**，
        // 与 `__type` / `__name` 同理 —— 保留键是语法元数据，不该被 strict 契约
        // 当成「未声明字段」拒掉（S4 才轮到「声明校验」，届时它读的就是这份数据）。
        if !bare.featags.is_empty() {
            m.insert("__featags".into(), Value::Array(bare.featags.clone()));
        }
        m.insert("__type".into(), Value::Str(key.to_string()));
        // 裸块参数全部保留：首个作 __name，其余放入 __args 数组，
        // 不再静默丢弃（P1-3：`server web prod {}` 的 web/prod 都应可见）。
        if !name_args.is_empty() {
            m.insert("__name".into(), name_args[0].clone());
            if name_args.len() > 1 {
                m.insert("__args".into(), Value::Array(name_args[1..].to_vec()));
            }
        }
        Ok(())
    }

    /// featag **S5 校验**：块头用到的每个 featag 都必须在**本块应用的契约**里声明过，
    /// 且参数个数 / 类型与声明相符；**类型不符**走既有类型错误码（同因同码）。
    ///
    /// `contract_names` 是 `finish_bare_block` 里已解析到的契约名（多契约 = 声明并集）。
    /// 诊断一律写全 `<契约>.<featag>`：作用域契约私有，只写名字会分不清是谁的（§5）。
    fn check_featags(&self, bare: &BareCtx, contract_names: &[String]) -> Result<(), SmlError> {
        let decls: Vec<&FeatagDecl> = contract_names
            .iter()
            .filter_map(|n| self.contracts.get(n))
            .flat_map(|c| c.featags.iter())
            .collect();
        let owner = contract_names.first().map(|s| s.as_str()).unwrap_or_default();

        for item in &bare.featags {
            let fname = item
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let where_ = if owner.is_empty() {
                fname.clone()
            } else {
                format!("{owner}.{fname}")
            };
            let args = item.get("args");

            let Some(decl) = decls.iter().find(|d| d.name == fname) else {
                // 未声明：把「本块有没有契约」和「契约里声明了什么」分开说
                let mut avail: Vec<&str> = decls.iter().map(|d| d.name.as_str()).collect();
                avail.sort_unstable();
                avail.dedup();
                let hint = if contract_names.is_empty() {
                    "本块没有应用任何契约 —— featag 的作用域是契约私有的，先 `@contract` 声明它"
                        .to_string()
                } else if avail.is_empty() {
                    format!("契约 `{owner}` 里一个 featag 都没声明")
                } else {
                    format!(
                        "契约 `{owner}` 里可用：{}",
                        avail
                            .iter()
                            .map(|n| format!("!{n}"))
                            .collect::<Vec<_>>()
                            .join(" ")
                    )
                };
                return Err(SmlError::from_code_title(E_CONTRACT_021)
                    .with_detail(format!("`{where_}` 未声明；{hint}")));
            };

            match (&decl.spec, args) {
                (None, None) => {}
                (None, Some(_)) => {
                    return Err(SmlError::from_code_title(E_CONTRACT_021).with_detail(format!(
                        "`{where_}` 声明为无参（纯标签），使用处不能带参数"
                    )))
                }
                (Some(_), None) => {
                    return Err(SmlError::from_code_title(E_CONTRACT_021).with_detail(format!(
                        "`{where_}` 声明为带参，使用处必须写成 `{fname}[…]`"
                    )))
                }
                (Some(spec), Some(payload)) => {
                    // 数组类型 ⇒ 参数**表**整体校验（`!obey: [int]` ↔ `obey[1, 2]`）；
                    // 其它类型 ⇒ 只接受 1 个参数，校验那一个（`!cert: str` ↔ `cert["…"]`）。
                    // `auto`（取默认值）不校验：值来自声明本身，S6 写入时再取。
                    if !is_auto_marker(payload) {
                        let v = Self::featag_value(&where_, spec, payload)?;
                        sml_contract::check_value(&where_, &where_, spec, &v, &self.contracts)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// 读一个 featag 落点：首词 + 紧跟的 `[n]` / `[n].key` 片段，**拼回原文**再交给 `Path::parse`。
    ///
    /// 为什么要拼：词法把 `rules[0].name` 切成 `Word("rules") LBrack Word("0") RBrack Word(".name")`
    /// —— 方括号是**结构符号**，而落点需要它们作为**文本**。这里只做「相邻即拼接」，不解释语义；
    /// 解释（段类型、越界等）仍归 `Path` / `write_target`，两件事别混。
    fn take_featag_target_text(&mut self, name: &str) -> Result<String, SmlError> {
        let mut raw = match self.next() {
            Some(Tok::Word(s)) | Some(Tok::Str(s)) => s.into_owned(),
            other => {
                return Err(SmlError::from_code_title(E_CONTRACT_020)
                    .with_detail(format!("`{name}` 的 `->` 后须落点，得 {other:?}")))
            }
        };
        loop {
            match self.peek() {
                Some(Tok::LBrack) => {
                    self.next();
                    match self.next() {
                        Some(Tok::Word(s)) => {
                            raw.push('[');
                            raw.push_str(&s);
                        }
                        other => {
                            return Err(SmlError::from_code_title(E_CONTRACT_020).with_detail(
                                format!("`{name}` 的落点 `{raw}[…]` 里须写数组下标，得 {other:?}"),
                            ))
                        }
                    }
                    match self.next() {
                        Some(Tok::RBrack) => raw.push(']'),
                        other => {
                            return Err(SmlError::from_code_title(E_CONTRACT_020).with_detail(
                                format!("`{name}` 的落点 `{raw}` 的下标未闭合，得 {other:?}"),
                            ))
                        }
                    }
                }
                // `rules[0].name`：`.` 开头的词紧跟在 `]` 之后 ⇒ 仍属于本落点
                Some(Tok::Word(w)) if w.starts_with('.') && raw.ends_with(']') => {
                    let w = w.clone().into_owned();
                    raw.push_str(&w);
                    self.next();
                }
                _ => break,
            }
        }
        Ok(raw)
    }

    /// featag **S8**：`any` 类型默认禁用，契约里写 `!any` 才放行（§10 Q7）。
    ///
    /// 门控 `type.featag`（与其它 v5 行为一致）：没开时整段不查，v4 文档里的 `any` 照旧可用。
    /// 报码复用 `E-FEATURE-001`（因相同：都是「这项能力没被放行」），detail 说清是**契约级开关**。
    fn check_any_allowed(&self, contract_names: &[String]) -> Result<(), SmlError> {
        let mut seen: Vec<String> = Vec::new();
        for n in contract_names {
            if let Some(c) = self.contracts.get(n) {
                Self::walk_any(c, &self.contracts, &mut seen)?;
            }
        }
        Ok(())
    }

    /// 递归查字段里的 `any`（含 `[any]` 这种数组元素位置）。`seen` 防递归契约绕圈。
    fn walk_any(
        contract: &Contract,
        contracts: &BTreeMap<String, Contract>,
        seen: &mut Vec<String>,
    ) -> Result<(), SmlError> {
        if seen.iter().any(|s| s == &contract.name) {
            return Ok(());
        }
        seen.push(contract.name.clone());
        for (fname, spec) in &contract.fields {
            if type_uses_any(&spec.ty) && !contract.allow_any {
                return Err(SmlError::from_code_msg(E_FEATURE_001).with_detail(format!(
                    "契约 `{}` 的字段 `{fname}` 用了 `any`（等于放弃类型校验）；\
                     若确实需要，请在该契约里写 `!any` 显式放开",
                    contract.name
                )));
            }
            if let TypeSpec::ContractRef(name) = &spec.ty {
                if let Some(inner) = contracts.get(name) {
                    Self::walk_any(inner, contracts, seen)?;
                }
            }
        }
        Ok(())
    }

    /// 一个 featag 的**值**：数组类型 ⇒ 整张参数表；其它类型 ⇒ 必须恰好 1 个参数。
    ///
    /// S5（校验）与 S6（写值）共用这一个换算 —— 否则会出现「校验按一种口径、写进去按另一种」。
    fn featag_value(where_: &str, spec: &FieldSpec, payload: &Value) -> Result<Value, SmlError> {
        if matches!(spec.ty, TypeSpec::Array(_)) {
            return Ok(payload.clone());
        }
        let list = payload.as_array().map(|a| a.as_slice()).unwrap_or(&[]);
        if list.len() != 1 {
            return Err(SmlError::from_code_title(E_CONTRACT_021).with_detail(format!(
                "`{where_}` 的参数类型不是数组，只接受 1 个参数（实际给了 {} 个）",
                list.len()
            )));
        }
        Ok(list[0].clone())
    }

    /// featag **S6：按落点写值**（§6.1 规则 1–5）。
    ///
    /// - 落点**已存在 ⇒ 报错**（禁止覆盖；静默跳过会让「以为写进去了」变成最难查的错）；
    /// - 父对象默认**自动创建**（那是「插入」不是「覆盖」）；`!strict-paths` 打开后要求父对象已存在；
    /// - `auto` ⇒ 取声明里的 `default`；
    /// - 无落点 ⇒ 纯标签，只留在 `__featags`（这里直接跳过）。
    fn fill_featag_targets(
        &mut self,
        m: &mut Map,
        bare: &BareCtx,
        contract_names: &[String],
    ) -> Result<(), SmlError> {
        let decls: Vec<&FeatagDecl> = contract_names
            .iter()
            .filter_map(|n| self.contracts.get(n))
            .flat_map(|c| c.featags.iter())
            .collect();
        let strict = contract_names
            .iter()
            .filter_map(|n| self.contracts.get(n))
            .any(|c| c.strict_paths);
        let owner = contract_names.first().map(|s| s.as_str()).unwrap_or_default();

        for item in &bare.featags {
            let fname = item
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let where_ = if owner.is_empty() {
                fname.clone()
            } else {
                format!("{owner}.{fname}")
            };
            // 未声明 / 无落点 / 没给参数 都在 S5 报过或无须处理
            let Some(decl) = decls.iter().find(|d| d.name == fname) else {
                continue;
            };
            let Some(target) = &decl.target else {
                continue;
            };
            let Some(payload) = item.get("args") else {
                continue;
            };

            let value = if is_auto_marker(payload) {
                decl.spec.as_ref().and_then(|s| s.default.clone()).ok_or_else(|| {
                    SmlError::from_code_title(E_CONTRACT_022).with_detail(format!(
                        "`{where_}` 的参数是 `auto`（取声明里的默认值），但该声明没有 `default`"
                    ))
                })?
            } else {
                match &decl.spec {
                    Some(spec) => Self::featag_value(&where_, spec, payload)?,
                    // 无类型声明却写了参数：S5 已经报过「声明为无参」，这里不重复报
                    None => payload
                        .as_array()
                        .and_then(|a| a.first().cloned())
                        .unwrap_or(Value::Null),
                }
            };

            write_target(m, target.segments(), value, strict, &where_)?;
        }
        Ok(())
    }

    /// 把一条字段值写入父块节点，含「同名冲突 → 提升为数组」的既定语义
    /// （静默改数据：SML 既定语义，改为告警）。
    ///
    /// ⚠️ 走 `Map::insert_or_merge_owned` 而**不是** `get_mut` + `insert`：后者是两次查找
    /// （`BTreeMap::insert` 只查一次），保序档下实测多花约 30% 解析时间。
    /// 冲突分支里也**原地**改写（`remove` + `insert` 会把键挪到尾部，
    /// 那是把实现细节泄漏成了行为差异）。
    fn insert_field_val(&mut self, node: &mut Map, key: String, key_idx: usize, val: Value) {
        insert_or_merge_owned(node, key, val, |key, existing, val| {
            let msg = match existing {
                Value::Array(a) => format!(
                    "sml: 键 `{key}` 在同一块内再次出现，已追加进既有数组\
                     （追加后 {} 项）",
                    a.len() + 1
                ),
                _ => format!(
                    "sml: 键 `{key}` 在同一块内重复出现，已**提升为数组**\
                     （`{key}` 现在是 [原值, 新值]）"
                ),
            };
            self.push_key_diag(DiagnosticKind::DuplicateKey, key_idx, msg);
            match existing {
                Value::Array(a) => a.push(val),
                _ => {
                    let old = std::mem::replace(existing, Value::Null);
                    *existing = Value::Array(vec![old, val]);
                }
            }
        });
    }

    /// 子容器闭合后，按 `cont` 把结果交回父帧：`Index` 追加进父数组，`Field` 写入父块
    /// （`when == Some(false)` 时 `@when` 宣布丢弃，不写入）。
    fn merge_value(&mut self, cont: Cont, val: Value, stack: &mut [Frame]) {
        match cont {
            Cont::Root => unreachable!("merge_value 不应收到根帧 cont"),
            Cont::Index => {
                if let Some(Frame::Array { arr, .. }) = stack.last_mut() {
                    arr.push(val);
                }
            }
            Cont::Field { key, key_idx, when } => {
                if when == Some(false) {
                    return;
                }
                if let Some(Frame::Block { state, .. }) = stack.last_mut() {
                    self.insert_field_val(&mut state.node, key, key_idx, val);
                }
            }
        }
    }

    /// 值位置的单步（等价旧 `parse_value`）：标量 / `@for` 直接 `Leaf`；容器（`{` / `[` /
    /// 裸块）消费 opening、压好 path/ns、构造 `Cont` 后返回 `Open(child)`。
    fn value_step(
        &mut self,
        key: &str,
        colon: bool,
        key_idx: usize,
        when: Option<bool>,
        weak: Option<PendingWeak>,
    ) -> Result<ValueStep, SmlError> {
        // `@weakbind` 只认块：先判「这一项有没有可能成块」，否则每个叶子分支都得写一遍。
        // 判据与下面实际走哪一支保持一致：裸块 / `{`。标量、数组、`@for` 都不算块。
        let bare_ahead = !colon && self.bare_block_ahead();
        if weak.is_some() && !(bare_ahead || matches!(self.peek(), Some(Tok::LBrace))) {
            return Err(SmlError::from_code_msg(E_PARSE_029)
                .with_detail("`@weakbind` 后面必须紧跟一个块（标量 / 数组 / `@for` 都不算）"));
        }
        // 值位置的 `@for var in ... { }` 指令：有界循环展开为数组。
        #[cfg(feature = "when")]
        if let Some(Tok::At) = self.peek() {
            if let Some(Tok::Word(w)) = self.peek_at(1) {
                if w == "for" {
                    return Ok(ValueStep::Leaf(self.parse_for_value()?));
                }
            }
        }
        // 无冒号且后继是裸词: 可能是裸块 `type [name] { }`
        if bare_ahead {
            // 路径压 Key(key)：裸块在结果树里的位置就是「键 key 的值」。
            self.path_stack.push(PathSeg::Key(key.to_string()));
            // 弱绑定在此落地：键已压栈，此刻的 `path_stack` 就是目标块的位置。
            if let Some(w) = weak {
                self.record_weak_bind(w);
            }
            let frame = self.bare_child_frame(key, Cont::Field { key: key.to_string(), key_idx, when })?;
            return Ok(ValueStep::Open(frame));
        }
        match self.peek().cloned() {
            Some(Tok::LBrace) => {
                self.next();
                self.path_stack.push(PathSeg::Key(key.to_string()));
                // 弱绑定在此落地（与裸块同理：键已压栈）。
                if let Some(w) = weak {
                    self.record_weak_bind(w);
                }
                Ok(ValueStep::Open(Frame::Block {
                    state: BlockFrame {
                        node: self.new_map(),
                        applied_contracts: Vec::new(),
                        pending_when: None,
                        pending_weakbind: None,
                        closing: Some(Tok::RBrace),
                        bare: None,
                    },
                    cont: Cont::Field { key: key.to_string(), key_idx, when },
                }))
            }
            Some(Tok::LBrack) => {
                self.next();
                self.path_stack.push(PathSeg::Key(key.to_string()));
                Ok(ValueStep::Open(Frame::Array {
                    arr: Vec::new(),
                    cont: Cont::Field { key: key.to_string(), key_idx, when },
                }))
            }
            Some(tok @ (Tok::Word(_) | Tok::Str(_))) => {
                let v = match tok {
                    Tok::Word(w) => coerce_word(&w, &self.fragments, self.features, &self.ns_prefix(), Some(&self.env))?,
                    Tok::Str(s) => {
                        // 引号串承认 `${var}`（@for 循环变量）与 `$env.NAME` 内联插值。受
                        // `Feature::Env` 约束（与裸词路径一致，否则盗用引号串绕过限制）。
                        if s.contains("$env.") && !self.features.has(Feature::Env) {
                            return Err(SmlError::from_code_msg(E_FEATURE_002)
                                .with_detail(format!("字符串 `\"{s}\"` 无法解析")));
                        }
                        Value::Str(self.interp_str(&s))
                    }
                    _ => unreachable!(),
                };
                self.next();
                Ok(ValueStep::Leaf(v))
            }
            // 键后无值: `key }` / `key ]` / `key ,` / 行尾 —— key 本身即值 (片段引用/裸词)
            Some(Tok::RBrace) | Some(Tok::RBrack) | Some(Tok::Comma) | None => {
                if colon {
                    Ok(ValueStep::Leaf(Value::Null))
                } else {
                    Ok(ValueStep::Leaf(coerce_word(
                        key,
                        &self.fragments,
                        self.features,
                        &self.ns_prefix(),
                        Some(&self.env),
                    )?))
                }
            }
            _ => Err(SmlError::from_code_msg(E_PARSE_012)),
        }
    }

    /// 消费裸块参数（`[args…]`，Word/Str）并建其体帧：确认后是 `{` 并消费之、压入该类型名
    /// 命名空间，返回 `Frame::Block`（`bare` 已填 `BareCtx`）。path（Key/Index）由调用方压。
    fn bare_child_frame(&mut self, type_name: &str, cont: Cont) -> Result<Frame, SmlError> {
        // featag（B3）：块头的**首词**（类型词）也不许以 `!` 开头 —— `!wea name { }` 里
        // `!wea` 到底是想写 featag 还是块名？块名无法加引号，故直接拒（§2.3 B3）。
        if self.features.has(Feature::Featag) && type_name.starts_with('!') {
            return Err(SmlError::from_code_msg(E_PARSE_028).with_detail(format!("`{type_name}`")));
        }
        // featag（S2 识别 + S3 落数据）：块头首词之后的 `!` 区，解析成**保序**的项列表，
        // 稍后由 `finish_bare_block` 写成 `__featags`。声明校验不做（那是 S4/S5）。
        let featags = if self.features.has(Feature::Featag) {
            self.parse_featag_region()?
        } else {
            Vec::new()
        };
        let mut args: Vec<Value> = Vec::new();
        while let Some(t) = self.peek().cloned() {
            match t {
                Tok::Word(w) => {
                    args.push(coerce_word(
                        &w,
                        &self.fragments,
                        self.features,
                        &self.ns_prefix(),
                        Some(&self.env),
                    )?);
                    self.next();
                }
                Tok::Str(_) => {
                    if let Some(Tok::Str(s)) = self.next() {
                        args.push(Value::Str(s.into_owned()));
                    }
                }
                _ => break,
            }
        }
        if self.peek() != Some(&Tok::LBrace) {
            return Err(SmlError::from_code_msg(E_PARSE_012));
        }
        self.next();
        // 进入子块 = 进入该 block 名字的命名空间
        self.ns_stack.push(type_name.to_string());
        Ok(Frame::Block {
            state: BlockFrame {
                node: self.new_map(),
                applied_contracts: Vec::new(),
                pending_when: None,
                pending_weakbind: None,
                closing: Some(Tok::RBrace),
                bare: Some(BareCtx { key: type_name.to_string(), args, featags }),
            },
            cont,
        })
    }

    /// S2：消费块头首词之后的 **featag 区**（`!critical obey[1, 2]` 与终止逗号）。
    ///
    /// 规则（`docs/swsml/spec/featag.md` §2.2 / §2.3）：
    /// - **无参项必须带 `!`**（`!critical`）—— 否则与块名不可分；
    /// - **带参项写成 `名[ … ]`**（`obey[1, 2]`），靠 `[` 与块名区分；
    /// - **`,` 是终止符**：区内部各项之间不写逗号；`[ ]` 内深度 ≥ 1 的逗号不算终止；
    /// - 区的结束 = 遇到「既不是 `!名` 也不是 `名[`」的词，或直接 `{`。
    ///
    /// 返回**保序**的项列表，每项形如 `{ name: "…" }` 或 `{ name: "…", args: [ … ] }`
    /// （S3 契约：不复用 `__args`，那是「块名之后的裸词」，两种语义混在一起会分不清）。
    /// 空列表 = 本块头没有 featag 区，调用方逐字走旧路径。
    fn parse_featag_region(&mut self) -> Result<Vec<Value>, SmlError> {
        let mut items: Vec<Value> = Vec::new();
        loop {
            let word = match self.peek() {
                Some(Tok::Word(w)) => Some(w.clone()),
                _ => None,
            };
            match word {
                Some(w) if w.starts_with('!') => {
                    // B1：`wea ! , name { }` —— `!` 后面什么都没有（`!!x` 同理）
                    let name = &w[1..];
                    if name.is_empty() || name.starts_with('!') {
                        return Err(
                            SmlError::from_code_msg(E_PARSE_026).with_detail(format!("`{w}`"))
                        );
                    }
                    self.next();
                    let mut item = self.new_map();
                    item.insert("name".into(), Value::Str(name.to_string()));
                    if self.peek() == Some(&Tok::LBrack) {
                        self.next(); // `parse_array` 假定 `[` 已被消费（与其它调用点一致）
                        item.insert("args".into(), self.parse_array()?);
                    }
                    items.push(Value::Object(item));
                }
                Some(w) if self.peek_at(1) == Some(&Tok::LBrack) => {
                    self.next(); // 名字本身不带 `!`，靠 `[` 与块名区分
                    self.next(); // 越过的 `[`（`parse_array` 的既有约定）
                    let mut item = self.new_map();
                    item.insert("name".into(), Value::Str(w.into_owned()));
                    // 显式写了 `[]` 就保留空数组：`obey[]` 与「没有参数槽」是两回事
                    item.insert("args".into(), self.parse_array()?);
                    items.push(Value::Object(item));
                }
                _ => break,
            }
        }
        if items.is_empty() {
            return Ok(items); // 本块头没有 featag 区
        }
        // `,` = 终止符（可省：`wea !critical { }` 直接进块体也成立）
        if self.peek() == Some(&Tok::Comma) {
            self.next();
        }
        // B3：区已结束，紧跟的词落在**块名**位置，`!` 开头 ⇒ 歧义（块名又无法加引号）
        if let Some(Tok::Word(w)) = self.peek() {
            if w.starts_with('!') {
                let w = w.clone();
                return Err(SmlError::from_code_msg(E_PARSE_028).with_detail(format!("`{w}`")));
            }
        }
        Ok(items)
    }

    /// 数组帧单步推进（等价旧 `parse_array_inner` 的循环体分成立）。
    fn array_step(&mut self, arr: &mut Vec<Value>) -> Result<Step, SmlError> {
        match self.peek() {
            None => Err(SmlError::from_code_msg(E_PARSE_001).with_detail("缺少结束符号 ]")),
            Some(&Tok::RBrack) => {
                self.next();
                Ok(Step::Finish)
            }
            Some(&Tok::RBrace) => Err(SmlError::from_code_title(E_PARSE_003).with_detail("}（数组应以 ] 闭合）")),
            Some(&Tok::Comma) => {
                self.next();
                Ok(Step::Continue)
            }
            Some(&Tok::LBrace) => {
                self.next();
                self.path_stack.push(PathSeg::Index(arr.len()));
                Ok(Step::Open(Frame::Block {
                    state: BlockFrame {
                        node: self.new_map(),
                        applied_contracts: Vec::new(),
                        pending_when: None,
                        pending_weakbind: None,
                        closing: Some(Tok::RBrace),
                        bare: None,
                    },
                    cont: Cont::Index,
                }))
            }
            Some(&Tok::LBrack) => {
                self.next();
                self.path_stack.push(PathSeg::Index(arr.len()));
                Ok(Step::Open(Frame::Array { arr: Vec::new(), cont: Cont::Index }))
            }
            Some(Tok::Word(_)) => {
                // 数组内的裸块 `Type [name] { ... }`：表达**有序**元素序列。
                if self.bare_block_ahead() {
                    let name = match self.next() {
                        Some(Tok::Word(s)) => s,
                        _ => unreachable!("bare_block_ahead 已确认当前 token 是 Word"),
                    };
                    self.path_stack.push(PathSeg::Index(arr.len()));
                    let frame = self.bare_child_frame(&name, Cont::Index)?;
                    Ok(Step::Open(frame))
                } else {
                    // 普通词元素：只借用游标，无需克隆
                    let w = self.next();
                    let v = match w {
                        Some(Tok::Word(s)) => coerce_word(
                            &s,
                            &self.fragments,
                            self.features,
                            &self.ns_prefix(),
                            Some(&self.env),
                        )?,
                        _ => unreachable!("上游已确认当前 token 是 Word"),
                    };
                    arr.push(v);
                    Ok(Step::Continue)
                }
            }
            Some(&Tok::Str(_)) => {
                if let Some(Tok::Str(s)) = self.next() {
                    arr.push(Value::Str(s.into_owned()));
                }
                Ok(Step::Continue)
            }
            _ => Ok(Step::Finish),
        }
    }

    /// 预扫描：当前位置是否形如裸块的**参数部分**（`[name...] {`），
    /// 即连续若干 `Word`/`Str` 后紧跟 `{`。不消费任何 token。
    ///
    /// featag 开启时还要跨过两样东西（否则 `wea !critical, name { }` 会被当成
    /// 「键 `wea` + 值 `!critical`」，当场在 B2 上报错 —— 这正是第一版踩到的坑）：
    /// - **`!名` / `名[ … ]` 区**：遇到 `[` 就按括号配平跳过去（配不平 ⇒ 不是块头，回落旧路径）；
    /// - **区的终止符 `,`**。
    fn bare_block_ahead(&self) -> bool {
        if !matches!(self.peek(), Some(Tok::Word(_)) | Some(Tok::Str(_))) {
            return false;
        }
        let featag = self.features.has(Feature::Featag);
        let mut probe = self.i;
        while probe < self.toks.len() {
            match &self.toks[probe] {
                Tok::Word(_) | Tok::Str(_) => probe += 1,
                Tok::Comma if featag => probe += 1,
                Tok::LBrack if featag => {
                    // 只数 [ ] / { } 的深度：配不平就当「不是块头」，交回旧路径报旧错
                    let mut depth = 0usize;
                    loop {
                        match self.toks.get(probe) {
                            Some(Tok::LBrack) | Some(Tok::LBrace) => {
                                depth += 1;
                                probe += 1;
                            }
                            Some(Tok::RBrack) | Some(Tok::RBrace) => {
                                if depth == 0 {
                                    return false;
                                }
                                depth -= 1;
                                probe += 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            Some(_) => probe += 1,
                            None => return false,
                        }
                    }
                }
                Tok::LBrace => return true,
                _ => return false,
            }
        }
        false
    }

    /// 值位置的 `@for var in a b c { ... }`：有界循环展开为数组。
    ///
    /// 受 cargo feature `when` 门控；运行时另受文档层 `for` 特性约束
    /// （需 `@feature enable for`，否则报「未启用」）。
    ///
    /// 关键约束（见 [`Feature::For`] 文档）：
    /// - 只在 `in` 后的**有限枚举列表**上遍历，无 `while`、无递归 —— LOOP 语言，非图灵完备；
    /// - 循环变量 `${var}` 只读，`${var}` 在循环体内文本插值；
    /// - 组合 `@when` × `@for`：外层 `@when` 作用于整个 `hosts: @for ...`（条件为假则
    ///   整段数组不出现），即「`@when` 过滤的是字段，不是某一轮迭代」——这是设计陷阱。
    #[cfg(feature = "when")]
    fn parse_for_value(&mut self) -> Result<Value, SmlError> {
        if !self.features.has(Feature::For) {
            return Err(SmlError::from_code_msg(E_FEATURE_001)
                .with_detail("`for`（请 `@feature enable for`）"));
        }
        // 解析 `var in a b c`，游标停在 `{` 处。
        let (var, items) = crate::cond::eval_for_header(self)?;
        // 切出循环体内部 token（不含两端括号），游标已越过 `}`。
        let body = self.slice_block_tokens()?;

        let mut arr = Vec::with_capacity(items.len());
        for item in &items {
            let mut child = self.spawn_for_child(body.clone(), &var, item);
            // 子解析器从块内部 token 起始，已无外层 `}`，故以 EOF 为结束（closing=None）。
            let val = match child.parse_block(None) {
                Ok(v) => v,
                // `collect_errors`：子解析器的错误并入父级，本轮迭代以 `Null` 占位，
                // 继续跑完其余枚举项（L16 的最后一块残余）。
                Err(e) if self.collect_errors => {
                    for ce in child.errors.drain(..) {
                        self.push_error(ce);
                    }
                    self.push_error(e);
                    Value::Null
                }
                Err(e) => return Err(e),
            };
            // 成功路径下子解析器也可能已收集到（非致命的）错误，同样并入父级。
            for ce in child.errors.drain(..) {
                self.push_error(ce);
            }
            arr.push(val);
        }
        Ok(Value::Array(arr))
    }

    /// 顶层数组「字面量」入口：与裸块/键块共用显式栈驱动（[`Parser::run_stack`]）。
    pub(crate) fn parse_array(&mut self) -> Result<Value, SmlError> {
        // 顶部数组中「字面量」的入口：与裸块/键块共用显式栈驱动。
        // root 帧不压 path、不弹 ns；`run_stack` 的深度守卫即原 wrapper 的 depth 检查。
        let root = Frame::Array {
            arr: Vec::new(),
            cont: Cont::Root,
        };
        self.run_stack(root)
    }
}

/// 这个类型里有没有 `any`（含 `[any]`）—— S8 的判据。
fn type_uses_any(ty: &TypeSpec) -> bool {
    match ty {
        TypeSpec::Any => true,
        TypeSpec::Array(inner) => type_uses_any(inner),
        _ => false,
    }
}

/// featag 参数是不是 `auto`（= 取声明里的默认值，§6.1 规则 5）。
///
/// ⚠️ **已知限制**：参数值在 `parse_array` 里已经变成了 `Value`，**词法形态丢了**，
/// 所以裸词 `auto` 与引号串 `"auto"` 在这里**无法区分** —— 两者都当 `auto`。
/// 真需要把字面量 "auto" 当参数时，目前只能避开（例如写 `["auto "]`）。
/// 要做到精确区分，得让 `parse_array` 把「是否裸词」带回值里，属**另一笔**改动，不塞进本片。
fn is_auto_marker(payload: &Value) -> bool {
    matches!(payload, Value::Array(a)
        if a.len() == 1 && matches!(&a[0], Value::Str(s) if s == "auto"))
}

/// featag **S6 写落点**（§6.1 规则 1–3 + §10 Q20 的数组下标语义）：逐段下钻，最后一段写值。
///
/// - 落点已存在 ⇒ `E-CONTRACT-022`（**禁止覆盖**）；
/// - 父对象缺失 ⇒ 默认**自动创建**（「插入」不是「覆盖」）；`strict`（`!strict-paths`）时 ⇒ `E-CONTRACT-023`；
/// - 路径上某键已是非对象 ⇒ `E-CONTRACT-023`（往里放键没有意义）；
/// - **数组下标（Q20，2026-11-04 用户定）**：`n < len` 且元素为 `null` ⇒ 补缺；元素非 null ⇒ `022`；
///   `n == len` ⇒ **追加**（`strict` 下报 `023`）；`n > len` ⇒ `025`（**绝不补 null**）。
///
/// ⚠️ **数组本身不存在时不自动创建**（对象可以、数组不行）：造一个多长的数组、中间填什么，
/// 没有无歧义的答案；凭空补 null 又违背 `docs/swsml/spec/null.md` N1–N5。
fn write_target(
    map: &mut Map,
    segs: &[Seg],
    value: Value,
    strict: bool,
    where_: &str,
) -> Result<(), SmlError> {
    let Some((seg, rest)) = segs.split_first() else {
        return Ok(()); // 空路径（`Path::new(vec![])`）＝ 不落任何键
    };
    let Seg::Key(key) = seg else {
        // 落点从块（对象）出发，第一段必须是键；下标开头在声明期就该被 `Path` 挡住
        return Err(SmlError::from_code_title(E_CONTRACT_020)
            .with_detail(format!("`{where_}` 的落点须以键名开头")));
    };

    if rest.is_empty() {
        if map.contains_key(key) {
            return Err(SmlError::from_code_title(E_CONTRACT_022).with_detail(format!(
                "`{where_}` 的落点 `{key}` 已存在（featag 不能覆盖已有值）"
            )));
        }
        map.insert(key.clone(), value);
        return Ok(());
    }

    match map.get_mut(key) {
        Some(existing) => write_in_value(existing, rest, value, strict, where_),
        None => {
            if strict {
                return Err(SmlError::from_code_title(E_CONTRACT_023).with_detail(format!(
                    "`{where_}` 的落点路径上父对象 `{key}` 不存在（本契约开了 `!strict-paths`）"
                )));
            }
            // 下一段就是下标 ⇒ 需要一个不存在的数组 ⇒ 没法凭空造
            if matches!(rest.first(), Some(Seg::Index(_))) {
                return Err(SmlError::from_code_title(E_CONTRACT_023).with_detail(format!(
                    "`{where_}` 的落点路径上数组 `{key}` 不存在（数组不会凭空创建：长度与中间元素无解）"
                )));
            }
            let mut child = Map::ordered();
            write_target(&mut child, rest, value, strict, where_)?;
            map.insert(key.clone(), Value::Object(child));
            Ok(())
        }
    }
}

/// 从**已存在的值**继续下钻（键段 / 下标段都可能出现）。
fn write_in_value(
    cur: &mut Value,
    segs: &[Seg],
    value: Value,
    strict: bool,
    where_: &str,
) -> Result<(), SmlError> {
    let Some((seg, rest)) = segs.split_first() else {
        return Ok(());
    };
    match seg {
        Seg::Key(key) => match cur {
            Value::Object(m) => write_target(m, segs, value, strict, where_),
            other => Err(SmlError::from_code_title(E_CONTRACT_023).with_detail(format!(
                "`{where_}` 的落点路径上 `{key}` 已是{}，没法往里写键",
                sml_value::describe_value(other)
            ))),
        },
        Seg::Index(n) => {
            let Value::Array(items) = cur else {
                return Err(SmlError::from_code_title(E_CONTRACT_023).with_detail(format!(
                    "`{where_}` 的落点里下标 `[{n}]` 作用在{}上，不是数组",
                    sml_value::describe_value(cur)
                )));
            };
            let len = items.len();
            if *n < len {
                if rest.is_empty() {
                    // `null` 是「空位」⇒ 可以填；非 null 是已有值 ⇒ 禁止覆盖
                    if matches!(items[*n], Value::Null) {
                        items[*n] = value;
                        return Ok(());
                    }
                    return Err(SmlError::from_code_title(E_CONTRACT_022).with_detail(format!(
                        "`{where_}` 的落点 `[{n}]` 已有非 null 值（featag 不能覆盖已有值）"
                    )));
                }
                // 还有后续段：null 是「空位」，可以就地立一个空对象继续往下写
                // （这与「补缺」同源：空位可以被填，非空值不许覆盖）
                if matches!(items[*n], Value::Null) {
                    items[*n] = Value::Object(Map::ordered());
                }
                return write_in_value(&mut items[*n], rest, value, strict, where_);
            }
            if *n == len {
                if strict {
                    return Err(SmlError::from_code_title(E_CONTRACT_023).with_detail(format!(
                        "`{where_}` 的落点要追加 `[{n}]`，但本契约开了 `!strict-paths`（只允许写已有元素）"
                    )));
                }
                if rest.is_empty() {
                    items.push(value);
                    return Ok(());
                }
                items.push(Value::Object(Map::ordered()));
                let last = items.len() - 1;
                return write_in_value(&mut items[last], rest, value, strict, where_);
            }
            Err(SmlError::from_code_title(E_CONTRACT_025).with_detail(format!(
                "`{where_}` 的落点 `[{n}]` 越界：该数组只有 {len} 个元素（越界不补 null）"
            )))
        }
    }
}