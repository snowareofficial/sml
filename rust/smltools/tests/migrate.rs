// SPDX-License-Identifier: MulanPSL-2.0
//! `--migrate` 的验收（`docs/swsml/spec/version.md` §4.5）。
//!
//! 钉五条：
//! 1. **默认 scan 不写文件**，有命中退出码 1（CI 可直接拦）；
//! 2. **apply 幂等**：第二次 apply 零改动，随后 scan 也是 0 命中；
//! 3. **规则可单跑**（`--rule`），未知 id 报 `E-CLI-008` 且退出码 2；
//! 4. `rules` 不需要输入文件，列全 `id / 适用版本集 / 说明`；
//! 5. `--migrate` 与 `-o` 等互斥（`E-CLI-003`），5.0 族只提示不迁移。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn tmp(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("smlmig_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("创建临时目录");
    p
}

fn write(p: &Path, body: &str) {
    std::fs::write(p, body).expect("写文件");
}

/// 跑一次 CLI，返回 `(stdout, stderr, 退出码)`。
fn run(args: &[&str]) -> (String, String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_smltools"))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("spawn smltools");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.code().unwrap_or(-1),
    )
}

const SRC: &str = "@version v3\n@feature base v2\nk: 1\n";

#[test]
fn scan_reports_without_touching_the_file() {
    let dir = tmp("scan");
    let doc = dir.join("a.sml");
    write(&doc, SRC);

    let (stdout, stderr, code) = run(&["-i", doc.to_str().unwrap(), "--migrate", "scan"]);
    assert_eq!(code, 1, "有命中 ⇒ 退出码 1：stderr={stderr}");
    assert!(stdout.contains("version-literal"), "应点名规则：{stdout}");
    assert!(stdout.contains("@version v3"), "应给出原写法：{stdout}");
    assert!(stdout.contains("@version 30"), "应给出新写法：{stdout}");
    assert!(stdout.contains("feature-base-literal"), "{stdout}");
    assert_eq!(
        std::fs::read_to_string(&doc).unwrap(),
        SRC,
        "scan 绝不能写文件"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn apply_is_idempotent() {
    let dir = tmp("apply");
    let doc = dir.join("a.sml");
    write(&doc, SRC);

    let (_, stderr, code) = run(&["-i", doc.to_str().unwrap(), "--migrate", "apply"]);
    assert_eq!(code, 0, "apply 成功：{stderr}");
    let after = std::fs::read_to_string(&doc).unwrap();
    assert_eq!(after, "@version 30\n@feature base 20\nk: 1\n");

    // 第二次 apply：零改动、文件逐字节不变
    let (_, stderr2, code2) = run(&["-i", doc.to_str().unwrap(), "--migrate", "apply"]);
    assert_eq!(code2, 0);
    assert!(stderr2.contains("没有可迁移的写法"), "{stderr2}");
    assert_eq!(std::fs::read_to_string(&doc).unwrap(), after, "apply 必须幂等");

    // 再 scan：0 命中 ⇒ 退出码 0
    let (stdout, _, code3) = run(&["-i", doc.to_str().unwrap(), "--migrate", "scan"]);
    assert_eq!(code3, 0, "迁移后不该再有命中：{stdout}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rule_filter_and_unknown_rule() {
    let dir = tmp("filter");
    let doc = dir.join("a.sml");
    write(&doc, SRC);

    let (_, stderr, code) = run(&[
        "-i",
        doc.to_str().unwrap(),
        "--migrate",
        "apply",
        "--rule",
        "feature-base-literal",
    ]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(
        std::fs::read_to_string(&doc).unwrap(),
        "@version v3\n@feature base 20\nk: 1\n",
        "只跑指定规则"
    );

    let (_, stderr2, code2) = run(&[
        "-i",
        doc.to_str().unwrap(),
        "--migrate",
        "scan",
        "--rule",
        "nosuch-rule",
    ]);
    assert_eq!(code2, 2, "未知规则 id ⇒ 用法错误");
    assert!(stderr2.contains("E-CLI-008"), "{stderr2}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn rules_lists_every_rule_without_input() {
    let (stdout, _, code) = run(&["--migrate", "rules"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("version-literal"), "{stdout}");
    assert!(stdout.contains("feature-base-literal"), "{stdout}");
    assert!(stdout.contains("适用版本集"), "应有说明列：{stdout}");
}

#[test]
fn future_family_is_only_a_note_and_conflicts_are_rejected() {
    let dir = tmp("future");
    let doc = dir.join("a.sml");
    write(&doc, "@version 50 \"camilia\" dev\nk: 1\n");

    let (stdout, stderr, code) = run(&["-i", doc.to_str().unwrap(), "--migrate", "scan"]);
    assert_eq!(code, 0, "不可迁移 ⇒ 0 命中、退出码 0：{stderr}");
    assert!(stdout.is_empty(), "不该有命中行：{stdout}");
    assert!(stderr.contains("5.0 族"), "应提示为什么不迁：{stderr}");

    let (_, stderr2, code2) = run(&[
        "-i",
        doc.to_str().unwrap(),
        "--migrate",
        "scan",
        "-o",
        dir.join("out.sml").to_str().unwrap(),
    ]);
    assert_eq!(code2, 2, "--migrate 与 -o 互斥");
    assert!(stderr2.contains("E-CLI-003"), "{stderr2}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn directive_arg_needs_the_whitelist_flag() {
    let dir = tmp("dirarg");
    let doc = dir.join("a.sml");
    // 位置参数形式：裸词参数紧跟 `{`
    write(&doc, "@form F { a: 1 }\n");

    // 不给白名单 ⇒ 一个字都不动（分不清扩展指令与片段定义）
    let (_, stderr, code) = run(&["-i", doc.to_str().unwrap(), "--migrate", "apply"]);
    assert_eq!(code, 0, "{stderr}");
    assert_eq!(std::fs::read_to_string(&doc).unwrap(), "@form F { a: 1 }\n");

    // 显式点名该规则却没给指令表 ⇒ 依赖缺失，不许静默零命中
    let (_, stderr2, code2) = run(&[
        "-i",
        doc.to_str().unwrap(),
        "--migrate",
        "scan",
        "--rule",
        "directive-name-arg",
    ]);
    assert_eq!(code2, 2, "缺 --migrate-directive 应报错");
    assert!(stderr2.contains("E-CLI-003"), "{stderr2}");

    // 给了白名单 ⇒ 改写成显式 `name:`，且幂等
    let (_, stderr3, code3) = run(&[
        "-i",
        doc.to_str().unwrap(),
        "--migrate",
        "apply",
        "--migrate-directive",
        "form",
    ]);
    assert_eq!(code3, 0, "{stderr3}");
    assert_eq!(
        std::fs::read_to_string(&doc).unwrap(),
        "@form name: F { a: 1 }\n"
    );
    let (stdout4, _, code4) = run(&[
        "-i",
        doc.to_str().unwrap(),
        "--migrate",
        "scan",
        "--migrate-directive",
        "form",
    ]);
    assert_eq!(code4, 0, "二次扫描应 0 命中：{stdout4}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn backup_writes_a_bak_file() {
    let dir = tmp("backup");
    let doc = dir.join("a.sml");
    write(&doc, "@version v1\nk: 1\n");

    let (_, stderr, code) = run(&[
        "-i",
        doc.to_str().unwrap(),
        "--migrate",
        "apply",
        "--backup",
    ]);
    assert_eq!(code, 0, "{stderr}");
    let bak = PathBuf::from(format!("{}.bak", doc.display()));
    assert!(bak.is_file(), "--backup 应写 .bak：{stderr}");
    assert_eq!(std::fs::read_to_string(&bak).unwrap(), "@version v1\nk: 1\n");
    assert_eq!(std::fs::read_to_string(&doc).unwrap(), "@version 10\nk: 1\n");
    let _ = std::fs::remove_dir_all(&dir);
}
