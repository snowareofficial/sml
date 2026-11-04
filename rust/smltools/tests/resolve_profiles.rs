// SPDX-License-Identifier: MulanPSL-2.0
//! `--resolve-profiles` 的验收：`@weakdoc` 的名字落到具体 `.smlex` 文件上。
//!
//! 钉四件事（`docs/weakdoc-spec-draft.md` §3）：
//! 1. **项目级**：自输入文件所在目录**向上**找 `.smlex/profiles/`；
//! 2. **显式目录优先**：`--profile-dir` 命中就不再看后面几层；
//! 3. **`--no-profiles`** 关掉三命名空间发现；
//! 4. 未命中记 `-` 且退出码为 1（不能静默当"有"）。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn tmp(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("smlprof_{}_{}", tag, std::process::id()));
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

/// 跑一次 CLI，返回 `(stdout, 退出码)`。
fn run(args: &[&str]) -> (String, i32) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_smltools"))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn smltools");
    let out = child.wait_with_output().expect("wait");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        out.status.code().unwrap_or(-1),
    )
}

const DOC: &str = "@version v1\n@feature enable type.weak\n@weakdoc api-docs\n@weakdoc missing-one\na: 1\n";

#[test]
fn project_scope_is_found_and_missing_is_reported() {
    let dir = tmp("project");
    write(&dir.join(".smlex/profiles/api-docs.smlex"), "x");
    let doc = dir.join("doc.sml");
    write(&doc, DOC);

    let (out, code) = run(&["-i", doc.to_str().unwrap(), "--resolve-profiles"]);
    assert!(
        out.contains("api-docs\tproject\t"),
        "应命中项目级：{out}"
    );
    assert!(out.contains("missing-one\t-\t"), "未命中应记 `-`：{out}");
    assert_eq!(code, 1, "有未命中 ⇒ 退出码 1：{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn nested_subdir_still_finds_the_project_root() {
    let dir = tmp("nested");
    write(&dir.join(".smlex/profiles/api-docs.smlex"), "x");
    let doc = dir.join("docs/deep/doc.sml");
    write(&doc, "@version v1\n@feature enable type.weak\n@weakdoc api-docs\na: 1\n");

    let (out, code) = run(&["-i", doc.to_str().unwrap(), "--resolve-profiles"]);
    assert!(out.contains("api-docs\tproject\t"), "向上查找应命中：{out}");
    assert_eq!(code, 0, "全部命中 ⇒ 退出码 0：{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn explicit_dir_wins_over_project() {
    let dir = tmp("explicit");
    write(&dir.join("proj/.smlex/profiles/api-docs.smlex"), "x");
    write(&dir.join("extra/api-docs.smlex"), "x");
    let doc = dir.join("proj/doc.sml");
    write(&doc, "@version v1\n@feature enable type.weak\n@weakdoc api-docs\na: 1\n");

    let (out, _) = run(&[
        "-i",
        doc.to_str().unwrap(),
        "--resolve-profiles",
        "--profile-dir",
        dir.join("extra").to_str().unwrap(),
    ]);
    assert!(out.contains("api-docs\texplicit\t"), "显式目录应优先：{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_profiles_turns_discovery_off() {
    let dir = tmp("noprof");
    write(&dir.join(".smlex/profiles/api-docs.smlex"), "x");
    let doc = dir.join("doc.sml");
    write(&doc, "@version v1\n@feature enable type.weak\n@weakdoc api-docs\na: 1\n");

    let (out, code) = run(&[
        "-i",
        doc.to_str().unwrap(),
        "--resolve-profiles",
        "--no-profiles",
    ]);
    assert!(out.contains("api-docs\t-\t"), "关掉发现后应未命中：{out}");
    assert_eq!(code, 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 写一个文件并立刻关闭：仅用于确认 `--profile-dir` 之外没有副作用。
#[allow(dead_code)]
fn touch(p: &Path) {
    let mut f = std::fs::File::create(p).expect("create");
    f.write_all(b"x").expect("write");
}
