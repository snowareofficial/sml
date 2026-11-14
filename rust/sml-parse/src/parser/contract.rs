// SPDX-License-Identifier: MulanPSL-2.0
//! 契约体与字段规格的解析（从 `parser.rs` 抽出，**纯搬运**）。
//!
//! 子模块是 `parser` 的后代，可访问 `Parser` 的私有字段；`use super::*;` 复用父模块的导入，
//! 所以这里只看得见「方法本体」，不用重复一长串 `use sml_codes::…`。

use super::*;

impl<'a> Parser<'a> {

    /// 解析契约体：逐条读 `field: <类型> [修饰符...]`，并收集 `@requires` 声明的特性。
    ///
    /// 契约体目前仅支持两类条目：
    /// - 字段规格 `field: <类型> ...`（主路径）；
    /// - 指令 `@requires feat1, feat2`（特性前置声明）。
    ///
    /// 其余 `@xxx` 一律报错而非静默退化成片段（这正是本项目反复踩的「静默失效」坑：
    /// 不认识的 `@xxx` 若走片段定义分支，会被悄无声息地当成名为 `xxx` 的片段消费掉）。
    pub(super) fn parse_contract_body(
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
    pub(super) fn unwrap_type_call(s: &str) -> Option<String> {
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
    pub(super) fn reject_legacy_modifier_in_type(w: &str) -> Result<(), SmlError> {
        if let Some(sym) = w.chars().find(|c| *c == '?' || *c == '=') {
            return Err(SmlError::from_code_title(E_PARSE_006)
                .with_detail(format!("契约修饰符 `{sym}` 已废弃（用 `optional` / `default`）")));
        }
        Ok(())
    }

    /// 解析单个字段的类型与修饰符
    pub(super) fn parse_field_spec(&mut self) -> Result<FieldSpec, SmlError> {
        // 命中外置类型时，把校验器带出去（塞进 `FieldSpec::ext`，校验期无需再查表）。
        let mut ext_ty: Option<Arc<dyn TypeCheck>> = None;
        let ty = match self.next() {
            Some(Tok::Word(w)) => {
                // 遗留符号修饰符 `?` / `=` 已废弃（2026-10-30），五端一律 E-PARSE-006。
                //
                // 为什么**不在词法层**把它们切成分隔符：`?` 在 `@pattern` 里是量词
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

    pub(super) fn parse_modifier_value(&mut self) -> Result<Value, SmlError> {
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

    pub(super) fn parse_spec_number(&mut self) -> Result<f64, SmlError> {
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
}
