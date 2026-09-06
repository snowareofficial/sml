//! regex 逃生舱：把 regex 子集**编译进同一个 IR**，而非引入第二个引擎。
//!
//! # 为什么这样才干净
//!
//! 常见做法是内嵌原生 regex、委托给宿主引擎（Python `re` / JS `RegExp`），
//! 代价是那段**立刻失去线性时间保证**——ReDoS 又回来了，而且跨语言行为还不一致
//! （`\d` 在 JS 是 ASCII、Python 是 Unicode）。
//!
//! 这里改成：regex 只是模式的**另一种前端**，解析后编译成 [`Pat`]，
//! 交给同一个 Thompson NFA 执行。因此：
//! - 天然无回溯，逃生舱**不逃掉安全保证**
//! - 语义在所有语言绑定下一致
//!
//! # 明确不支持（安全原因，不是偷懒）
//!
//! - 反向引用 `\1`：与自动机模型根本冲突（NP 完全）
//! - lookaround `(?=)` `(?!)` `(?<=)` `(?<!)`：破坏线性时间与流式处理
//!
//! 遇到这些会**报错**而非静默降级——静默降级是本项目一贯要消灭的行为。

use crate::{Class, Pat};

pub fn parse_regex(src: &str) -> Result<Pat, String> {
    let mut p = Parser {
        chars: src.chars().collect(),
        i: 0,
    };
    let pat = p.parse_alt()?;
    if let Some(c) = p.peek() {
        return Err(format!(
            "sml: regex 在位置 {} 处有无法解析的内容 `{}`",
            p.i, c
        ));
    }
    Ok(pat)
}

struct Parser {
    chars: Vec<char>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.i).copied()
    }

    fn parse_alt(&mut self) -> Result<Pat, String> {
        let mut alts = vec![self.parse_seq()?];
        while self.peek() == Some('|') {
            self.i += 1;
            alts.push(self.parse_seq()?);
        }
        Ok(if alts.len() == 1 {
            alts.pop().unwrap()
        } else {
            Pat::Alt(alts)
        })
    }

    fn parse_seq(&mut self) -> Result<Pat, String> {
        let mut seq = Vec::new();
        while let Some(c) = self.peek() {
            match c {
                '|' | ')' => break,
                // 全文匹配语义，锚点是隐含的
                '^' | '$' => self.i += 1,
                _ => seq.push(self.parse_repeat()?),
            }
        }
        Ok(Pat::Seq(seq))
    }

    fn parse_repeat(&mut self) -> Result<Pat, String> {
        let atom = self.parse_atom()?;
        match self.peek() {
            Some('*') => {
                self.i += 1;
                self.skip_lazy()?;
                Ok(Pat::Repeat { pat: Box::new(atom), min: 0, max: None })
            }
            Some('+') => {
                self.i += 1;
                self.skip_lazy()?;
                Ok(Pat::Repeat { pat: Box::new(atom), min: 1, max: None })
            }
            Some('?') => {
                self.i += 1;
                self.skip_lazy()?;
                Ok(Pat::Repeat { pat: Box::new(atom), min: 0, max: Some(1) })
            }
            Some('{') => {
                let (min, max) = self.parse_counted()?;
                self.skip_lazy()?;
                Ok(Pat::Repeat { pat: Box::new(atom), min, max })
            }
            _ => Ok(atom),
        }
    }

    /// 无回溯引擎没有贪婪概念，`*?` 的 `?` 直接忽略。
    /// 不报错是因为它与「懒惰匹配」的意图在此引擎下自动成立。
    fn skip_lazy(&mut self) -> Result<(), String> {
        if self.peek() == Some('?') {
            self.i += 1;
        }
        Ok(())
    }

    fn parse_counted(&mut self) -> Result<(usize, Option<usize>), String> {
        if self.peek() != Some('{') {
            return Err("sml: 期望 `{`".into());
        }
        self.i += 1;
        let start = self.i;
        while self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            self.i += 1;
        }
        let lo: usize = self.chars[start..self.i]
            .iter()
            .collect::<String>()
            .parse()
            .map_err(|_| "sml: `{n}` 量词缺少数字".to_string())?;

        let (min, max) = if self.peek() == Some(',') {
            self.i += 1;
            let s2 = self.i;
            while self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                self.i += 1;
            }
            if s2 == self.i {
                (lo, None) // {n,}
            } else {
                let hi: usize = self.chars[s2..self.i]
                    .iter()
                    .collect::<String>()
                    .parse()
                    .map_err(|_| "sml: `{n,m}` 量词非法".to_string())?;
                (lo, Some(hi))
            }
        } else {
            (lo, Some(lo)) // {n}
        };

        if self.peek() != Some('}') {
            return Err("sml: 量词缺少 `}`".into());
        }
        self.i += 1;
        Ok((min, max))
    }

    fn parse_atom(&mut self) -> Result<Pat, String> {
        match self.peek() {
            Some('(') => {
                self.i += 1;
                // 拒绝 lookaround 与其它 `(?...)` 扩展
                if self.peek() == Some('?') {
                    let rest: String = self.chars[self.i..].iter().take(6).collect();
                    return Err(format!(
                        "sml: regex 不支持 `(?...)` 扩展（lookaround / 命名组等，见 `{rest}…`）；\
                         它们会破坏线性时间保证"
                    ));
                }
                let inner = self.parse_alt()?;
                if self.peek() != Some(')') {
                    return Err("sml: regex 分组缺少 `)`".into());
                }
                self.i += 1;
                Ok(inner)
            }
            Some('[') => Ok(Pat::Class(self.parse_char_class()?)),
            Some('.') => {
                self.i += 1;
                Ok(Pat::Class(Class::Any))
            }
            Some('\\') => self.parse_escape(),
            Some(c) => {
                self.i += 1;
                Ok(Pat::Lit(c.to_string()))
            }
            None => Err("sml: regex 意外结束".into()),
        }
    }

    fn parse_escape(&mut self) -> Result<Pat, String> {
        if self.peek() != Some('\\') {
            return Err("sml: 期望 `\\`".into());
        }
        self.i += 1;
        let c = self.peek().ok_or_else(|| "sml: `\\` 后缺少字符".to_string())?;
        self.i += 1;
        let cls = match c {
            'd' => Class::Digit,
            'D' => Class::Not(Box::new(Class::Digit)),
            'w' => Class::Word,
            'W' => Class::Not(Box::new(Class::Word)),
            's' => Class::Space,
            'S' => Class::Not(Box::new(Class::Space)),
            // 反向引用：明确拒绝
            '1'..='9' => {
                return Err(format!(
                    "sml: regex 不支持反向引用 `\\{c}`（与线性时间匹配根本冲突）；\
                     请改用命名捕获 + 等值比较"
                ))
            }
            // 其余按字面转义
            other => Class::One(other),
        };
        Ok(Pat::Class(cls))
    }

    fn parse_char_class(&mut self) -> Result<Class, String> {
        if self.peek() != Some('[') {
            return Err("sml: 期望 `[`".into());
        }
        self.i += 1;
        let negated = self.peek() == Some('^');
        if negated {
            self.i += 1;
        }

        let mut items: Vec<Class> = Vec::new();
        loop {
            match self.peek() {
                None => return Err("sml: 字符类缺少 `]`".into()),
                Some(']') => {
                    self.i += 1;
                    break;
                }
                _ => items.push(self.parse_class_item()?),
            }
        }

        let cls = if items.len() == 1 {
            items.pop().unwrap()
        } else {
            Class::Set(items)
        };
        Ok(if negated {
            Class::Not(Box::new(cls))
        } else {
            cls
        })
    }

    fn parse_class_item(&mut self) -> Result<Class, String> {
        let c = self.peek().ok_or_else(|| "sml: 字符类意外结束".to_string())?;
        if c == '\\' {
            self.i += 1;
            let e = self.peek().ok_or_else(|| "sml: `\\` 后缺少字符".to_string())?;
            self.i += 1;
            return Ok(match e {
                'd' => Class::Digit,
                'w' => Class::Word,
                's' => Class::Space,
                other => Class::One(other),
            });
        }
        self.i += 1;
        // 范围 a-z
        if self.peek() == Some('-') && self.chars.get(self.i + 1) != Some(&']') {
            self.i += 1;
            let hi = self.peek().ok_or_else(|| "sml: 字符范围缺少上界".to_string())?;
            self.i += 1;
            return Ok(Class::Range(c, hi));
        }
        Ok(Class::One(c))
    }
}
