// SPDX-License-Identifier: MulanPSL-2.0
//! `@weakdoc` 的**用户全局 / 系统**两层命名空间（`docs/swsml/spec/weakdoc.md` §3）。
//!
//! 已有的 `resolve_profiles.rs` 覆盖「显式目录 / 项目级向上查找 / `--no-profiles`」；
//! 这里补最后两层，并**只通过子进程环境变量重定向**（绝不碰真实用户目录）：
//!
//! | 层 | Windows | 其它 |
//! |---|---|---|
//! | user | `%APPDATA%\sml\profiles` | `$XDG_CONFIG_HOME/sml/profiles`，缺失则 `$HOME/.config/sml/profiles` |
//! | system | `%ProgramData%\sml\profiles` | `/usr/share/sml/profiles`（**写死**，见文末说明） |
//!
//! ⚠️ 非 Windows 的 system 层是**写死路径**，要覆盖它必须往 `/usr/share` 写文件（需 root），
//! 故本文件在非 Windows 上**不测 system**，只在 `system_layer_is_hardcoded_on_unix` 里钉住这一事实。
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn tmp(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("smlns_{}_{}", tag, std::process::id()));
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

/// 跑一次 `--resolve-profiles`，返回 `(stdout, 退出码)`。
///
/// `envs` 里的键会**覆盖**子进程环境；值为 `None` 时表示 `env_remove`（用来模拟「变量不存在」）。
fn run(doc: &Path, extra: &[&str], envs: &[(&str, Option<&Path>)]) -> (String, i32) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_smltools"));
    cmd.args(["-i", doc.to_str().unwrap(), "--resolve-profiles"]);
    cmd.args(extra);
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (k, v) in envs {
        match v {
            Some(p) => {
                cmd.env(k, p);
            }
            None => {
                cmd.env_remove(k);
            }
        }
    }
    let out = cmd.spawn().expect("spawn smltools").wait_with_output().expect("wait");
    (
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
        out.status.code().unwrap_or(-1),
    )
}

const DOC: &str = "@version v1\n@feature enable type.weak\n@weakdoc api-docs\na: 1\n";

/// 用户全局层：Windows 走 `%APPDATA%`，其它走 `$XDG_CONFIG_HOME`。
#[test]
fn user_namespace_is_found() {
    let root = tmp("user");
    let user_root = root.join("userroot");
    write(&user_root.join("sml/profiles/api-docs.smlex"), "name: api-docs\n");

    let doc = root.join("doc.sml");
    write(&doc, DOC);

    #[cfg(windows)]
    let envs: Vec<(&str, Option<&Path>)> = vec![("APPDATA", Some(&user_root)), ("ProgramData", None)];
    #[cfg(not(windows))]
    let envs: Vec<(&str, Option<&Path>)> = vec![("XDG_CONFIG_HOME", Some(&user_root)), ("HOME", None)];

    let (out, code) = run(&doc, &[], &envs);
    assert!(out.contains("api-docs\tuser\t"), "应命中用户全局层：{out}");
    assert_eq!(code, 0, "命中即退出码 0：{out}");
    let _ = std::fs::remove_dir_all(&root);
}

/// 项目级**优先于**用户全局。
#[test]
fn project_beats_user_namespace() {
    let root = tmp("proj_user");
    let user_root = root.join("userroot");
    write(&user_root.join("sml/profiles/api-docs.smlex"), "name: api-docs\n");
    write(&root.join(".smlex/profiles/api-docs.smlex"), "name: api-docs\n");

    let doc = root.join("doc.sml");
    write(&doc, DOC);

    #[cfg(windows)]
    let envs: Vec<(&str, Option<&Path>)> = vec![("APPDATA", Some(&user_root)), ("ProgramData", None)];
    #[cfg(not(windows))]
    let envs: Vec<(&str, Option<&Path>)> = vec![("XDG_CONFIG_HOME", Some(&user_root)), ("HOME", None)];

    let (out, _) = run(&doc, &[], &envs);
    assert!(out.contains("api-docs\tproject\t"), "项目级应胜出：{out}");
    let _ = std::fs::remove_dir_all(&root);
}

/// 显式 `--profile-dir` **优先于**用户全局。
#[test]
fn explicit_dir_beats_user_namespace() {
    let root = tmp("explicit_user");
    let user_root = root.join("userroot");
    let extra = root.join("extra");
    write(&user_root.join("sml/profiles/api-docs.smlex"), "name: api-docs\n");
    write(&extra.join("api-docs.smlex"), "name: api-docs\n");

    let doc = root.join("doc.sml");
    write(&doc, DOC);

    #[cfg(windows)]
    let envs: Vec<(&str, Option<&Path>)> = vec![("APPDATA", Some(&user_root)), ("ProgramData", None)];
    #[cfg(not(windows))]
    let envs: Vec<(&str, Option<&Path>)> = vec![("XDG_CONFIG_HOME", Some(&user_root)), ("HOME", None)];

    let (out, _) = run(&doc, &["--profile-dir", extra.to_str().unwrap()], &envs);
    assert!(out.contains("api-docs\texplicit\t"), "显式目录应胜出：{out}");
    let _ = std::fs::remove_dir_all(&root);
}

/// 非 Windows：`XDG_CONFIG_HOME` 不在时回落到 `$HOME/.config/sml/profiles`。
#[cfg(not(windows))]
#[test]
fn home_fallback_when_xdg_is_missing() {
    let root = tmp("home_fb");
    let home_root = root.join("home");
    write(
        &home_root.join(".config/sml/profiles/api-docs.smlex"),
        "name: api-docs\n",
    );

    let doc = root.join("doc.sml");
    write(&doc, DOC);

    let envs: Vec<(&str, Option<&Path>)> = vec![("XDG_CONFIG_HOME", None), ("HOME", Some(&home_root))];
    let (out, code) = run(&doc, &[], &envs);
    assert!(out.contains("api-docs\tuser\t"), "应回落到 $HOME/.config：{out}");
    assert_eq!(code, 0);
    let _ = std::fs::remove_dir_all(&root);
}

/// Windows：系统层走 `%ProgramData%\sml\profiles`（可用环境变量重定向 ⇒ 可测）。
#[cfg(windows)]
#[test]
fn system_namespace_is_found_on_windows() {
    let root = tmp("system");
    let program_data = root.join("pd");
    write(&program_data.join("sml/profiles/api-docs.smlex"), "name: api-docs\n");

    let doc = root.join("doc.sml");
    write(&doc, DOC);

    // 用户层指到空目录，确保命中的是 system 而不是 user
    let empty_user = root.join("empty");
    std::fs::create_dir_all(&empty_user).expect("建空目录");
    let envs: Vec<(&str, Option<&Path>)> = vec![
        ("APPDATA", Some(&empty_user)),
        ("ProgramData", Some(&program_data)),
    ];

    let (out, code) = run(&doc, &[], &envs);
    assert!(out.contains("api-docs\tsystem\t"), "应命中系统层：{out}");
    assert_eq!(code, 0);
    let _ = std::fs::remove_dir_all(&root);
}

/// 非 Windows：系统层是**写死**的 `/usr/share/sml/profiles` —— 钉住这个事实，
/// 避免以后有人以为它可重定向（要测它得往 `/usr/share` 写文件，需 root）。
#[cfg(not(windows))]
#[test]
fn system_layer_is_hardcoded_on_unix() {
    let root = tmp("sys_fixed");
    let doc = root.join("doc.sml");
    write(&doc, DOC);

    // 把能重定向的都指到空目录；系统层仍是 /usr/share
    let empty = root.join("empty");
    std::fs::create_dir_all(&empty).expect("建空目录");
    let envs: Vec<(&str, Option<&Path>)> = vec![("XDG_CONFIG_HOME", Some(&empty)), ("HOME", Some(&empty))];
    let (out, code) = run(&doc, &[], &envs);

    if Path::new("/usr/share/sml/profiles").is_dir() {
        // 环境里真装了系统级 profile 时不做断言（可能命中，与是否写死无关）
        return;
    }
    assert!(out.contains("api-docs\t-\t"), "系统层不存在时应未命中：{out}");
    assert_eq!(code, 1, "未命中 ⇒ 退出码 1");
    let _ = std::fs::remove_dir_all(&root);
}
