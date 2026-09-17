// SPDX-License-Identifier: MulanPSL-2.0
//! 解析期条件/重复原语：`@when`（已实现）与 `@for`。
//!
//! 本模块由 cargo feature `when` 门控。
//!
//! 刻意**不**独立成 crate：需要直接操作 `Parser` 的词法游标
//! （`next`/`peek`/`peek_at`），外置会造成 parse ↔ cond 循环依赖。

use sml_feature::Feature;
use sml_lex::Tok;

use crate::parser::Parser;

/// `@when` 支持的条件形式（**闭集**）：
///
/// ```text
/// @when $env.NAME              # 真值：值非空且不等于 "0" / "false"
/// @when $env.NAME == "value"   # 相等（右侧可为引号串或裸词）
/// @when $env.NAME != "value"   # 不等
/// ```
///
/// # 为什么不做成通用表达式
///
/// 一旦支持 `&&` / `||` / 括号 / 函数调用，就必须引入表达式求值器，进而需要
/// 沙箱、超时、禁 IO 等一整套防护，且与「SML 是纯数据格式」的定位冲突。
/// 这里只做**闭集模式匹配**：解析器直接读取 1 或 3 个 token，没有任何求值。
/// 想表达复合条件请拆成多个 `@when`（或用外层构建工具按环境生成不同文件）。
///
/// 只读取 `$env.*`：环境变量是唯一在解析期就有确定值、且语义无歧义的输入。
/// 不支持引用文档内其它字段，避免产生求值顺序与前向引用的复杂语义。
///
/// 词法器没有 `=`/`==` 特殊 token，`==` 会以 `Word("==")` 出现，故本函数
/// 无需改动 tokenize 即可工作。
pub fn eval_when_cond(p: &mut Parser) -> Result<bool, String> {
    // 左侧：必须是 `$env.NAME`
    let lhs = match p.next() {
        Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
        other => {
            return Err(format!(
                "sml: `@when` 后须条件（如 `$env.ENV == \"prod\"`），得 {:?}",
                other
            ))
        }
    };
    let var = lhs.strip_prefix("$env.").ok_or_else(|| {
        format!(
            "sml: `@when` 的条件左侧只支持 `$env.NAME`，得 `{lhs}`\
             （暂不支持文档内字段引用）"
        )
    })?;
    if var.is_empty() {
        return Err("sml: `@when` 的 `$env.` 后须变量名".into());
    }
    if !p.features.has(Feature::Env) {
        return Err(format!(
            "sml: `@when $env.{var}` 需要特性 `env`，但当前特性集已禁用"
        ));
    }
    let actual = p.env_var(var);

    // 运算符：可选。`==` / `!=` 不是特殊 token，词法器会切成 Word。
    let op = match p.peek() {
        Some(Tok::Word(w)) if w == "==" || w == "!=" || w == "=" => {
            let op = w.clone();
            p.next();
            Some(op)
        }
        _ => None,
    };

    match op {
        None => {
            // 真值测试：空串 / "0" / "false" 视为假
            Ok(!actual.is_empty() && actual != "0" && actual != "false")
        }
        Some(op) => {
            let rhs = match p.next() {
                Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                other => {
                    return Err(format!(
                        "sml: `@when` 的 `{op}` 后须比较值，得 {:?}",
                        other
                    ))
                }
            };
            // 漏写比较值时（`@when $env.X ==` 后直接换行写下一个字段），
            // 下一个字段名会被当成比较值吃掉，只在更后面才报出莫名的
            // 「期望键」错误。这里提前识别并给出准确提示。
            if p.peek() == Some(&Tok::Colon) {
                return Err(format!(
                    "sml: `@when` 的 `{op}` 后缺少比较值（写成了 `{op}` 后直接换行）；\
                     正确形式如 `@when $env.ENV == \"prod\"`"
                ));
            }
            // 单 `=` 按 `==` 处理（与多数配置语言一致），但要求显式写出
            let eq = actual == rhs;
            Ok(if op == "!=" { !eq } else { eq })
        }
    }
}

/// 解析 `@for` 的「头部」：`var in a b c`（不含循环体）。
///
/// 调用前游标应停在 `@`（即 `parse_value` 已 peek 到 `@` + `for` 但**未消费**）。
/// 消费顺序：`@` → `for` → `<var>` → `in` → 各枚举项，最后游标停在 `{`（循环体左花括号），
/// 由 [`crate::Parser::slice_block_tokens`] 接管并切出循环体。
///
/// - `<var>` 必须是裸词（循环变量名），循环体内以 `${var}` 只读引用；
/// - `in` 后的枚举项只接受裸词或引号串（**有限列表**，不允许 `$env.*` 展开为多个项，
///   那是另一层次的「有界」语义，留待将来）；
/// - 至少须有 1 个枚举项，否则报错（空循环体无意义）。
pub fn eval_for_header(p: &mut Parser) -> Result<(String, Vec<String>), String> {
    // 消费 `@`
    match p.next() {
        Some(Tok::At) => {}
        other => {
            return Err(format!(
                "sml: 内部错误：eval_for_header 期望 `@`，得 {:?}",
                other
            ))
        }
    }
    // 消费 `for`
    match p.next() {
        Some(Tok::Word(w)) if w == "for" => {}
        other => {
            return Err(format!(
                "sml: 内部错误：eval_for_header 期望 `for`，得 {:?}",
                other
            ))
        }
    }
    // 循环变量名
    let var = match p.next() {
        Some(Tok::Word(w)) => w,
        other => {
            return Err(format!(
                "sml: `@for` 后须为循环变量名（裸词），得 {:?}",
                other
            ))
        }
    };
    // 关键字 `in`
    match p.next() {
        Some(Tok::Word(w)) if w == "in" => {}
        other => {
            return Err(format!(
                "sml: `@for` 变量 `{var}` 后须为关键字 `in`，得 {:?}",
                other
            ))
        }
    }
    // 收集枚举项，直到 `{`（循环体）
    let mut items = Vec::new();
    loop {
        match p.peek() {
            Some(Tok::LBrace) => break, // 停在 '{'，交给 slice_block_tokens
            Some(Tok::Word(w)) => {
                let w = w.clone();
                p.next();
                items.push(w);
            }
            Some(Tok::Str(s)) => {
                let s = s.clone();
                p.next();
                items.push(s);
            }
            Some(other) => {
                return Err(format!(
                    "sml: `@for ... in` 后须枚举项或 `{{`，得 {:?}",
                    other
                ))
            }
            None => return Err("sml: `@for` 缺少循环体 `{ ... }`".into()),
        }
    }
    if items.is_empty() {
        return Err("sml: `@for` 的 `in` 后至少须有一个枚举项".into());
    }
    Ok((var, items))
}
