//! `.smlex` profile：读取 + `checks` 执行（格式见 `docs/swsml/spec/smlex.md` v1.0）。
//!
//! ⚠️ **全在工具侧**：库不认识 profile（`@weakdoc` 只记名字），所以
//! 「有 / 无 `.smlex`，解析结果完全一致」自动成立。
//!
//! 本模块只消费 `checks` 三项；`applies_to` / `hints` / `emit` 分别属匹配、编辑器与生成器，
//! 尚未接（见格式文档 §5 的实现进度）。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use sml::Value;

/// `checks` 块：v1.0 裁决「不带参数」，三项都是纯清单。
#[derive(Debug, Default)]
pub struct Checks {
    /// 顶层必须出现的键。
    pub require_top_level: Vec<String>,
    /// 文档里不许出现的契约名（查定义侧）。
    pub forbid_contracts: Vec<String>,
    /// 任意层级都不许出现的键名。
    pub forbid_keys: Vec<String>,
}

/// `applies_to` 块：工具据此**建议**启用（绝不自动挂）。
#[derive(Debug, Default)]
pub struct AppliesTo {
    /// 路径模式（`*` 不跨 `/`，`**` 跨）。
    pub glob: Vec<String>,
    /// 文档里必须**已定义**这些契约，才认为适用。
    pub requires: Vec<String>,
}

/// 一份已加载的 profile。
#[derive(Debug)]
pub struct Profile {
    pub name: String,
    pub checks: Checks,
    pub applies_to: AppliesTo,
    /// 命中的文件路径（诊断里带上，方便定位是哪一层命名空间生效）。
    pub path: PathBuf,
}

/// 一条检查结果：`(码, 文案)`。
pub type Finding = (&'static str, String);

impl Profile {
    /// 读并校验一份 `.smlex`：`name` 必须存在且等于文件名 stem（格式不变量 3）。
    pub fn load(path: &Path) -> Result<Profile, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("读取失败：{}: {e}", path.display()))?;
        let v = sml::parse(&text).map_err(|e| format!("不是合法 SML：{}: {e}", path.display()))?;

        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        let name = match v.get("name") {
            Some(Value::Str(s)) => s.clone(),
            _ => {
                return Err(format!(
                    "{}: 缺必填字段 `name`（应为 `{stem}`）",
                    path.display()
                ))
            }
        };
        if name != stem {
            return Err(format!(
                "{}: `name` 与文件名不一致（写的是 `{name}`，文件是 `{stem}.smlex`）",
                path.display()
            ));
        }

        let mut checks = Checks::default();
        if let Some(block) = v.get("checks") {
            checks.require_top_level = strs(block.get("require_top_level"));
            checks.forbid_contracts = strs(block.get("forbid_contracts"));
            checks.forbid_keys = strs(block.get("forbid_keys"));
        }

        let mut applies_to = AppliesTo::default();
        if let Some(block) = v.get("applies_to") {
            applies_to.glob = strs(block.get("glob"));
            applies_to.requires = strs(block.get("requires"));
        }

        Ok(Profile {
            name,
            checks,
            applies_to,
            path: path.to_path_buf(),
        })
    }

    /// 执行三项检查。`contracts` 是文档里 `@contract` 定义出来的名字集合。
    ///
    /// **只读**：只看 `value` 与契约名，绝不改任何数据。
    pub fn run(&self, value: &Value, contracts: &BTreeSet<String>) -> Vec<Finding> {
        let mut out: Vec<Finding> = Vec::new();

        for k in &self.checks.require_top_level {
            if value.get(k).is_none() {
                out.push((
                    sml_codes::E_LINT_002,
                    format!("profile `{}` 要求顶层字段 `{k}`，文档里没有", self.name),
                ));
            }
        }

        for c in &self.checks.forbid_contracts {
            if contracts.contains(c) {
                out.push((
                    sml_codes::E_LINT_003,
                    format!("profile `{}` 禁止契约 `{c}`，文档里定义了它", self.name),
                ));
            }
        }

        for k in &self.checks.forbid_keys {
            if let Some(path) = find_key(value, k, "") {
                out.push((
                    sml_codes::E_LINT_004,
                    format!(
                        "profile `{}` 禁止键 `{k}`，文档里出现在 `{path}`",
                        self.name
                    ),
                ));
            }
        }

        out
    }
}

impl Profile {
    /// 这个 profile **看起来**适用于这份文档吗？（只用于提示，绝不自动挂）
    ///
    /// 判据：`requires` 里的契约都已定义，**且** `glob` 里至少一条命中文档路径。
    /// 没写 `applies_to` 的 profile 一律不猜（`false`）—— 宁缺勿滥。
    pub fn suggests_for(&self, file: Option<&Path>, contracts: &BTreeSet<String>) -> bool {
        if self.applies_to.glob.is_empty() && self.applies_to.requires.is_empty() {
            return false;
        }
        if !self
            .applies_to
            .requires
            .iter()
            .all(|c| contracts.contains(c))
        {
            return false;
        }
        if self.applies_to.glob.is_empty() {
            return true; // 只写了 requires
        }
        match file {
            Some(f) => self.applies_to.glob.iter().any(|g| glob_matches_path(g, f)),
            None => false, // 从 stdin 读：没有路径可比
        }
    }
}

/// 取一个「字符串清单」：数组逐个取串，单值也认（宽容一点，别为格式细节报错）。
fn strs(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|x| match x {
                Value::Str(s) => Some(s.clone()),
                _ => None,
            })
            .collect(),
        Some(Value::Str(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

/// 路径匹配：**相对模式**允许从任一目录边界起匹配（`docs/api/**/*.sml` 能命中绝对路径的尾部）。
fn glob_matches_path(pattern: &str, file: &Path) -> bool {
    let pat = pattern.replace('\\', "/");
    let text = file.to_string_lossy().replace('\\', "/");
    let pb = pat.as_bytes();
    let tb = text.as_bytes();
    // 绝对模式（`/...` 或 `C:/...`）：整体匹配
    let is_abs = pat.starts_with('/') || (pb.len() > 1 && pb[1] == b':');
    if is_abs {
        return glob_match(pb, tb);
    }
    let mut starts = vec![0usize];
    starts.extend((0..tb.len()).filter(|&i| tb[i] == b'/').map(|i| i + 1));
    starts.into_iter().any(|s| glob_match(pb, &tb[s..]))
}

/// 最小 glob：`?` 单字符、`*` 不跨 `/`、`**` 跨 `/`（`**/` 也匹配零层目录）。
///
/// 不为一个 profile 路径匹配引一整个 glob crate —— 这里只需要这两种通配符。
fn glob_match(p: &[u8], t: &[u8]) -> bool {
    if p.is_empty() {
        return t.is_empty();
    }
    if p.len() >= 2 && p[0] == b'*' && p[1] == b'*' {
        let rest = &p[2..];
        // `**/` 允许零层：`docs/**/*.sml` 应命中 `docs/a.sml`
        if rest.first() == Some(&b'/') && glob_match(&rest[1..], t) {
            return true;
        }
        for i in 0..=t.len() {
            if glob_match(rest, &t[i..]) {
                return true;
            }
        }
        return false;
    }
    match p[0] {
        b'*' => {
            let rest = &p[1..];
            for i in 0..=t.len() {
                if glob_match(rest, &t[i..]) {
                    return true;
                }
                if i == t.len() || t[i] == b'/' {
                    break;
                }
            }
            false
        }
        b'?' => !t.is_empty() && t[0] != b'/' && glob_match(&p[1..], &t[1..]),
        c => !t.is_empty() && t[0] == c && glob_match(&p[1..], &t[1..]),
    }
}

/// 递归找第一个同名键，返回它的点分路径（只报第一处，避免大树刷屏）。
fn find_key(v: &Value, key: &str, prefix: &str) -> Option<String> {
    match v {
        Value::Object(m) => {
            for (k, sub) in m {
                // `k` 的类型随 `Map` 的底层容器而变（BTreeMap / IndexMap），
                // 显式 `to_string()` 让两种档编译同一份代码。
                let p: String = if prefix.is_empty() {
                    k.to_string()
                } else {
                    format!("{prefix}.{k}")
                };
                if k == key {
                    return Some(p);
                }
                if let Some(hit) = find_key(sub, key, &p) {
                    return Some(hit);
                }
            }
            None
        }
        Value::Array(a) => a.iter().enumerate().find_map(|(i, sub)| {
            find_key(sub, key, &format!("{prefix}[{i}]"))
        }),
        _ => None,
    }
}
