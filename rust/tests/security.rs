// SPDX-License-Identifier: MulanPSL-2.0
//! 安全回归测试
//!
//! 本文件覆盖 R-1 ~ R-8 的修复点。每个用例都对应一个可被构造的真实攻击载荷，
//! 若将来重构不慎回退了防护，这些用例会立刻失败。
//!
//! 构造约定：多数 emit 后端在**顶层**是按「字段名 → 子节点」遍历的，
//! 因此要触发某个块类型的分支（`__type`），需把该块放在**内层**包一层。

use std::collections::BTreeMap;

use sml::emit::{
    to_custom, to_latex, to_markdown, to_slint, to_svg, to_xml, CustomOptions, CustomRule,
    LatexOptions, MarkdownOptions, SlintOptions, SvgOptions, XmlOptions,
};
use sml::{compile_regex, parse_with_features, regex_matches, Feature, FeatureSet, Value};

fn obj(pairs: &[(&str, Value)]) -> Value {
    let mut m = BTreeMap::new();
    for (k, v) in pairs {
        m.insert(k.to_string(), v.clone());
    }
    Value::Object(m)
}

fn s(v: &str) -> Value {
    Value::Str(v.to_string())
}

/// 拆分 Markdown 图片语法 `![ALT](SRC "TITLE")` 为三段。
///
/// 按**结构**断言（而不是在整个输出里搜危险子串）才能避免假阳性：
/// 同一个 `javascript:` 落在 alt 里只是无害文本，落在 src 里才是攻击。
/// 若输入不是图片语法会 panic —— 这本身就是「未进入 img 分支」的信号。
fn split_img(md: &str) -> (String, String, String) {
    let t = md.trim();
    assert!(t.starts_with("!["), "未进入 img 分支，输出为：{md}");
    let rest = &t[2..];
    let (alt, tail) = rest.split_once("](").expect("图片语法缺少 `](`");
    let tail = tail.trim_end();
    // 去掉结尾的 `)`，剩余形如 `a.png` 或 `a.png "title"`
    let tail = tail.strip_suffix(')').unwrap_or(tail);
    match tail.split_once(" \"") {
        Some((src, title)) => (
            alt.to_string(),
            src.to_string(),
            title.strip_suffix('"').unwrap_or(title).to_string(),
        ),
        None => (alt.to_string(), tail.to_string(), String::new()),
    }
}

// ---------------------------------------------------------------------------
// R-1：$env 特性绕过
// ---------------------------------------------------------------------------

/// 调用方禁用 `env` 特性后，**引号串**里的 `$env.X` 也必须被拒绝，
/// 否则文档可绕过特性白名单读取任意环境变量。
#[test]
fn env_feature_cannot_be_bypassed_via_quoted_string() {
    let allowed = FeatureSet::baseline().without(Feature::Env);

    // 裸词形态：一贯被拒绝
    assert!(parse_with_features("a: $env.PATH\n", allowed).is_err());
    // 引号串形态：修复前可通过，修复后必须同样被拒绝
    let r = parse_with_features("a: \"$env.PATH\"\n", allowed);
    assert!(r.is_err(), "引号串 `$env.X` 绕过了 env 特性限制：{r:?}");
}

/// 允许 env 时，引号串 `$env.X` 仍应正常内联（不能因为加固而破坏功能）。
#[test]
fn env_inline_still_works_when_enabled() {
    let mut env = BTreeMap::new();
    env.insert("SML_SEC_TEST_VAR".to_string(), "hello".to_string());
    let (v, _) = sml::parse_with_features_env(
        "a: \"$env.SML_SEC_TEST_VAR\"\nb: $env.SML_SEC_TEST_VAR\n",
        FeatureSet::baseline(),
        env,
    )
    .unwrap();
    assert_eq!(v.get("a"), Some(&s("hello")));
    assert_eq!(v.get("b"), Some(&s("hello")));
}

/// 环境变量**覆盖表**只作用于本次解析，不写入进程环境。
#[test]
fn env_overrides_do_not_leak_into_process_env() {
    let mut env = BTreeMap::new();
    env.insert("SML_SEC_OVERRIDE".to_string(), "injected".to_string());
    let (v, _) = sml::parse_with_features_env(
        "a: $env.SML_SEC_OVERRIDE\n",
        FeatureSet::baseline(),
        env,
    )
    .unwrap();
    assert_eq!(v.get("a"), Some(&s("injected")));
    // 进程环境里不应出现该变量
    assert!(std::env::var("SML_SEC_OVERRIDE").is_err());
}

/// 覆盖表**遮蔽**同名进程环境变量。
#[test]
fn env_override_shadows_process_env() {
    let mut env = BTreeMap::new();
    env.insert("SML_SEC_SHADOW".to_string(), "override".to_string());
    let (v, _) = sml::parse_with_features_env(
        "a: $env.SML_SEC_SHADOW\n",
        FeatureSet::baseline(),
        env,
    )
    .unwrap();
    assert_eq!(v.get("a"), Some(&s("override")));
}

// ---------------------------------------------------------------------------
// R-2：Markdown img 的 alt / title 注入
// ---------------------------------------------------------------------------

#[test]
fn markdown_img_alt_and_title_are_sanitized() {
    // 必须**包一层**：to_markdown 把顶层对象当文档根按字段遍历，
    // 只有内层的 `__type` 才会进入 img 分支。
    let v = obj(&[(
        "doc",
        obj(&[
            ("__type", s("img")),
            ("src", s("a.png")),
            ("alt", s("x\" onerror=\"alert(1)")),
            ("title", s("t\" onload=\"alert(2)")),
        ]),
    )]);
    let md = to_markdown(&v, &MarkdownOptions::new()).unwrap();
    // 确认确实进入了 img 分支（否则下面的断言是假阳性）
    let (alt, src, title) = split_img(&md);
    assert_eq!(src, "a.png", "src 被篡改：{md}");
    // 核心判定：alt / title **不含裸双引号**。渲染为 `<img alt="…" title="…">`
    // 时，只有裸引号才能闭合属性并追加事件处理器（`&quot;` 是属性内的字面引号，
    // 由 HTML 解析器解码，不会闭合属性）。
    assert!(
        !alt.contains('"'),
        "alt 含未转义引号，可闭合 HTML 属性：{md}"
    );
    assert!(
        !title.contains('"'),
        "title 含未转义引号，可闭合 HTML 属性：{md}"
    );
    // 载荷文本本身保留（`onerror=…` 作为 alt/title 的**字面内容**是无害的，
    // 它落在属性值内部，不会成为 HTML 属性）——真正的安全边界是「无裸引号」。
    // 这里验证注入用的引号确实被清除/实体化：
    assert!(
        alt.contains("&quot;") && !alt.contains("=\""),
        "alt 中的引号未实体化：{md}"
    );
}

#[test]
fn markdown_img_alt_cannot_break_out_of_syntax() {
    let v = obj(&[(
        "doc",
        obj(&[
            ("__type", s("img")),
            ("src", s("a.png")),
            ("alt", s("x](javascript:alert(1) \"y")),
        ]),
    )]);
    let md = to_markdown(&v, &MarkdownOptions::new()).unwrap();
    let (alt, src, _title) = split_img(&md);
    // 关键：`](` 在源码中只能出现一次（作为 alt→src 的分隔）。
    // alt 中的 `]` 已转义为 `&#93;`，无法伪造出第二个链接目标。
    assert_eq!(
        md.matches("](").count(),
        1,
        "alt 通过 `](` 重开了链接语法：{md}"
    );
    // src 必须仍是原本的 a.png，不能被 alt 里的载荷顶替成 javascript:
    assert_eq!(src, "a.png", "src 被 alt 顶替：{md}");
    assert!(
        !src.contains("javascript:"),
        "javascript: 落入 src 位置：{md}"
    );
    // alt 中的载荷已失去结构意义（`[`/`]`/引号均被实体化）
    assert!(alt.contains("&#93;"), "alt 中的 `]` 未实体化：{md}");
    assert!(!alt.contains('"'), "alt 含裸引号：{md}");
}

// ---------------------------------------------------------------------------
// R-4：Markdown HTML 透传
// ---------------------------------------------------------------------------

#[test]
fn markdown_html_passthrough_rejects_script_tag() {
    let v = obj(&[("doc", obj(&[("__type", s("script")), ("text", s("alert(1)"))]))]);
    let mut opt = MarkdownOptions::new();
    opt.html_passthrough = true;
    // 危险标签应被拒绝（返回 Err），而不是产出可执行 HTML
    assert!(
        to_markdown(&v, &opt).is_err(),
        "透传模式放行了 <script> 标签"
    );
}

#[test]
fn markdown_html_passthrough_escapes_body() {
    let v = obj(&[(
        "doc",
        obj(&[
            ("__type", s("div")),
            ("text", s("<script>alert(1)</script>")),
        ]),
    )]);
    let mut opt = MarkdownOptions::new();
    opt.html_passthrough = true;
    let md = to_markdown(&v, &opt).unwrap();
    assert!(
        !md.contains("<script>"),
        "透传标签体未转义，可注入任意 HTML：{md}"
    );
}

// ---------------------------------------------------------------------------
// R-3：SVG / XML 事件属性与 URI scheme
// ---------------------------------------------------------------------------

#[test]
fn svg_drops_event_handler_attrs() {
    let v = obj(&[(
        "root",
        obj(&[
            ("__type", s("svg")),
            ("onload", s("alert(1)")),
            ("onclick", s("alert(2)")),
        ]),
    )]);
    let out = to_svg(&v, &SvgOptions::default()).unwrap();
    assert!(
        !out.contains("onload") && !out.contains("onclick"),
        "SVG 输出仍含事件处理器属性：{out}"
    );
}

#[test]
fn svg_blocks_javascript_uri() {
    let v = obj(&[(
        "root",
        obj(&[("__type", s("a")), ("href", s("javascript:alert(1)"))]),
    )]);
    let out = to_svg(&v, &SvgOptions::default()).unwrap();
    assert!(
        !out.contains("javascript:"),
        "SVG href 未过滤 javascript: scheme：{out}"
    );
}

#[test]
fn svg_keeps_safe_relative_href() {
    let v = obj(&[(
        "root",
        obj(&[("__type", s("a")), ("href", s("#target"))]),
    )]);
    let out = to_svg(&v, &SvgOptions::default()).unwrap();
    assert!(out.contains("href=\"#target\""), "安全相对引用被误杀：{out}");
}

#[test]
fn xml_drops_event_handler_attrs_and_bad_uri() {
    let v = obj(&[(
        "root",
        obj(&[
            ("__type", s("node")),
            ("onerror", s("alert(1)")),
            ("src", s("file:///etc/passwd")),
            ("href", s("https://example.com/ok")),
        ]),
    )]);
    let out = to_xml(&v, &XmlOptions::default()).unwrap();
    assert!(!out.contains("onerror"), "XML 输出含事件属性：{out}");
    assert!(!out.contains("file:///etc/passwd"), "XML src 未过滤：{out}");
    assert!(
        out.contains("https://example.com/ok"),
        "安全 URL 被误杀：{out}"
    );
}

// ---------------------------------------------------------------------------
// R-6：custom 后端输出放大 DoS
// ---------------------------------------------------------------------------

/// 模板里 `{nested}` 出现多次时，递归渲染的输出会指数放大。
/// 一个小输入 + 深嵌套必须被输出长度上限挡住，而不是 OOM。
#[test]
fn custom_output_amplification_is_capped() {
    // 构造 40 层深的嵌套对象
    let mut v = obj(&[("__type", s("node")), ("text", s("leaf"))]);
    for _ in 0..40 {
        v = obj(&[("__type", s("node")), ("child", v)]);
    }
    let opt = CustomOptions {
        base: Default::default(),
        // 每层输出自身并递归 3 遍子树 → 3^40 倍放大
        rules: vec![CustomRule {
            match_type: Some("node".to_string()),
            match_key: None,
            template: "<{value}>{nested}{nested}{nested}".to_string(),
        }],
        exclude: Default::default(),
        include_only: None,
    };
    let r = to_custom(&v, &opt);
    assert!(
        r.is_err(),
        "输出放大未被上限阻断，实际输出 {} 字节",
        r.as_ref().map(|x| x.len()).unwrap_or(0)
    );
}

/// 正常（不放大的）模板不应被上限误伤。
#[test]
fn custom_normal_template_still_works() {
    let v = obj(&[("__type", s("node")), ("text", s("hi"))]);
    let opt = CustomOptions {
        base: Default::default(),
        rules: vec![CustomRule {
            match_type: Some("node".to_string()),
            match_key: None,
            template: "<{value}/>".to_string(),
        }],
        exclude: Default::default(),
        include_only: None,
    };
    assert_eq!(to_custom(&v, &opt).unwrap(), "<hi/>");
}

// ---------------------------------------------------------------------------
// R-7：LaTeX 注入
// ---------------------------------------------------------------------------

#[test]
fn latex_documentclass_cannot_inject_preamble() {
    let v = obj(&[("doc", obj(&[("__type", s("p")), ("text", s("hello"))]))]);
    let mut opt = LatexOptions::default();
    // 试图用 `}` 闭合 \documentclass{} 后追加 \write18
    opt.documentclass = "article}\\write18{id}\\documentclass{".to_string();
    let out = to_latex(&v, &opt).unwrap();
    assert!(
        !out.contains("\\write18"),
        "documentclass 注入了 preamble 代码：{out}"
    );
}

#[test]
fn latex_verbatim_end_is_neutralized() {
    let body = "safe text\n\\end{verbatim}\\write18{id}\\begin{verbatim}";
    let v = obj(&[(
        "doc",
        obj(&[("__type", s("code")), ("text", s(body))]),
    )]);
    let out = to_latex(&v, &LatexOptions::default()).unwrap();
    assert!(
        !out.contains("\\end{verbatim}\\write18"),
        "verbatim 结束标记未被中和，内容逃逸到文档顶层：{out}"
    );
}

#[test]
fn latex_math_rejects_dangerous_primitives() {
    let v = obj(&[(
        "doc",
        obj(&[("__type", s("math")), ("text", s("x + \\input{/etc/passwd}"))]),
    )]);
    let mut opt = LatexOptions::default();
    opt.math = true;
    assert!(
        to_latex(&v, &opt).is_err(),
        "数学块放行了 \\input 等危险原语"
    );
}

#[test]
fn latex_math_accepts_plain_formula() {
    let v = obj(&[(
        "doc",
        obj(&[("__type", s("math")), ("text", s("E = mc^2"))]),
    )]);
    let mut opt = LatexOptions::default();
    opt.math = true;
    let out = to_latex(&v, &opt).unwrap();
    assert!(out.contains("$E = mc^2$"), "正常公式被误杀：{out}");
}

// ---------------------------------------------------------------------------
// R-9：Markdown 代码块 info string（lang）注入
//
// 背景：`code_fence` 原先只按**代码体**计算围栏长度，同一条语句里的 `lang`
// 却被原样拼接。Markdown 的 info string 只能是单行标识符，带换行的 lang 会
// 提前结束围栏起始行，把 `# 标题` / `<script>` 泄到代码块之外（XSS）。
// 这是此前「P1-3 围栏逃逸」修复的残留面——当时只补了 body 侧。
// ---------------------------------------------------------------------------

/// custom emit 规则模板：`{key}={value}` 之外**必须**带 `{nested}`，
/// 否则对象的子字段根本不会被渲染，无法用于验证「同对象其它字段是否受 exclude 影响」。
const CUSTOM_TPL: &str = "rules: [ { match: \"*\" template: \"{key}={value}{nested}\" } ]";

/// 取出 Markdown 输出中**第一个**围栏代码块的（info string, 代码体）。
///
/// 按结构断言而非全文搜子串：只有「落在围栏之外」的注入才是漏洞，
/// 落在代码体里的 `# x` 只是无害文本。
fn first_code_block(md: &str) -> (String, Vec<String>) {
    let lines: Vec<&str> = md.lines().collect();
    let open = lines
        .iter()
        .position(|l| l.trim_start().starts_with("```"))
        .unwrap_or_else(|| panic!("未输出代码围栏：{md}"));
    let rel = lines[open + 1..]
        .iter()
        .position(|l| l.trim_start().starts_with("```"))
        .unwrap_or_else(|| panic!("围栏未闭合，内容逃逸到代码块之外：{md}"));
    let close = open + 1 + rel;
    let info = lines[open].trim_start().trim_start_matches('`').to_string();
    let body: Vec<String> = lines[open + 1..close].iter().map(|l| l.to_string()).collect();
    (info, body)
}

#[test]
fn markdown_code_lang_cannot_escape_fence() {
    // 端到端：攻击载荷写在真实 SML 文本里（\n 为 SML 字符串转义）。
    // `code` 必须是**顶层**对象才会走 markdown 的代码块分支。
    let src = "code { lang: \"js\\n\\n# INJECTED\\n\\n<script>alert(1)</script>\" text: \"body\" }";
    let (v, _feats) = parse_with_features(src, FeatureSet::all()).expect("解析失败");
    let md = to_markdown(&v, &MarkdownOptions::default()).unwrap();

    let (info, body) = first_code_block(&md);
    // 注入内容既不能出现在 info string（会进 class 属性），
    // 更不能成为围栏之外的正文——后者正是本次修复的核心。
    for bad in ["<script>", "alert(1)", "# INJECTED", "\n"] {
        assert!(!info.contains(bad), "lang 残留危险内容 {bad:?}：{info:?}");
    }
    assert_eq!(body, vec!["body".to_string()], "代码体被污染：{body:?}");
}

#[test]
fn markdown_code_lang_keeps_normal_language() {
    // 对照：正常语言标注不得被误杀（含 `+` `#` `.` `-` 的标识符）
    for lang in ["rust", "c++", "csharp", "objective-c", "f#", "python3.11"] {
        let v = obj(&[(
            "doc",
            obj(&[("__type", s("code")), ("lang", s(lang)), ("text", s("x"))]),
        )]);
        let md = to_markdown(&v, &MarkdownOptions::default()).unwrap();
        let (info, _) = first_code_block(&md);
        assert_eq!(info, lang, "正常语言标注被清洗破坏：{info:?}");
    }
}

#[test]
fn markdown_code_body_fence_still_grows() {
    // 对照：body 侧围栏长度自适应（既有防护不得回退）。同样需顶层 code。
    let v = obj(&[("__type", s("code")), ("text", s("x\n```\n# ESCAPED\n```"))]);
    let md = to_markdown(&v, &MarkdownOptions::default()).unwrap();
    let (_, body) = first_code_block(&md);
    assert!(
        body.contains(&"# ESCAPED".to_string()),
        "body 侧围栏未生效，正常内容丢失：{body:?}"
    );
}

// ---------------------------------------------------------------------------
// R-10：Markdown 表格单元格结构字符
//
// `escape_text` 只有 XML 语义（& < > " '），不认识 Markdown 的表格定界符：
// `|` 会伪造额外列，换行会伪造额外行。二者都能让不可信数据「看起来像」
// 表头或另一条记录，属于数据呈现欺骗。
// ---------------------------------------------------------------------------

fn table_rows(md: &str) -> Vec<String> {
    md.lines()
        .filter(|l| l.trim_start().starts_with('|'))
        .map(|l| l.trim().to_string())
        .collect()
}

#[test]
fn markdown_table_cell_cannot_forge_columns() {
    let v = obj(&[(
        "doc",
        obj(&[
            ("__type", s("table")),
            ("header", Value::Array(vec![s("A"), s("B")])),
            (
                "rows",
                Value::Array(vec![Value::Array(vec![s("x|y|z"), s("q")])]),
            ),
        ]),
    )]);
    let md = to_markdown(&v, &MarkdownOptions::default()).unwrap();
    let rows = table_rows(&md);
    // 表头 + 分隔 + 1 行数据，每行都必须是 2 列
    assert_eq!(rows.len(), 3, "行数被伪造：{rows:?}");
    for r in &rows {
        // `|` 已被转义为 `\|`，故不再产生额外列分隔
        let cols = r.trim_matches('|').split(" | ").count();
        assert_eq!(cols, 2, "列数被伪造：{r:?}");
    }
    assert!(
        rows[2].contains(r"x\|y\|z"),
        "单元格内的 | 未转义：{:?}",
        rows[2]
    );
}

#[test]
fn markdown_table_cell_cannot_forge_rows() {
    let v = obj(&[(
        "doc",
        obj(&[
            ("__type", s("table")),
            ("header", Value::Array(vec![s("A"), s("B")])),
            (
                "rows",
                Value::Array(vec![Value::Array(vec![s("x\n| evil | evil |"), s("q")])]),
            ),
        ]),
    )]);
    let md = to_markdown(&v, &MarkdownOptions::default()).unwrap();
    let rows = table_rows(&md);
    assert_eq!(rows.len(), 3, "换行伪造了额外表格行：{rows:?}");
}

#[test]
fn markdown_table_keeps_normal_cells() {
    let v = obj(&[(
        "doc",
        obj(&[
            ("__type", s("table")),
            ("header", Value::Array(vec![s("name"), s("note")])),
            (
                "rows",
                Value::Array(vec![Value::Array(vec![s("a & b"), s("<tag>")])]),
            ),
        ]),
    )]);
    let md = to_markdown(&v, &MarkdownOptions::default()).unwrap();
    let rows = table_rows(&md);
    assert_eq!(rows.len(), 3, "正常表格行数异常：{rows:?}");
    assert!(rows[2].contains("a &amp; b"), "XML 转义丢失：{:?}", rows[2]);
    assert!(rows[2].contains("&lt;tag&gt;"), "XML 转义丢失：{:?}", rows[2]);
}

// ---------------------------------------------------------------------------
// R-11：Markdown 字段名注入
//
// 字段名被原样拼进 `### name` 与 `- **key**: value`。字段名同样可能来自
// 不可信输入，其中的换行会逃逸出当前块（伪造标题/新列表项），
// `*` `_` 等会破坏 `**key**` 的定界。
// ---------------------------------------------------------------------------

#[test]
fn markdown_field_name_cannot_forge_heading() {
    let src = "doc { cfg { \"a**\\n\\n# INJECTED\\n\\n- \": v } }";
    let (v, _feats) = parse_with_features(src, FeatureSet::all()).expect("解析失败");
    let md = to_markdown(&v, &MarkdownOptions::default()).unwrap();

    // 判据：不得存在「只由 `#` 开头的行」（伪造的标题）
    for line in md.lines() {
        assert!(
            !line.trim_start().starts_with("# "),
            "字段名伪造了 Markdown 标题：{md}"
        );
    }
    assert!(!md.contains("\n\n\n"), "字段名制造了异常的空行块：{md}");
}

#[test]
fn markdown_field_name_keeps_normal_keys() {
    // 对照：常规字段名（含下划线等）不得被破坏
    let v = obj(&[(
        "cfg",
        obj(&[("max_retries", Value::Int(3)), ("a.b", Value::Int(1))]),
    )]);
    let md = to_markdown(&v, &MarkdownOptions::default()).unwrap();
    assert!(md.contains("max_retries"), "常规字段名被破坏：{md}");
    assert!(md.contains("a.b"), "常规字段名被破坏：{md}");
}

// ---------------------------------------------------------------------------
// R-12：正则 include 的 ReDoS 步数预算
//
// 预算若是 `backtrack_match` 的局部变量，则**每个起始位置都能重新拿到**
// 全额 2M 步，实际总开销 = 起点数 × 2M。而 glob/regex include 会对目录中
// 每个文件名调用一次 `regex_matches`，恶意模式足以挂死整个解析。
// 修复后预算由 `regex_matches` 统一持有并跨起点共享。
// ---------------------------------------------------------------------------

#[test]
fn regex_step_budget_is_shared_across_start_positions() {
    // 恶意模式：嵌套量词 `a*a*...*b`，文本为纯 a（永不匹配）。
    // 修复前：100 个起点 × 每起点 2M 步 → 数十秒级；修复后：总计 2M 步 → 亚秒级。
    let re = compile_regex(&("a*".repeat(20) + "b"));
    let text = "a".repeat(100);

    let t = std::time::Instant::now();
    let matched = regex_matches(&re, &text);
    let elapsed = t.elapsed();

    assert!(!matched, "纯 a 文本不应匹配 `a*b`");
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "ReDoS 预算未按起点共享，耗时 {elapsed:?}（修复前为数十秒级）"
    );
}

#[test]
fn regex_still_matches_correctly() {
    // 对照：共享预算不得破坏正常匹配语义。
    // MiniRegex 的语法边界：
    //   - `\x` 只做**字面**转义，不支持 `\d` 等字符类简写；
    //   - 量词 `+` `?` `*` 作用于**前一个原子**（普通字符 / `.` / `[a-z]` 字符类），
    //     不支持分组 `(...)` 与 `{m,n}`。
    let re = compile_regex(r"^widget_[0-9][0-9]\.sml$");
    assert!(regex_matches(&re, "widget_01.sml"), "正常文件名应匹配");
    assert!(!regex_matches(&re, "widget_x1.sml"), "字符类范围误匹配");
    assert!(!regex_matches(&re, "other_01.sml"), "不应匹配其它前缀");

    // 非锚定：子串匹配
    let re2 = compile_regex(r"\.sml$");
    assert!(regex_matches(&re2, "a/b/c.sml"), "非锚定结尾匹配失效");
    assert!(!regex_matches(&re2, "a/b/c.txt"), "非锚定结尾误匹配");

    // `+` = 一个或多个。
    // 2026-09-18 修复量词 off-by-one：修复前原子被强制消费一次、量词只管**额外**次数，
    // 故 `x+` 等价 `xx*`（`^ab+c$` 匹配 `abbc` 却不匹配 `abc`）。
    // 本测试此前按旧行为断言并注明「锁定现状」，现已改为正确语义。
    let re3 = compile_regex(r"^ab+c$");
    assert!(regex_matches(&re3, "abc"), "`+` 应接受恰好一次");
    assert!(regex_matches(&re3, "abbc"), "`+` 多次匹配失效");
    assert!(!regex_matches(&re3, "ac"), "`+` 不应匹配零次");
    assert!(regex_matches(&re3, "abbbc"), "`+` 应吃下连续多个 b");
    assert!(!regex_matches(&re3, "abxbc"), "`+` 不应跨过非 b 字符");

    // `?` = 零或一次、`*` = 零或多次（与 `+` 同一次修复）
    assert!(regex_matches(&compile_regex(r"^ab?c$"), "ac"), "`?` 应允许零次");
    assert!(regex_matches(&compile_regex(r"^ab?c$"), "abc"), "`?` 应允许一次");
    assert!(
        !regex_matches(&compile_regex(r"^ab?c$"), "abbc"),
        "`?` 不应允许两次"
    );
    assert!(regex_matches(&compile_regex(r"^a*b$"), "b"), "`*` 应允许零次");
    assert!(
        !regex_matches(&compile_regex(r"^a*b$"), "aaac"),
        "`*` 不应吃下非 a 字符"
    );

    // 量词现在也作用于字符类（修复前 `[0-9]+` 会失效）
    assert!(
        regex_matches(&compile_regex(r"^[0-9]+$"), "01"),
        "`+` 应作用于字符类"
    );
    assert!(
        !regex_matches(&compile_regex(r"^[0-9]+$"), "1a"),
        "`+` 不应吃下类外字符"
    );

    // `^` 与 `$` 同时出现时两端都要卡住。
    // 同一次修复翻出的第二个缺陷：修复前只判「能匹配」，`^conf\.sml$` 会匹配
    // `conf.sml.bak`（前缀匹配），对按文件名做 include 过滤的场景是危险的松判。
    let re6 = compile_regex(r"^conf\.sml$");
    assert!(regex_matches(&re6, "conf.sml"), "正常文件名应匹配");
    assert!(
        !regex_matches(&re6, "conf.sml.bak"),
        "`$` 在 `^` 同时存在时也必须生效（修复前会误匹配）"
    );
    assert!(!regex_matches(&re6, "x.conf.sml"), "`^` 仍须从头匹配");

    // 字符类范围与取反
    let re4 = compile_regex(r"^[a-c][a-c]\.txt$");
    assert!(regex_matches(&re4, "ab.txt"), "字符类匹配失效");
    assert!(!regex_matches(&re4, "ad.txt"), "字符类范围误匹配");
    assert!(regex_matches(&compile_regex(r"^[^0-9]+$"), "abc"), "取反类失效");
    assert!(!regex_matches(&compile_regex(r"^[^0-9]+$"), "a1"), "取反类误匹配");

    // `*` 与 `.`
    let re5 = compile_regex(r"^a.*z$");
    assert!(regex_matches(&re5, "abcz"), "`*` + `.` 匹配失效");
    assert!(!regex_matches(&re5, "abcy"), "`*` 结尾误匹配");
}

// ---------------------------------------------------------------------------
// R-13：Slint 字符串字面量换行
//
// 转义只覆盖 `\` 与 `"`，字面换行会产出 Slint 无法编译的跨行字符串。
// （`"` 已转义，故不构成注入；此处修复的是语法破坏。）
// ---------------------------------------------------------------------------

#[test]
fn slint_string_value_escapes_newlines() {
    let v = obj(&[(
        "app",
        obj(&[
            ("__type", s("component")),
            ("name", s("App")),
            ("label", s("line1\nline2\ttab")),
        ]),
    )]);
    let out = to_slint(&v, &SlintOptions::default()).unwrap();
    assert!(
        out.contains(r#"label: "line1\nline2\ttab";"#),
        "Slint 字符串字面量未转义换行/制表符：{out}"
    );
    // 断言该属性确实是单行的（跨行即编译失败）
    let label_line = out.lines().find(|l| l.contains("label:")).unwrap();
    assert!(label_line.trim_end().ends_with(';'), "属性未单行终结：{out}");
}

#[test]
fn slint_string_value_keeps_normal_text() {
    let v = obj(&[(
        "app",
        obj(&[
            ("__type", s("component")),
            ("name", s("App")),
            ("label", s(r#"say "hi""#)),
        ]),
    )]);
    let out = to_slint(&v, &SlintOptions::default()).unwrap();
    assert!(out.contains(r#""say \"hi\"""#), "引号转义丢失：{out}");
}

// ---------------------------------------------------------------------------
// R-14：custom emit 的 exclude 过滤被 `text` 字段绕过
//
// 子节点遍历处已按 `field_allowed` 过滤 `text`，但填充 `{value}` 时又
// 直接 `v.get("text")` 取回，使 `exclude: ["text"]` 形同虚设。
// ---------------------------------------------------------------------------

#[test]
fn custom_exclude_applies_to_text_field() {
    let gen = sml::parse(CUSTOM_TPL).unwrap();
    let opt = CustomOptions::from_generator(&gen)
        .unwrap()
        .exclude_fields(&["text"]);

    let v = obj(&[("secret", obj(&[("text", s("SUPER_SECRET"))]))]);
    let out = to_custom(&v, &opt).unwrap();
    assert!(
        !out.contains("SUPER_SECRET"),
        "exclude:[\"text\"] 被绕过，敏感字段泄漏：{out}"
    );
}

#[test]
fn custom_exclude_still_allows_other_fields() {
    // 对照：只排除 text 时，同对象的其它字段应照常渲染
    let gen = sml::parse(CUSTOM_TPL).unwrap();
    let opt = CustomOptions::from_generator(&gen)
        .unwrap()
        .exclude_fields(&["text"]);

    let v = obj(&[("rec", obj(&[("text", s("HIDDEN")), ("name", s("VISIBLE"))]))]);
    let out = to_custom(&v, &opt).unwrap();
    assert!(!out.contains("HIDDEN"), "text 未被排除：{out}");
    assert!(out.contains("VISIBLE"), "同对象其它字段被误杀：{out}");
}

#[test]
fn custom_include_only_applies_to_text_field() {
    // 白名单方向同样不得被绕过。
    // 白名单须含父键 `rec`，否则整棵子树在入口即被裁掉，测不到 text 绕过。
    let gen = sml::parse(CUSTOM_TPL).unwrap();
    let opt = CustomOptions::from_generator(&gen)
        .unwrap()
        .include_fields(&["rec", "name"]);

    let v = obj(&[("rec", obj(&[("text", s("HIDDEN")), ("name", s("VISIBLE"))]))]);
    let out = to_custom(&v, &opt).unwrap();
    assert!(!out.contains("HIDDEN"), "include_only 被 text 绕过：{out}");
    assert!(out.contains("VISIBLE"), "白名单字段未渲染：{out}");
}

// ---------------------------------------------------------------------------
// W16：受限正则的三条失败路径必须**报码**，而不是静默判「不匹配」
//
// 改前：模式过长 / 模式非法 / 步数超预算全都表现为「目录里没有匹配的文件」——
// 用户看到的是「我的 glob 没匹配上」，而不是「我的模式写错了」。这三条码分别是
// E-LIMIT-007 / E-PARSE-025 / E-LIMIT-002（引擎层只报原因，码在 sml-include 映射）。
// ---------------------------------------------------------------------------

#[test]
fn regex_failures_report_codes() {
    use sml::{FeatureSet, MAX_REGEX_LEN};

    let base = std::env::temp_dir().join(format!("sml-w16-regex-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).expect("建临时目录失败");
    std::fs::write(base.join("alpha.sml"), "a: 1\n").unwrap();
    std::fs::write(base.join("beta.sml"), "b: 2\n").unwrap();
    let feats = FeatureSet::all();

    // ① 正对照：合法模式照常命中（收紧不得误伤）
    let hits = sml_include::parse::glob_or_regex_dir(&base, "", Some(r"^[ab].*\.sml$"), feats)
        .expect("合法正则应成功");
    assert_eq!(hits.len(), 2, "合法正则应命中 2 个文件：{hits:?}");

    // ② 量词之前没有原子 ⇒ E-PARSE-025
    let e = sml_include::parse::glob_or_regex_dir(&base, "", Some("^*a"), feats)
        .expect_err("非法模式应报错");
    assert_eq!(e.code(), "E-PARSE-025", "实得：{}", e.message());

    // ③ 字符类未闭合 ⇒ 同样 E-PARSE-025
    let e = sml_include::parse::glob_or_regex_dir(&base, "", Some("[abc"), feats)
        .expect_err("未闭合字符类应报错");
    assert_eq!(e.code(), "E-PARSE-025", "实得：{}", e.message());

    // ④ 模式过长 ⇒ E-LIMIT-007（改前 `compile_regex` 会给一个「永不匹配」的正则）
    let long = "a".repeat(MAX_REGEX_LEN + 1);
    let e = sml_include::parse::glob_or_regex_dir(&base, "", Some(&long), feats)
        .expect_err("超长模式应报错");
    assert_eq!(e.code(), "E-LIMIT-007", "实得：{}", e.message());

    // ⑤ 引擎层：checked 入口把两类失败**分开**报（这是上层能映射出不同码的前提）
    match sml::compile_regex_checked("^*a") {
        Err(sml::RegexError::Illegal { .. }) => {}
        other => panic!("非法模式应报 Illegal，实际：{other:?}"),
    }
    match sml::compile_regex_checked(&long) {
        Err(sml::RegexError::TooLong { len, max }) => {
            assert_eq!(max, MAX_REGEX_LEN);
            assert!(len > max);
        }
        other => panic!("超长模式应报 TooLong，实际：{other:?}"),
    }

    // ⑥ 步数预算耗尽 ⇒ E-LIMIT-002（改前静默判「不匹配」）
    //    同一份输入走**宽松**入口仍是 false（既有 40+ 处断言依赖它）；
    //    走 checked 入口才把它变成可报的失败。
    let pat = "a*".repeat(20) + "b";
    let text = "a".repeat(100);
    let re = sml::compile_regex_checked(&pat).unwrap();
    assert!(!sml::regex_matches(&re, &text));
    match sml::regex_matches_checked(&re, &text) {
        Err(sml::RegexError::Budget { .. }) => {}
        other => panic!("预算耗尽应报 Budget，实际：{other:?}"),
    }

    let _ = std::fs::remove_dir_all(&base);
}
