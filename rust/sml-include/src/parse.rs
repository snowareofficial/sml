// SPDX-License-Identifier: MulanPSL-2.0
//! SML 模块化（`include` / `import`）的**指令解析**部分。
//!
//! 负责：识别 include 行、解析 `as ns` 命名空间、挑键 `{a, b}`、
//! glob 通配与正则匹配、路径解析。
//! 实际的**跨文件展开**在 [`crate::expand`]。

use std::path::{Path, PathBuf};

use sml_codes::{
    E_FEATURE_001, E_INCLUDE_001, E_INCLUDE_005, E_INCLUDE_008, E_INCLUDE_009, E_LIMIT_002,
    E_LIMIT_007, E_PARSE_025, SmlError,
};
use sml_feature::{Feature, FeatureSet};
use sml_regex::{compile_regex_checked, regex_matches_checked, RegexError};

/// 值嵌套深度上限：防止 `a{a{a{ ... }}}` 这类深度嵌套触发递归下降的栈溢出。
///
/// 与 [`MAX_INCLUDE_DEPTH`] 互补 —— 后者只保护 include 的文件嵌套，不保护
/// 单个文档内部块/数组的嵌套。栈溢出在 Rust 中是 abort，
/// **无法被 catch_unwind 捕获**，因此必须在递归入口主动限深，
/// 而不是依赖上层错误处理。
///
/// 128 层与 serde_json 的 RECURSION_LIMIT 对齐，远超任何真实配置所需。


/// 嵌套深度上限：既防栈溢出，也让异常深层的引用尽早失败
pub const MAX_INCLUDE_DEPTH: usize = 32;

/// 单次 include 展开的总文件读取次数上限（防指数膨胀 DoS）。
/// 仅按「被实际展开的文件个数」计数（非深度），覆盖菱形重复包含导致的 2^N 爆炸。
pub const MAX_INCLUDE_EXPANSIONS: u64 = 10_000;

/// 剥离行尾注释，正确跳过引号内的 `#`（如 `key: "a#b"` 中的 # 不是注释起点）
pub fn strip_line_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut in_quote = false;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => in_quote = !in_quote,
            // 引号内的反斜杠会转义下一个字符，需整体跳过
            b'\\' if in_quote => i += 1,
            b'#' if !in_quote => return &line[..i],
            _ => {}
        }
        i += 1;
    }
    line
}

/// 单个 include 目标的解析结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeTarget {
    /// 相对路径或裸名（无扩展名时按 `implicit-ns` 推导 `as`）。
    pub raw: String,
    /// 命名空间（点分路径 `a.b.c`）。`None` 表示普通内联。
    /// 若 `raw` 无扩展名且开启 `implicit-ns`，则自动填充为文件名。
    pub namespace: Option<String>,
    /// 是否经 `import` 关键字（语义等同 `include`）。
    pub via_import: bool,
    /// 部分引用：仅从目标文件挑出这些顶层键并入（命名空间包裹时同样只挑这些）。
    /// `None` 表示整文件（不挑键）。
    pub keys: Option<Vec<String>>,
}

/// 解析一行 include / import 指令，返回 0..N 个目标。
///
/// 支持形态（逗号分隔多目标，`import` 为 `include` 别名）：
/// - `include "x.sml"`                普通内联（带扩展名、无 as）
/// - `include "foo"`                  无扩展名 ⇒ 默认 `as foo`（implicit-ns）
/// - `include "x.sml" as ui.form`     命名空间内联（点分路径）
/// - `include "a", "b" as y, "c"`     多目标（multi-include）
/// - `import ui.buttons, admin.panel`  import 别名
/// - `include "*.sml"`                glob 通配（需 `glob-include`）
/// - `include re:"widget_.*\.sml"`    正则匹配（需 `regex-include`）
///
/// 部分引用（挑键）—— 两种等价写法：
/// - `import "x.sml" as w { a, b }`          挑键 a,b，挂到命名空间 w
/// - `import { a, b } as w in "x.sml"`        等价写法（键列表在前）
/// 省略 `as w` 则挑出的键直接平铺到当前作用域：
/// - `import "x.sml" { a, b }`
/// - `import { a, b } in "x.sml"`
/// 注：部分引用只作用于单文件目标，不与 glob/regex 通配组合。
///
/// 返回 `Ok(None)` 表示该行不是 include 指令；`Err` 表示特性未开启等语义错误。
pub fn parse_include_line(line: &str, features: FeatureSet) -> Result<Option<Vec<IncludeTarget>>, SmlError> {
    let content = strip_line_comment(line).trim();
    let content = content.strip_prefix('@').unwrap_or(content).trim_start();
    // 轻量手写解析，不依赖 tokenize（避免 `*` 等字符在 tokenize 阶段被误判）。
    // 形式：`include "x" [as ns], "y" as ns2, ...`（import 等价）
    let (via_import, rest) = if let Some(r) = content.strip_prefix("include ") {
        (false, r.trim_start())
    } else if let Some(r) = content.strip_prefix("import ") {
        (true, r.trim_start())
    } else {
        return Ok(None);
    };
    if !features.has(Feature::Include) {
        return Err(SmlError::new(
            E_FEATURE_001,
            "sml: 当前特性集禁用了 include/import（include 特性未启用）",
        ));
    }
    let mut targets: Vec<IncludeTarget> = Vec::new();
    let mut rest = rest;
    loop {
        // 两种部分引用语法：
        //   ①  import "x.sml" [as w] { a, b }
        //   ②  import { a, b } [as w] in "x.sml"
        // 先探测是否以 `{` 开头（语法②）
        let (raw, ns, keys, tail) = if rest.trim_start().starts_with('{') {
            // 语法②：键列表在前
            let (keys, after) = parse_key_list(rest.trim_start())?;
            let after = after.trim_start();
            // 可选 `as ns`
            let (ns, after) = if let Some(stripped) = after.strip_prefix("as ") {
                let (n, t) = match next_token(stripped.trim_start()) {
                    Some((n, t)) => (Some(n), t.trim_start()),
                    None => return Ok(None),
                };
                (n, t)
            } else {
                (None, after)
            };
            // 必须跟 `in "path"` 取目标文件
            let after = after.trim_start();
            let after = match after.strip_prefix("in ") {
                Some(a) => a.trim_start(),
                None => {
                    return Err(SmlError::new(
                        E_INCLUDE_008,
                        "sml: `import { keys } ...` 必须接 `in \"file\"` 指定目标文件",
                    ))
                }
            };
            let (path, t) = match next_token(after) {
                Some((p, t)) => (p, t),
                None => return Ok(None),
            };
            (path, ns, Some(keys), t)
        } else {
            // 语法①：路径在前
            let (path, tail0) = match next_token(rest) {
                Some((p, t)) => (p, t),
                None => {
                    if targets.is_empty() && rest.trim().is_empty() {
                        return Ok(None);
                    } else {
                        break;
                    }
                }
            };
            let mut r = tail0.trim_start();
            // 可选 `as ns`
            let mut ns: Option<String> = None;
            if let Some(stripped) = r.strip_prefix("as ") {
                let (n, t) = match next_token(stripped.trim_start()) {
                    Some((n, t)) => (n, t),
                    None => return Ok(None),
                };
                ns = Some(n);
                r = t.trim_start();
            }
            // 可选 `{ keys }`
            let keys = if r.starts_with('{') {
                let (k, after) = parse_key_list(r)?;
                r = after.trim_start();
                Some(k)
            } else {
                None
            };
            (path, ns, keys, r)
        };
        targets.push(finalize_target(
            raw,
            ns,
            via_import,
            features,
            keys,
        ));
        // 逗号分隔多目标（用已修剪的 tail 判断是否还有下一个目标）
        if let Some(stripped) = tail.strip_prefix(',') {
            if !features.has(Feature::MultiInclude) {
                // 与其余三类（include/glob/regex）保持一致：特性未启用时返回 Err，
                // 而非静默 `Ok(None)` 把整行当普通内容解析导致数据污染
                // （此前会注入垃圾键且零报错）。
                return Err(SmlError::new(
                    E_FEATURE_001,
                    "sml: 多目标 include 需要特性 `multi-include`（请 @feature enable multi-include）",
                ));
            }
            rest = stripped.trim_start();
            continue;
        } else {
            // `rest = tail` 在此处是死赋值：紧接着 break，循环外也没有再读 rest
            // （unused_assignments 指出的就是这一处）。
            break;
        }
    }
    if targets.is_empty() {
        return Ok(None);
    }
    // 特性预检查：glob / regex 模式在解析阶段就拦截（避免走到普通路径解析引发诡异错误）
    for t in &targets {
        // 部分引用只作用于单文件目标，不能与 glob/regex 通配组合
        if t.keys.is_some() && (t.raw.contains('*') || t.raw.starts_with("re:")) {
            return Err(SmlError::new(
                E_INCLUDE_009,
                "sml: 部分引用 `{ keys }` 不能配合 glob/regex 通配（请指定单个文件）",
            ));
        }
        // 先查 re: 前缀（正则模式里的 `*` 是元字符，不是 glob 通配）
        if t.raw.starts_with("re:") {
            if !features.has(Feature::RegexInclude) {
                return Err(SmlError::new(
                    E_FEATURE_001,
                    "sml: 正则 include 需要特性 `regex-include`（请 @feature enable regex-include）",
                ));
            }
            continue;
        }
        if t.raw.contains('*') && !features.has(Feature::GlobInclude) {
            return Err(SmlError::new(
                E_FEATURE_001,
                "sml: 通配 include 需要特性 `glob-include`（请 @feature enable glob-include）",
            ));
        }
    }
    Ok(Some(targets))
}

/// 从字符串开头提取下一个 token：引号串（支持 `\"` 与 `\\`）或直到空白/逗号/`as` 的裸词。
/// 返回 (token 文本, 剩余字符串)。
pub fn next_token(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start();
    if s.is_empty() {
        return None;
    }
    if s.starts_with('"') {
        // 引号串（按字节处理，路径通常为 ASCII）
        let bytes = s.as_bytes();
        let mut i = 1;
        let mut out = String::new();
        while i < bytes.len() {
            if bytes[i] == b'"' {
                i += 1;
                break;
            }
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                // 转义：保留转义后的字符（\. -> .，\" -> " 等）
                i += 1;
                out.push(bytes[i] as char);
                i += 1;
            } else {
                out.push(bytes[i] as char);
                i += 1;
            }
        }
        Some((out, &s[i..]))
    } else {
        // 裸词：取到空白或逗号
        let end = s
            .find(|c: char| c.is_whitespace() || c == ',')
            .unwrap_or(s.len());
        let (tok, tail) = s.split_at(end);
        Some((tok.trim().to_string(), tail))
    }
}

/// 解析 `{ a, b, c }` 形式的键列表，返回 (键名集合, 剩余字符串)。
/// 键名可为裸词或引号串。遇到非 `{` 开头时返回错误。
pub fn parse_key_list(s: &str) -> Result<(Vec<String>, &str), SmlError> {
    let s = s.trim_start();
    let Some(body) = s.strip_prefix('{') else {
        return Err(SmlError::new(
            E_INCLUDE_005,
            "sml: 期望 `{ key1, key2, ... }` 键列表",
        ));
    };
    let close = body.find('}').ok_or_else(|| {
        SmlError::new(E_INCLUDE_005, "sml: 键列表缺少闭合 `}`")
    })?;
    let inner = &body[..close];
    let mut keys: Vec<String> = Vec::new();
    for part in inner.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        // 支持引号串键，其余按裸词（去引号）
        if let Some(q) = part.strip_prefix('"') {
            let q = q.strip_suffix('"').unwrap_or(q);
            keys.push(q.to_string());
        } else {
            keys.push(part.to_string());
        }
    }
    if keys.is_empty() {
        return Err(SmlError::new(
            E_INCLUDE_005,
            "sml: 键列表不能为空（至少指定一个键）",
        ));
    }
    Ok((keys, &body[close + 1..]))
}

/// 根据原始路径与可选命名空间，套用 implicit-ns 规则，产出最终目标。
pub fn finalize_target(
    raw: String,
    ns: Option<String>,
    via_import: bool,
    features: FeatureSet,
    keys: Option<Vec<String>>,
) -> IncludeTarget {
    let namespace = match ns {
        Some(n) => Some(n),
        None => {
            // 部分引用（指定了 keys）且无显式 `as`：强制平铺到当前作用域，
            // 不触发 implicit-ns 自动命名空间（否则挑出的键会被塞进文件名命名空间）。
            if keys.is_some() {
                None
            } else if via_import || (features.has(Feature::ImplicitNs) && !raw.contains('.')) {
                // `import a.b.c`：点分一律视为命名空间路径，自动 `as a.b.c`
                // `include "foo"`（无点）：implicit-ns 默认以文件名为命名空间
                Some(raw.clone())
            } else {
                None
            }
        }
    };
    IncludeTarget {
        raw,
        namespace,
        via_import,
        keys,
    }
}

/// 把一个 include 目标解析为 0..N 个实际文件路径（已相对 `base` 解析、未 canonicalize）。
///
/// 支持：
/// - glob：`raw` 含 `*` 且开启 `glob-include` → 遍历 `base` 下直接条目做 `*` 通配匹配
/// - 正则：`raw` 以 `re:"..."` 形式且开启 `regex-include` → 遍历 `base` 下条目做最小正则匹配
/// - ext-rewrite：开启 `ext-rewrite` 时允许 `raw` 带非 `.sml` 扩展名（否则按原补 `.sml` 逻辑）
/// - 普通：`import` 点分转目录层级、裸名补 `.sml`
pub fn resolve_target_paths(
    t: &IncludeTarget,
    base: &Path,
    features: FeatureSet,
) -> Result<Vec<PathBuf>, SmlError> {
    // 正则模式：re:"<pattern>"
    if let Some(pat) = t.raw.strip_prefix("re:") {
        if !features.has(Feature::RegexInclude) {
            return Err(SmlError::new(
                E_FEATURE_001,
                "sml: 正则 include 需要特性 `regex-include`（请 @feature enable regex-include）",
            ));
        }
        let pat = pat.trim_matches('"');
        // 模式可含目录前缀（如 re:"lib/widget_.*"）：拆出目录并入 base（归一化分隔符）
        let pat = pat.replace('/', std::path::MAIN_SEPARATOR_STR);
        let (dir, pat) = split_dir(&pat);
        return glob_or_regex_dir(&base.join(dir), pat, Some(pat), features);
    }
    // glob 模式：含 `*`
    if t.raw.contains('*') {
        if !features.has(Feature::GlobInclude) {
            return Err(SmlError::new(
                E_FEATURE_001,
                "sml: 通配 include 需要特性 `glob-include`（请 @feature enable glob-include）",
            ));
        }
        let normalized = t.raw.replace('/', std::path::MAIN_SEPARATOR_STR);
        let (dir, pat) = split_dir(&normalized);
        return glob_or_regex_dir(&base.join(dir), pat, None, features);
    }
    // 普通路径
    let path = if t.via_import {
        // import 的「点分模块名」语义：仅当 raw 既无路径分隔、又不显式带 .sml 扩展名时，
        // 才把点当作目录层级分隔（a.b.c -> a/b/c.sml）。
        // 若显式写了路径或扩展名（如 "advanced_inc/widget_a.sml"），按字面路径处理。
        if t.raw.contains(std::path::MAIN_SEPARATOR) || t.raw.ends_with(".sml") {
            base.join(&t.raw)
        } else {
            let rel = t
                .raw
                .split('.')
                .collect::<Vec<_>>()
                .join(std::path::MAIN_SEPARATOR_STR);
            base.join(rel).with_extension("sml")
        }
    } else if t.raw.contains('.') {
        // 带扩展名：默认直接读该文件
        // 开启 ext-rewrite 时允许非 .sml 扩展名（当 sml 解析）；关闭时若非 .sml 也允许读，
        // 但语义上仍要求文件存在，由 canonicalize 报错兜底。
        let _ = features.has(Feature::ExtRewrite);
        base.join(&t.raw)
    } else {
        base.join(format!("{}.sml", t.raw))
    };
    Ok(vec![path])
}

/// 遍历 `base` 目录的直接条目，按 glob（`pattern` 含 `*`）或正则（`regex` 为 Some）匹配，
/// 把 `a/b/pattern` 拆成 (`a/b`, `pattern`)，便于把目录部分并入 base。
pub fn split_dir(pat: &str) -> (&str, &str) {
    match pat.rfind(std::path::MAIN_SEPARATOR) {
        Some(idx) => (&pat[..idx], &pat[idx + 1..]),
        None => ("", pat),
    }
}

/// 把受限正则引擎的失败原因映射成码（W16）。
///
/// 引擎层（零依赖的 `sml-regex`）只报「为什么失败」，码在本层映射 ——
/// 与各端「引擎给原因、上层给码」的分层一致（见 `errors/codes.sml`）：
/// 过长 ⇒ `E-LIMIT-007`、语法非法 ⇒ `E-PARSE-025`、步数超预算 ⇒ `E-LIMIT-002`。
fn map_regex_err(e: RegexError) -> SmlError {
    let code = match e {
        RegexError::TooLong { .. } => E_LIMIT_007,
        RegexError::Illegal { .. } => E_PARSE_025,
        RegexError::Budget { .. } => E_LIMIT_002,
    };
    SmlError::new(code, format!("include 的受限正则：{e}"))
}

/// 返回命中的完整路径。目录本身不作为命中（仅文件）。
pub fn glob_or_regex_dir(
    base: &Path,
    pattern: &str,
    regex: Option<&str>,
    _features: FeatureSet,
) -> Result<Vec<PathBuf>, SmlError> {
    let mut hits: Vec<PathBuf> = Vec::new();
    let entries = std::fs::read_dir(base).map_err(|e| {
        SmlError::new(
            E_INCLUDE_001,
            format!("include 目录读取失败 {}: {e}", base.display()),
        )
    })?;
    // 用于正则匹配的模式字符串（不含 re: 前缀与引号）。
    // W16：走**显式失败**的入口 —— 模式过长 / 非法都在这里就报码，
    // 不再变成「永不匹配」（那会让用户以为「目录里没有匹配的文件」）。
    let re = match regex {
        Some(r) => Some(compile_regex_checked(r).map_err(map_regex_err)?),
        None => None,
    };
    for ent in entries {
        let ent = ent.map_err(|e| {
            SmlError::new(E_INCLUDE_001, format!("include 目录遍历失败: {e}"))
        })?;
        let p = ent.path();
        if p.is_dir() {
            continue; // 只匹配文件
        }
        let name = match p.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        let matched = if let Some(re) = &re {
            // 步数预算耗尽同样**报码**（E-LIMIT-002），而不是静默「不匹配」
            regex_matches_checked(re, name).map_err(map_regex_err)?
        } else {
            // glob：`pattern` 形如 `*.sml` 或 `widgets/*.sml`；这里只处理文件名部分的通配
            let pat_file = pattern.rsplit(std::path::MAIN_SEPARATOR).next().unwrap_or(pattern);
            glob_matches(pat_file, name)
        };
        if matched {
            hits.push(p);
        }
    }
    // 结果按文件名排序，保证跨平台顺序稳定
    hits.sort();
    Ok(hits)
}

/// 手写最小 glob 匹配（仅支持 `*` 通配，匹配整个文件名）。
pub fn glob_matches(pattern: &str, text: &str) -> bool {
    // 将 `a*b*c` 拆分为字面段，段间用 `*` 连接
    let segs: Vec<&str> = pattern.split('*').collect();
    if segs.is_empty() {
        return text.is_empty();
    }
    let mut pos = 0usize;
    // 首段若非 `*` 开头，必须前缀匹配
    if !pattern.starts_with('*') {
        if !text[pos..].starts_with(segs[0]) {
            return false;
        }
        pos += segs[0].len();
    }
    for seg in &segs[if pattern.starts_with('*') { 0 } else { 1 }..] {
        if seg.is_empty() {
            continue;
        }
        match text[pos..].find(seg) {
            Some(idx) => pos += idx + seg.len(),
            None => return false,
        }
    }
    // 末段若非 `*` 结尾，必须后缀匹配
    if !pattern.ends_with('*') {
        if pos != text.len() {
            return false;
        }
    }
    true
}

