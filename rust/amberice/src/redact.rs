// SPDX-License-Identifier: MulanPSL-2.0
//! Write-side redaction (P11 / D53): a **pure value transform**, no format bytes.
//! 写前脱敏（P11 / D53）：**纯值变换**，不碰格式字节。
//!
//! # 边界（必须记住）
//!
//! - 只在**写入之前**生效：先 `apply`，再 `dump` / `append`。格式不提供事后局部抹除。
//! - 规则**永不写进文件**：文件里没有「哪些键敏感」的清单（manifest 是明文，清单本身就是泄露面）。
//! - `history: keep` 下旧值仍留在旧层里，`history --at N` 读得到；要真正消失只能整份重写。
//! - 规则按加入顺序应用；`Hash` 哈希的是**应用到此规则时**的子树，所以前序规则会影响后续哈希。
//!
//! # 三种动作
//!
//! | 动作 | 结果 |
//! |---|---|
//! | [`RedactAction::Drop`] | 删键 / 删数组元素（剩余顺序不变） |
//! | [`RedactAction::Mask`] | 替换为固定 `"***"`，**不保留长度** |
//! | [`RedactAction::Hash`] | 替换为 `sha256(canonical bytes)` 的小写十六进制 |
//!
//! `hash` 对低熵值（手机号、身份证号）可被字典反推 —— 它只适合高熵值（token / 密钥）。
//! 路径不存在时是 no-op（可选字段很常见）；路径形状不匹配则明确报错。

use sml_value::{Path, PathError, Seg, Value};

/// Fixed mask value. Length is deliberately **not** preserved — length itself leaks.
/// 固定遮盖值。**不保留长度** —— 长度本身就是泄露。
pub const MASK: &str = "***";

/// What to do with the matched value. ｜ 命中值怎么处理。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedactAction {
    /// Delete the key / array element. ｜ 删键或数组元素。
    Drop,
    /// Replace with [`MASK`]. ｜ 替换为 [`MASK`]。
    Mask,
    /// Replace with `sha256(canonical bytes)` hex. ｜ 替换为摘要十六进制。
    Hash,
}

/// One rule: a path plus an action. ｜ 一条规则：路径 + 动作。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactRule {
    /// Where to apply. ｜ 作用路径。
    pub path: Path,
    /// What to do. ｜ 做什么。
    pub action: RedactAction,
}

/// Ordered rule set. Rules are applied in insertion order. ｜ 有序规则集，按加入顺序应用。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RedactRules {
    rules: Vec<RedactRule>,
}

/// Why redaction failed. ｜ 脱敏失败的原因。
#[derive(Debug)]
#[non_exhaustive]
pub enum RedactError {
    /// The path string is not valid. ｜ 路径字符串不合法。
    BadPath(PathError),
    /// The same path was added twice. ｜ 同一路径被加了两次。
    DuplicatePath(String),
    /// Rules are malformed, or the path does not fit the value's shape.
    /// 规则不合法，或路径与值的形状不匹配。
    BadRules(String),
    /// The subtree could not be canonicalised for `hash`. ｜ `hash` 时子树无法 canonical 化。
    Canonical(String),
}

impl std::fmt::Display for RedactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RedactError::BadPath(e) => write!(f, "脱敏路径不合法：{e}"),
            RedactError::DuplicatePath(p) => write!(f, "脱敏路径 `{p}` 重复"),
            RedactError::BadRules(why) => write!(f, "脱敏规则不合法：{why}"),
            RedactError::Canonical(why) => write!(f, "脱敏哈希失败：{why}"),
        }
    }
}

impl std::error::Error for RedactError {}

impl From<PathError> for RedactError {
    fn from(e: PathError) -> Self {
        RedactError::BadPath(e)
    }
}

impl RedactRules {
    /// Empty rule set. ｜ 空规则集。
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one rule. Duplicate paths are rejected (last-wins would hide a typo).
    /// 加一条规则；重复路径直接拒绝（last-wins 会把笔误藏起来）。
    pub fn add(&mut self, path: &str, action: RedactAction) -> Result<(), RedactError> {
        self.add_rule(RedactRule {
            path: Path::parse(path)?,
            action,
        })
    }

    /// Add an already-built rule; the only way to express the root path (`Path::new(vec![])`).
    /// 加一条已构造好的规则；根路径（`Path::new(vec![])`）只能走这里。
    pub fn add_rule(&mut self, rule: RedactRule) -> Result<(), RedactError> {
        if self.rules.iter().any(|r| r.path == rule.path) {
            return Err(RedactError::DuplicatePath(rule.path.to_string()));
        }
        self.rules.push(rule);
        Ok(())
    }

    /// Shorthand for [`RedactAction::Drop`]. ｜ [`RedactAction::Drop`] 的简写。
    pub fn drop_path(&mut self, path: &str) -> Result<(), RedactError> {
        self.add(path, RedactAction::Drop)
    }

    /// Shorthand for [`RedactAction::Mask`]. ｜ [`RedactAction::Mask`] 的简写。
    pub fn mask_path(&mut self, path: &str) -> Result<(), RedactError> {
        self.add(path, RedactAction::Mask)
    }

    /// Shorthand for [`RedactAction::Hash`]. ｜ [`RedactAction::Hash`] 的简写。
    pub fn hash_path(&mut self, path: &str) -> Result<(), RedactError> {
        self.add(path, RedactAction::Hash)
    }

    /// Parse rules from a `Value` (the caller parses SML / JSON elsewhere — `amberice` stays
    /// parser-free, D3). Shape: `[ { path: "a.b", action: "drop" | "mask" | "hash" } ]`.
    /// 从 `Value` 解析规则（文本解析归调用方，`amberice` 不拖解析器，D3）。
    pub fn from_value(v: &Value) -> Result<Self, RedactError> {
        let Value::Array(items) = v else {
            return Err(RedactError::BadRules("规则必须是数组".to_string()));
        };
        let mut out = Self::new();
        for item in items {
            let Value::Object(m) = item else {
                return Err(RedactError::BadRules("每条规则必须是对象".to_string()));
            };
            let path = match m.get("path") {
                Some(Value::Str(s)) => s.clone(),
                _ => return Err(RedactError::BadRules("规则缺 `path`".to_string())),
            };
            let action = match m.get("action") {
                None => RedactAction::Drop,
                Some(Value::Str(s)) if s == "drop" => RedactAction::Drop,
                Some(Value::Str(s)) if s == "mask" => RedactAction::Mask,
                Some(Value::Str(s)) if s == "hash" => RedactAction::Hash,
                Some(other) => {
                    return Err(RedactError::BadRules(format!(
                        "未知 action `{other:?}`（只认 drop / mask / hash）"
                    )))
                }
            };
            out.add(&path, action)?;
        }
        Ok(out)
    }

    /// Number of rules. ｜ 规则条数。
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// No rules? ｜ 是否为空。
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Apply every rule in order. Missing paths are a no-op; shape mismatches are errors.
    /// 按顺序应用；路径缺失是 no-op，形状不匹配则报错。
    pub fn apply(&self, v: &mut Value) -> Result<(), RedactError> {
        for rule in &self.rules {
            apply_rule(v, rule)?;
        }
        Ok(())
    }
}

fn apply_rule(root: &mut Value, rule: &RedactRule) -> Result<(), RedactError> {
    let segs = rule.path.segments();
    if segs.is_empty() {
        return match rule.action {
            RedactAction::Drop => Err(RedactError::BadRules("不能删除根值".to_string())),
            RedactAction::Mask => {
                *root = Value::Str(MASK.to_string());
                Ok(())
            }
            RedactAction::Hash => {
                let h = hash_value(root)?;
                *root = Value::Str(h);
                Ok(())
            }
        };
    }

    let (last, parents) = segs.split_last().expect("segs is non-empty");
    let mut cur = root;
    for (at, seg) in parents.iter().enumerate() {
        match (seg, cur) {
            (Seg::Key(k), Value::Object(m)) => {
                let Some(next) = m.get_mut(k) else {
                    return Ok(()); // 缺失：no-op（可选字段很常见）
                };
                cur = next;
            }
            (Seg::Index(i), Value::Array(a)) => {
                let Some(next) = a.get_mut(*i) else {
                    return Ok(());
                };
                cur = next;
            }
            _ => {
                return Err(RedactError::BadRules(format!(
                    "路径 `{}` 第 {} 段与值形状不匹配",
                    rule.path,
                    at + 1
                )))
            }
        }
    }

    match (last, cur) {
        (Seg::Key(k), Value::Object(m)) => {
            if !m.contains_key(k) {
                return Ok(());
            }
            match rule.action {
                RedactAction::Drop => {
                    m.remove(k);
                }
                RedactAction::Mask => {
                    m.insert(k.clone(), Value::Str(MASK.to_string()));
                }
                RedactAction::Hash => {
                    let h = {
                        let Some(v) = m.get(k) else { return Ok(()) };
                        hash_value(v)?
                    };
                    m.insert(k.clone(), Value::Str(h));
                }
            }
            Ok(())
        }
        (Seg::Index(i), Value::Array(a)) => {
            if *i >= a.len() {
                return Ok(());
            }
            match rule.action {
                RedactAction::Drop => {
                    a.remove(*i);
                }
                RedactAction::Mask => {
                    a[*i] = Value::Str(MASK.to_string());
                }
                RedactAction::Hash => {
                    let h = hash_value(&a[*i])?;
                    a[*i] = Value::Str(h);
                }
            }
            Ok(())
        }
        _ => Err(RedactError::BadRules(format!(
            "路径 `{}` 的末段与值形状不匹配",
            rule.path
        ))),
    }
}

/// `sha256(canonical bytes)` as lowercase hex. ｜ 摘要的小写十六进制。
fn hash_value(v: &Value) -> Result<String, RedactError> {
    let bytes = v
        .canonical_bytes()
        .map_err(|e| RedactError::Canonical(e.to_string()))?;
    let digest = crate::hash::digest(crate::hash::HashAlgo::Sha256, &bytes)
        .map_err(|e| RedactError::Canonical(e.to_string()))?;
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    Ok(s)
}
