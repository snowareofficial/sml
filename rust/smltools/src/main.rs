// Copyright (C) SNOWARE
// SPDX-License-Identifier: MulanPSL-2.0
//! `smltools` — the SML command-line transpiler (partial emit backends / doc-site integration). ｜ `smltools` — SML 命令行转译器（部分 emit / 文档站集成）。
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
//! smltools -i app.json --from json --to sml   # 迁移：JSON -> SML
//! smltools -i chip.svd --from xml --to sml    # 迁移：XML -> SML（扩展名自动推断）
//! ```
//!
//! `--from` 取值：`sml`（默认）/`json`/`toml`/`yaml`/`xml`，缺省按扩展名推断。
//! `--to` 取值：`md`/`markdown`/`xml`/`svg`/`latex`/`slint`/`lvgl`/`custom`/`sml`。
//! `--hugo <dir>` 会让 markdown 输出裹上最小 Hugo front matter，并以 `<name>.md`
//! 落盘（`<name>` 取 `--title` > 文档顶层 `title` 字段 > 输入文件名 stem > `"doc"`），
//! 可直接被 `hugo` 收录。
//!
//! 注：这里曾写「以 `@feature base` 指定的名称落盘」，与实现不符 ——
//! `@feature base` 只声明基准版本集（`10` / `1.0` / `v1` …），承载不了名字；落盘名实际来自
//! 上面的推断链（见 `infer_title`）。
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
// `to_sml` 不再直接调用：SML 输出走带检查的 `sml::to_sml_checked`（W16，见 emit()）
use sml::{Edition, Map, parse, Feature, FeatureSet, Value};
// ⚠️ `E_INCLUDE_001/004/012` **不再由本 crate 抛出**：include 展开已统一交给库的
// `sml::parse_file`（原先那份 CLI 自有的行级预处理已删除），这些码由 `sml-include` 带出。
use sml_codes::{
    SmlError, E_CLI_001, E_CLI_002, E_CLI_003, E_CLI_004, E_CLI_005, E_CLI_006, E_CLI_007,
    E_CLI_008,
    E_FEATURE_004, E_INTERNAL_001, E_IO_001, E_IO_003, E_IO_004, E_IO_005, E_IO_006, E_IO_007,
    E_MIGRATE_017, I_FEATURE_001,
};
use std::path::Path;

mod highlight;
mod lint;
mod profile;
mod toc;
mod toml;
mod xml;
mod yaml;

/// 输入格式（迁移用）：SML 是原生格式，JSON / TOML / YAML / XML 是「迁入」格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputFormat {
    Sml,
    Json,
    Toml,
    Yaml,
    Xml,
}

impl InputFormat {
    fn parse(s: &str) -> Option<InputFormat> {
        match s.to_ascii_lowercase().as_str() {
            "sml" => Some(InputFormat::Sml),
            "json" => Some(InputFormat::Json),
            "toml" => Some(InputFormat::Toml),
            "yaml" | "yml" => Some(InputFormat::Yaml),
            "xml" => Some(InputFormat::Xml),
            _ => None,
        }
    }

    /// 该格式认的文件扩展名。
    ///
    /// 单一事实来源：扩展名列表原先在 `detect()` 里手写一遍、在目录批量里又用
    /// `name()` 拼一遍，两处漂移就会出「单文件认、整目录不认」这类怪事。
    ///
    /// `.svd` 归入 XML：CMSIS-SVD 本来就是一份 XML，芯片描述文件是迁移场景里
    /// 体量最大的一类，让用户为此多敲一次 `--from xml` 没有意义。
    fn extensions(&self) -> &'static [&'static str] {
        match self {
            InputFormat::Sml => &["sml"],
            InputFormat::Json => &["json"],
            InputFormat::Toml => &["toml"],
            InputFormat::Yaml => &["yaml", "yml"],
            InputFormat::Xml => &["xml", "svd"],
        }
    }

    fn matches_ext(&self, ext: &str) -> bool {
        self.extensions().iter().any(|e| e.eq_ignore_ascii_case(ext))
    }

    /// 按扩展名推断（显式 `--from` 优先于此）。
    fn detect(path: Option<&Path>) -> InputFormat {
        const MIGRATABLE: [InputFormat; 4] = [
            InputFormat::Json,
            InputFormat::Toml,
            InputFormat::Yaml,
            InputFormat::Xml,
        ];
        if let Some(ext) = path.and_then(|p| p.extension()).and_then(|e| e.to_str()) {
            for f in MIGRATABLE {
                if f.matches_ext(ext) {
                    return f;
                }
            }
        }
        InputFormat::Sml
    }

    fn name(&self) -> &'static str {
        match self {
            InputFormat::Sml => "sml",
            InputFormat::Json => "json",
            InputFormat::Toml => "toml",
            InputFormat::Yaml => "yaml",
            InputFormat::Xml => "xml",
        }
    }
}

/// 决定输入格式：显式 `--from` > 扩展名推断 > SML。
fn resolve_input_format(
    explicit: Option<&str>,
    input: Option<&Path>,
) -> Result<InputFormat, SmlError> {
    match explicit {
        // `E-CLI-001`：显式指定时取值校验；按扩展名推断不会走到这里。
        Some(s) => InputFormat::parse(s).ok_or_else(|| {
            SmlError::from_code_msg(E_CLI_001).with_detail(format!("`{s}`（sml|json|toml|yaml|xml）"))
        }),
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
    /// **编辑器高亮生成**：把「高亮定制 SML」在原版 TextMate 基线上升级为
    /// 定制化的 `sml.tmLanguage.json`（VSCode / VSIX 用）。
    ///
    /// 与其它后端不同，这里输入不是数据而是**定制声明**（`directives` / `elements` /
    /// `types` / `rules`），语义见 `highlight` 模块文档。
    TmLanguage,
    /// **编辑器定制包**：一份 SML 定制 → 多份产物（语法 + 颜色主题 + 项目内生效片段
    /// + Zed 查询与主题），输出到 `-o` 指定的**目录**。
    ///
    /// 与其它后端不同，它是「一对多」的：语法给扩展编译用，settings 片段给项目就地生效用，
    /// Zed 侧另有 Tree-sitter 查询与主题。
    Highlight,
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
    const ALL: [Format; 13] = [
        Format::Markdown,
        Format::Json,
        Format::Toml,
        Format::TmLanguage,
        Format::Highlight,
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
            "tmlanguage" | "tm" => Some(Format::TmLanguage),
            "highlight" | "editor" => Some(Format::Highlight),
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
            Format::TmLanguage => "tmlanguage",
            Format::Highlight => "highlight",
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

    /// 目标格式：md(默认) / json / toml / tmlanguage / xml / svg / latex / slint / lvgl / html / custom / sml
    ///
    /// `--to tmlanguage` 是特例：输入为**高亮定制声明**（directives/elements/types/rules），
    /// 输出为升级后的 TextMate 高亮文件（在原版基线上增补），供 VSCode / VSIX 使用。
    #[arg(long = "to", alias = "format", default_value = "md")]
    format: String,

    /// 输入格式：sml(默认) / json / toml / yaml / xml。
    ///
    /// 缺省按输入文件扩展名推断（`.json` → json，`.toml` → toml，
    /// `.yaml`/`.yml` → yaml，`.xml`/`.svd` → xml，其余 → sml）；
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

    /// 显式声明解析版本集（1.0..4.0 即 v1..v4；开发中的 5.0 / 代号 camelia 也收）；缺省按文档自身的 `@version`
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

    /// 数学块原样透传：Markdown（`$$…$$`）与 LaTeX（`$..$` / `equation` 环境）都认这个开关。
    ///
    /// 默认关闭：数学内容无法转义（转义会破坏公式语义），关闭时公式退化为
    /// 转义后的纯文本 / `<div class="math">\[…\]</div>`。仅在**信任输入源**时开启。
    #[arg(long = "math")]
    math: bool,

    /// 在输出前插入目录（仅 `--to md` / `--to html`）。
    ///
    /// 标题取自 `h1..h6` / `topic` / `section`；**有 `label` 的条目才是链接**，
    /// 没给 `label` 的只列标题（锚点得先有个名字，不能瞎造）。
    /// ⚠️ 标题顺序：块用 `children` 数组时才保源序，否则按字段名排序（BTreeMap 语义）。
    #[arg(long = "toc")]
    toc: bool,

    /// 契约推断（任务 E）：从数据实例反推 `@contract`。
    ///
    /// 输入为一份 SML/JSON 文档：根是**对象数组**（每个元素是一条实例），
    /// 或单个对象（按单实例处理）。输出默认是 SML `@contract` 文本；配合
    /// `--json` 切到 JSON 字段类型表。推断严格不猜 default/min/max/enum/@type，
    /// 多实例时字段在部分实例缺失才标记 optional，生成的契约默认 loose。
    #[arg(long = "infer-contract")]
    infer_contract: bool,

    /// 与 `--infer-contract` 配合：输出 JSON 形式的字段类型表（默认 SML）。
    #[arg(long = "json")]
    json: bool,

    /// 导出**内置契约表**（JSON）：输入是一份只含 `@contract` 的 SML，
    /// 输出符合 `docs/weakbind-spec-draft.md` §6.2 的 schema，供编辑器/插件读取。
    ///
    /// 与 `--to` 互不干扰：本模式不产出转换结果，只把契约定义导成数据。
    #[arg(long = "dump-builtins")]
    dump_builtins: bool,

    /// 解析 `@weakdoc` 引用：按「显式 → 项目 → 用户全局 → 系统」查找同名 `.smlex`。
    ///
    /// 输出 TAB 分隔三列：`名字 / 作用域 / 路径`；未命中的记 `-` 且退出码为 1。
    /// **只做定位**，不解释 profile 内容（内容语义未定，见 `docs/weakdoc-spec-draft.md`）。
    #[arg(long = "resolve-profiles")]
    resolve_profiles: bool,

    /// 追加一个显式 profile 目录（优先级最高，可用于测试与 CI 复现）
    #[arg(long = "profile-dir")]
    profile_dir: Option<PathBuf>,

    /// 关闭三命名空间发现（只用 `--profile-dir`）
    #[arg(long = "no-profiles")]
    no_profiles: bool,

    /// 从 `.smlb` 信封读入（二进制），再按 `--to` 输出
    ///
    /// 信封里的主载荷必须是 `Value`（即由 SML 文本写成的）；字节载荷（`--to-smlb` 之外的
    /// 归档类文件）会明确报错，而不是当成空文档。
    #[arg(long = "from-smlb")]
    from_smlb: bool,

    /// 把结果写成 `.smlb` 信封（二进制），忽略 `--to`
    ///
    /// 与 `--hugo` / `--zola` / `--lint` 互斥（它们是纯文本产物）。
    #[arg(long = "to-smlb")]
    to_smlb: bool,

    /// `--to-smlb` 用的 zstd 级别（需构建带 `compress` feature；缺省不压）
    #[arg(long = "smlb-compress", value_name = "N")]
    smlb_compress: Option<i32>,
}

/// 按输入格式把文本转成 `Value`。
///
/// SML 走完整解析（版本 / 特性 / include / 契约）；JSON / TOML / YAML / XML 是
/// **迁入**格式，直接解析成数据，不参与 SML 的版本与特性机制。
fn load_input(text: &str, args: &Args) -> Result<Value, SmlError> {
    match args.input_format {
        // ⚠️ `--from-smlb` 时传 `&None`：`text` 已经从信封解出来了，若把路径传下去，
        // `parse_with` 会**按路径重读文件**（拿二进制当 UTF-8 读，报 "stream did not
        // contain valid UTF-8"），而且 include 在写信封时就已展开完，没有基准目录可解析。
        InputFormat::Sml => {
            let path = if args.from_smlb { &None } else { &args.input };
            parse_with(text, path, args.feature)
        }
        // 复用 crate 内既有实现（与 C-ABI 同款：零依赖、带深度限制与 UTF-8 修正）
        // E-MIGRATE-017：「非法 JSON」与「嵌套过深」在底层共用一个空值，故合为一码。
        InputFormat::Json => sml::json_to_value(text).ok_or_else(|| {
            SmlError::from_code_msg(E_MIGRATE_017)
        }),
        InputFormat::Toml => toml::parse(text).map_err(|e| e.context("TOML 解析失败：")),
        InputFormat::Yaml => yaml::parse(text).map_err(|e| e.context("YAML 解析失败：")),
        InputFormat::Xml => xml::parse(text).map_err(|e| e.context("XML 解析失败：")),
    }
}

/// 目录批量转换：把 `dir` 下所有匹配输入格式的文件逐个转换，写入 `--output` 目录。
///
/// 这是「可选文件夹结构」的最小可用形态 —— 存量配置仓库（一堆 `.json`/`.yaml`/`.toml`）
/// 可以整目录迁进 SML，而不必逐个文件敲命令。输出文件与原文件同名，仅扩展名改为目标格式。
///
/// 刻意只做**同名平铺**而不复刻子目录：迁移场景下先看一眼结果，再决定怎么组织，
/// 比自动猜测目录结构更安全（猜错会把文件写到意外位置）。
fn convert_dir(dir: &Path, args: &Args) -> Result<usize, SmlError> {
    // E-CLI-005：缺省输出会污染源目录，故直接拒绝而不是猜。
    let out_dir = args.output.as_ref().ok_or_else(|| {
        SmlError::from_code_msg(E_CLI_005)
            .with_detail("目录模式下必须用 -o/--output 指定输出目录（避免污染源目录）")
    })?;
    // E-IO-003：写入文件或创建目录失败。
    std::fs::create_dir_all(out_dir)
        .map_err(|e| SmlError::from_code_msg(E_IO_003).with_detail(format!("{}: {e}", out_dir.display())))?;

    let want = args.input_format;
    // E-IO-004：目录批量模式的入口，与单文件读取失败（E-IO-001）分开。
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| SmlError::from_code_msg(E_IO_004).with_detail(format!("{}: {e}", dir.display())))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| want.matches_ext(e))
                    .unwrap_or(false)
        })
        .collect();
    entries.sort(); // 输出顺序稳定，便于 diff

    let mut n = 0usize;
    for p in entries {
        // E-IO-001：读取失败（文档入口）。
        let text = std::fs::read_to_string(&p)
            .map_err(|e| SmlError::from_code_msg(E_IO_001).with_detail(format!("{}: {e}", p.display())))?;
        // 用 context 补上文件名前缀而**保留内层码**（不能 format 成 String 否则码会丢）。
        let value = load_input(&text, args).map_err(|e| e.context(&format!("{}: ", p.display())))?;
        let value = if args.strip { strip_value(&value) } else { value };
        let out =
            emit(&value, args.format, args).map_err(|e| e.context(&format!("{}: ", p.display())))?;
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
        let out_path = out_dir.join(format!("{stem}.{}", args.format.name()));
        // E-IO-003：写入失败。
        std::fs::write(&out_path, out)
            .map_err(|e| SmlError::from_code_msg(E_IO_003).with_detail(format!("{}: {e}", out_path.display())))?;
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
            let mut out = Map::new();
            for (k, val) in m {
                if k == "__name" || k == "__type" {
                    continue;
                }
                out.insert(k.to_string(), strip_value(val));
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
    feature: Option<Edition>,
    hugo: Option<PathBuf>,
    hugo_lang: Option<String>,
    hugo_section: String,
    zola: Option<PathBuf>,
    zola_section: String,
    zola_build: bool,
    title: Option<String>,
    custom_rules: Option<PathBuf>,
    math: bool,
    toc: bool,
    infer_contract: bool,
    json: bool,
    dump_builtins: bool,
    resolve_profiles: bool,
    profile_dir: Option<PathBuf>,
    no_profiles: bool,
    /// 从 `.smlb` 信封读入（二进制） ｜ read the input as a `.smlb` envelope
    from_smlb: bool,
    /// 写成 `.smlb` 信封（二进制） ｜ write the result as a `.smlb` envelope
    to_smlb: bool,
    /// `--to-smlb` 的 zstd 级别 ｜ zstd level for `--to-smlb`
    smlb_compress: Option<i32>,
}

/// `--feature` 的取值解析：与语言层**同一套字面量**（`10` / `1.0` / `v1` / `50` / `camelia`）。
///
/// **Dev 代也收**（用户 2026-11-04 定）：命令行没有 `dev` 槽位，但「用开发中的功能集门控一下看看」
/// 是真实需求（例如在 v4 文档上预演 5.0 的默认位）。代价是必须**说清**——
/// 收下后由 `parse_with` 打一条 note，避免「以为自己在用已发布版本集」。
///
/// 失败一律 `E-FEATURE-004`（命令行传入非法版本名归此码，不是新的语言错误）。
fn edition_arg(v: &str) -> Result<Edition, SmlError> {
    Edition::parse(v)
        .or_else(|| sml::edition_by_codename(v).map(|r| r.edition))
        .ok_or_else(|| {
            SmlError::from_code_title(E_FEATURE_004)
                .with_detail(format!("`{v}`（1.0..4.0，即 v1..v4；开发中还有 5.0 / camelia）"))
        })
}

fn parse_args(cli: Cli) -> Result<Args, SmlError> {
    // E-CLI-002：输出格式取值未知（文案附带全部可用取值）。
    let format = Format::parse(&cli.format).ok_or_else(|| {
        SmlError::from_code_msg(E_CLI_002)
            .with_detail(format!("`{}`（可选：{}）", cli.format, Format::names()))
    })?;
    let input_format = resolve_input_format(cli.from.as_deref(), cli.input.as_deref())?;
    let feature = match cli.feature.as_deref() {
        None => None,
        Some(v) => Some(edition_arg(v)?),
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
        toc: cli.toc,
        infer_contract: cli.infer_contract,
        json: cli.json,
        dump_builtins: cli.dump_builtins,
        resolve_profiles: cli.resolve_profiles,
        profile_dir: cli.profile_dir,
        no_profiles: cli.no_profiles,
        from_smlb: cli.from_smlb,
        to_smlb: cli.to_smlb,
        smlb_compress: cli.smlb_compress,
    })
}

/// 读取输入：文件或 stdin。
fn read_input(input: &Option<PathBuf>) -> Result<String, SmlError> {
    match input {
        // E-IO-001：读取文件失败（文档入口；include 展开那条读取路径同码）。
        Some(p) => std::fs::read_to_string(p)
            .map_err(|e| SmlError::from_code_msg(E_IO_001).with_detail(format!("{}: {e}", p.display()))),
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
                // E-IO-006：读取标准输入失败。
                Ok(Err(e)) => Err(SmlError::from_code_msg(E_IO_006).with_detail(format!("{e}"))),
                // E-IO-005：用法提示而非读文件失败 —— 标准输入是终端且未给输入文件时给出。
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(
                    SmlError::from_code_msg(E_IO_005).with_detail(
                        "请用 `-i <file.sml>` 指定文件，或用管道 `cat x.sml | smltools`。\n\
                         运行 `smltools --help` 查看完整用法。",
                    ),
                ),
                // E-IO-006：通道中断（码表把它与「读取失败」合为一条）。
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    Err(SmlError::from_code_msg(E_IO_006))
                }
            }
        }
    }
}

/// 把 CLI 声明的版本集折算成「调用方允许特性集」，用于真正的版本门控。
///
/// 门控只动 `BarewordStr` 一位，且**判据是查表**（`FeatureSet::for_edition(e)` 里有没有
/// `bareword-string`）：有 ⇒ 允许裸词即字符串；没有（`2.0` 起）⇒ 要求显式引号。
/// 其余能力（glob / regex / multi-include …）仍沿用全集、由文档 `@feature enable` 自行决定，
/// 避免 `--feature` 顺手砍掉与版本无关的高级能力。
///
/// ⚠️ **已知缺口（两条，登记在 TODO.md 排雷区）**：
/// 1. **这里只折算了一处版本差异** —— 另一处是 `3.0`/`4.0` 的片段定义参数形式
///    （`4.0` 起废弃 `@f Server prod { }` 的位置参数，改显式 `type:` / `name:`）。
///    解析器实际是「两种写法都收 + 位置参数给弃用诊断」，**没有做版本门控**
///    ⇒ 声明 `--feature v3` 时这一处仍按 `4.0` 处理，故 `parse_with` 会就此给出
///    `I-FEATURE-001` 信息提示（见下方）。
/// 2. **为什么不用 `for_edition` 作为允许集**：它的基线是 `FeatureSet::baseline()`，
///    而 CLI 的允许集必须是 `FeatureSet::all()` —— 直接换过去会把「文档 `@feature enable`
///    能开启哪些能力」的口径从 all 收窄到 baseline，属**行为变更**。
///    故这里只取「裸词那一位」的结论，其余位仍给全集。
fn allowed_for_edition(e: Edition) -> FeatureSet {
    if FeatureSet::for_edition(e).has(Feature::BarewordStr) {
        FeatureSet::all()
    } else {
        FeatureSet::all().without(Feature::BarewordStr)
    }
}

/// 按指定版本解析。**显式给了 `--feature vN` 时才真正按版本裁剪**：走库的特性
/// 门控入口（`parse_file_features` / `parse_with_features`），把上面折算出的允许集
/// 交进去 ⇒ 严格模式下再写裸词字符串会由解析器带 `E-FEATURE-005` 报出，
/// 而不再是原先那条「仅作提示」的 note。未声明 `--feature` 时维持原入口。
///
/// 指定了输入文件时，先按**输入文件所在目录**递归展开 `include`/`@include`
/// 指令（把按章节拆分的「结构文件 + 数据文件」拼成完整文档），再交给 `parse`
/// 解析。stdin 输入没有基准目录，只能回退到 `parse`（此时 `include` 无法解析
/// 相对路径，建议用 `-i` 文件输入）。
fn parse_with(
    text: &str,
    input_path: &Option<PathBuf>,
    feature: Option<Edition>,
) -> Result<Value, SmlError> {
    // `I-FEATURE-001`：声明的版本**有一部分差异未做门控**时提示，不要静默。
    // 当前唯一的一处是 `3.0`（旧写法 v3）—— 字符串引号规则已按该代生效，但片段定义的
    // 位置参数形式仍按 `4.0` 处理（两种写法都收 + 位置参数给弃用诊断），`3.0` 该有的
    // 「位置参数合法、不告警」没有实现。`1.0` / `2.0` / `4.0` 暂无此类缺口，故不提示。
    if let Some(v) = feature {
        if v == Edition::V3 {
            eprintln!(
                "smltools: note: 声明 `--feature {}`：字符串引号规则已按该版本生效；\
                 但片段定义的位置参数形式仍按 v4 处理（会产出弃用诊断），这一步尚未做版本门控 [{}]",
                v.name(),
                I_FEATURE_001
            );
        }
        // Dev 代（目前只有 `5.0` / `camelia`）：功能集**未定稿**，门控结果只代表「此刻」。
        // 收下它是给「预演一下 5.0 的默认位」用的，但必须说清 —— 否则用户会以为这是承诺过的口径。
        if !v.is_supported() {
            let codename = sml::EDITIONS
                .iter()
                .find(|r| r.edition == v)
                .and_then(|r| r.codename)
                .map(|c| format!(" / 代号 `{c}`"))
                .unwrap_or_default();
            eprintln!(
                "smltools: note: `--feature {}`{codename} 是**开发中**的版本集，其默认功能集尚未定稿；\
                 本次门控只代表当前实现，不要写进对外文档 [{}]",
                v.name(),
                I_FEATURE_001
            );
        }
    }
    match (feature, input_path) {
        // 有输入文件 ⇒ **交给库的 `parse_file`**（给了 `--feature` 时为
        // `parse_file_features`，同一层入口、同样自带 include 展开）：与 C-ABI
        // `sml_load_file`、编辑器后端、以及 `examples/advanced.sml` 头部写明的
        // 推荐用法走**同一条**路径。
        //
        // 为什么不再自己展开：CLI 原有一份"行级 include 展开"（只认行首单个引号路径），
        // 与库的 `sml-include` 分叉，实测三处后果，其中第一处是**静默数据丢失**：
        //   · `include "common.sml", "secrets.sml" as sec`
        //       ⇒ 只展开第一个，`continue` 把整行后半段丢掉 → 第二个文件**无声消失**；
        //   · `include "inc/*.sml"` ⇒ 当字面路径读 ⇒ `E-INCLUDE-001`（误导为"文件读取失败"）；
        //   · `import { key } as w in "f.sml"` ⇒ 报 `E-INCLUDE-012`，而库把该写法列为
        //     **等价写法**（`rust/sml-include/src/parse.rs:80`）⇒ `examples/advanced.sml` 自己跑不过。
        // 交给库之后，`multi-include` / `as ns` / 挑键 `{ k }` / `glob` / `regex` / `@feature`
        // 全由库负责 —— 两处实现**只可能一致**。
        (Some(v), Some(p)) => sml::parse_file_features(p, allowed_for_edition(v))
            .map_err(|e| e.context("parse error: ")),
        (Some(v), None) => sml::parse_with_features(text, allowed_for_edition(v))
            .map(|(val, _)| val)
            .map_err(|e| e.context("parse error: ")),
        (None, Some(p)) => sml::parse_file(p).map_err(|e| e.context("parse error: ")),
        // stdin 没有基准目录，include 的相对路径无从解析（建议改用 `-i <file>`）。
        (None, None) => parse(text).map_err(|e| e.context("parse error: ")),
    }
}

// 说明：此处原先挂着一份 **CLI 自带的行级 include 展开**（`expand_includes` /
// `expand_includes_impl`）。它与库的 `sml-include` 是两份实现，且已实测分叉
// （多目标只展开第一个且**静默丢弃后半行**、glob 当字面路径、库列为等价写法的
// `import { k } as w in "f"` 被判非法 ⇒ `examples/advanced.sml` 自己跑不过）。
// 已整体删除，统一走库的 `sml::parse_file`（见 `parse_with`），杜绝再次漂移。

/// 选择后端做部分翻译。
///
/// `swsml::emit::*` 已统一返回 `Result<_, SmlError>`：**码由后端自己带上**
/// （递归深度 `E-LIMIT-004`、custom 输出超长 `E-LIMIT-005`、custom 数组循环
/// `E-LIMIT-006`、custom 规则文档非法 `E-EXT-006`、其余后端自身失败 `E-CLI-007`），
/// 本函数只做**原样透传**，不再按文案前缀猜码。
/// 本模块自带的两个编辑器后端（tmlanguage / highlight）同样直接返回带码错误。
fn emit(value: &Value, fmt: Format, args: &Args) -> Result<String, SmlError> {
    let rendered = emit_plain(value, fmt, args)?;
    Ok(insert_toc(rendered, value, fmt, args.toc))
}

/// `--toc`：只在 Markdown / HTML 上落（其余后端没有「目录」这个概念的落点）。
///
/// 条目从**值树**抽（见 `toc` 模块），所以不受输出文本里代码块/围栏的干扰。
fn insert_toc(rendered: String, value: &Value, fmt: Format, toc: bool) -> String {
    if !toc {
        return rendered;
    }
    let entries = toc::collect(value);
    match fmt {
        Format::Markdown => {
            let block = toc::render_markdown(&entries);
            if block.is_empty() {
                return rendered;
            }
            let mut out = block;
            out.push_str(&rendered);
            out
        }
        Format::Html => {
            let block = toc::render_html(&entries);
            if block.is_empty() {
                return rendered;
            }
            // 完整文档要插在 `<body>` 之后；片段（无 body）就直接前置。
            match rendered.find("<body") {
                Some(pos) => match rendered[pos..].find('>') {
                    Some(gt) => {
                        let cut = pos + gt + 1;
                        let mut out = String::with_capacity(rendered.len() + block.len() + 1);
                        out.push_str(&rendered[..cut]);
                        out.push('\n');
                        out.push_str(&block);
                        out.push_str(&rendered[cut..]);
                        out
                    }
                    None => format!("{block}{rendered}"),
                },
                None => format!("{block}{rendered}"),
            }
        }
        _ => rendered,
    }
}

/// 真正干活的那一层（原 `emit`）。`--toc` 的后处理在 [`emit`] 里，不混进来。
fn emit_plain(value: &Value, fmt: Format, args: &Args) -> Result<String, SmlError> {
    match fmt {
        // W16：走**带检查**的序列化 —— 深度超限要报 `E-LIMIT-004`，
        // 不能让 `to_sml` 静默写 `/* …深度超限… */ null` 占位文本（用户拿到的是
        // 看着合法、回读变成 null 的文档）。`to_sml` 本身仍是不失败的（见它的文档）。
        Format::Sml => sml::to_sml_checked(value),
        Format::Markdown => {
            let opt = MarkdownOptions {
                base: EmitOptions::default(),
                math: args.math,
                ..Default::default()
            };
            sml::emit::to_markdown(value, &opt)
        }
        // JSON：直接复用 crate 内既有的 `jsonify`（与 C-ABI 同款实现，零依赖、带转义），
        // 不再另写一份序列化，避免两处行为漂移。
        Format::Json => Ok(sml::jsonify(value)),
        Format::Toml => Ok(toml::to_toml(value)),
        // 高亮生成：输入是「定制声明」，输出是升级后的 tmLanguage（基线 + 增补）
        Format::TmLanguage => highlight::generate(value),
        // 定制包是一对多的（多文件、多目录），无法用「返回一个字符串」的 emit 表达，
        // 由 main 单独处理（见那里的 generate_package 分支）。
        // E-INTERNAL-001：走到了不应到达的分支（main 本应先拦截）。
        Format::Highlight => Err(SmlError::from_code_msg(E_INTERNAL_001)
            .with_detail("编辑器定制包由 main 单独处理（多文件输出）")),
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
            // E-CLI-003：依赖项缺失（`--to custom` 必须配 `--custom-rules`）。
            let rules_path = args.custom_rules.as_ref().ok_or_else(|| {
                SmlError::from_code_title(E_CLI_003)
                    .with_detail("`--to custom` 需要配合 `--custom-rules <file.sml>` 指定规则文档")
            })?;
            // E-IO-001：规则文件读取失败（与文档入口同格）。
            let rules_text = std::fs::read_to_string(rules_path).map_err(|e| {
                SmlError::from_code_msg(E_IO_001).with_detail(format!("{}: {e}", rules_path.display()))
            })?;
            // E-CLI-006：规则文档本身是 SML，解析失败的内层原因是 E-LEX-* / E-PARSE-*
            // （内层文案随消息一并保留，外层码换成「规则文档解析失败」）。
            let gen = parse(&rules_text).map_err(|e| {
                e.with_code(E_CLI_006)
                    .context(&format!("解析规则文档 {} 失败: ", rules_path.display()))
            })?;
            // E-EXT-006：规则文档非法（缺 rules / rules 为空 / 某条规则缺模板）——
            // 码由 `from_generator` 自己带上，这里原样透传。
            let opt = CustomOptions::from_generator(&gen)?;
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
) -> Result<(), SmlError> {
    // E-INTERNAL-001：走到了不应到达的分支（调用点应已保证 --hugo 存在）。
    let hugo_dir = args.hugo.as_ref().ok_or_else(|| {
        SmlError::from_code_msg(E_INTERNAL_001).with_detail("write_hugo called without --hugo")
    })?;

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
    // E-IO-003：写入文件或创建目录失败。
    std::fs::create_dir_all(&dest)
        .map_err(|e| SmlError::from_code_msg(E_IO_003).with_detail(format!("{}: {e}", dest.display())))?;
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
        .map_err(|e| SmlError::from_code_msg(E_IO_003).with_detail(format!("{}: {e}", out_path.display())))?;
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
) -> Result<(), SmlError> {
    // E-INTERNAL-001：走到了不应到达的分支（调用点应已保证 --zola 存在）。
    let zola_dir = args.zola.as_ref().ok_or_else(|| {
        SmlError::from_code_msg(E_INTERNAL_001).with_detail("write_zola called without --zola")
    })?;

    let inferred = infer_title(value, input_path, &args.title);
    let stem = sanitize_filename(&inferred);

    let mut dest = zola_dir.clone();
    if !args.zola_section.is_empty() {
        dest = dest.join(sanitize_section(&args.zola_section));
    }
    // E-IO-003：写入文件或创建目录失败。
    std::fs::create_dir_all(&dest)
        .map_err(|e| SmlError::from_code_msg(E_IO_003).with_detail(format!("{}: {e}", dest.display())))?;
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
        .map_err(|e| SmlError::from_code_msg(E_IO_003).with_detail(format!("{}: {e}", out_path.display())))?;
    eprintln!("wrote {}", out_path.display());

    // 可选：生成后调用 `zola build` 渲染静态站点。
    if args.zola_build {
        // E-IO-007：外部工具不可用（未安装或无法启动），或其构建失败。
        let zola = match which_zola() {
            Some(p) => p,
            None => {
                return Err(SmlError::from_code_msg(E_IO_007).with_detail(
                    "`--zola-build` 需要本机安装 `zola`（请先安装或加入 PATH）",
                ))
            }
        };
        eprintln!("running `zola build` in {}", zola_dir.display());
        let status = std::process::Command::new(&zola)
            .arg("build")
            .current_dir(zola_dir)
            .status()
            .map_err(|e| SmlError::from_code_msg(E_IO_007).with_detail(format!("{e}")))?;
        if !status.success() {
            return Err(SmlError::from_code_msg(E_IO_007)
                .with_detail(format!("zola build 失败（exit {:?}）", status.code())));
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
///    而 `@feature base` 只接受**版本集**（`10` / `1.0` / `v1` …，见 `docs/version-naming.md`），
///    本来也承载不了名字 —— 注释与实现各说各话。
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

/// `--dump-builtins`：把输入文档里的 `@contract` 定义导出成编辑器可读的 JSON。
///
/// schema 见 `docs/weakbind-spec-draft.md` §6.2（`schema` / `contracts[]` / 字段规格）。
/// **为什么不从 JSON 反向加载**：`Contract` 含 `Arc<dyn TypeCheck / Modifier>`，
/// 无法反序列化 ⇒ 内置集只能「用 SML 写 → 解析 → 导出/注入」。
fn dump_builtins(args: &Args) -> Result<(), SmlError> {
    let text = read_input(&args.input)?;
    let out = sml::parse_with(&text, sml::ParseOptions::new().capture_contracts(true))
        .map_err(|e| e.context("parse error: "))?;
    let table = out.contracts.unwrap_or_default();
    let json = builtins_json(&table);
    match &args.output {
        Some(p) => std::fs::write(p, json.as_bytes())
            .map_err(|e| SmlError::from_code_msg(E_IO_003).with_detail(format!("{}: {e}", p.display()))),
        None => {
            print!("{json}");
            Ok(())
        }
    }
}

/// JSON 字符串字面量（含控制字符转义）。
///
/// 不复用 `escape_front_matter`：那个是给 YAML/TOML 用的，控制字符走 `\u{..}`
/// （YAML 语法），写进 JSON 不合法。JSON 只认 `\uXXXX`（4 位、无花括号）。
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `TypeSpec` → JSON（schema 见草案 §6.2 的 `ty.kind` 表）。
fn builtins_ty_json(t: &sml::TypeSpec) -> String {
    use sml::TypeSpec as T;
    match t {
        T::Any => "{\"kind\":\"any\"}".to_string(),
        T::Str => "{\"kind\":\"str\"}".to_string(),
        T::Int => "{\"kind\":\"int\"}".to_string(),
        T::Num => "{\"kind\":\"num\"}".to_string(),
        T::Bool => "{\"kind\":\"bool\"}".to_string(),
        T::Array(inner) => format!("{{\"kind\":\"array\",\"of\":{}}}", builtins_ty_json(inner)),
        T::Enum(vals) => format!(
            "{{\"kind\":\"enum\",\"values\":[{}]}}",
            vals.iter().map(|v| json_str(v)).collect::<Vec<_>>().join(", ")
        ),
        T::ContractRef(n) => format!("{{\"kind\":\"contract\",\"name\":{}}}", json_str(n)),
        // 模式只导名字：`Pat` 没有稳定的源串表示，导出个「编译后的模式」对编辑器没用
        T::Pattern { name, .. } => format!("{{\"kind\":\"pattern\",\"name\":{}}}", json_str(name)),
        T::Ext(n) => format!("{{\"kind\":\"ext\",\"name\":{}}}", json_str(n)),
    }
}

/// 一条字段规格 → JSON。
fn builtins_field_json(name: &str, s: &sml::FieldSpec) -> String {
    let opt_num = |v: Option<f64>| v.map(|x| x.to_string()).unwrap_or_else(|| "null".to_string());
    format!(
        "{{ \"name\": {}, \"ty\": {}, \"required\": {}, \"default\": {}, \"min\": {}, \"max\": {}, \
         \"from_featag\": {}, \"ext\": {}, \"ext_mods\": [{}] }}",
        json_str(name),
        builtins_ty_json(&s.ty),
        s.required,
        s.default
            .as_ref()
            .map(sml::jsonify)
            .unwrap_or_else(|| "null".to_string()),
        opt_num(s.min),
        opt_num(s.max),
        s.from_featag,
        match &s.ext {
            Some(e) => json_str(e.name()),
            None => "null".to_string(),
        },
        s.mods.iter().map(|m| json_str(m.name())).collect::<Vec<_>>().join(", ")
    )
}

/// 整张内置契约表 → JSON（输出确定：契约名与字段名都按 `BTreeMap` 序）。
fn builtins_json(contracts: &std::collections::BTreeMap<String, sml::Contract>) -> String {
    let mut out = String::from("{\n  \"schema\": 1,\n  \"contracts\": [\n");
    for (i, (name, c)) in contracts.iter().enumerate() {
        if i > 0 {
            out.push_str(",\n");
        }
        let strict = match c.strictness {
            sml::Strictness::Strict => "strict",
            sml::Strictness::Warn => "warn",
            sml::Strictness::Loose => "loose",
        };
        let feats = c
            .featags
            .iter()
            .map(|f| {
                format!(
                    "{{ \"name\": {}, \"has_spec\": {}, \"target\": {} }}",
                    json_str(&f.name),
                    f.spec.is_some(),
                    f.target
                        .as_ref()
                        .map(|t| json_str(&format!("{t}")))
                        .unwrap_or_else(|| "null".to_string())
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str("    {\n");
        out.push_str(&format!("      \"name\": {},\n", json_str(name)));
        out.push_str(&format!("      \"strictness\": \"{strict}\",\n"));
        out.push_str(&format!(
            "      \"requires\": [{}],\n",
            c.requires.iter().map(|r| json_str(r)).collect::<Vec<_>>().join(", ")
        ));
        out.push_str(&format!("      \"allow_any\": {},\n", c.allow_any));
        out.push_str(&format!("      \"strict_paths\": {},\n", c.strict_paths));
        out.push_str(&format!("      \"featags\": [{feats}],\n"));
        out.push_str("      \"fields\": [\n");
        for (j, (fname, spec)) in c.fields.iter().enumerate() {
            if j > 0 {
                out.push_str(",\n");
            }
            out.push_str("        ");
            out.push_str(&builtins_field_json(fname, spec));
        }
        out.push_str("\n      ]\n    }");
    }
    out.push_str("\n  ]\n}\n");
    out
}

/// 读一个 `.smlb` 信封，把它的 `VALUE` 载荷还原成 SML 文本。
/// Read a `.smlb` envelope and render its `VALUE` payload back to SML text.
///
/// 字节信封（`EMBED` 主内容）会**明确报错**，而不是假装成空文档 —— 那是「拿错文件」与
/// 「文件是空的」两件事，混一起会让人查很久。
#[cfg(feature = "smlb")]
fn read_smlb_input(input: &Option<PathBuf>) -> Result<String, SmlError> {
    use std::io::Read as _;
    let bytes = match input {
        Some(p) => std::fs::read(p).map_err(|e| {
            // E-IO-001：读取失败（文档入口）。
            SmlError::from_code_msg(E_IO_001).with_detail(format!("{}: {e}", p.display()))
        })?,
        None => {
            let mut buf = Vec::new();
            std::io::stdin()
                .read_to_end(&mut buf)
                .map_err(|e| SmlError::from_code_msg(E_IO_001).with_detail(format!("stdin: {e}")))?;
            buf
        }
    };
    sml::smlb_to_sml(&bytes).map_err(|e| {
        SmlError::from_code_msg(E_IO_001).with_detail(format!("不是可读的 .smlb：{e}"))
    })
}

/// 没开 `smlb` feature 时的同名入口：明确说「本构建不支持」，而不是编译不过。
/// Without the `smlb` feature the option still parses — it just refuses, with a reason.
#[cfg(not(feature = "smlb"))]
fn read_smlb_input(_input: &Option<PathBuf>) -> Result<String, SmlError> {
    Err(SmlError::from_code_msg(E_CLI_003)
        .with_detail("本构建未启用 `smlb` feature，读不了 `.smlb` 信封"))
}

/// 把值写成 `.smlb` 信封（`--to-smlb`）。
/// Write the value as a `.smlb` envelope.
#[cfg(feature = "smlb")]
fn write_smlb_output(value: &sml::Value, args: &Args) -> ExitCode {
    let bytes = match sml::value_to_smlb(value, args.smlb_compress) {
        Ok(b) => b,
        Err(e) => {
            // E-CLI-007：输出后端报错。
            eprintln!("smltools: {e} [{}]", E_CLI_007);
            return ExitCode::from(2);
        }
    };
    match &args.output {
        Some(p) => match std::fs::write(p, &bytes) {
            Ok(()) => {
                eprintln!("wrote {}（{} B）", p.display(), bytes.len());
                ExitCode::SUCCESS
            }
            Err(e) => {
                // E-IO-003：写入文件失败。
                eprintln!("smltools: write {}: {e} [{}]", p.display(), E_IO_003);
                ExitCode::from(2)
            }
        },
        None => {
            let mut o = std::io::stdout().lock();
            let _ = o.write_all(&bytes);
            ExitCode::SUCCESS
        }
    }
}

/// 没开 `smlb` feature 时的 `--to-smlb`：明确拒绝。
#[cfg(not(feature = "smlb"))]
fn write_smlb_output(_value: &sml::Value, _args: &Args) -> ExitCode {
    eprintln!(
        "smltools: 本构建未启用 `smlb` feature，写不了 `.smlb` 信封（用 `--features smlb` 重新构建）[{}]",
        E_CLI_003
    );
    ExitCode::from(2)
}

/// `--resolve-profiles`：把 `@weakdoc` 的名字落到具体文件路径上。
///
/// **只在工具侧存在**：库不认识这些路径（`@weakdoc` 只记名字），所以
/// 「有 / 无 profile，解析结果完全一致」自动成立，不靠测试兜底。
fn resolve_profiles_mode(args: &Args, text: &str) -> Result<u8, SmlError> {
    let out = match (&args.input, &args.input_format) {
        (Some(p), InputFormat::Sml) => sml::parse_file_with(p, sml::ParseOptions::new()),
        _ => sml::parse_with(text, sml::ParseOptions::new()),
    }
    .map_err(|e| e.context("parse error: "))?;

    let roots = if args.no_profiles {
        Vec::new()
    } else {
        profile_search_roots(&args.input, args.profile_dir.as_ref())
    };

    let mut missing = 0u8;
    for wd in &out.weak_docs {
        let mut hit: Option<(&'static str, std::path::PathBuf)> = None;
        for (scope, dir) in &roots {
            let cand = dir.join(format!("{}.smlex", wd.name));
            if cand.is_file() {
                hit = Some((*scope, cand));
                break;
            }
        }
        match hit {
            Some((scope, path)) => println!("{}\t{}\t{}", wd.name, scope, path.display()),
            None => {
                missing = 1;
                println!("{}\t-\t(未找到)", wd.name);
            }
        }
    }
    Ok(missing)
}

/// 三命名空间的搜索根，**顺序即优先级**：
/// 显式 `--profile-dir` → 项目（自输入文件所在目录向上找 `.smlex/profiles`）→ 用户全局 → 系统。
///
/// 按名拼路径 => 同层不可能重名，因此不存在「同层冲突」这条分支。
fn profile_search_roots(
    input: &Option<PathBuf>,
    explicit: Option<&PathBuf>,
) -> Vec<(&'static str, PathBuf)> {
    let mut out: Vec<(&'static str, PathBuf)> = Vec::new();
    if let Some(d) = explicit {
        out.push(("explicit", d.clone()));
    }

    // 项目级：向上找 `.smlex/profiles`
    let cwd = std::env::current_dir().unwrap_or_default();
    let start = match input.as_ref().and_then(|p| p.parent()) {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => cwd,
    };
    let mut cur = start;
    loop {
        let cand = cur.join(".smlex").join("profiles");
        if cand.is_dir() {
            out.push(("project", cand));
            break;
        }
        let Some(parent) = cur.parent() else { break };
        if parent == cur {
            break;
        }
        cur = parent.to_path_buf();
    }

    // 用户全局
    if cfg!(windows) {
        if let Some(v) = std::env::var_os("APPDATA") {
            out.push(("user", PathBuf::from(v).join("sml").join("profiles")));
        }
    } else if let Some(v) = std::env::var_os("XDG_CONFIG_HOME") {
        out.push(("user", PathBuf::from(v).join("sml").join("profiles")));
    } else if let Some(v) = std::env::var_os("HOME") {
        out.push((
            "user",
            PathBuf::from(v).join(".config").join("sml").join("profiles"),
        ));
    }

    // 系统
    if cfg!(windows) {
        if let Some(v) = std::env::var_os("ProgramData") {
            out.push(("system", PathBuf::from(v).join("sml").join("profiles")));
        }
    } else {
        out.push(("system", PathBuf::from("/usr/share/sml/profiles")));
    }
    out
}

/// 执行文档所挂 profile 的 `checks`（**只在工具侧**：库不认识 profile）。
///
/// 三种情形分开对待：
/// - 三处命名空间都没命中 ⇒ **warning**（用户可能只是没装它）；
/// - 命中但加载失败/格式不对 ⇒ **error**（用户显式挂了它，规则不生效不能静默）；
/// - 加载成功 ⇒ 跑三项检查，命中即 error（项目自己定的硬规则）。
fn apply_profile_checks(report: &mut lint::Report, args: &Args, text: &str) {
    use std::collections::BTreeSet;

    if args.no_profiles {
        return;
    }
    // 文档本身有语法错时 lint 已经报过，profile 检查没有意义。
    let Ok(out) = sml::parse_with(text, sml::ParseOptions::new().capture_contracts(true)) else {
        return;
    };
    // 注意：**不能**在 `weak_docs` 为空时提前返回 —— 没被引用的 profile 也要按
    // `applies_to` 给提示，那正是「让作者发现还有这么一份 profile」的路径。
    let roots = profile_search_roots(&args.input, args.profile_dir.as_ref());
    let who = args
        .input
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<stdin>".to_string());

    // `forbid_contracts` 查定义侧：契约必须先定义才能应用，定义侧已覆盖来源。
    let mut contracts: BTreeSet<String> = BTreeSet::new();
    if let Some(cs) = &out.contracts {
        contracts.extend(cs.keys().cloned());
    }

    for wd in &out.weak_docs {
        let hit = roots.iter().find_map(|(scope, dir)| {
            let cand = dir.join(format!("{}.smlex", wd.name));
            if cand.is_file() {
                Some((*scope, cand))
            } else {
                None
            }
        });
        let Some((scope, path)) = hit else {
            report.messages.push(format!(
                "{who}: warning: 未找到 profile `{}`（三处命名空间都没命中，跳过它的 checks）[{}]",
                wd.name,
                sml_codes::E_LINT_005
            ));
            continue;
        };
        match profile::Profile::load(&path) {
            Ok(p) => {
                for (code, msg) in p.run(&out.value, &contracts) {
                    report.messages.push(format!(
                        "{who}: error: {msg}（profile: {scope} {}）[{code}]",
                        p.path.display()
                    ));
                    report.has_error = true;
                }
            }
            Err(m) => {
                report.messages.push(format!(
                    "{who}: error: profile 无法加载：{m} [{}]",
                    sml_codes::E_LINT_005
                ));
                report.has_error = true;
            }
        }
    }

    // 没被引用的 profile：按 `applies_to` 给一条**提示**（不自动挂，也不改退出码）。
    let referenced: BTreeSet<&str> = out.weak_docs.iter().map(|w| w.name.as_str()).collect();
    for (scope, dir) in &roots {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()) != Some("smlex") {
                continue;
            }
            let stem = p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();
            if referenced.contains(stem.as_str()) {
                continue;
            }
            let Ok(prof) = profile::Profile::load(&p) else {
                continue; // 没被引用又读不出来：不打扰（真正报错交给被引用的那条路径）
            };
            if prof.suggests_for(args.input.as_deref(), &contracts) {
                report.messages.push(format!(
                    "{who}: note: profile `{}` 的适用条件命中了本文档，但没有 `@weakdoc {}`（{scope} {}）[{}]",
                    prof.name,
                    prof.name,
                    p.display(),
                    sml_codes::I_LINT_001
                ));
            }
        }
    }
}

fn main() -> ExitCode {
    // 解析器是递归下降，承诺到 128 层（`MAX_VALUE_DEPTH`）；而 Windows 主线程栈
    // 默认只有 1 MB —— 深文档会在 parse 里直接爆栈（进程 abort，错误码都来不及打，
    // `--lint` 的 W-LINT-005 也到不了）。挪到**显式大栈线程**跑：递归深度由
    // 我们的限制决定，不由宿主线程的默认栈决定。
    // （实测：70 层嵌套在主线程即溢出 —— smltools/tests/error_codes.rs 的
    // `lint_deep_nesting_is_wlint_005` 钉着这条。）
    let handle = match std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(real_main)
    {
        Ok(h) => h,
        Err(_) => return ExitCode::from(1),
    };
    match handle.join() {
        Ok(code) => code,
        // 子线程 panic（含爆栈）：与原先主线程 panic 的退出码对齐
        Err(_) => std::process::exit(101),
    }
}

fn real_main() -> ExitCode {
    // E-CLI-008：命令行用法错误（未知参数 / 缺取值 / 非法枚举）。
    //
    // clap 自身的用法错误原先由 clap **直接打印并 exit 2、不带码**。这里用
    // `try_parse` 接管这层输出：在 clap 的消息上补码，但**不改写**它的用法提示
    // （那是有用的帮助文本），退出码仍是 2（与 E-CLI-001..007 一致）。
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        // `--help` / `--version` 也走 Err 通道，但那是**正常输出**（exit code 0）：
        // 原样打印到对应流，不能被当作用法错误处理。
        Err(e) if e.exit_code() == 0 => {
            let _ = e.print();
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            let err = SmlError::from_code_msg(E_CLI_008).with_detail(e.render().to_string());
            eprintln!("smltools: {err}");
            return ExitCode::from(2);
        }
    };

    let args = match parse_args(cli) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("smltools: {e}");
            return ExitCode::from(2);
        }
    };

    // 互斥 / 依赖校验（E-CLI-003：互斥项同时给出，或依赖项缺失）
    if args.zola_build && args.zola.is_none() {
        let e = SmlError::from_code_title(E_CLI_003)
            .with_detail("--zola-build 必须与 --zola <dir> 一起使用");
        eprintln!("smltools: {e}");
        return ExitCode::from(2);
    }
    if args.hugo.is_some() && args.zola.is_some() {
        let e = SmlError::from_code_title(E_CLI_003).with_detail("--hugo 与 --zola 互斥，请只选其一");
        eprintln!("smltools: {e}");
        return ExitCode::from(2);
    }

    // 内置契约表导出：只导数据、不做转换（与 `--to` 无关，故放在最前面短路）
    if args.dump_builtins {
        return match dump_builtins(&args) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("smltools: {e}");
                ExitCode::from(2)
            }
        };
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

    // 组合校验：信封是二进制，与「纯文本产物 / 纯文本检查」互斥（E-CLI-003）。
    if args.to_smlb && (args.hugo.is_some() || args.zola.is_some() || args.lint) {
        eprintln!(
            "smltools: --to-smlb 与 --hugo / --zola / --lint 互斥（后三者产出纯文本）[{}]",
            E_CLI_003
        );
        return ExitCode::from(2);
    }
    if args.from_smlb && args.input_format != InputFormat::Sml {
        eprintln!(
            "smltools: --from-smlb 时不能再指定 --from {}（信封自带格式）[{}]",
            args.input_format.name(),
            E_CLI_003
        );
        return ExitCode::from(2);
    }

    // `--from-smlb`：先解信封，再当 SML 文本往下走（后面的管线完全不变）
    let text = if args.from_smlb {
        match read_smlb_input(&args.input) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("smltools: {e}");
                return ExitCode::from(2);
            }
        }
    } else {
        match read_input(&args.input) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("smltools: {e}");
                return ExitCode::from(2);
            }
        }
    };

    // 弱 profile 引用解析：只做定位（`docs/weakdoc-spec-draft.md` §3）
    if args.resolve_profiles {
        return match resolve_profiles_mode(&args, &text) {
            Ok(code) => ExitCode::from(code),
            Err(e) => {
                eprintln!("smltools: {e}");
                ExitCode::from(2)
            }
        };
    }

    // lint 模式：只做静态检查，不产出转换结果
    if args.lint {
        if args.input_format != InputFormat::Sml {
            // E-CLI-004：lint 只能检查 SML 文档。
            eprintln!(
                "smltools: --lint 只能检查 SML 文档（当前 --from {}）[{}]",
                args.input_format.name(),
                E_CLI_004
            );
            return ExitCode::from(2);
        }
        let mut report = lint::check(&text, &args.input);
        // `.smlex` 的 checks：只在工具侧执行（`docs/smlex-format-draft.md` §3.2）。
        apply_profile_checks(&mut report, &args, &text);
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

    // 契约推断（任务 E）：从数据实例反推 `@contract` / JSON 字段表。
    // 与常规 emit 互斥 —— 命中即走此分支，不再进入下面的 emit/Hugo/Zola 逻辑。
    if args.infer_contract {
        let result = match sml::infer::infer_from_value("Inferred", &value) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("smltools: {e}");
                return ExitCode::from(2);
            }
        };
        for w in &result.warnings {
            eprintln!("smltools: note: {w}");
        }
        let rendered = if args.json {
            result.to_json()
        } else {
            result.to_sml()
        };
        return match &args.output {
            Some(p) => match std::fs::write(p, &rendered) {
                Ok(()) => {
                    eprintln!("wrote {}", p.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("smltools: write {}: {e} [{}]", p.display(), E_IO_003);
                    ExitCode::from(2)
                }
            },
            None => {
                let mut o = std::io::stdout().lock();
                let _ = o.write_all(rendered.as_bytes());
                let _ = o.write_all(b"\n");
                ExitCode::SUCCESS
            }
        };
    }

    // 编辑器定制包：一份输入 → 多份产物，按相对路径写到 -o 目录下（缺省当前目录）
    if args.format == Format::Highlight {
        let out_dir = args.output.clone().unwrap_or_else(|| PathBuf::from("."));
        let items = match highlight::generate_package(&value) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("smltools: {e}");
                return ExitCode::from(1);
            }
        };
        for it in &items {
            let p = out_dir.join(&it.path);
            if let Some(parent) = p.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    let e = SmlError::from_code_msg(E_IO_003).with_detail(format!("{}: {e}", parent.display()));
                    eprintln!("smltools: {e}");
                    return ExitCode::from(2);
                }
            }
            if let Err(e) = std::fs::write(&p, &it.content) {
                let e = SmlError::from_code_msg(E_IO_003).with_detail(format!("{}: {e}", p.display()));
                eprintln!("smltools: {e}");
                return ExitCode::from(2);
            }
            eprintln!("wrote {}", p.display());
        }
        return ExitCode::SUCCESS;
    }

    // `--to-smlb`：直接写二进制信封。**不走 emit** —— 那条管线是「文本进、文本出」。
    if args.to_smlb {
        return write_smlb_output(&value, &args);
    }

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
                    // E-IO-003：写入文件失败。
                    eprintln!(
                        "smltools: write {}: {e} [{}]",
                        p.display(),
                        E_IO_003
                    );
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一个深度 `levels` 的嵌套对象（用于触发后端的递归深度闸）。
    fn deep_value(levels: usize) -> Value {
        let mut v = Value::Str("x".to_string());
        for _ in 0..levels {
            let mut m = Map::new();
            m.insert("child".to_string(), v);
            v = Value::Object(m);
        }
        v
    }

    /// `--feature` 的字面量口径：数字 / 点号 / `vN` / **代号**都收，Dev 代也收。
    #[test]
    fn feature_arg_accepts_published_and_dev_spellings() {
        for s in ["4", "40", "4.0", "v4"] {
            assert_eq!(edition_arg(s).map(|e| e.name()).unwrap(), "4.0", "`{s}`");
        }
        // Dev 代：数字 / 点号 / `vN` / 代号 四种写法同一个版本集
        for s in ["50", "5.0", "v5", "camelia"] {
            assert_eq!(edition_arg(s).map(|e| e.name()).unwrap(), "5.0", "`{s}`");
        }
        // 最小可用集合仍在
        assert_eq!(edition_arg("v1").map(|e| e.name()).unwrap(), "1.0");
    }

    #[test]
    fn feature_arg_rejects_nonsense_with_004() {
        let e = edition_arg("nope").unwrap_err();
        assert_eq!(e.code(), sml_codes::E_FEATURE_004);
        assert!(format!("{e}").contains("camelia"), "文案应提示开发中的版本集：{e}");
    }

    /// ⚠️ 这组用例喂的是**真实的上游错误**（直接调用 `sml::emit::*` 让它自己报错），
    /// 而不是本文件手写的字面量 —— 因此它钉的是「码真的由 emit 后端带上」，
    /// 上游换码会让这里**响亮地**失败。
    ///
    /// 历史：本模块原先靠 `backend_error` 按**文案前缀**猜码，用例喂的是手写字面量，
    /// 所以上游一改文案，码会静默退化成兜底码而用例照样绿（W21 根治后已删除该函数）。
    #[test]
    fn emit_depth_error_carries_limit_004_from_upstream() {
        let e = sml::emit::to_markdown(&deep_value(200), &MarkdownOptions::new()).unwrap_err();
        assert_eq!(e.code(), sml_codes::E_LIMIT_004);
    }

    #[test]
    fn emit_custom_array_loop_carries_limit_006_from_upstream() {
        // 规则模板里带 `{items:...}`，待渲染值是一个超过 100000 元素的数组。
        let rules = sml::parse("rules: [ { match: \"big\" template: \"{items:{item}\\n}\" } ]\n")
            .expect("规则文档应可解析");
        let opt = CustomOptions::from_generator(&rules).expect("规则文档应合法");
        let mut m = Map::new();
        m.insert("big".to_string(), Value::Array(vec![Value::Int(1); 100_001]));
        let e = sml::emit::to_custom(&Value::Object(m), &opt).unwrap_err();
        assert_eq!(e.code(), sml_codes::E_LIMIT_006);
    }

    #[test]
    fn emit_custom_output_limit_carries_limit_005_from_upstream() {
        // 每层模板把 `{nested}` 展开 3 次 → 40 层递归足够撞上 8MiB 输出上限。
        let rules = sml::parse(
            "rules: [ { match: \"node\" template: \"<{value}>{nested}{nested}{nested}\" } ]\n",
        )
        .expect("规则文档应可解析");
        let opt = CustomOptions::from_generator(&rules).expect("规则文档应合法");
        let mut v = Value::Str("leaf".to_string());
        for _ in 0..40 {
            let mut m = Map::new();
            m.insert("__type".to_string(), Value::Str("node".to_string()));
            m.insert("child".to_string(), v);
            v = Value::Object(m);
        }
        let e = sml::emit::to_custom(&v, &opt).unwrap_err();
        assert_eq!(e.code(), sml_codes::E_LIMIT_005);
    }

    /// `from_generator` 的三条失败路径（缺 `rules` / `rules` 为空 / 某条规则缺 `template`）
    /// 都必须真的能触发，且都带 `E-EXT-006`。
    #[test]
    fn custom_rules_doc_error_carries_ext_006_from_upstream() {
        for src in [
            "norules: 1\n",                          // 缺 rules
            "rules: []\n",                           // rules 为空
            "rules: [ { match: \"x\" } ]\n",         // 某条规则缺 template
        ] {
            let gen = sml::parse(src).expect("文档应可解析");
            let e = CustomOptions::from_generator(&gen).unwrap_err();
            assert_eq!(e.code(), sml_codes::E_EXT_006, "输入：{src}");
        }
    }

    #[test]
    fn markdown_backend_error_carries_cli_007_from_upstream() {
        let v = sml::parse("table { rows: [ a ] }\n").expect("文档应可解析");
        let e = sml::emit::to_markdown(&v, &MarkdownOptions::new()).unwrap_err();
        assert_eq!(e.code(), sml_codes::E_CLI_007);
    }
}
