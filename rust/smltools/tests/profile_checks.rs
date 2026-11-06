// SPDX-License-Identifier: MulanPSL-2.0
//! `.smlex` 的 `checks` 执行（`docs/swsml/spec/smlex.md` v1.0 §3.2）。
//!
//! 钉三件事：
//! 1. 三项检查真的生效（缺顶层字段 / 禁用契约 / 禁用键），且都按 error 处理；
//! 2. 合规文档不产生 error（退出码 0）；
//! 3. profile 找不到只是 **warning**（用户可能只是没装它），不阻断。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn tmp(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("smlchk_{}_{}", tag, std::process::id()));
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

/// 跑 `--lint`，返回 `(全部输出, 退出码)`。
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

const PROFILE: &str = "name: api-docs\nversion: 1\nchecks {\n  require_top_level: [title]\n  forbid_contracts: [Draft]\n  forbid_keys: [tmp]\n}\n";

#[test]
fn three_checks_all_fire() {
    let dir = tmp("fire");
    write(&dir.join(".smlex/profiles/api-docs.smlex"), PROFILE);
    let doc = dir.join("doc.sml");
    write(
        &doc,
        "@version v1\n@feature enable type.weak\n@weakdoc api-docs\n\
         @contract Draft { a: str }\nbody {\n  tmp: 1\n}\n",
    );

    let (out, code) = lint(&doc);
    assert!(out.contains("E-LINT-002"), "应报缺顶层字段：{out}");
    assert!(out.contains("E-LINT-003"), "应报禁用契约：{out}");
    assert!(out.contains("E-LINT-004"), "应报禁用键：{out}");
    assert_eq!(code, 1, "有 error ⇒ 退出码 1：{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn conforming_document_passes() {
    let dir = tmp("clean");
    write(&dir.join(".smlex/profiles/api-docs.smlex"), PROFILE);
    let doc = dir.join("doc.sml");
    write(
        &doc,
        "@version v1\n@feature enable type.weak\n@weakdoc api-docs\ntitle: hello\nbody {\n  ok: 1\n}\n",
    );

    let (out, code) = lint(&doc);
    assert_eq!(code, 0, "合规文档应通过：{out}");
    assert!(!out.contains("E-LINT-00"), "不该有 profile 错误：{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// profile 找不到 ⇒ warning，不阻断（退出码仍是 0）。
#[test]
fn missing_profile_is_only_a_warning() {
    let dir = tmp("missing");
    let doc = dir.join("doc.sml");
    write(
        &doc,
        "@version v1\n@feature enable type.weak\n@weakdoc nosuch\ntitle: hi\n",
    );

    let (out, code) = lint(&doc);
    assert!(out.contains("未找到 profile"), "应提示找不到：{out}");
    assert!(out.contains("E-LINT-005"), "同一码（profile 不可用）：{out}");
    assert_eq!(code, 0, "warning 不该阻断：{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// `.smlex` 里 `name` 与文件名不一致 ⇒ error（格式不变量 3）。
#[test]
fn name_mismatch_is_an_error() {
    let dir = tmp("mismatch");
    write(
        &dir.join(".smlex/profiles/api-docs.smlex"),
        "name: other\nversion: 1\nchecks {\n  require_top_level: [title]\n}\n",
    );
    let doc = dir.join("doc.sml");
    write(
        &doc,
        "@version v1\n@feature enable type.weak\n@weakdoc api-docs\ntitle: hi\n",
    );

    let (out, code) = lint(&doc);
    assert!(out.contains("profile 无法加载"), "应报加载失败：{out}");
    assert!(out.contains("E-LINT-005"), "{out}");
    assert_eq!(code, 1, "error ⇒ 退出码 1：{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[allow(dead_code)]
fn touch(p: &Path) {
    let mut f = std::fs::File::create(p).expect("create");
    f.write_all(b"x").expect("write");
}
