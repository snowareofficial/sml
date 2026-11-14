// SPDX-License-Identifier: MulanPSL-2.0
//! @ 指令处理：@is / @contract / @version / @feature / @when / 片段与弱绑定声明
//!
//! 从 `parser/mod.rs` 抽出，纯搬运。子模块是 `parser` 的后代，可直接用 `Parser` 的私有字段；
//! `use super::*;` 复用父模块的导入。

use super::*;

impl<'a> Parser<'a> {
    // 契约体 / 字段规格：见 `contract.rs`（本文件只留调用点）。


    /// `@` 指令与片段定义的处理体 —— 从 [`parse_block_inner`] 抽出。
    ///
    /// **为什么抽出来**：`parse_block_inner` 曾是一个 ~558 行的巨函数，debug 构建下
    /// 栈帧大到「递归到 `MAX_VALUE_DEPTH`(128) 层就撑爆默认 2 MiB 线程栈」。
    /// 本分支独占 ~350 行，是肥帧主因 —— 抽出后这些局部量不再占据**每一层递归**
    /// 的栈帧。详见 `TODO.md` 排雷区雷 1。
    ///
    /// 返回 `Ok(())` 表示本条已处理完毕，等价于原地的 `continue`。
    pub(super) fn parse_at_directive(
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
            // 两条停机判据缺一不可：
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
}
