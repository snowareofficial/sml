// Copyright (C) SNOWARE
// SPDX-License-Identifier: MulanPSL-2.0
//! `smlconv` — SML 命令行转译器（部分 emit / 文档站集成）。
//!
//! ⚠️ **实验性 (EXPERIMENTAL)**：本 crate 已从 `swsml` 主 crate 拆分为独立发布
//! （版本 `0.1.5`），CLI 接口与 emit 后端组合仍可能随用户反馈调整，暂不做语义化
//! 稳定性承诺。生产关键路径请勿依赖其精确行为，请关注版本号变更日志。
//!
//! 把一份 SML 文档经解析后，使用选定的 emit 后端翻译为目标文本：
//!
//! ```text
//! smlconv -i doc.sml --to md            # SML -> Markdown
//! smlconv -i doc.sml --to xml -o d.xml  # SML -> XML
//! cat doc.sml | smlconv --to svg        # 管道：stdin -> stdout
//! smlconv -i doc.sml --hugo content/zh  # 生成 content/zh/doc.md（含 front matter）
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
    CustomOptions, EmitOptions, LatexOptions, MarkdownOptions, SlintOptions, SvgOptions,
    XmlOptions, to_custom, to_lvgl,
};
use clap::Parser;
use std::io::IsTerminal;
use sml::{parse, to_sml, Value, Version};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Format {
    /// 原样回显（SML 序列化）。
    Sml,
    #[default]
    Markdown,
    Xml,
    Svg,
    Latex,
    Slint,
    Lvgl,
    Custom,
}

impl Format {
    fn parse(s: &str) -> Option<Format> {
        match s.to_ascii_lowercase().as_str() {
            "md" | "markdown" => Some(Format::Markdown),
            "xml" => Some(Format::Xml),
            "svg" => Some(Format::Svg),
            "latex" | "tex" => Some(Format::Latex),
            "slint" => Some(Format::Slint),
            "lvgl" => Some(Format::Lvgl),
            "custom" => Some(Format::Custom),
            "sml" => Some(Format::Sml),
            _ => None,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Format::Sml => "sml",
            Format::Markdown => "markdown",
            Format::Xml => "xml",
            Format::Svg => "svg",
            Format::Latex => "latex",
            Format::Slint => "slint",
            Format::Lvgl => "lvgl",
            Format::Custom => "custom",
        }
    }
}

#[derive(clap::Parser)]
#[command(
    name = "smlconv",
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

    /// 目标格式：md(默认) / xml / svg / latex / slint / lvgl / custom / sml
    #[arg(long = "to", alias = "format", default_value = "md")]
    format: String,

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
}

/// 把 clap 解析结果转换为内部使用的运行时参数。
struct Args {
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    format: Format,
    feature: Option<Version>,
    hugo: Option<PathBuf>,
    hugo_lang: Option<String>,
    hugo_section: String,
    zola: Option<PathBuf>,
    zola_section: String,
    zola_build: bool,
    title: Option<String>,
    custom_rules: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let cli = Cli::parse();
    let format = Format::parse(&cli.format)
        .ok_or_else(|| format!("unknown format `{}` (md|xml|svg|latex|slint|lvgl|custom|sml)", cli.format))?;
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
        feature,
        hugo: cli.hugo,
        hugo_lang: cli.hugo_lang,
        hugo_section: cli.hugo_section,
        zola: cli.zola,
        zola_section: cli.zola_section,
        zola_build: cli.zola_build,
        title: cli.title,
        custom_rules: cli.custom_rules,
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
                    "未检测到输入：请用 `-i <file.sml>` 指定文件，或用管道 `cat x.sml | smlconv`。\n\
                     运行 `smlconv --help` 查看完整用法。"
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
fn parse_with(text: &str, feature: Option<Version>) -> Result<Value, String> {
    if let Some(v) = feature {
        if v != Version::V4 {
            eprintln!(
                "smlconv: note: 解析器以 v4 模式工作；声明 `--feature {}` 仅作提示",
                v.name()
            );
        }
    }
    parse(text)
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
    args: &Args,
    input_path: &Option<PathBuf>,
) -> Result<(), String> {
    let hugo_dir = args
        .hugo
        .as_ref()
        .ok_or_else(|| "internal: write_hugo called without --hugo".to_string())?;

    // 计算目标文件名：优先 @feature base，否则取输入文件名 stem，否则 "doc"
    let inferred = infer_title(&Value::Null, input_path, &args.title);
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
    args: &Args,
    input_path: &Option<PathBuf>,
) -> Result<(), String> {
    let zola_dir = args
        .zola
        .as_ref()
        .ok_or_else(|| "internal: write_zola called without --zola".to_string())?;

    let inferred = infer_title(&Value::Null, input_path, &args.title);
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
                    "smlconv: --zola-build 需要本机安装 `zola`（未找到，请先安装或将 zola 加入 PATH）"
                        .to_string(),
                )
            }
        };
        eprintln!("running `zola build` in {}", zola_dir.display());
        let status = std::process::Command::new(&zola)
            .arg("build")
            .current_dir(zola_dir)
            .status()
            .map_err(|e| format!("smlconv: 无法启动 zola: {e}"))?;
        if !status.success() {
            return Err(format!("smlconv: zola build 失败 (exit {:?})", status.code()));
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

/// 尝试从 SML 文本中提取 `@feature base <name>` 作为标题；失败则退回输入名或 "doc"。
fn infer_title(value: &Value, input: &Option<PathBuf>, explicit: &Option<String>) -> String {
    if let Some(t) = explicit {
        return t.clone();
    }
    // value 顶层若有 __name（来自 @name 片段）也可借用
    if let Value::Object(m) = value {
        if let Some(Value::Str(s)) = m.get("__name") {
            return s.clone();
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
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
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
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
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
            eprintln!("smlconv: {e}");
            return ExitCode::from(2);
        }
    };

    // 互斥 / 依赖校验
    if args.zola_build && args.zola.is_none() {
        eprintln!("smlconv: --zola-build 必须与 --zola <dir> 一起使用");
        return ExitCode::from(2);
    }
    if args.hugo.is_some() && args.zola.is_some() {
        eprintln!("smlconv: --hugo 与 --zola 互斥，请只选其一");
        return ExitCode::from(2);
    }

    let text = match read_input(&args.input) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("smlconv: {e}");
            return ExitCode::from(2);
        }
    };

    let value = match parse_with(&text, args.feature) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("smlconv: parse error: {e}");
            return ExitCode::from(1);
        }
    };

    let rendered = match emit(&value, args.format, &args) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("smlconv: emit error: {e}");
            return ExitCode::from(1);
        }
    };

    // Hugo 模式：忽略 -o，直接落盘为 .md
    if args.hugo.is_some() {
        match write_hugo(&rendered, &args, &args.input) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("smlconv: {e}");
                ExitCode::from(2)
            }
        }
    } else if args.zola.is_some() {
        // Zola 模式：忽略 -o，直接落盘为 TOML-front-matter 的 .md
        match write_zola(&rendered, &args, &args.input) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("smlconv: {e}");
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
                    eprintln!("smlconv: write {}: {e}", p.display());
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
