// SPDX-License-Identifier: MulanPSL-2.0
//! smltools CLI 的「**触发条件 → 期望码**」回归。
//!
//! 这一组用例的职责不是「smltools 报错了」，而是钉住**报的是哪个码**：
//! 错误码是跨端契约（唯一事实来源 `errors/codes.sml`），文案可以改、码不能改。
//!
//! 为什么走**真实二进制**而不是直调内部函数：本 crate 是 `[[bin]]`（没有 lib），
//! 而用户真正看到的是 `smltools: …` 这行 stderr。直接驱动进程才能同时验：
//! 码真的被打印出来、打印在 stderr、退出码对得上。
//!
//! 每例都用 `--from` 显式指定格式，避免依赖扩展名推断（推断是另一回事）。

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// 一次 CLI 调用的产物。
struct Out {
    stdout: String,
    stderr: String,
    code: Option<i32>,
}

impl Out {
    /// stdout + stderr，方便统一检索码。
    fn all(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_smltools")
}

fn drive(args: &[&str], stdin: Option<&str>) -> Out {
    let mut cmd = Command::new(bin());
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
    let mut child = cmd.spawn().expect("启动 smltools 失败");
    if let Some(data) = stdin {
        child
            .stdin
            .take()
            .expect("stdin 管道应在")
            .write_all(data.as_bytes())
            .expect("写入 stdin 失败");
    }
    let out = child.wait_with_output().expect("等待 smltools 失败");
    let o = Out {
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        code: out.status.code(),
    };
    // 全局不变式：CLI 输出里不得出现 `smltools: smltools:` 这类**重复工具前缀**。
    // 成因：`Err` 文案若自带 `smltools: `，外层统一 `eprintln!("smltools: {e}")` 就会叠两层，
    // 让用户以为消息被嵌了一层。修复只删内层前缀；注释能提醒人，**断言才拦得住人**。
    // 放在 `drive`（所有用例的唯一出口）里，于是每条 CLI 调用都过一遍这条不变式。
    assert!(
        !o.all().contains("smltools: smltools:"),
        "CLI 输出出现重复前缀 `smltools: smltools:`：\n--- stdout ---\n{}\n--- stderr ---\n{}",
        o.stdout,
        o.stderr
    );
    o
}

/// 断言一次调用（含给定 stdin）的输出里出现期望码，并返回产物（供退出码断言）。
fn assert_code(args: &[&str], stdin: Option<&str>, want: &str) -> Out {
    let o = drive(args, stdin);
    assert!(
        o.all().contains(want),
        "args={args:?} 期望含码 {want}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        o.stdout,
        o.stderr
    );
    o
}

/// 独立的临时目录（进程号 + 标签），写几个字节的输入文件用。
fn tmpdir(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("smltools-codes-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("建临时目录失败");
    p
}

fn write_file(dir: &PathBuf, name: &str, content: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, content).expect("写临时文件失败");
    p
}

fn s(p: &PathBuf) -> String {
    p.to_str().expect("临时路径应为 UTF-8").to_string()
}

// ================= 命令行与用法（CLI） =================

#[test]
fn unknown_input_format_is_cli_001() {
    let o = assert_code(&["--from", "nosuch"], Some("k: 1\n"), "E-CLI-001");
    assert_eq!(o.code, Some(2), "参数错误应以退出码 2 结束");
}

#[test]
fn unknown_output_format_is_cli_002() {
    let o = assert_code(&["--to", "nosuch"], Some("k: 1\n"), "E-CLI-002");
    assert_eq!(o.code, Some(2), "参数错误应以退出码 2 结束");
}

#[test]
fn mutually_exclusive_hugo_zola_is_cli_003() {
    assert_code(&["--hugo", "x", "--zola", "y"], Some("k: 1\n"), "E-CLI-003");
}

#[test]
fn zola_build_without_zola_is_cli_003() {
    assert_code(&["--zola-build"], Some("k: 1\n"), "E-CLI-003");
}

#[test]
fn custom_without_rules_is_cli_003() {
    assert_code(&["--to", "custom"], Some("k: 1\n"), "E-CLI-003");
}

#[test]
fn lint_on_non_sml_is_cli_004() {
    assert_code(&["--lint", "--from", "json"], Some("{}\n"), "E-CLI-004");
}

#[test]
fn directory_mode_without_output_is_cli_005() {
    let dir = tmpdir("clidir");
    write_file(&dir, "a.json", "{}");
    assert_code(&["-i", &s(&dir), "--from", "json"], None, "E-CLI-005");
}

#[test]
fn rules_document_parse_failure_is_cli_006() {
    let dir = tmpdir("clirules");
    let rules = write_file(&dir, "rules.sml", "a {\n");
    assert_code(
        &["--to", "custom", "--custom-rules", &s(&rules)],
        Some("k: 1\n"),
        "E-CLI-006",
    );
}

#[test]
fn backend_error_is_cli_007() {
    // markdown 后端：`table` 块缺 `header` 数组 → 后端报错
    assert_code(&["--to", "md"], Some("table { rows: [ a ] }\n"), "E-CLI-007");
}

#[test]
fn clap_unknown_argument_is_cli_008() {
    // clap 自身的用法错误原先**不带码**（直接打印并 exit 2）；现由 smltools 补码。
    let o = assert_code(&["--nosuch-flag"], None, "E-CLI-008");
    assert_eq!(o.code, Some(2), "用法错误应保留退出码 2");
    // clap 的用法提示必须保留：那是有用的帮助文本，不能被改写掉。
    assert!(
        o.all().contains("Usage"),
        "应保留 clap 的用法提示：\n{}",
        o.all()
    );
}

#[test]
fn clap_missing_value_is_cli_008() {
    let o = assert_code(&["--to"], None, "E-CLI-008");
    assert_eq!(o.code, Some(2), "缺取值应保留退出码 2");
}

#[test]
fn clap_help_and_version_are_not_errors() {
    // `--help` / `--version` 走 clap 的 Err 通道，但那是**正常输出**：
    // 不得补上 E-CLI-008，退出码必须是 0，且打印到 stdout。
    let help = drive(&["--help"], None);
    assert_eq!(help.code, Some(0), "--help 应正常退出");
    assert!(!help.all().contains("E-CLI-008"), "--help 不能被当作用法错误");
    assert!(help.stdout.contains("Usage"), "--help 应把用法打印到 stdout");

    let ver = drive(&["--version"], None);
    assert_eq!(ver.code, Some(0), "--version 应正常退出");
    assert!(!ver.all().contains("E-CLI-008"), "--version 不能被当作用法错误");
    assert!(ver.stdout.contains("smltools"), "--version 应把版本打印到 stdout");
}

// ================= 迁入格式（MIGRATE） =================

#[test]
fn bad_json_is_migrate_017() {
    assert_code(&["--from", "json"], Some("{bad"), "E-MIGRATE-017");
}

#[test]
fn toml_syntax_error_is_migrate_011() {
    assert_code(&["--from", "toml"], Some("a = \n"), "E-MIGRATE-011");
}

#[test]
fn toml_table_conflict_is_migrate_012() {
    assert_code(&["--from", "toml"], Some("a = 1\n[a]\nb = 2\n"), "E-MIGRATE-012");
}

#[test]
fn yaml_structure_error_is_migrate_013() {
    assert_code(&["--from", "yaml"], Some("a: 1\n  b: 2\n"), "E-MIGRATE-013");
}

#[test]
fn yaml_flow_unclosed_is_migrate_014() {
    assert_code(&["--from", "yaml"], Some("a: [1, 2\n"), "E-MIGRATE-014");
}

#[test]
fn yaml_undefined_alias_is_migrate_015() {
    assert_code(&["--from", "yaml"], Some("a: *nope\n"), "E-MIGRATE-015");
}

#[test]
fn xml_top_level_text_is_migrate_001() {
    assert_code(&["--from", "xml"], Some("hello <r/>"), "E-MIGRATE-001");
}

#[test]
fn xml_unclosed_tag_is_migrate_002() {
    assert_code(&["--from", "xml"], Some("<r>"), "E-MIGRATE-002");
}

#[test]
fn xml_empty_tag_name_is_migrate_003() {
    assert_code(&["--from", "xml"], Some("< r/>"), "E-MIGRATE-003");
}

#[test]
fn xml_close_mismatch_is_migrate_004() {
    assert_code(&["--from", "xml"], Some("<r>\n  <a>x</r>\n"), "E-MIGRATE-004");
}

#[test]
fn xml_stray_close_tag_is_migrate_005() {
    assert_code(&["--from", "xml"], Some("<r>a</r></x>"), "E-MIGRATE-005");
}

#[test]
fn xml_bad_attribute_is_migrate_006() {
    assert_code(&["--from", "xml"], Some("<r a=1/>"), "E-MIGRATE-006");
}

#[test]
fn xml_unclosed_comment_is_migrate_007() {
    assert_code(&["--from", "xml"], Some("<r><!-- oops </r>"), "E-MIGRATE-007");
}

#[test]
fn xml_unsupported_decl_is_migrate_008() {
    assert_code(&["--from", "xml"], Some("<r><!ENTITY x \"y\"></r>"), "E-MIGRATE-008");
}

#[test]
fn xml_unknown_entity_is_migrate_009() {
    assert_code(&["--from", "xml"], Some("<r>&nbsp;</r>"), "E-MIGRATE-009");
}

#[test]
fn xml_bad_numeric_entity_is_migrate_010() {
    assert_code(&["--from", "xml"], Some("<r>&#xZZ;</r>"), "E-MIGRATE-010");
}

// ================= 上限（LIMIT） =================

#[test]
fn xml_depth_bomb_is_limit_001() {
    let src = format!("{}x{}", "<a>".repeat(200), "</a>".repeat(200));
    assert_code(&["--from", "xml"], Some(&src), "E-LIMIT-001");
}

// ================= 语言层（LEX / PARSE / INCLUDE） =================

#[test]
fn sml_parse_error_carries_its_code() {
    assert_code(&[], Some("a {\n"), "E-PARSE-001");
}

#[test]
fn include_read_failure_is_include_001() {
    let dir = tmpdir("incmiss");
    let main = write_file(&dir, "main.sml", "include \"nope.sml\"\n");
    assert_code(&["-i", &s(&main)], None, "E-INCLUDE-001");
}

#[test]
fn include_depth_limit_is_include_004() {
    let dir = tmpdir("incdepth");
    let main = write_file(&dir, "self.sml", "include \"self.sml\"\n");
    assert_code(&["-i", &s(&main)], None, "E-INCLUDE-004");
}

#[test]
fn include_unquoted_path_is_include_012() {
    // 未加引号的 include 路径是**写法非法**，不是「文件缺失或读取失败」。
    // 改动前这里错报 E-INCLUDE-001（用户拿它去查会被误导）。
    let dir = tmpdir("incunquoted");
    let main = write_file(&dir, "main.sml", "include nope.sml\n");
    let o = assert_code(&["-i", &s(&main)], None, "E-INCLUDE-012");
    assert_eq!(o.code, Some(1), "语言层/展开失败应以退出码 1 结束");
}

/// 不变式：任何 CLI 输出都不得出现**重复工具前缀** `smltools: smltools:`。
///
/// 成因：这几条 `Err` 文案原先自带 `smltools: `（`E-INCLUDE-012` 未加引号、
/// `E-INCLUDE-001` 读取失败、`E-INCLUDE-004` 嵌套超限），外层再统一
/// `eprintln!("smltools: {e}")` 就叠成两层。
///
/// `drive()` 已对**每次**调用断言同一不变式；这里再显式钉一份，让规则有个可读的用例名。
#[test]
fn cli_output_has_no_duplicate_tool_prefix() {
    let dir = tmpdir("noprefix");
    let unquoted = write_file(&dir, "unquoted.sml", "include nope.sml\n");
    let missing = write_file(&dir, "missing.sml", "include \"nope.sml\"\n");
    let depth = write_file(&dir, "depth.sml", "include \"depth.sml\"\n");
    for path in [&unquoted, &missing, &depth] {
        let o = drive(&["-i", &s(path)], None);
        assert!(
            !o.all().contains("smltools: smltools:"),
            "{} 出现重复前缀：\n{}",
            path.display(),
            o.all()
        );
    }
}

// ================= 特性（FEATURE） =================

#[test]
fn unknown_feature_version_is_feature_004() {
    assert_code(&["--feature", "v9"], Some("k: 1\n"), "E-FEATURE-004");
}

#[test]
fn feature_note_is_i_feature_001() {
    assert_code(&["--feature", "v1"], Some("k: 1\n"), "I-FEATURE-001");
}

// ================= 扩展点（EXT） =================

#[test]
fn invalid_scope_name_is_ext_007() {
    assert_code(
        &["--to", "tmlanguage"],
        Some("scopeName: bad\ndirectives: [ form ]\n"),
        "E-EXT-007",
    );
}

#[test]
fn invalid_color_is_ext_007() {
    let dir = tmpdir("extcolor");
    assert_code(
        &["--to", "highlight", "-o", &s(&dir)],
        Some("colors: { a.b: \"red\" }\n"),
        "E-EXT-007",
    );
}

#[test]
fn rules_doc_without_rules_is_ext_006() {
    let dir = tmpdir("extrules");
    let rules = write_file(&dir, "rules.sml", "norules: 1\n");
    assert_code(
        &["--to", "custom", "--custom-rules", &s(&rules)],
        Some("k: 1\n"),
        "E-EXT-006",
    );
}

// ================= 输入输出（IO） =================

#[test]
fn missing_input_file_is_io_001() {
    let dir = tmpdir("ionofile");
    assert_code(&["-i", &s(&dir.join("nope.sml"))], None, "E-IO-001");
}

#[test]
fn write_to_directory_is_io_003() {
    let dir = tmpdir("iowrite");
    let input = write_file(&dir, "in.sml", "k: 1\n");
    assert_code(&["-i", &s(&input), "-o", &s(&dir)], None, "E-IO-003");
}

#[test]
fn no_input_at_all_is_io_005() {
    // stdin 是管道且迟迟没有数据（不是 TTY），smltools 超时后应报「未检测到输入」。
    let mut child = Command::new(bin())
        .args(["--to", "md"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("启动 smltools 失败");
    // 关键：**握住** stdin 不放（既不写也不关），模拟「终端在等键盘输入」。
    let held = child.stdin.take().expect("stdin 管道应在");
    let out = child.wait_with_output().expect("等待 smltools 失败");
    drop(held);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("E-IO-005"),
        "期望 E-IO-005，实际 stderr：{stderr}"
    );
}

// ================= lint 诊断（LINT） =================

#[test]
fn lint_tab_indent_is_lint_001() {
    let dir = tmpdir("linttab");
    let f = write_file(&dir, "tab.sml", "\tk: 1\n");
    assert_code(&["--lint", "-i", &s(&f)], None, "E-LINT-001");
}

#[test]
fn lint_warnings_carry_codes() {
    let dir = tmpdir("lintwarn");
    // 未引用片段 / 未应用契约 / 重复键 / 空值字段，四类告警各一。
    // 片段用无参数写法 `@frag { .. }`：位置参数形式自 v4 起已废弃（会先报 E-PARSE-005）。
    let src = "@frag { a: 1 }\n@contract Unused { a: int }\nk: 1\nk: 2\nempty:\n";
    let f = write_file(&dir, "warn.sml", src);
    let o = drive(&["--lint", "-i", &s(&f)], None);
    let all = o.all();
    for want in ["W-LINT-001", "W-LINT-002", "W-LINT-003", "W-LINT-004"] {
        assert!(all.contains(want), "期望含 {want}，实际：\n{all}");
    }
}

#[test]
fn lint_deep_nesting_is_wlint_005() {
    let dir = tmpdir("lintdeep");
    let mut src = String::new();
    for i in 0..70 {
        src.push_str(&"  ".repeat(i));
        src.push_str(&format!("k{i} {{\n"));
    }
    for i in (0..70).rev() {
        src.push_str(&"  ".repeat(i));
        src.push_str("}\n");
    }
    let f = write_file(&dir, "deep.sml", &src);
    assert_code(&["--lint", "-i", &s(&f)], None, "W-LINT-005");
}
