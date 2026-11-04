// SPDX-License-Identifier: MulanPSL-2.0
//! `applies_to` 提示（`docs/smlex-format-draft.md` §3.1）：**只提示、不自动挂**。
//!
//! 钉三件事：
//! 1. `glob` 命中且没被引用 ⇒ `I-LINT-001` 提示，退出码仍是 0；
//! 2. `requires` 里的契约没定义 ⇒ 不提示（宁缺勿滥）；
//! 3. 已经被 `@weakdoc` 引用的 profile ⇒ 不再重复提示。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn tmp(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("smlsug_{}_{}", tag, std::process::id()));
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

fn lint(doc: &Path) -> (String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_smltools"))
        .args(["--lint", "-i", doc.to_str().unwrap()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn")
        .wait_with_output()
        .expect("wait");
    (
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn glob_hit_without_reference_suggests() {
    let dir = tmp("hit");
    write(
        &dir.join(".smlex/profiles/api-docs.smlex"),
        "name: api-docs\nversion: 1\napplies_to {\n  glob: [\"**/doc.sml\"]\n}\n",
    );
    let doc = dir.join("doc.sml");
    write(&doc, "@version v1\ntitle: hi\n");

    let (out, code) = lint(&doc);
    assert!(out.contains("I-LINT-001"), "应给提示：{out}");
    assert!(out.contains("api-docs"), "提示里要点名 profile：{out}");
    assert_eq!(code, 0, "提示不该影响退出码：{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn requires_not_satisfied_means_no_suggestion() {
    let dir = tmp("req");
    write(
        &dir.join(".smlex/profiles/api-docs.smlex"),
        "name: api-docs\nversion: 1\napplies_to {\n  glob: [\"**/doc.sml\"]\n  requires: [Endpoint]\n}\n",
    );
    let doc = dir.join("doc.sml");
    write(&doc, "@version v1\ntitle: hi\n");

    let (out, _) = lint(&doc);
    assert!(!out.contains("I-LINT-001"), "契约没定义就不该提示：{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn referenced_profile_is_not_suggested_again() {
    let dir = tmp("ref");
    write(
        &dir.join(".smlex/profiles/api-docs.smlex"),
        "name: api-docs\nversion: 1\napplies_to {\n  glob: [\"**/doc.sml\"]\n}\n",
    );
    let doc = dir.join("doc.sml");
    write(
        &doc,
        "@version v1\n@feature enable type.weak\n@weakdoc api-docs\ntitle: hi\n",
    );

    let (out, code) = lint(&doc);
    assert!(!out.contains("I-LINT-001"), "已引用的不该再提示：{out}");
    assert_eq!(code, 0, "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}
