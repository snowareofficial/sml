// SPDX-License-Identifier: MulanPSL-2.0
//! SML 语法分析器：递归下降，消费 [`Tok`] 序列产出 [`Value`] 树。

use std::collections::BTreeMap;
use std::sync::Arc;

use sml_codes::{
    E_CONTRACT_001, E_CONTRACT_010, E_CONTRACT_011, E_CONTRACT_012, E_CONTRACT_013, E_CONTRACT_018,
    E_EXT_003, E_EXT_004, E_EXT_005, E_EXT_008, E_FEATURE_001, E_FEATURE_002, E_FEATURE_003,
    E_INCLUDE_007, E_LIMIT_001, E_PARSE_001, E_PARSE_002, E_PARSE_003, E_PARSE_004, E_PARSE_005,
    E_PARSE_006, E_PARSE_007, E_PARSE_011, E_PARSE_012, E_PARSE_013, E_PARSE_017,
    E_PARSE_019, E_PARSE_020, E_PARSE_021, E_PARSE_022, E_PARSE_023, SmlError,
};
// 这两个只在 `when` feature（@when/@for）下用到；关掉该 feature 时下游会报 unused_imports
#[cfg(feature = "when")]
use sml_codes::{E_PARSE_016, E_PARSE_018};
use sml_contract::{suggest_contract_name, Contract, ContractExt, FieldSpec, Modifier, TypeCheck, TypeSpec};
use sml_feature::{Feature, FeatureSet};
use sml_lex::{Span, Tok, coerce_word, lookup_env};
use sml_value::{MAX_VALUE_DEPTH, Value};

use crate::contract_bridge::{apply_contract, apply_contracts};
use crate::contract_map::{ContractMap, PathSeg};
use crate::ext::{Diagnostic, DiagnosticKind, DirectiveTable, Outcome};

/// 解析器。`pub` 仅为让 `cond` 等子模块复用游标，不对外暴露。
pub struct Parser {
    pub(crate) toks: Vec<Tok>,
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
    /// 外置契约类型表（由下游注册，见 [`crate::ext`]）。
    pub(crate) contract_ext: Arc<ContractExt>,
}

impl Parser {
    /// 查 `$env.X`：先查本次解析的覆盖表，再回落进程环境。
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

    /// 查 `@for` 循环变量 `${var}`（只读绑定）。未绑定返回 None。
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
            if c == '$' {
                if chars.peek() == Some(&'{') {
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
            }
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
    fn slice_block_tokens(&mut self) -> Result<Vec<Tok>, SmlError> {
        if self.peek() != Some(&Tok::LBrace) {
            return Err(SmlError::new(E_PARSE_018, "sml: `@for` 后须 `{ ... }` 循环体"));
        }
        self.next(); // 消费 '{'
        let mut depth = 1usize;
        let mut inner = Vec::new();
        loop {
            match self.next() {
                None => {
                    return Err(SmlError::new(E_PARSE_001, 
                        "sml: `@for` 循环体未闭合（遇到文件结尾，缺少结束符号 }）",
                    ))
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
        let is_explicit = matches!(self.peek(), Some(Tok::Word(w)) if w.as_str() == "name")
            && matches!(self.peek_at(1), Some(Tok::Colon));
        if is_explicit {
            self.next(); // `name`
            self.next(); // `:`
            return match self.next() {
                Some(Tok::Word(s)) | Some(Tok::Str(s)) => Ok(Some(s)),
                other => Err(SmlError::new(E_EXT_003, format!(
                    "sml: 扩展指令 `@{fname}` 的 `name:` 后须值，得 {other:?}"
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
            return Ok(Some(w));
        }
        Ok(None)
    }

    /// 同 `slice_block_tokens`：`@for` 专用，随 feature `when` 一起编译。
    #[cfg(feature = "when")]
    fn spawn_for_child(&self, inner: Vec<Tok>, var: &str, item: &str) -> Parser {
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
            depth: self.depth + 1,
            ns_stack: self.ns_stack.clone(),
            path_stack: self.path_stack.clone(),
            // 子解析器的映射记录暂不回传父级（同 diags：回传要跨 &mut 借用，
            // 而 `@for` 体内应用契约的场景极少）。克隆只为路径上下文正确。
            contract_map: self.contract_map.clone(),
            env: self.env.clone(),
            loop_vars,
            directives: Arc::clone(&self.directives),
            // 子解析器独立收集：`@for` 体内触发的诊断暂不回传父级
            // （回传需要跨 &mut 借用，收益不抵复杂度；循环体通常不含弃用写法）。
            diags: Vec::new(),
            warn_key_conflict: self.warn_key_conflict,
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

    /// 前看当前 token。`pub` 仅为让 `cond` 等子模块复用游标，不对外暴露。
    pub fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i)
    }
    /// 前看 n 个 token（n=0 等价于 [`Self::peek`]）。
    /// 用于区分「关键字参数 `type:` `name:`」与「恰巧同名的片段名」。
    fn peek_at(&self, n: usize) -> Option<&Tok> {
        self.toks.get(self.i + n)
    }
    /// 消费并返回当前 token。`pub` 仅为让 `cond` 等子模块复用游标，不对外暴露。
    pub fn next(&mut self) -> Option<Tok> {
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
    fn parse_contract_body(&mut self, requires: &mut Vec<String>) -> Result<BTreeMap<String, FieldSpec>, SmlError> {
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
                    return Err(SmlError::new(E_PARSE_001, "sml: 契约体未闭合（缺少结束符号 }）"));
                }
                Some(Tok::Comma) => {
                    self.next();
                }
                Some(Tok::At) => {
                    // —— 契约体指令：当前仅 `@requires` ——
                    self.next(); // 消费 @
                    let dir = match self.next() {
                        Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                        _ => return Err(SmlError::new(E_PARSE_011, "sml: @ 后需指令名")),
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
                        return Err(SmlError::new(
                            E_PARSE_005,
                            format!(
                                "sml: 契约体内不支持指令 `@{}`（契约体仅支持 `@requires`）",
                                dir
                            ),
                        ));
                    }
                }
                _ => {
                    let key = match self.next() {
                        Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                        other => {
                            return Err(SmlError::new(E_PARSE_006, format!("sml: 契约字段期望键, 得 {:?}", other)))
                        }
                    };
                    if self.peek() == Some(&Tok::Colon) {
                        self.next();
                    } else {
                        return Err(SmlError::new(E_PARSE_006, format!("sml: 契约字段 `{}` 后须有冒号", key)));
                    }
                    let spec = self.parse_field_spec()?;
                    fields.insert(key, spec);
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

    /// 解析单个字段的类型与修饰符
    fn parse_field_spec(&mut self) -> Result<FieldSpec, SmlError> {
        // 命中外置类型时，把校验器带出去（塞进 `FieldSpec::ext`，校验期无需再查表）。
        let mut ext_ty: Option<Arc<dyn TypeCheck>> = None;
        let ty = match self.next() {
            Some(Tok::Word(w)) => match w.as_str() {
                "str" => TypeSpec::Str,
                "int" => TypeSpec::Int,
                "num" => TypeSpec::Num,
                "bool" => TypeSpec::Bool,
                "any" => TypeSpec::Any,
                "enum" => {
                    if self.peek() != Some(&Tok::LBrack) {
                        return Err(SmlError::new(E_PARSE_023, "sml: `enum` 后须为 [ ... ]"));
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
                                vals.push(s);
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
                                rest = s;
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
                                SmlError::new(
                                    E_CONTRACT_013,
                                    format!("sml: 类型 `{other}` 的模式编译失败：{e}"),
                                )
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
            Some(Tok::LBrack) => {
                let inner = match self.next() {
                    Some(Tok::Word(w)) => match w.as_str() {
                        "str" => TypeSpec::Str,
                        "int" => TypeSpec::Int,
                        "num" => TypeSpec::Num,
                        "bool" => TypeSpec::Bool,
                        "any" => TypeSpec::Any,
                        // 数组元素也接受外置类型（如 `[image]`）
                        other => {
                            match self.contract_ext.type_arc(other) {
                                Some(t) => {
                                    ext_ty = Some(t);
                                    TypeSpec::Ext(other.to_string())
                                }
                                None => {
                                    return Err(SmlError::new(E_CONTRACT_012, format!("sml: 未知数组元素类型 `{}`", other)))
                                }
                            }
                        }
                    },
                    other => {
                        return Err(SmlError::new(E_PARSE_006, format!("sml: 数组元素类型期望标识符, 得 {:?}", other)))
                    }
                };
                if self.peek() == Some(&Tok::RBrack) {
                    self.next();
                }
                TypeSpec::Array(Box::new(inner))
            }
            other => return Err(SmlError::new(E_PARSE_006, format!("sml: 字段类型期望标识符, 得 {:?}", other))),
        };

        // 修饰符：required / optional / default <值> / min <数> / max <数>
        let mut required = true;
        let mut default = None;
        let mut min = None;
        let mut max = None;
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
                Some(Tok::Word(w)) => match w.as_str() {
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
                            Some(Tok::Str(s)) => Value::Str(s),
                            other => {
                                return Err(SmlError::new(E_PARSE_021, format!("sml: default 期望值, 得 {:?}", other)))
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
        };
        // 外置修饰符：解析期改写规格，并把值带进校验期（供 `Modifier::check` 使用）。
        for (m, v) in pending_mods {
            // 外置修饰符的解析期钩子：失败原因由注册方给出，这里补上码。
            m.apply(&mut spec, &v)
                .map_err(|e| SmlError::new(E_CONTRACT_011, e))?;
            spec.ext_data.insert(m.name().to_string(), v);
            spec.mods.push(m);
        }
        Ok(spec)
    }

    /// 解析外置修饰符的取值：裸词 / 引号串 / 块 / 数组。
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
                Ok(Value::Str(s))
            }
            Some(Tok::LBrace) => {
                self.next();
                self.parse_block(Some(Tok::RBrace))
            }
            Some(Tok::LBrack) => {
                self.next();
                self.parse_array()
            }
            other => Err(SmlError::new(E_EXT_005, format!("sml: 修饰符期望取值，得 {other:?}"))),
        }
    }

    fn parse_spec_number(&mut self) -> Result<f64, SmlError> {
        match self.next() {
            Some(Tok::Word(w)) => {
                let f = w
                    .parse::<f64>()
                    .map_err(|_| SmlError::new(E_PARSE_022, format!("sml: 期望数字, 得 `{}`", w)))?;
                // 拒绝 NaN/inf 作为边界：Rust 的 "nan".parse::<f64>() == Ok(NaN)，
                // 而 NaN 的所有比较均为 false，会让 min/max 校验被静默绕过
                // （审计 #2）。
                if !f.is_finite() {
                    return Err(SmlError::new(E_CONTRACT_010, format!("sml: 数字边界必须为有限值, 得 `{}`", w)));
                }
                Ok(f)
            }
            other => Err(SmlError::new(E_PARSE_022, format!("sml: 期望数字, 得 {:?}", other))),
        }
    }

    /// 解析对象/块, 直到遇到 closing (None=顶层)。
    /// 外层 wrapper：深度守卫，防止 `a{a{a{ ... }}}` 无限递归导致栈溢出。
    /// 实际实现见 [`Parser::parse_block_inner`]。
    pub(crate) fn parse_block(&mut self, closing: Option<Tok>) -> Result<Value, SmlError> {
        // 口径（各端一致，**已实测钉住**）：`depth` 是「进入本块**前**已进入的层数」
        // （根块为 0），故允许 `depth == MAX_VALUE_DEPTH` —— 第 128 层块放行、
        // 第 129 层报此码。
        // ⚠️ 这里原先是 `>=`：128 层就被拒，而文案写的是「**超过** 128 层」，
        //    行为与文案自相矛盾；且与 C++ / JS / Lua 实测边界（128 放行 / 129 报）
        //    差一格。改动前用闭合嵌套 `("a { "):rep(N) + ("} "):rep(N)` 实测量过五端：
        //    Rust 127 / C 127 / C++ 128 / JS 128 / Lua 128。
        if self.depth > MAX_VALUE_DEPTH {
            return Err(SmlError::new(E_LIMIT_001, format!(
                "sml: 嵌套过深（超过 {} 层），疑似递归或恶意输入",
                MAX_VALUE_DEPTH
            )));
        }
        self.depth += 1;
        let r = self.parse_block_inner(closing);
        self.depth -= 1;
        r
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
        node: &mut BTreeMap<String, Value>,
        applied_contracts: &mut Vec<String>,
        pending_when: &mut Option<bool>,
    ) -> Result<(), SmlError> {
        // @name { ... } 片段定义 (不进主树)
        self.next();
        let fname = match self.next() {
            Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
            _ => return Err(SmlError::new(E_PARSE_011, "sml: @ 后需片段名")),
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
                .map_err(|e| SmlError::new(E_EXT_008, e))?;
            match &outcome {
                // 元数据块：文档里写了，解析结果里不出现
                Outcome::Discard => {}
                // 展开进主树：值必须是对象，其字段合并进指令所在的块
                Outcome::Emit(Value::Object(m)) => node.extend(m.clone()),
                Outcome::Emit(other) => {
                    return Err(SmlError::new(E_EXT_004, format!(
                        "sml: 扩展指令 `@{fname}` 的 Emit 须返回对象（用于合并进所在块），得 {other:?}"
                    )))
                }
            }
            return Ok(());
        }
        // —— 契约定义：`@contract Name { ... }` ——
        if fname == "contract" {
            if !self.features.has(Feature::Contract) {
                return Err(SmlError::new(E_FEATURE_001, "@contract 需要特性 `contract`，但当前特性集已禁用"));
            }
            let cname = match self.next() {
                Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                other => {
                    return Err(SmlError::new(E_PARSE_019, format!("sml: @contract 后须契约名, 得 {:?}", other)))
                }
            };
            // 可选修饰符 `loose` / `strict`：显式声明严格度。
            // - loose  → 允许契约未声明的字段（放宽必须写出来）
            // - strict → 显式严格（与默认等价，写出仅为可读性/团队规范）
            // 均复用裸词，不引入新 token。对齐 JS 实现：js/sml.mjs
            // 同样接受这两个词。
            let mut allow_extra = false;
            if let Some(Tok::Word(w)) = self.peek().cloned() {
                if w == "loose" {
                    allow_extra = true;
                    self.next();
                } else if w == "strict" {
                    self.next();
                }
            }
            if self.peek() != Some(&Tok::LBrace) {
                return Err(SmlError::new(E_PARSE_019, format!("sml: @contract {} 后须 {{ ... }}", cname)));
            }
            self.next();
            let mut requires: Vec<String> = Vec::new();
            let fields = self.parse_contract_body(&mut requires)?;
            // 定义时校验（不是应用时）：契约体里 `@requires` 声明的特性都必须已开启。
            // 特性集在解析前已由 `scan.rs` 把全文的 `@feature` 指令一并剥离并应用，
            // 因此此处校验的是「文档在任意位置声明过的特性」的并集，与书写顺序无关。
            for req in &requires {
                let f = Feature::from_name(req).ok_or_else(|| {
                    SmlError::new(
                        E_FEATURE_003,
                        format!(
                            "sml: 契约 `{}` 的 `@requires` 引用了未知特性 `{}`",
                            cname, req
                        ),
                    )
                })?;
                if !self.features.has(f) {
                    return Err(SmlError::new(
                        E_CONTRACT_018,
                        format!(
                            "sml: 契约 `{}` 要求特性 `{}` 已开启，但当前特性集未开启它（请用 `@feature enable {}`）",
                            cname, req, req
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
                    allow_extra,
                    requires,
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
                matches!(self.peek(), Some(Tok::Word(w)) if w.as_str() == "name")
                    && matches!(self.toks.get(self.i + 1), Some(Tok::Colon));
            if is_type_decl {
                self.next(); // 消费 name
                self.next(); // 消费 :
                let tname = match self.next() {
                    Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                    other => {
                        return Err(SmlError::new(E_PARSE_019, format!(
                            "sml: @type name: 后须类型名, 得 {:?}",
                            other
                        )))
                    }
                };
                if self.peek() != Some(&Tok::LBrace) {
                    return Err(SmlError::new(E_PARSE_019, format!("sml: @type {} 后须 {{ ... }} 模式体", tname)));
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
                return Err(SmlError::new(E_FEATURE_001, "@is 需要特性 `contract`，但当前特性集已禁用"));
            }
            let raw = match self.next() {
                Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                other => {
                    return Err(SmlError::new(E_PARSE_019, format!("sml: @is 后须契约名, 得 {:?}", other)))
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
            let mut queue: Vec<(String, usize)> = vec![(raw, self.i - 1)];
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
                            // 后跟 `:` 且又不是契约名 ⇒ 它是键，停
                            if matches!(self.peek_at(1), Some(Tok::Colon))
                                && self.resolve_contract_word(&w).is_err()
                            {
                                break;
                            }
                            self.next();
                            prev = idx;
                            queue.push((w, idx));
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
                match self.resolve_contract_word(r) {
                    Ok(n) => {
                        if !applied_contracts.contains(&n) {
                            applied_contracts.push(n);
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
                    return Err(SmlError::new(E_FEATURE_001, 
                        "@when 需要特性 `when`，请先写 `@feature enable when`（该特性默认关闭）",
                    ));
                }
                if pending_when.is_some() {
                    return Err(SmlError::new(E_PARSE_016, "sml: `@when` 连续出现两次；它只作用于紧邻的下一个字段/块"));
                }
                let cond = crate::cond::eval_when_cond(self)?;
                *pending_when = Some(cond);
                return Ok(());
            }
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
                    return Err(SmlError::new(E_PARSE_020, format!(
                        "sml: 片段 `@{fname}` 的参数 `{kw}:` 后须值, 得 {:?}",
                        other
                    )))
                }
            };
            if kw == "type" {
                if ftype.is_some() {
                    return Err(SmlError::new(E_PARSE_020, format!(
                        "sml: 片段 `@{fname}` 的 `type:` 参数重复"
                    )));
                }
                ftype = Some(val);
            } else {
                if farg.is_some() {
                    return Err(SmlError::new(E_PARSE_020, format!(
                        "sml: 片段 `@{fname}` 的 `name:` 参数重复"
                    )));
                }
                farg = Some(val);
            }
        }
        // 既非 `{` 也非流末尾：既可能是拼错的指令，也可能是旧的位置参数形式。
        // 两种意图无法区分，故错误信息同时给出两条排查指引。
        if !matches!(self.peek(), Some(Tok::LBrace) | None) {
            return Err(SmlError::new(E_PARSE_005, format!(
                "sml: `@{fname}` 不是合法指令且缺少片段体 {{ ... }}；\
                 若本意是「片段定义」，其参数须显式写作 `type: X` 与 `name: Y`\
                 （如 `@{fname} type: Server name: prod {{ .. }}`），\
                 位置参数形式（`@{fname} X [Y] {{ .. }}`）自 v4 起已废弃，\
                 不带参数时写作 `@{fname} {{ .. }}`；\
                 若本意是「指令」，请检查拼写（合法指令：contract / is / version / feature）"
            )));
        }
        if self.peek() == Some(&Tok::LBrace) {
            self.next();
            let blk = self.parse_block(Some(Tok::RBrace))?;
            let mut sub = match &blk {
                Value::Object(m) => m.clone(),
                other => {
                    let mut m = BTreeMap::new();
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
                return Err(SmlError::new(E_FEATURE_001, format!(
                    "sml: 片段定义 `@{}` 需要特性 `fragment`，但当前特性集已禁用",
                    fname
                )));
            }
            // 命名空间前缀隔离：片段定义按当前 ns 栈路径注册
            self.fragments.insert(self.qualify(&fname), Value::Object(sub));
        } else {
            // `@name` 既不是已知指令（contract/is）也不是片段定义（后无 `{`）：
            // 必须报错，而非静默吞掉后续行/块（P0-2：单独 `@` 会清空整个文档）。
            return Err(SmlError::new(E_PARSE_005, format!(
                "sml: `@{}` 不是合法指令且缺少片段体 {{ ... }}，无法解析",
                fname
            )));
        }
        Ok(())
    }
    pub(crate) fn parse_block_inner(&mut self, closing: Option<Tok>) -> Result<Value, SmlError> {
        let mut node: BTreeMap<String, Value> = BTreeMap::new();
        // 块内若声明了 `@is Name`（可多条、可一名多词），在块解析完成后应用契约
        let mut applied_contracts: Vec<String> = Vec::new();
        // `@when <条件>` 的求值结果，作用于**紧邻的下一个**键/块（消费后清空）。
        // None = 无待应用的条件；Some(true/false) = 下一个字段应保留/丢弃。
        let mut pending_when: Option<bool> = None;
        loop {
            let tok = match self.peek().cloned() {
                None => {
                    // 文件结尾：若本块期望闭合符号（嵌套块/数组），则未闭合，必须报错；
                    // 否则为顶层正常结束。
                    if closing.is_some() {
                        let want = match closing {
                            Some(Tok::RBrace) => "}",
                            Some(Tok::RBrack) => "]",
                            _ => unreachable!(),
                        };
                        return Err(SmlError::new(E_PARSE_001, format!(
                            "sml: 未闭合的块/数组（遇到文件结尾，缺少结束符号 {}）",
                            want
                        )));
                    }
                    break;
                }
                Some(t) => t,
            };
            match tok {
                Tok::RBrace | Tok::RBrack => {
                    if let Some(cl) = &closing {
                        if *cl == tok {
                            self.next();
                            break;
                        }
                        // 嵌套块内遇到类型不匹配的右符号（如 `}` 与 `]` 混用）：
                        // 必须报错，而非静默吞掉，否则后续内容会被错误吞并。
                        let want = match cl {
                            Tok::RBrace => "}",
                            Tok::RBrack => "]",
                            _ => unreachable!(),
                        };
                        let got = match tok {
                            Tok::RBrace => "}",
                            Tok::RBrack => "]",
                            _ => unreachable!(),
                        };
                        return Err(SmlError::new(E_PARSE_002, format!(
                            "sml: 块/数组未正确闭合：期望 {}，却遇到 {}",
                            want, got
                        )));
                    }
                    // 顶层遇到多余的右括号 `}` / `]`：必须报错（之前静默忽略，
                    // 会掩盖作者漏写的 `key:`、错配括号等问题）。
                    let got = match tok {
                        Tok::RBrace => "}",
                        Tok::RBrack => "]",
                        _ => unreachable!(),
                    };
                    return Err(SmlError::new(E_PARSE_003, format!("sml: 多余的结束符号 {}", got)));
                }
                Tok::Comma => {
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
                    } else {
                        return Err(SmlError::new(E_PARSE_007, 
                            "sml: 非预期的逗号（裸词中不可含逗号，请用 [..] 数组或 \"...\" 引号）",
                        ));
                    }
                }
                // 孤立 `@`：其后没有片段名/指令名。必须报错——否则其后紧跟的
                // 块会被当作片段体消费，导致内容被静默丢弃（不报错）。
                Tok::BareAt => {
                    return Err(SmlError::new(E_PARSE_004, 
                        "sml: 孤立的 `@` 不是合法指令；片段定义须写作 `@name { ... }`（`@` 与名字之间不可有空白），或删除该 `@`",
                    ));
                }
                Tok::At => {
                    self.parse_at_directive(&mut node, &mut applied_contracts, &mut pending_when)?;
                }
                _ => {
                    // key
                    let key = match self.next() {
                        Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                        other => {
                            // 位置指向**刚消费掉的那个 token**（游标已越过它）：
                            // `a: B::C` 报的是「期望键, 得 Some(Colon)」，此前没有任何位置。
                            let pos = self.i.saturating_sub(1);
                            return Err(self.err_at(
                                E_PARSE_006,
                                format!("sml: 期望键, 得 {:?}", other),
                                pos,
                            ));
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
                        return Err(SmlError::new(E_PARSE_013, 
                            "sml: 键名位置不支持 `${var}` 插值（@for 的循环变量仅用于值位置）。\
                             若需动态键，请改用数组表达，例如 `hosts: @for h in a b { name: \"${h}\" }`",
                        ));
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
                        let resolved = coerce_word(
                            &key,
                            &self.fragments,
                            self.features,
                            &self.ns_prefix(),
                            Some(&self.env),
                        )?;
                        // `@when` 条件为假：片段已解析（token 已消费）但不写入
                        if let Some(false) = pending_when.take() {
                            continue;
                        }
                        match &resolved {
                            Value::Object(m) => {
                                for (fk, fv) in m {
                                    // 静默改数据（雷 5 · A 组第 5 项）：片段合并是**直接赋值**，
                                    // 不走下方「同名提升为数组」的统一规则 ⇒ 父块已有的同名键
                                    // 会被**整个丢掉**，且不报错。语义保留（ch03/ch08 既定），
                                    // 但必须让调用方看见 —— 故记一条 KeyOverwritten。
                                    if node.contains_key(fk) {
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
                                    node.insert(fk.clone(), fv.clone());
                                }
                            }
                            other => {
                                return Err(SmlError::new(E_INCLUDE_007, format!(
                                    "sml: 片段 `{}` 展开结果不是对象，无法合并: {:?}",
                                    key, other
                                )));
                            }
                        }
                        continue;
                    }
                    let val = self.parse_value(&key, colon)?;
                    // `@when` 条件为假：值已解析（token 已消费），但不写入父节点。
                    // 必须先解析再丢弃，否则其 token 会被当成下一个键，导致误报。
                    if let Some(false) = pending_when.take() {
                        continue;
                    }
                    // 同名冲突 -> 提升为数组
                    //
                    // 静默改数据（雷 5 · A 组第 4 项）：这是 SML **既定语义**（`k: 1` + `k: 2`
                    // ⇒ `k: [1, 2]`），值没丢，但对象结构被改写 —— 下游按标量读 `k` 的代码
                    // 会拿到数组。语义保留，改为告警（不静默）。
                    if let Some(existing) = node.get_mut(&key) {
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
                                let old = node.remove(&key).unwrap();
                                node.insert(key, Value::Array(vec![old, val]));
                            }
                        }
                    } else {
                        node.insert(key, val);
                    }
                }
            }
        }
        // 悬空的 `@when`：条件后没有跟随任何字段/块，几乎可以肯定是笔误
        //（例如把 `@when` 写在了块末尾，或条件本想作用于块内却被写到了块外）。
        // 静默忽略会让作者以为条件生效了，故显式报错。
        if pending_when.is_some() {
            return Err(SmlError::new(E_PARSE_017, "sml: `@when` 后未跟随任何字段/块；它只作用于紧邻的下一个字段/块"));
        }
        // 块结束：若声明了 `@is`，应用契约（填默认值 + 校验 + 严格性检查）
        if applied_contracts.len() == 1 {
            // 单契约：**逐字走原路径**（错误文案 / 行为与旧版完全一致）
            let cname = &applied_contracts[0];
            let hint = suggest_contract_name(cname, &self.contracts)
                .map(|c| format!("（你是不是想写 `{c}`？）"))
                .unwrap_or_default();
            let c = self
                .contracts
                .get(cname)
                .cloned()
                .ok_or_else(|| {
                    SmlError::new(E_CONTRACT_001, format!("sml: 未定义的契约 `{}`{}", cname, hint))
                })?;
            // 用命名空间栈拼出当前块路径（如 `api`、`ns.api`），作为错误定位前缀
            apply_contract(&c, &mut node, &self.contracts, &self.ns_prefix())?;
        } else if applied_contracts.len() > 1 {
            // 多契约：叠加（D3：strict 优先 / 字段并集 / 冲突 ⇒ E-CONTRACT-016）
            apply_contracts(
                &applied_contracts,
                &mut node,
                &self.contracts,
                &self.ns_prefix(),
            )?;
        }
        // 契约映射：本块（路径 = path_stack，调用方已把键/下标压好）应用了哪些契约。
        // 仅在应用**成功**后记 —— 失败即整篇报错，不存在半张映射。
        if let Some(map) = &mut self.contract_map {
            for n in &applied_contracts {
                map.record(self.path_stack.clone(), n.clone());
            }
        }
        Ok(Value::Object(node))
    }

    /// 解析一个值 (在 key 之后)
    /// 预扫描：当前位置是否形如裸块的**参数部分**（`[name...] {`），
    /// 即连续若干 `Word`/`Str` 后紧跟 `{`。不消费任何 token。
    fn bare_block_ahead(&self) -> bool {
        if !matches!(self.peek(), Some(Tok::Word(_)) | Some(Tok::Str(_))) {
            return false;
        }
        let mut probe = self.i;
        while probe < self.toks.len() {
            match &self.toks[probe] {
                Tok::Word(_) | Tok::Str(_) => probe += 1,
                Tok::LBrace => return true,
                _ => return false,
            }
        }
        false
    }

    /// 解析裸块体：调用前类型名本身已消费，当前位置处于参数处；
    /// `key` 即写入块内的 `__type`。返回解析出的块。
    fn parse_bare_block(&mut self, key: &str) -> Result<Value, SmlError> {
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
                        args.push(Value::Str(s));
                    }
                }
                _ => break,
            }
        }
        if self.peek() != Some(&Tok::LBrace) {
            return Err(SmlError::new(E_PARSE_012, "sml: 语法错误"));
        }
        self.next();
        // 进入子块 = 进入该 block 名字的命名空间
        self.ns_stack.push(key.to_string());
        let mut sub = self.parse_block(Some(Tok::RBrace))?;
        self.ns_stack.pop();
        if let Value::Object(m) = &mut sub {
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
            if self.features.has(Feature::TypedBlock) && self.features.has(Feature::Contract) {
                // 命名空间隔离：先按裸名查，再按当前 ns 前缀查（与 `@is` 一致）
                let resolve = |w: &str| -> Option<String> {
                    if self.contracts.contains_key(w) {
                        Some(w.to_string())
                    } else {
                        let q = self.qualify(w);
                        if self.contracts.contains_key(&q) {
                            Some(q)
                        } else {
                            // opt-in `case-insensitive`：与 `@is` 同一条兜底口径
                            self.contract_by_first_letter(w)
                        }
                    }
                };
                if let Some(r) = resolve(key) {
                    names.push(r);
                    for a in &args {
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
                if !names.is_empty()
                    && args.len() > consumed_args
                    && !self.features.has(Feature::BlockName)
                {
                    if let Value::Str(w) = &args[consumed_args] {
                        let hint = suggest_contract_name(w, &self.contracts)
                            .map(|c| format!("（你是不是想写 `{c}`？）"))
                            .unwrap_or_default();
                        return Err(SmlError::new(
                            E_CONTRACT_001,
                            format!(
                                "sml: 块级标注 `{key}` 后的 `{w}` 不是已定义的契约；\
                                 若本意是把它作为块名，请先 `@feature enable block-name`{}",
                                hint
                            ),
                        ));
                    }
                }
                let mut applied = false;
                if names.len() == 1 {
                    // 单契约：**逐字走原路径**（错误文案 / 行为与旧版完全一致）
                    let cname = &names[0];
                    let hint = suggest_contract_name(cname, &self.contracts)
                        .map(|c| format!("（你是不是想写 `{c}`？）"))
                        .unwrap_or_default();
                    let c = self.contracts.get(cname).cloned().ok_or_else(|| {
                        SmlError::new(E_CONTRACT_001, format!("sml: 未定义的契约 `{}`{}", cname, hint))
                    })?;
                    apply_contract(&c, m, &self.contracts, &self.ns_prefix())?;
                    applied = true;
                } else if names.len() > 1 {
                    // 多契约：叠加（D3：strict 优先 / 字段并集 / 冲突 ⇒ E-CONTRACT-016）
                    apply_contracts(&names, m, &self.contracts, &self.ns_prefix())?;
                    applied = true;
                }
                if applied {
                    // 契约映射：path_stack 此刻仍含本块的键/下标（调用方在
                    // `parse_bare_block` 返回后才 pop），正是本块在结果树里的位置。
                    if let Some(map) = &mut self.contract_map {
                        for n in &names {
                            map.record(self.path_stack.clone(), n.clone());
                        }
                    }
                }
            }
            // __name 的起点：多契约时跳过被算作契约词的 args（首个非契约词是块名）；
            // 单契约 / 无契约与旧版完全一致（args[0]）
            let name_args: &[Value] = if names.len() > 1 {
                &args[consumed_args..]
            } else {
                &args[..]
            };
            m.insert("__type".into(), Value::Str(key.to_string()));
            // 裸块参数全部保留：首个作 __name，其余放入 __args 数组，
            // 不再静默丢弃（P1-3：`server web prod {}` 的 web/prod 都应可见）。
            if !name_args.is_empty() {
                m.insert("__name".into(), name_args[0].clone());
                if name_args.len() > 1 {
                    m.insert("__args".into(), Value::Array(name_args[1..].to_vec()));
                }
            }
        }
        Ok(sub)
    }

    fn parse_value(&mut self, key: &str, colon: bool) -> Result<Value, SmlError> {
        // 值位置的 `@for var in ... { }` 指令：有界循环展开为数组。
        // 受 cargo feature `when` 门控（与 `@when` 同属 cond 解析期指令家族）。
        #[cfg(feature = "when")]
        if let Some(Tok::At) = self.peek() {
            if let Some(Tok::Word(w)) = self.peek_at(1) {
                if w == "for" {
                    return self.parse_for_value();
                }
            }
        }
        // 无冒号且后继是裸词: 可能是裸块 `type [name] { }`
        if !colon && self.bare_block_ahead() {
            // 路径压 Key(key)：裸块在结果树里的位置就是「键 key 的值」。
            // （`parse_bare_block` 内部不自压 —— 数组里的裸块由调用方压 Index。）
            self.path_stack.push(PathSeg::Key(key.to_string()));
            let v = self.parse_bare_block(key);
            self.path_stack.pop();
            return v;
        }
        match self.peek().cloned() {
            Some(Tok::LBrace) => {
                self.next();
                // 进入子对象 = 路径多一段 Key（契约映射用；ns_stack 语义不变）
                self.path_stack.push(PathSeg::Key(key.to_string()));
                let v = self.parse_block(Some(Tok::RBrace));
                self.path_stack.pop();
                v
            }
            Some(Tok::LBrack) => {
                self.next();
                self.path_stack.push(PathSeg::Key(key.to_string()));
                let v = self.parse_array();
                self.path_stack.pop();
                v
            }
            Some(tok @ (Tok::Word(_) | Tok::Str(_))) => {
                let v = match tok {
                    Tok::Word(w) => coerce_word(
                        &w,
                        &self.fragments,
                        self.features,
                        &self.ns_prefix(),
                        Some(&self.env),
                    )?,
                    Tok::Str(s) => {
                        // 引号串承认 `${var}`（@for 循环变量）与 `$env.NAME` 内联插值。
                        // 必须**与裸词路径一致**地受 `Feature::Env` 约束：否则调用方
                        // 禁用 env 特性后，文档仍可用引号串绕过限制读取任意环境变量。
                        if s.contains("$env.") && !self.features.has(Feature::Env) {
                            return Err(SmlError::new(E_FEATURE_002, format!(
                                "sml: 当前特性集禁用了 `$env`（env），字符串 `\"{}\"` 无法解析",
                                s
                            )));
                        }
                        Value::Str(self.interp_str(&s))
                    }
                    _ => unreachable!(),
                };
                self.next();
                Ok(v)
            }
            // 键后无值: `key }` / `key ]` / `key ,` / 行尾 —— key 本身即值 (片段引用/裸词)
            Some(Tok::RBrace) | Some(Tok::RBrack) | Some(Tok::Comma) | None => {
                if colon {
                    // 有冒号但无值: 空值
                    Ok(Value::Null)
                } else {
                    Ok(coerce_word(
                        key,
                        &self.fragments,
                        self.features,
                        &self.ns_prefix(),
                        Some(&self.env),
                    )?)
                }
            }
            _ => Err(SmlError::new(E_PARSE_012, "sml: 语法错误")),
        }
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
            return Err(SmlError::new(E_FEATURE_001, 
                "sml: `@for` 未启用，需在文档中 `@feature enable for`（或在契约中声明）",
            ));
        }
        // 解析 `var in a b c`，游标停在 `{` 处。
        let (var, items) = crate::cond::eval_for_header(self)?;
        // 切出循环体内部 token（不含两端括号），游标已越过 `}`。
        let body = self.slice_block_tokens()?;

        let mut arr = Vec::with_capacity(items.len());
        for item in &items {
            let mut child = self.spawn_for_child(body.clone(), &var, item);
            // 子解析器从块内部 token 起始，已无外层 `}`，故以 EOF 为结束（closing=None）。
            let val = child.parse_block(None)?;
            arr.push(val);
        }
        Ok(Value::Array(arr))
    }

    /// 外层 wrapper：深度守卫，防止深度嵌套数组触发递归下降的栈溢出。
    /// 实际实现见 [`Parser::parse_array_inner`]。
    pub(crate) fn parse_array(&mut self) -> Result<Value, SmlError> {
        // 与 `parse_block` 同一口径（`>` 而非 `>=`，128 层放行 / 129 层报）——
        // 两处必须一起改，否则块嵌套与数组嵌套的边界会差一格。
        if self.depth > MAX_VALUE_DEPTH {
            return Err(SmlError::new(E_LIMIT_001, format!(
                "sml: 嵌套过深（超过 {} 层），疑似递归或恶意输入",
                MAX_VALUE_DEPTH
            )));
        }
        self.depth += 1;
        let r = self.parse_array_inner();
        self.depth -= 1;
        r
    }

    pub(crate) fn parse_array_inner(&mut self) -> Result<Value, SmlError> {
        let mut arr = Vec::new();
        loop {
            match self.peek().cloned() {
                None => {
                    // 顶层数组未闭合（缺少 `]`）：必须报错，而非按 EOF 静默收尾。
                    return Err(SmlError::new(E_PARSE_001, "sml: 未闭合的数组（遇到文件结尾，缺少结束符号 ]）"));
                }
                Some(Tok::RBrack) => {
                    self.next();
                    break;
                }
                Some(Tok::RBrace) => {
                    // 顶层数组遇到多余的 `}`：必须报错，而非静默忽略。
                    return Err(SmlError::new(E_PARSE_003, "sml: 多余的结束符号 }（数组应以 ] 闭合）"));
                }
                Some(Tok::Comma) => {
                    self.next();
                }
                Some(Tok::LBrace) => {
                    self.next();
                    self.path_stack.push(PathSeg::Index(arr.len()));
                    let v = self.parse_block(Some(Tok::RBrace));
                    self.path_stack.pop();
                    arr.push(v?);
                }
                Some(Tok::LBrack) => {
                    self.next();
                    // 走 wrapper 以复用 depth 守卫（否则嵌套数组会绕过 128 层限制导致栈溢出 DoS）
                    self.path_stack.push(PathSeg::Index(arr.len()));
                    let v = self.parse_array();
                    self.path_stack.pop();
                    arr.push(v?);
                }
                Some(Tok::Word(w)) => {
                    // 数组内的裸块 `Type [name] { ... }`：表达**有序**元素序列。
                    // 对象字段用 BTreeMap 存储（键有序、不可保书写序），因此
                    // 需要保序的子元素（UI 布局、文档章节）需写作数组。
                    if self.bare_block_ahead() {
                        self.next(); // 消费类型名
                        self.path_stack.push(PathSeg::Index(arr.len()));
                        let v = self.parse_bare_block(&w);
                        self.path_stack.pop();
                        arr.push(v?);
                    } else {
                        arr.push(coerce_word(
                            &w,
                            &self.fragments,
                            self.features,
                            &self.ns_prefix(),
                            Some(&self.env),
                        )?);
                        self.next();
                    }
                }
                Some(Tok::Str(_)) => {
                    if let Some(Tok::Str(s)) = self.next() {
                        arr.push(Value::Str(s));
                    }
                }
                _ => break,
            }
        }
        Ok(Value::Array(arr))
    }
}
