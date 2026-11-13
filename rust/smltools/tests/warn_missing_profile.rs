// SPDX-License-Identifier: MulanPSL-2.0
//! `W-CLI-001` 的验收：普通解析路径下 `@weakdoc` 找不到 profile **只告警**。
//!
//! 钉四条（`errors/codes.sml` 的 `W-CLI-001` 条目 / `docs/swsml/spec/weakdoc.md` §3）：
//! 1. **不阻断**：退出码仍是 0，转换产物照常写到 stdout；
//! 2. **命中就不报**：`--profile-dir` 里有同名 `.smlex` 时零告警；
//! 3. **`--no-profiles` 不报**：用户明确关掉发现，就别再唠叨；
//! 4. **`--lint` 用同一个码**（`W-CLI-001`），而 `--resolve-profiles` 保持自己的出口
//!    （未命中退出码 1，且不掺 `W-CLI-001`）。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn tmp(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("smlwcli_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("创建临时目录");
    p
}

fn write(p: &Path, body: &str) {
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).expect("创建父目录");
    }
    std::fs::write(p, body).expect("写文件");
}

/// 跑一次 CLI，返回 `(stdout, stderr, 退出码)`。
fn run_capture(args: &[&str]) -> (String, String, i32) {
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

const DOC_MISSING: &str = "@version v1\n@feature enable type.weak\n@weakdoc missing-one\na: 1\n";
const DOC_FOUND: &str = "@version v1\n@feature enable type.weak\n@weakdoc api-docs\na: 1\n";

#[test]
fn missing_profile_warns_but_exits_zero() {
    let dir = tmp("missing");
    let doc = dir.join("doc.sml");
    write(&doc, DOC_MISSING);
    // 对照组：同一份数据**不带** `@weakdoc`。产物必须逐字节相同
    // （「有 / 无 profile 引用，解析结果完全一致」是 weakdoc.md 的硬判据）。
    let plain = dir.join("plain.sml");
    write(
        &plain,
        "@version v1\n@feature enable type.weak\na: 1\n",
    );
    let empty = dir.join("empty-profiles");
    std::fs::create_dir_all(&empty).expect("建空目录");

    let (stdout, stderr, code) = run_capture(&[
        "-i",
        doc.to_str().unwrap(),
        "--profile-dir",
        empty.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "只告警，退出码仍是 0：stderr={stderr}");
    assert!(
        stderr.contains("[W-CLI-001]") && stderr.contains("missing-one"),
        "应报 W-CLI-001 并点名 profile：{stderr}"
    );
    assert!(
        !stdout.contains("W-CLI-001"),
        "告警只能进 stderr，不能污染产物：{stdout}"
    );

    let (baseline, _, base_code) = run_capture(&[
        "-i",
        plain.to_str().unwrap(),
        "--profile-dir",
        empty.to_str().unwrap(),
    ]);
    assert_eq!(base_code, 0);
    assert_eq!(
        stdout, baseline,
        "`@weakdoc` 只影响告警，产物必须逐字节相同"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn found_profile_does_not_warn() {
    let dir = tmp("found");
    let profiles = dir.join("profiles");
    write(&profiles.join("api-docs.smlex"), "x");
    let doc = dir.join("doc.sml");
    write(&doc, DOC_FOUND);

    let (_, stderr, code) = run_capture(&[
        "-i",
        doc.to_str().unwrap(),
        "--profile-dir",
        profiles.to_str().unwrap(),
    ]);
    assert_eq!(code, 0);
    assert!(
        !stderr.contains("W-CLI-001"),
        "命中时不该有这条告警：{stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_profiles_suppresses_the_warning() {
    let dir = tmp("noprof");
    let doc = dir.join("doc.sml");
    write(&doc, DOC_MISSING);

    let (_, stderr, code) = run_capture(&["-i", doc.to_str().unwrap(), "--no-profiles"]);
    assert_eq!(code, 0);
    assert!(
        !stderr.contains("W-CLI-001"),
        "--no-profiles 是「别去找」，就不该唠叨：{stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn lint_uses_the_same_cli_code() {
    let dir = tmp("lint");
    let doc = dir.join("doc.sml");
    write(&doc, DOC_MISSING);
    let empty = dir.join("empty-profiles");
    std::fs::create_dir_all(&empty).expect("建空目录");

    let (_, stderr, code) = run_capture(&[
        "-i",
        doc.to_str().unwrap(),
        "--lint",
        "--profile-dir",
        empty.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "warning 不设 has_error：{stderr}");
    assert!(
        stderr.contains("[W-CLI-001]"),
        "lint 侧的未命中也要用 W-CLI-001（不是 E-LINT-005）：{stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn resolve_profiles_keeps_its_own_exit() {
    let dir = tmp("resolve");
    let doc = dir.join("doc.sml");
    write(&doc, DOC_MISSING);
    let empty = dir.join("empty-profiles");
    std::fs::create_dir_all(&empty).expect("建空目录");

    let (stdout, stderr, code) = run_capture(&[
        "-i",
        doc.to_str().unwrap(),
        "--resolve-profiles",
        "--profile-dir",
        empty.to_str().unwrap(),
    ]);
    assert_eq!(code, 1, "显式查询未命中仍是退出码 1：{stdout}");
    assert!(
        stdout.contains("missing-one\t-\t"),
        "仍按表格输出未命中行：{stdout}"
    );
    assert!(
        !stderr.contains("W-CLI-001"),
        "显式查询不掺普通解析的告警：{stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
