// Copyright (C) SNOWARE
// SPDX-License-Identifier: MulanPSL-2.0
//! `smltools` — SML 命令行转译器（部分 emit / 文档站集成）。
//!
//! ⚠️ **实验性 (EXPERIMENTAL)**：本 crate 已从 `swsml` 主 crate 拆分为独立发布
//! （版本 `0.1.5`），CLI 接口与 emit 后端组合仍可能随用户反馈调整，暂不做语义化
//! 稳定性承诺。生产关键路径请勿依赖其精确行为，请关注版本号变更日志。
//!
//! 把一份 SML 文档经解析后，使用选定的 emit 后端翻译为目标文本：
//!
//! ```text
//! smltools -i doc.sml --to md            # SML -> Markdown
//! smltools -i doc.sml --to xml -o d.xml  # SML -> XML
//! cat doc.sml | smltools --to svg        # 管道：stdin -> stdout
//! smltools -i doc.sml --hugo content/zh  # 生成 content/zh/doc.md（含 front matter）
//! ```
//!
//! `--to` 取值：`md`/`markdown`/`xml`/`svg`/`latex`/`slint`/`lvgl`/`custom`/`sml`。
//! `--hugo <dir>` 会让 markdown 输出裹上最小 Hugo front matter，并以输入文件名
//! （或 `@feature base` 指定的名称）落盘为 `.md`，可直接被 `hugo` 收录。
//!
//! 退出码：0 成功；1 解析/翻译失败；2 参数/IO 错误。

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use sml::emit::{
    CustomOptions, EmitOptions, HtmlOptions, LatexOptions, MarkdownOptions, SlintOptions,
    SvgOptions, XmlOptions, to_custom, to_html, to_lvgl,
};
use clap::Parser;
use sml::{parse, to_sml, Value, Version};
use std::path::Path;

mod lint;
mod toml;
mod yaml;

/// 输入格式（迁移用）：SML 是原生格式，JSON / YAML 是「迁入」格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputFormat {
    Sml,
    Json,
    Toml,
    Yaml,
}

impl InputFormat {
    fn parse(s: &str) -> Option<InputFormat> {
        match s.to_ascii_lowercase().as_str() {
            "sml" => Some(InputFormat::Sml),
            "json" => Some(InputFormat::Json),
            "toml" => Some(InputFormat::Toml),
            "yaml" | "yml" => Some(InputFormat::Yaml),
            _ => None,
        }
    }

    /// 按扩展名推断（显式 `--from` 优先于此）。
    fn detect(path: Option<&Path>) -> InputFormat {
        match path.and_then(|p| p.extension()).and_then(|e| e.to_str()) {
            Some(e) if e.eq_ignore_ascii_case("json") => InputFormat::Json,
            Some(e) if e.eq_ignore_ascii_case("toml") => InputFormat::Toml,
            Some(e) if e.eq_ignore_ascii_case("yaml") || e.eq_ignore_ascii_case("yml") => {
                InputFormat::Yaml
            }
            _ => InputFormat::Sml,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            InputFormat::Sml => "sml",
            InputFormat::Json => "json",
            InputFormat::Toml => "toml",
            InputFormat::Yaml => "yaml",
        }
    }
}

/// 决定输入格式：显式 `--from` > 扩展名推断 > SML。
fn resolve_input_format(explicit: Option<&str>, input: Option<&Path>) -> Result<InputFormat, String> {
    match explicit {
        Some(s) => {
            InputFormat::parse(s).ok_or_else(|| format!("unknown input format `{s}` (sml|json|yaml)"))
        }
        None => Ok(InputFormat::detect(input)),
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Format {
    /// 原样回显（SML 序列化）。
    Sml,
    /// JSON 序列化。用途不是"替代 SML"，而是**对接现有工具链**：
    /// jq / 各类 JSON 库 / 只吃 JSON 的 API —— 让 SML 融进既有生态，
    /// 而不必要求对方先支持 SML。与 `--from json` 一起构成迁移闭环。
    Json,
    /// TOML 序列化。对接 Cargo / pyproject / 各类 TOML 配置生态。
    /// TOML 顶层必须是表，故非对象输入会得到空文档。
    Toml,
    #[default]
    Markdown,
    Xml,
    Svg,
    Latex,
    Slint,
    Lvgl,
    Custom,
    Html,
}

impl Format {
    /// 全部格式。用于 `--to` 报错提示 —— 原先提示里的
    /// `(md|xml|svg|latex|slint|lvgl|html|custom|sml)` 是**手写**的，
    /// 与 `name()` 两处维护；0.6.1 新增 `html` 时就得靠人工同步两个地方。
    const ALL: [Format; 11] = [
        Format::Markdown,
        Format::Json,
        Format::Toml,
        Format::Xml,
        Format::Svg,
        Format::Latex,
        Format::Slint,
        Format::Lvgl,
        Format::Html,
        Format::Custom,
        Format::Sml,
    ];

    /// 供错误提示用的格式名列表（由 [`Self::ALL`] + `name()` 生成，不会再漂移）。
    fn names() -> String {
        Self::ALL
            .iter()
            .map(|f| f.name())
            .collect::<Vec<_>>()
            .join("|")
    }

    fn parse(s: &str) -> Option<Format> {
        match s.to_ascii_lowercase().as_str() {
            "md" | "markdown" => Some(Format::Markdown),
            "json" => Some(Format::Json),
            "toml" => Some(Format::Toml),
            "xml" => Some(Format::Xml),
            "svg" => Some(Format::Svg),
            "latex" | "tex" => Some(Format::Latex),
            "slint" => Some(Format::Slint),
            "lvgl" => Some(Format::Lvgl),
            "html" | "htm" => Some(Format::Html),
            "custom" => Some(Format::Custom),
            "sml" => Some(Format::Sml),
            _ => None,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Format::Sml => "sml",
            Format::Json => "json",
            Format::Toml => "toml",
            Format::Markdown => "markdown",
            Format::Xml => "xml",
            Format::Svg => "svg",
            Format::Latex => "latex",
            Format::Slint => "slint",
            Format::Lvgl => "lvgl",
            Format::Html => "html",
            Format::Custom => "custom",
        }
    }
}

#[derive(clap::Parser)]
#[command(
    name = "smltools",
    version,
    about = "SML 转换工具：把 SML 源转换为多种格式，并可直接对接 Hugo / Zola 静态站点生成器（实验性）",
    long_about = None
)]
struct Cli {
    /// 输入文件；缺省时从 stdin 读取
    #[arg(short = 'i', long = "input")]
    input: Option<PathBuf>,

    /// 输出文件；缺省时写 stdout（Hugo/Zola 模式忽略此项，直接落盘）
    #[arg(short = 'o', long = "output")]
    output: Option<PathBuf>,

    /// 目标格式：md(默认) / json / toml / xml / svg / latex / slint / lvgl / html / custom / sml
    #[arg(long = "to", alias = "format", default_value = "md")]
    format: String,

    /// 输入格式：sml(默认) / json / toml / yaml。
    ///
    /// 缺省按输入文件扩展名推断（`.json` → json，`.toml` → toml，
    /// `.yaml`/`.yml` → yaml，其余 → sml）；
    /// 从 stdin 读且未显式指定时按 sml 处理。
    ///
    /// 迁移示例：`smltools -i app.json --from json --to sml > app.sml`
    #[arg(long = "from", alias = "input-format")]
    from: Option<String>,

    /// 剥离 SML 专有痕迹再转换：内部标记键 `__name`/`__type`，以及浮点的原始字面量文本。
    ///
    /// 片段 / 契约 / include / `$env` / `@when` 在解析期就已消解，解析结果本身已是纯数据；
    /// `--strip` 清掉剩余那两处，使输出能被 JSON 等格式**无损**消化。
    #[arg(long = "strip")]
    strip: bool,

    /// 静态检查模式（不产出转换结果）：报解析错误、未使用的片段/契约、tab 缩进、
    /// 空值字段、过深嵌套等。存在 error 级问题时退出码为 1。
    #[arg(long = "lint")]
    lint: bool,

    /// 显式声明解析版本（v1..v4）；缺省按文档声明或 V4
    #[arg(long = "feature")]
    feature: Option<String>,

    /// Hugo 集成：生成带 YAML front matter 的 .md 到该目录
    #[arg(long = "hugo")]
    hugo: Option<PathBuf>,

    /// Hugo 内容语言子目录（如 zh / en）
    #[arg(long = "hugo-lang")]
    hugo_lang: Option<String>,

    /// Hugo 章节（content 下的子目录，默认 docs）
    #[arg(long = "hugo-section", default_value = "docs")]
    hugo_section: String,

    /// Zola 集成：生成带 TOML front matter 的 .md 到该目录
    #[arg(long = "zola")]
    zola: Option<PathBuf>,

    /// Zola 章节（content 下的子目录，默认 docs）
    #[arg(long = "zola-section", default_value = "docs")]
    zola_section: String,

    /// 生成后自动调用本机 `zola build` 渲染站点（需已安装 zola）
    #[arg(long = "zola-build")]
    zola_build: bool,

    /// front matter 的 title（默认取文件名或推断）
    #[arg(long = "title")]
    title: Option<String>,

    /// 自定义生成器规则文件（仅 `--to custom` 时需要）：含 `rules` 数组的 SML 文档
    #[arg(long = "custom-rules")]
    custom_rules: Option<PathBuf>,

    /// LaTeX：放行数学块原样透传（`math`/`equation` → `$..$` / `equation` 环境）。
    ///
    /// 默认关闭：数学内容无法转义（转义会破坏公式语义），关闭时公式退化为
    /// 转义后的纯文本。仅在**信任输入源**时开启。
    #[arg(long = "math")]
    math: bool,
}

/// 按输入格式把文本转成 `Value`。
///
/// SML 走完整解析（版本 / 特性 / include / 契约）；JSON 与 YAML 是**迁入**格式，
/// 直接解析成数据，不参与 SML 的版本与特性机制。
fn load_input(text: &str, args: &Args) -> Result<Value, String> {
    match args.input_format {
        InputFormat::Sml => {
            parse_with(text, &args.input, args.feature).map_err(|e| format!("parse error: {e}"))
        }
        // 复用 crate 内既有实现（与 C-ABI 同款：零依赖、带深度限制与 UTF-8 修正）
        InputFormat::Json => sml::json_to_value(text)
            .ok_or_else(|| "JSON 解析失败：不是合法 JSON，或嵌套过深".to_string()),
        InputFormat::Toml => toml::parse(text).map_err(|e| format!("TOML 解析失败：{e}")),
        InputFormat::Yaml => yaml::parse(text).map_err(|e| format!("YAML 解析失败：{e}")),
    }
}

/// 目录批量转换：把 `dir` 下所有匹配输入格式的文件逐个转换，写入 `--output` 目录。
///
/// 这是「可选文件夹结构」的最小可用形态 —— 存量配置仓库（一堆 `.json`/`.yaml`/`.toml`）
/// 可以整目录迁进 SML，而不必逐个文件敲命令。输出文件与原文件同名，仅扩展名改为目标格式。
///
/// 刻意只做**同名平铺**而不复刻子目录：迁移场景下先看一眼结果，再决定怎么组织，
/// 比自动猜测目录结构更安全（猜错会把文件写到意外位置）。
fn convert_dir(dir: &Path, args: &Args) -> Result<usize, String> {
    let out_dir = args
        .output
        .as_ref()
        .ok_or_else(|| "目录模式下必须用 -o/--output 指定输出目录（避免污染源目录）".to_string())?;
    std::fs::create_dir_all(out_dir).map_err(|e| format!("mkdir {}: {e}", out_dir.display()))?;

    let want_ext = args.input_format.name();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("read_dir {}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case(want_ext))
                    .unwrap_or(false)
        })
        .collect();
    entries.sort(); // 输出顺序稳定，便于 diff

    let mut n = 0usize;
    for p in entries {
        let text =
            std::fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
        let value = load_input(&text, args).map_err(|e| format!("{}: {e}", p.display()))?;
        let value = if args.strip { strip_value(&value) } else { value };
        let out = emit(&value, args.format, args).map_err(|e| format!("{}: {e}", p.display()))?;
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
        let out_path = out_dir.join(format!("{stem}.{}", args.format.name()));
        std::fs::write(&out_path, out).map_err(|e| format!("write {}: {e}", out_path.display()))?;
        eprintln!("{} -> {}", p.display(), out_path.display());
        n += 1;
    }
    Ok(n)
}

/// 剥离 SML 专有痕迹（`--strip`）。
///
/// 片段 / 契约 / include / `$env` / `@when` 都在**解析期**消解，解析结果本身已是纯数据；
/// 真正会把「SML 特色」带进其它格式的只剩两处：
/// 1. 内部标记键 `__name` / `__type` —— 块级类型标注留下的脚手架；
/// 2. 浮点的原始字面量 —— `1.50` 这种写法只有 SML 保留，JSON 只能给出 `1.5`。
///
/// 清掉这两者，输出即可被 JSON 等格式无损消化（`1.5` 是等值数值，不是信息丢失）。
fn strip_value(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let mut out = std::collections::BTreeMap::new();
            for (k, val) in m {
                if k == "__name" || k == "__type" {
                    continue;
                }
                out.insert(k.clone(), strip_value(val));
            }
            Value::Object(out)
        }
        Value::Array(a) => Value::Array(a.iter().map(strip_value).collect()),
        Value::Float(f, _) => Value::float(*f),
        other => other.clone(),
    }
}

/// 把 clap 解析结果转换为内部使用的运行时参数。
struct Args {
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    format: Format,
    input_format: InputFormat,
    strip: bool,
    lint: bool,
    feature: Option<Version>,
    hugo: Option<PathBuf>,
    hugo_lang: Option<String>,
    hugo_section: String,
    zola: Option<PathBuf>,
    zola_section: String,
    zola_build: bool,
    title: Option<String>,
    custom_rules: Option<PathBuf>,
    math: bool,
}

fn parse_args() -> Result<Args, String> {
    let cli = Cli::parse();
    let format = Format::parse(&cli.format)
        .ok_or_else(|| format!("unknown format `{}` (可选：{})", cli.format, Format::names()))?;
    let input_format = resolve_input_format(cli.from.as_deref(), cli.input.as_deref())?;
    let feature = match cli.feature.as_deref() {
        None => None,
        Some(v) => {
            let ver = match v {
                "v1" | "1" => Version::V1,
                "v2" | "2" => Version::V2,
                "v3" | "3" => Version::V3,
                "v4" | "4" => Version::V4,
                _ => return Err(format!("unknown feature version `{v}` (v1..v4)")),
            };
            Some(ver)
        }
    };
    Ok(Args {
        input: cli.input,
        output: cli.output,
        format,
        input_format,
        strip: cli.strip,
        lint: cli.lint,
        feature,
        hugo: cli.hugo,
        hugo_lang: cli.hugo_lang,
        hugo_section: cli.hugo_section,
        zola: cli.zola,
        zola_section: cli.zola_section,
        zola_build: cli.zola_build,
        title: cli.title,
        custom_rules: cli.custom_rules,
        math: cli.math,
    })
}

/// 读取输入：文件或 stdin。
fn read_input(input: &Option<PathBuf>) -> Result<String, String> {
    match input {
        Some(p) => std::fs::read_to_string(p)
            .map_err(|e| format!("read {}: {e}", p.display())),
        None => {
            // 没有 -i 时从 stdin 读。但 stdin 若是交互式终端（无管道/重定向），
            // read_to_string 会无限等待用户输入，表现为“卡死”。
            // 这里用一个带超时的后台读取：若短时间内无数据（TTY 裸敲），
            // 直接报错提示用法，绝不无限等待。
            let (tx, rx) = std::sync::mpsc::channel::<std::io::Result<String>>();
            std::thread::spawn(move || {
                let mut s = String::new();
                let r = std::io::stdin().read_to_string(&mut s).map(|_| s);
                let _ = tx.send(r);
            });
            // 500ms 内没任何输入（说明在等终端键盘），判定为“无输入”。
            match rx.recv_timeout(std::time::Duration::from_millis(500)) {
                Ok(Ok(s)) => Ok(s),
                Ok(Err(e)) => Err(format!("read stdin: {e}")),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(
                    "未检测到输入：请用 `-i <file.sml>` 指定文件，或用管道 `cat x.sml | smltools`。\n\
                     运行 `smltools --help` 查看完整用法。"
                        .to_string(),
                ),
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(
                    "读取 stdin 时通道中断（内部错误）".to_string(),
                ),
            }
        }
    }
}

/// 按指定版本解析。当前 crate 的 `parse` 统一使用 CURRENT(V4) 解析器，
/// `--feature` 用于显式声明文档意图版本；V4 之前文件一般也能被兼容解析。
///
/// 指定了输入文件时，先按**输入文件所在目录**递归展开 `include`/`@include`
/// 指令（把按章节拆分的「结构文件 + 数据文件」拼成完整文档），再交给 `parse`
/// 解析。stdin 输入没有基准目录，只能回退到 `parse`（此时 `include` 无法解析
/// 相对路径，建议用 `-i` 文件输入）。
fn parse_with(text: &str, input_path: &Option<PathBuf>, feature: Option<Version>) -> Result<Value, String> {
    if let Some(v) = feature {
        if v != Version::V4 {
            eprintln!(
                "smltools: note: 解析器以 v4 模式工作；声明 `--feature {}` 仅作提示",
                v.name()
            );
        }
    }
    match input_path {
        Some(p) => {
            let base = p.parent().unwrap_or_else(|| Path::new("."));
            let expanded = expand_includes(text, base)?;
            parse(&expanded)
        }
        None => parse(text),
    }
}

/// 行级展开 `include "path"` / `@include "path"` / `import "path"` 指令。
///
/// 路径相对「包含方文件所在目录」递归解析（与 smlsml 的 include 语义一致）；
/// 嵌套展开受 16 层深度保护，防止循环包含导致无限递归。仅支持普通文件路径，
/// 不含 glob / regex 模式包含（满足「结构文件按章节 include 数据文件」需求）。
fn expand_includes(text: &str, base: &Path) -> Result<String, String> {
    expand_includes_impl(text, base, 0)
}

fn expand_includes_impl(text: &str, base: &Path, depth: usize) -> Result<String, String> {
    if depth > 16 {
        return Err("smltools: include 嵌套超过 16 层".into());
    }
    let mut out = String::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        let core = trimmed.strip_prefix('@').unwrap_or(trimmed);
        let rest = core
            .strip_prefix("include ")
            .or_else(|| core.strip_prefix("import "));
        if let Some(rest) = rest {
            let path = rest.trim();
            if let Some(rel) = path
                .strip_prefix('"')
                .and_then(|s| s.find('"').map(|i| &s[..i]))
            {
                let full = base.join(rel);
                let inc = std::fs::read_to_string(&full)
                    .map_err(|e| format!("smltools: include 读取 {} 失败: {e}", full.display()))?;
                let inc_base = full.parent().unwrap_or(base);
                out.push_str(&expand_includes_impl(&inc, inc_base, depth + 1)?);
                out.push('\n');
                continue;
            }
            return Err(format!("smltools: include 路径解析失败: {line}"));
        }
        out.push_str(line);
        out.push('\n');
    }
    Ok(out)
}

/// 选择后端做部分翻译。
fn emit(value: &Value, fmt: Format, args: &Args) -> Result<String, String> {
    match fmt {
        Format::Sml => Ok(to_sml(value)),
        Format::Markdown => {
            let opt = MarkdownOptions {
                base: EmitOptions::default(),
                ..Default::default()
            };
            sml::emit::to_markdown(value, &opt)
        }
        // JSON：直接复用 crate 内既有的 `jsonify`（与 C-ABI 同款实现，零依赖、带转义），
        // 不再另写一份序列化，避免两处行为漂移。
        Format::Json => Ok(sml::jsonify(value)),
        Format::Toml => Ok(toml::to_toml(value)),
        Format::Xml => {
            let opt = XmlOptions {
                base: EmitOptions {
                    standalone: true,
                    ..Default::default()
                },
                ..Default::default()
            };
            sml::emit::to_xml(value, &opt)
        }
        Format::Svg => {
            let opt = SvgOptions::default();
            sml::emit::to_svg(value, &opt)
        }
        Format::Latex => {
            let opt = LatexOptions {
                base: EmitOptions::default(),
                math: args.math,
                ..Default::default()
            };
            sml::emit::to_latex(value, &opt)
        }
        Format::Slint => {
            let opt = SlintOptions::default();
            sml::emit::to_slint(value, &opt)
        }
        Format::Lvgl => {
            let opt = XmlOptions {
                base: EmitOptions {
                    indent: 2,
                    ..Default::default()
                },
                ..Default::default()
            };
            to_lvgl(value, &opt)
        }
        Format::Html => {
            let opt = HtmlOptions {
                base: EmitOptions::default(),
                ..Default::default()
            };
            to_html(value, &opt)
        }
        Format::Custom => {
            // 自定义生成器需要规则文件：读取 → 解析 → 构建 CustomOptions
            let rules_path = args
                .custom_rules
                .as_ref()
                .ok_or_else(|| "`--to custom` 需要配合 `--custom-rules <file.sml>` 指定规则文档".to_string())?;
            let rules_text = std::fs::read_to_string(rules_path)
                .map_err(|e| format!("read {}: {e}", rules_path.display()))?;
            let gen = parse(&rules_text)
                .map_err(|e| format!("解析规则文档 {} 失败: {e}", rules_path.display()))?;
            let opt = CustomOptions::from_generator(&gen)
                .map_err(|e| format!("构建 custom 规则失败: {e}"))?;
            to_custom(value, &opt)
        }
    }
}

/// Hugo 集成：包裹最小 front matter 并以 `<name>.md` 落盘。
fn write_hugo(
    body: &str,
    value: &Value,
    args: &Args,
    input_path: &Option<PathBuf>,
) -> Result<(), String> {
    let hugo_dir = args
        .hugo
        .as_ref()
        .ok_or_else(|| "internal: write_hugo called without --hugo".to_string())?;

    // 计算文件名/标题：--title > 文档顶层 title 字段 > 输入文件名 stem > "doc"
    // （此处曾传 &Value::Null，使文档内的标题信息永远读不到；现传入解析结果）
    let inferred = infer_title(value, input_path, &args.title);
    let stem = sanitize_filename(&inferred);

    let mut dest = hugo_dir.clone();
    if let Some(lang) = &args.hugo_lang {
        dest = dest.join(sanitize_section(lang));
    }
    if !args.hugo_section.is_empty() {
        dest = dest.join(sanitize_section(&args.hugo_section));
    }
    std::fs::create_dir_all(&dest).map_err(|e| format!("mkdir {}: {e}", dest.display()))?;
    let out_path = dest.join(format!("{stem}.md"));

    let title = args
        .title
        .clone()
        .unwrap_or_else(|| inferred.clone());
    let fm = format!(
        "---\ntitle: \"{}\"\nlayout: \"single\"\ndate: {}\n---\n\n",
        escape_front_matter(&title),
        hugo_date()
    );
    let content = format!("{fm}{body}");
    std::fs::write(&out_path, content)
        .map_err(|e| format!("write {}: {e}", out_path.display()))?;
    eprintln!("wrote {}", out_path.display());
    Ok(())
}

/// Zola 集成：包裹 TOML front matter（`+++` 包裹）并以 `<name>.md` 落盘。
///
/// Zola 与 Hugo 的关键差异：
/// - front matter 用 TOML（而非 YAML），定界符为 `+++`；
/// - 默认内容目录即 `content/`，章节直接是 `content/<section>/`；
/// - 无语言子目录概念（多语言走 `content/<lang>/` 由调用方自行决定，这里不内置）。
fn write_zola(
    body: &str,
    value: &Value,
    args: &Args,
    input_path: &Option<PathBuf>,
) -> Result<(), String> {
    let zola_dir = args
        .zola
        .as_ref()
        .ok_or_else(|| "internal: write_zola called without --zola".to_string())?;

    let inferred = infer_title(value, input_path, &args.title);
    let stem = sanitize_filename(&inferred);

    let mut dest = zola_dir.clone();
    if !args.zola_section.is_empty() {
        dest = dest.join(sanitize_section(&args.zola_section));
    }
    std::fs::create_dir_all(&dest).map_err(|e| format!("mkdir {}: {e}", dest.display()))?;
    let out_path = dest.join(format!("{stem}.md"));

    let title = args
        .title
        .clone()
        .unwrap_or_else(|| inferred.clone());
    let fm = format!(
        "+++\ntitle = \"{}\"\ndate = {}\n+++\n\n",
        escape_front_matter(&title),
        hugo_date()
    );
    let content = format!("{fm}{body}");
    std::fs::write(&out_path, content)
        .map_err(|e| format!("write {}: {e}", out_path.display()))?;
    eprintln!("wrote {}", out_path.display());

    // 可选：生成后调用 `zola build` 渲染静态站点。
    if args.zola_build {
        let zola = match which_zola() {
            Some(p) => p,
            None => {
                return Err(
                    "smltools: --zola-build 需要本机安装 `zola`（未找到，请先安装或将 zola 加入 PATH）"
                        .to_string(),
                )
            }
        };
        eprintln!("running `zola build` in {}", zola_dir.display());
        let status = std::process::Command::new(&zola)
            .arg("build")
            .current_dir(zola_dir)
            .status()
            .map_err(|e| format!("smltools: 无法启动 zola: {e}"))?;
        if !status.success() {
            return Err(format!("smltools: zola build 失败 (exit {:?})", status.code()));
        }
        eprintln!("zola build 完成");
    }
    Ok(())
}

/// 探测本机 `zola` 可执行文件位置（优先 PATH，其次常见安装路径）。
/// 不引入额外依赖：PATH 探测用 `zola --version` 试跑，命中即返回 "zola"（交给
/// `Command` 在 PATH 中解析）；未命中则检查常见安装目录。
fn which_zola() -> Option<std::path::PathBuf> {
    // 1) PATH 中是否能直接调用
    if std::process::Command::new("zola")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return Some(std::path::PathBuf::from("zola"));
    }
    // 2) 常见安装目录兜底（Windows）
    for cand in [
        "C:\\Program Files\\zola\\zola.exe",
        "C:\\Program Files (x86)\\zola\\zola.exe",
        "C:\\tools\\zola\\zola.exe",
    ] {
        let p = std::path::PathBuf::from(cand);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// 推断文档标题：`--title` > 文档顶层 `title` 字段 > 输入文件名 stem > `"doc"`。
///
/// 两处历史问题在此一并纠正：
/// 1. 旧注释声称「从 SML 文本提取 `@feature base <name>`」，但实现里从来没有这段逻辑；
///    而 `@feature base` 只接受版本号（v1/v2/v3/v4），本来也承载不了名字 —— 注释与实现各说各话。
///    这里换成真正可用的推源头：文档顶层的 `title:` 字段。
/// 2. 旧实现会读顶层 `__name`，但两个调用点都传 `&Value::Null`，该分支**从未被触发过**；
///    顶层也基本不会出现 `__name`（它来自片段/块级标注的字段值，不是顶层键），属死代码，已移除。
fn infer_title(value: &Value, input: &Option<PathBuf>, explicit: &Option<String>) -> String {
    if let Some(t) = explicit {
        return t.clone();
    }
    if let Value::Object(m) = value {
        // `title: 我的小说` 与 `title: "我的小说"` 都取（裸词已是字符串）
        if let Some(Value::Str(s)) = m.get("title") {
            if !s.trim().is_empty() {
                return s.clone();
            }
        }
    }
    if let Some(p) = input {
        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
            return stem.to_string();
        }
    }
    "doc".to_string()
}

/// 文件名安全化：仅保留文件系统安全字符，其余替换为下划线。
fn sanitize_filename(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        return "doc".to_string();
    }
    s.chars()
        .map(|c| {
            // 保留 **Unicode** 字母数字：原先只认 ASCII，中文标题会被整体替换成
            // 下划线（`我的长篇小说` → `______`），文件名里的标题信息全丢。
            // 文件系统对 UTF-8 文件名无障碍，不必牺牲可读性。
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// front matter 字段值转义。
///
/// 只转义 `\` 和 `"` 是不够的：YAML(Hugo) 与 TOML(Zola) 的基本字符串都
/// **不允许未转义的换行**，若值里带 `\n`，攻击者就能闭合当前字段并在
/// front matter 中注入任意键（如 `layout`、`url`、`draft`、`aliases`），
/// 从而控制站点生成行为。故这里同时把换行/回车/制表符与其它控制字符转义，
/// 并挡掉 YAML 的文档分隔符。
fn escape_front_matter(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // 其余控制字符（含 \0）统一用短 unicode 转义，避免破坏解析器
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            _ => out.push(c),
        }
    }
    out
}

/// 章节名安全化：仅允许作为**单个**目录名使用，禁止路径分隔符与 `..`，
/// 避免 `--hugo-section ../../x` 把输出写到目标目录之外。
fn sanitize_section(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            // 同 sanitize_filename：保留 Unicode 字母数字（中文章节名不再被抹平）。
            // 安全性不受影响 —— `/`、`\` 仍被替换，首尾的 `.` 由下方 trim 掉，
            // 因此 `..` 与路径分隔符依旧无法逃出目标目录。
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches('.').to_string();
    if cleaned.is_empty() {
        "docs".to_string()
    } else {
        cleaned
    }
}

/// 生成稳定的 front matter date（零依赖占位；CI 中可用 sed 覆盖为真实日期）。
fn hugo_date() -> String {
    "1970-01-01T00:00:00Z".to_string()
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("smltools: {e}");
            return ExitCode::from(2);
        }
    };

    // 互斥 / 依赖校验
    if args.zola_build && args.zola.is_none() {
        eprintln!("smltools: --zola-build 必须与 --zola <dir> 一起使用");
        return ExitCode::from(2);
    }
    if args.hugo.is_some() && args.zola.is_some() {
        eprintln!("smltools: --hugo 与 --zola 互斥，请只选其一");
        return ExitCode::from(2);
    }

    // 目录批量模式：输入是目录时，逐个文件转换到 --output 目录（lint 模式不适用）
    if !args.lint {
        if let Some(dir) = args.input.as_ref().filter(|p| p.is_dir()) {
            return match convert_dir(dir, &args) {
                Ok(n) => {
                    eprintln!("smltools: 已转换 {n} 个文件");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("smltools: {e}");
                    ExitCode::from(2)
                }
            };
        }
    }

    let text = match read_input(&args.input) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("smltools: {e}");
            return ExitCode::from(2);
        }
    };

    // lint 模式：只做静态检查，不产出转换结果
    if args.lint {
        if args.input_format != InputFormat::Sml {
            eprintln!(
                "smltools: --lint 只能检查 SML 文档（当前 --from {}）",
                args.input_format.name()
            );
            return ExitCode::from(2);
        }
        let report = lint::check(&text, &args.input);
        for m in &report.messages {
            eprintln!("{m}");
        }
        if report.has_error {
            return ExitCode::from(1);
        }
        if report.messages.is_empty() {
            eprintln!("smltools: 未发现问题");
        }
        return ExitCode::SUCCESS;
    }

    let value = match load_input(&text, &args) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("smltools: {e}");
            return ExitCode::from(1);
        }
    };
    let value = if args.strip { strip_value(&value) } else { value };

    let rendered = match emit(&value, args.format, &args) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("smltools: emit error: {e}");
            return ExitCode::from(1);
        }
    };

    // Hugo 模式：忽略 -o，直接落盘为 .md
    if args.hugo.is_some() {
        match write_hugo(&rendered, &value, &args, &args.input) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("smltools: {e}");
                ExitCode::from(2)
            }
        }
    } else if args.zola.is_some() {
        // Zola 模式：忽略 -o，直接落盘为 TOML-front-matter 的 .md
        match write_zola(&rendered, &value, &args, &args.input) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("smltools: {e}");
                ExitCode::from(2)
            }
        }
    } else {
        match &args.output {
            Some(p) => match std::fs::write(p, &rendered) {
                Ok(()) => {
                    eprintln!("wrote {}", p.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("smltools: write {}: {e}", p.display());
                    ExitCode::from(2)
                }
            },
            None => {
                let mut o = std::io::stdout().lock();
                let _ = o.write_all(rendered.as_bytes());
                let _ = o.write_all(b"\n");
                ExitCode::SUCCESS
            }
        }
    }
}
