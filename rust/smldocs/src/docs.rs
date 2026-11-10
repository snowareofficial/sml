// Copyright (C) SNOWARE
// SPDX-License-Identifier: MulanPSL-2.0
//! `smldocs` —— 本仓库文档元数据驱动的索引 / 体检 / 门禁（内部工具，不发布）。
//!
//! 用 `swsml` 自己读 `docs/meta.sml`（dogfooding：元数据格式就是 SML，解析器就是本项目）。
//!
//! ```text
//! smldocs --index    # 生成 docs/index.md（按 owner 分组，显示 status 与替代者）
//! smldocs --report   # 生成 docs/report.md（缺元数据 / 超期 / 断链 / stale 无替代者 / 未登记）
//! smldocs --check    # CI 门禁：上述任一命中即非零退出
//! smldocs --fix      # 只补 updated / status，不猜内容
//! ```
//!
//! 仓库根由 `--docs-root` 指定；缺省时依次在当前目录与上一级目录找 `docs/meta.sml`
//! （这样在 `rust/` 里跑 `cargo run -p smldocs -- --check` 与在仓库根跑都行）。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sml::{Map, Value};

const META: &str = "docs/meta.sml";
const INDEX: &str = "docs/index.md";
const REPORT: &str = "docs/report.md";
/// 生成物：它们自己不进 `meta.sml`（否则索引要描述索引）。
const GENERATED: [&str; 2] = ["docs/index.md", "docs/report.md"];
/// `updated` 超过这么多天没核对就报警。`archive` / `keep` 不查（历史不需要保鲜）。
const STALE_DAYS: i64 = 90;
const OWNERS: [&str; 4] = ["swsml", "amberice", "swml", "repo"];
const STATUSES: [&str; 5] = ["current", "draft", "stale", "archive", "keep"];

/// 运行参数（由 `main.rs` 解析后传入）。
#[derive(Debug, Default, Clone)]
pub struct Opts {
    pub index: bool,
    pub report: bool,
    pub check: bool,
    pub fix: bool,
    /// 仓库根（`--docs-root`）；`None` = 自动找。
    pub root: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct Doc {
    title: String,
    date: String,
    updated: String,
    owner: String,
    status: String,
    scope: String,
    superseded_by: Option<String>,
}

#[derive(Debug, Default)]
struct Findings {
    unregistered: Vec<String>,
    missing_file: Vec<String>,
    missing_field: Vec<String>,
    bad_value: Vec<String>,
    stale_no_superseder: Vec<String>,
    superseder_missing: Vec<String>,
    expired: Vec<String>,
    broken_links: Vec<String>,
}

impl Findings {
    fn total(&self) -> usize {
        self.unregistered.len()
            + self.missing_file.len()
            + self.missing_field.len()
            + self.bad_value.len()
            + self.stale_no_superseder.len()
            + self.superseder_missing.len()
            + self.expired.len()
            + self.broken_links.len()
    }
}

fn as_obj(v: &Value) -> Option<&Map> {
    match v {
        Value::Object(m) => Some(m),
        _ => None,
    }
}

fn as_str(v: Option<&Value>) -> Option<String> {
    match v {
        Some(Value::Str(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
        _ => None,
    }
}

fn find_root(explicit: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(p) = explicit {
        return Ok(p);
    }
    let cwd = std::env::current_dir().map_err(|e| format!("取当前目录失败：{e}"))?;
    for cand in [cwd.clone(), cwd.join("..")] {
        if cand.join(META).is_file() {
            return Ok(cand.canonicalize().unwrap_or(cand));
        }
    }
    Err(format!(
        "找不到 {META}（在 {} 与上一级都没有）；用 --docs-root 指定仓库根",
        cwd.display()
    ))
}

fn rel_string(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}

fn load_meta(root: &Path) -> Result<Vec<Doc>, String> {
    let text = std::fs::read_to_string(root.join(META)).map_err(|e| format!("读 {META} 失败：{e}"))?;
    let value = sml::parse(&text).map_err(|e| format!("{META} 解析失败：{e}"))?;
    let arr = as_obj(&value)
        .and_then(|m| m.get("docs"))
        .and_then(as_obj)
        .and_then(|m| m.get("doc"))
        .ok_or_else(|| format!("{META} 缺少 `docs {{ doc: [ … ] }}` 结构"))?;
    let arr = match arr {
        Value::Array(a) => a,
        _ => return Err(format!("{META} 的 `docs.doc` 必须是数组")),
    };
    let mut out = Vec::new();
    for item in arr {
        let o = as_obj(item).ok_or_else(|| format!("{META} 的 `docs.doc` 元素必须是对象"))?;
        out.push(Doc {
            title: as_str(o.get("title")).unwrap_or_default(),
            date: as_str(o.get("date")).unwrap_or_default(),
            updated: as_str(o.get("updated")).unwrap_or_default(),
            owner: as_str(o.get("owner")).unwrap_or_default(),
            status: as_str(o.get("status")).unwrap_or_default(),
            scope: as_str(o.get("scope")).unwrap_or_default(),
            superseded_by: match o.get("superseded_by") {
                Some(Value::Str(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
                _ => None,
            },
        });
    }
    Ok(out)
}

fn walk_md(dir: &Path, root: &Path, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk_md(&p, root, out);
        } else if p.extension().and_then(|s| s.to_str()) == Some("md") {
            out.push(rel_string(root, &p));
        }
    }
}

// ── 日期（不引依赖：civil ↔ days 整数换算） ─────────────────────────────────

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn parse_date(s: &str) -> Option<i64> {
    let b: Vec<i64> = s.split('-').filter_map(|x| x.parse().ok()).collect();
    if b.len() != 3 {
        return None;
    }
    Some(days_from_civil(b[0], b[1], b[2]))
}

fn today() -> i64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    secs.div_euclid(86_400)
}

fn fmt_date(days: i64) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// `meta.sml` 内容摘要（FNV-1a 64）：生成物只跟数据源走。
/// 不记日期 / HEAD —— 那两者会在跨天或提交后制造假漂移。
fn meta_digest(root: &Path) -> String {
    let bytes = std::fs::read(root.join(META)).unwrap_or_default();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:08x}")
}

// ── Markdown 链接 ────────────────────────────────────────────────────────────

/// 抹掉行内代码（`` `…` ``）：代码里的 `byte[128](24)` 不是链接。
fn strip_inline_code(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    for c in line.chars() {
        if c == '`' {
            in_code = !in_code;
            out.push(' ');
        } else {
            out.push(if in_code { ' ' } else { c });
        }
    }
    out
}

fn extract_links(md: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut fenced = false;
    for (i, raw) in md.lines().enumerate() {
        let t = raw.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let chars: Vec<char> = strip_inline_code(raw).chars().collect();
        let mut k = 0usize;
        while k + 1 < chars.len() {
            if chars[k] == ']' && chars[k + 1] == '(' {
                let start = k + 2;
                let mut end = start;
                while end < chars.len() && chars[end] != ')' {
                    end += 1;
                }
                if end < chars.len() {
                    let target: String = chars[start..end].iter().collect();
                    out.push((i + 1, target.trim().to_string()));
                }
                k = end;
            } else {
                k += 1;
            }
        }
    }
    out
}

fn is_external(target: &str) -> bool {
    if target.is_empty() || target.starts_with('#') || target.starts_with('/') {
        return true;
    }
    if let Some(pos) = target.find(':') {
        let scheme = &target[..pos];
        if !scheme.is_empty()
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        {
            return true;
        }
    }
    false
}

fn check_links(root: &Path, docs: &[String]) -> Vec<String> {
    let mut bad = Vec::new();
    for rel in docs {
        let Ok(text) = std::fs::read_to_string(root.join(rel)) else { continue };
        let base = root
            .join(rel)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| root.to_path_buf());
        for (line, target) in extract_links(&text) {
            if is_external(&target) {
                continue;
            }
            let mut t = target.trim_start_matches('<').trim_end_matches('>').to_string();
            if let Some(p) = t.find('#') {
                t.truncate(p);
            }
            if let Some(p) = t.find('?') {
                t.truncate(p);
            }
            if t.is_empty() {
                continue;
            }
            let cand = if Path::new(&t).is_absolute() {
                PathBuf::from(&t)
            } else {
                base.join(t.replace("%20", " "))
            };
            if !cand.exists() {
                bad.push(format!("{rel}:{line} → {target}"));
            }
        }
    }
    bad
}

// ── 体检 ─────────────────────────────────────────────────────────────────────

fn inspect(root: &Path, docs: &[Doc]) -> Findings {
    let mut f = Findings::default();

    let mut on_disk = Vec::new();
    walk_md(&root.join("docs"), root, &mut on_disk);
    let registered: BTreeSet<&str> = docs.iter().map(|d| d.title.as_str()).collect();
    for rel in &on_disk {
        if !GENERATED.contains(&rel.as_str()) && !registered.contains(rel.as_str()) {
            f.unregistered.push(rel.clone());
        }
    }

    let t = today();
    for d in docs {
        if d.title.is_empty()
            || d.date.is_empty()
            || d.updated.is_empty()
            || d.owner.is_empty()
            || d.status.is_empty()
            || d.scope.is_empty()
        {
            f.missing_field
                .push(format!("{}（date/updated/owner/status/scope 缺一）", d.title));
        }
        if !d.owner.is_empty() && !OWNERS.contains(&d.owner.as_str()) {
            f.bad_value.push(format!("{}：owner = `{}`", d.title, d.owner));
        }
        if !d.status.is_empty() && !STATUSES.contains(&d.status.as_str()) {
            f.bad_value.push(format!("{}：status = `{}`", d.title, d.status));
        }
        if !d.title.is_empty() && !root.join(&d.title).is_file() {
            f.missing_file.push(d.title.clone());
        }
        if d.status == "stale" && d.superseded_by.is_none() {
            f.stale_no_superseder.push(d.title.clone());
        }
        if let Some(sb) = &d.superseded_by {
            if !root.join(sb).is_file() {
                f.superseder_missing.push(format!("{} → {sb}", d.title));
            }
        }
        if matches!(d.status.as_str(), "current" | "draft") {
            if let Some(u) = parse_date(&d.updated) {
                let age = t - u;
                if age > STALE_DAYS {
                    f.expired.push(format!("{}：updated {}（{} 天前）", d.title, d.updated, age));
                }
            }
        }
    }

    f.broken_links = check_links(root, &on_disk);
    f
}

fn write_sections(f: &Findings, out: &mut Vec<String>) {
    let mut section = |title: &str, items: &[String]| {
        out.push(format!("\n### {title}（{}）\n", items.len()));
        if items.is_empty() {
            out.push("- 无".to_string());
        } else {
            for it in items {
                out.push(format!("- {it}"));
            }
        }
    };
    section("未登记（在 docs/ 下但不是 meta.sml 的 doc）", &f.unregistered);
    section("登记的文档不存在", &f.missing_file);
    section("元数据缺字段", &f.missing_field);
    section("字段值非法（owner / status 不在枚举内）", &f.bad_value);
    section("status = stale 但没有 superseded_by", &f.stale_no_superseder);
    section("superseded_by 指向不存在的文件", &f.superseder_missing);
    section(&format!("updated 超过 {STALE_DAYS} 天未核对"), &f.expired);
    section("断链（Markdown 相对链接指向不存在的路径）", &f.broken_links);
}

// ── 生成 ─────────────────────────────────────────────────────────────────────

fn index_md(docs: &[Doc], f: &Findings, digest: &str) -> String {
    let mut s = String::new();
    s.push_str("# docs 索引\n\n");
    s.push_str(&format!(
        "> **本文件由 `cargo run -p smldocs -- --index` 生成，勿手改。**\n\
         > 数据源 `docs/meta.sml`（{} 份文档，摘要 `{digest}`）。摘要变了就必须重跑生成；\n\
         > 本文件不记生成日期 / HEAD，避免跨天或提交后假漂移。体检结论见 [`report.md`](report.md)。\n\n",
        docs.len()
    ));
    s.push_str(&format!(
        "## 一眼看结论\n\n\
         - 断链 **{}** 处；未登记 **{}** 份；`updated` 超期 **{}** 份；`stale` 无替代者 **{}** 份。\n\
         - 状态口径：`current` 可引用；`draft` 只能看不能用；`stale` 已作废；`archive` 只作历史。\n\n",
        f.broken_links.len(),
        f.unregistered.len(),
        f.expired.len(),
        f.stale_no_superseder.len()
    ));

    s.push_str("## 怎么读\n\n");
    s.push_str("1. 只想用 SML → 读站点教程（`site/content/*/book/`），不要进 `docs/`；\n");
    s.push_str("2. 想知道语法怎么定 → 本表 `swsml/spec/` 下 `status: current` 的那几份；\n");
    s.push_str("3. 想动手实现 → `*/plan/`；想查「还剩什么」→ `repo/ref/unfinished.md`；\n");
    s.push_str("4. 只想问「当年为什么这么定」→ `*/archive/`（只作历史，勿照做）。\n\n");
    s.push_str(
        "> 维护规则：新增文档必须先登记 `docs/meta.sml`；正文不写历史（历史进 `docs/changes.sml`）；\n\
         > 数字不手写，引用生成物或实测命令。`--check` 进 CI，任一命中即红。\n\n",
    );

    for owner in OWNERS {
        let mut rows: Vec<&Doc> = docs.iter().filter(|d| d.owner == owner).collect();
        rows.sort_by(|a, b| a.title.cmp(&b.title));
        if rows.is_empty() {
            continue;
        }
        s.push_str(&format!("## {owner}（{} 份）\n\n", rows.len()));
        s.push_str("| 文档 | 写于 | 最后核对 | 状态 | 替代者 | 职责 |\n|---|---|---|---|---|---|\n");
        for d in rows {
            let sup = d
                .superseded_by
                .as_ref()
                .map(|x| format!("`{x}`"))
                .unwrap_or_else(|| "—".to_string());
            s.push_str(&format!(
                "| [`{}`]({}) | {} | {} | **{}** | {} | {} |\n",
                short_name(&d.title),
                rel_link(&d.title),
                d.date,
                d.updated,
                d.status,
                sup,
                d.scope
            ));
        }
        s.push('\n');
    }
    s
}

/// 表格里只显示文件名（路径太长会把表撑爆），链接仍指向真实路径。
fn short_name(path: &str) -> String {
    match path.rsplit_once('/') {
        Some((dir, name)) => {
            let dir = dir.trim_start_matches("docs/");
            format!("{dir}/{name}")
        }
        None => path.to_string(),
    }
}

/// 从 `docs/index.md` 出发的相对链接。
fn rel_link(path: &str) -> String {
    path.trim_start_matches("docs/").to_string()
}

fn report_md(docs: &[Doc], f: &Findings, digest: &str) -> String {
    let mut s = String::new();
    s.push_str("# docs 体检报告\n\n");
    s.push_str(&format!(
        "> **本文件由 `cargo run -p smldocs -- --report` 生成，勿手改。**\n\
         > 数据源 `docs/meta.sml` 摘要 `{digest}`，检查 {} 份登记文档。任一项命中时 `docs --check` 非零退出。\n\
         > 本文件不记生成日期 / HEAD，避免跨天或提交后假漂移。\n\n",
        docs.len()
    ));
    s.push_str(&format!("## 合计\n\n**{} 处问题**。\n", f.total()));
    let mut lines = Vec::new();
    write_sections(f, &mut lines);
    s.push_str(&lines.join("\n"));
    s.push('\n');
    s.push_str("\n## 改哪里\n\n");
    s.push_str("- 未登记 → 在 `docs/meta.sml` 加一条 `doc { … }`；\n");
    s.push_str("- 超期 → 真核对一遍再改 `updated`（不是把日期往后挪）；\n");
    s.push_str("- 断链 → 改链接或改目标；两个都不对就删掉这条引用；\n");
    s.push_str("- `stale` 无替代者 → 要么写 `superseded_by`，要么把正文并入替代者后删掉本文。\n");
    s
}

// ── --fix ────────────────────────────────────────────────────────────────────

fn meta_sml(docs: &[Doc], header: &str) -> String {
    let mut s = String::from(header);
    s.push_str("docs {\n  doc: [\n");
    let mut rows: Vec<&Doc> = docs.iter().collect();
    rows.sort_by(|a, b| a.title.cmp(&b.title));
    for d in rows {
        let q = |x: &str| format!("\"{}\"", x.replace('"', "'"));
        let opt = |x: &Option<String>| match x {
            Some(v) => q(v),
            None => "null".to_string(),
        };
        s.push_str(&format!(
            "    {{ title: {} date: {} updated: {} owner: {} status: {} scope: {} supersedes: null superseded_by: {} }}\n",
            q(&d.title),
            q(&d.date),
            q(&d.updated),
            q(&d.owner),
            q(&d.status),
            q(&d.scope),
            opt(&d.superseded_by)
        ));
    }
    s.push_str("  ]\n}\n");
    s
}

fn fix(root: &Path, docs: &mut [Doc]) -> Result<Vec<String>, String> {
    let path = root.join(META);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("读 {META} 失败：{e}"))?;
    // 保留人在文件头写的注释块（第一条含 `docs {` 的行之前的一切）。
    let header: String = match text.find("docs {") {
        Some(i) => text[..i].to_string(),
        None => String::from("# docs/meta.sml\n\n"),
    };
    let t = fmt_date(today());
    let mut changed = Vec::new();
    for d in docs.iter_mut() {
        if d.updated.is_empty() {
            d.updated = t.clone();
            changed.push(format!("{}：补 updated = {t}", d.title));
        }
        if d.status.is_empty() {
            d.status = "draft".to_string();
            changed.push(format!("{}：补 status = draft（不猜成 current）", d.title));
        }
    }
    write_out(&path, &meta_sml(docs, &header), root)?;
    Ok(changed)
}

// ── 入口 ─────────────────────────────────────────────────────────────────────

pub fn run(opts: Opts) -> ExitCode {
    let root = match find_root(opts.root.clone()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("smldocs: {e}");
            return ExitCode::from(2);
        }
    };
    let mut docs = match load_meta(&root) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("smldocs: {e}");
            return ExitCode::from(2);
        }
    };

    if opts.fix {
        match fix(&root, &mut docs) {
            Ok(changed) => {
                if changed.is_empty() {
                    println!("smldocs: 无需修补");
                } else {
                    for c in &changed {
                        println!("smldocs: fix {c}");
                    }
                    println!("smldocs: 已重写 {META}（{} 条修补）", changed.len());
                }
            }
            Err(e) => {
                eprintln!("smldocs: {e}");
                return ExitCode::from(2);
            }
        }
    }

    let findings = inspect(&root, &docs);
    let digest = meta_digest(&root);

    if opts.index || (!opts.report && !opts.check) {
        let out = index_md(&docs, &findings, &digest);
        match write_out(&root.join(INDEX), &out, &root) {
            Ok(true) => println!("smldocs: 已生成 {INDEX}（{} 份文档） ｜ regenerated", docs.len()),
            Ok(false) => println!("smldocs: {INDEX} 无变更 ｜ up to date"),
            Err(e) => {
                eprintln!("smldocs: {e}");
                return ExitCode::from(2);
            }
        }
    }
    if opts.report {
        let out = report_md(&docs, &findings, &digest);
        match write_out(&root.join(REPORT), &out, &root) {
            Ok(true) => println!("smldocs: 已生成 {REPORT}（{} 处问题） ｜ regenerated", findings.total()),
            Ok(false) => println!("smldocs: {REPORT} 无变更 ｜ up to date"),
            Err(e) => {
                eprintln!("smldocs: {e}");
                return ExitCode::from(2);
            }
        }
    }
    if opts.check {
        if findings.total() == 0 {
            println!("smldocs: OK（{} 份文档，无断链 / 无未登记 / 无超期）", docs.len());
            return ExitCode::SUCCESS;
        }
        let mut lines = Vec::new();
        write_sections(&findings, &mut lines);
        for l in lines {
            if l.starts_with("- ") {
                eprintln!("smldocs: {l}");
            }
        }
        eprintln!(
            "smldocs: {} 处问题（详见 `cargo run -p smldocs -- --report`）",
            findings.total()
        );
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

/// 写生成物：内容相同则不写（幂等）；内容要变时先挡未提交改动。`true` = 真的写了文件。
fn write_out(path: &Path, text: &str, root: &Path) -> Result<bool, String> {
    if let Ok(old) = std::fs::read_to_string(path) {
        if old == text {
            return Ok(false);
        }
    }
    guard_uncommitted(root, path)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("建目录 {} 失败：{e}", dir.display()))?;
    }
    std::fs::write(path, text).map_err(|e| format!("写 {} 失败：{e}", path.display()))?;
    Ok(true)
}

/// 内容即将被覆盖时，目标文件若有未提交改动则拒绝；`git` 不可用一律 fail-closed。
fn guard_uncommitted(root: &Path, path: &Path) -> Result<(), String> {
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let out = std::process::Command::new("git")
        .args(["status", "--porcelain", "--", rel.as_str()])
        .current_dir(root)
        .output()
        .map_err(|e| {
            format!("未提交检测失败 ｜ uncommitted-check failed：无法运行 git ｜ cannot run git：{e}")
        })?;
    if !out.status.success() {
        return Err(format!(
            "未提交检测失败 ｜ uncommitted-check failed：git status {status} ｜ git status failed",
            status = out.status
        ));
    }
    if !String::from_utf8_lossy(&out.stdout).trim().is_empty() {
        return Err(format!(
            "拒绝覆盖未提交改动 ｜ refusing to overwrite uncommitted changes：{rel}（先提交或手动处理 ｜ commit or resolve it first）"
        ));
    }
    Ok(())
}
