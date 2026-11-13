// SPDX-License-Identifier: MulanPSL-2.0
//! `smltools --profiles-*`：`.smlex` 的列出 / 安装 / 卸载（端到端，走真实二进制）。
//!
//! 用 `APPDATA`（Windows）/ `XDG_CONFIG_HOME`（其它）把「用户层」钉在临时目录里，
//! 不碰开发机的真实 profile 目录。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_smltools"))
}

fn fresh_dir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("sml-profiles-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

/// 把用户层指到 `root`（Windows 用 `APPDATA`，其它用 `XDG_CONFIG_HOME`）。
fn with_user_root<'a>(cmd: &'a mut Command, root: &Path) -> &'a mut Command {
    if cfg!(windows) {
        cmd.env("APPDATA", root);
    } else {
        cmd.env("XDG_CONFIG_HOME", root);
    }
    cmd
}

fn profiles_dir(root: &Path) -> PathBuf {
    root.join("sml").join("profiles")
}

#[test]
fn install_list_remove_round_trip() {
    let root = fresh_dir("rt");
    let src = fresh_dir("src");
    fs::write(
        src.join("api-docs.smlex"),
        "name: api-docs\nversion: 1.0.0\nchecks: []\n",
    )
    .unwrap();

    // ① 目录安装
    let out = with_user_root(
        bin().args(["--profiles-install", src.to_str().unwrap(), "--profiles-scope", "user"]),
        &root,
    )
    .output()
    .unwrap();
    assert!(
        out.status.success(),
        "安装失败：{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let installed = profiles_dir(&root).join("api-docs.smlex");
    assert!(installed.is_file(), "没写进 {}", installed.display());

    // ② 列出（`--profiles-list` 不应因没有输入文件而失败）
    let out = with_user_root(bin().arg("--profiles-list"), &root)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(text.contains("api-docs"), "列表里没有 api-docs：{text}");

    // ③ 再装一次：没有 --profiles-force 必须拒绝（rc=1）
    let out = with_user_root(
        bin().args(["--profiles-install", src.to_str().unwrap(), "--profiles-scope", "user"]),
        &root,
    )
    .output()
    .unwrap();
    assert_eq!(out.status.code(), Some(1), "重复安装应当失败");
    assert!(String::from_utf8_lossy(&out.stderr).contains("已存在"));

    // ④ 卸载
    let out = with_user_root(
        bin().args(["--profiles-remove", "api-docs", "--profiles-scope", "user"]),
        &root,
    )
    .output()
    .unwrap();
    assert!(out.status.success());
    assert!(!installed.exists(), "卸载后文件还在");
}

#[test]
fn name_mismatch_is_rejected() {
    let root = fresh_dir("bad");
    let src = fresh_dir("badsrc");
    // 文件名是 `wrong.smlex`，内容里却写着 `name: api-docs` ⇒ E-LINT-005
    fs::write(
        src.join("wrong.smlex"),
        "name: api-docs\nversion: 1.0.0\nchecks: []\n",
    )
    .unwrap();

    let out = with_user_root(
        bin().args(["--profiles-install", src.to_str().unwrap(), "--profiles-scope", "user"]),
        &root,
    )
    .output()
    .unwrap();
    assert_eq!(out.status.code(), Some(1), "名字不一致应当失败");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("E-LINT-005"), "错误里应带 E-LINT-005：{err}");
    assert!(!profiles_dir(&root).join("wrong.smlex").exists());
}

#[test]
fn missing_source_is_an_error() {
    let root = fresh_dir("missing");
    let out = with_user_root(
        bin().args(["--profiles-install", "no-such-dir", "--profiles-scope", "user"]),
        &root,
    )
    .output()
    .unwrap();
    assert_eq!(out.status.code(), Some(2));
}
