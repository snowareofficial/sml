// SPDX-License-Identifier: MulanPSL-2.0
//! `.smlex` **加载失败**的四类情形（`docs/swsml/spec/weakdoc.md` §3、`docs/swsml/spec/smlex.md`）。
//!
//! 全部落在 `E-LINT-005`「profile 无法加载」，靠 detail 区分：
//! ① 读取失败（IO / 非 UTF-8）② 不是合法 SML ③ 缺必填 `name` ④ `name` 与文件名不一致。
//!
//! 另有一条**故意钉住现状**：profile **找不到**（三处命名空间都没命中）只是 warning、不阻断，
//! 但它复用了 `E-LINT-005` 这个 **E 级码** —— 见文末 `missing_profile_is_only_a_warning` 的注释。
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn tmp(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("smlperr_{}_{}", tag, std::process::id()));
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

/// 跑 `--lint`，返回 `(stdout+stderr, 退出码)`。
fn lint(doc: &Path) -> (String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_smltools"))
        .args(["--lint", "-i", doc.to_str().unwrap()])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn smltools")
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

/// 建一个「项目级 profile 目录 + 引用它的文档」，`profile_body` 决定 `.smlex` 的内容。
fn setup(tag: &str, profile_body: &str) -> (PathBuf, PathBuf) {
    let root = tmp(tag);
    write(&root.join(".smlex/profiles/api-docs.smlex"), profile_body);
    let doc = root.join("doc.sml");
    write(
        &doc,
        "@version v1\n@feature enable type.weak\n@weakdoc api-docs\ntitle: t\n",
    );
    (root, doc)
}

#[test]
fn invalid_sml_profile_is_reported() {
    // 少了右括号 ⇒ 不是合法 SML
    let (root, doc) = setup("badsml", "name: api-docs\nchecks {\n");
    let (out, code) = lint(&doc);
    assert!(out.contains("E-LINT-005"), "应报 E-LINT-005：{out}");
    assert!(out.contains("不是合法 SML"), "文案应点明「不是合法 SML」：{out}");
    assert_eq!(code, 1, "profile 加载失败必须阻断（退出码 1）：{out}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn name_mismatch_is_reported() {
    let (root, doc) = setup("namemismatch", "name: other-docs\nversion: 1\n");
    let (out, code) = lint(&doc);
    assert!(out.contains("E-LINT-005"), "应报 E-LINT-005：{out}");
    assert!(out.contains("不一致"), "文案应点明 `name` 与文件名不一致：{out}");
    assert_eq!(code, 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn missing_name_field_is_reported() {
    let (root, doc) = setup("noname", "version: 1\n");
    let (out, code) = lint(&doc);
    assert!(out.contains("E-LINT-005"), "应报 E-LINT-005：{out}");
    assert!(out.contains("name"), "文案应点明缺 `name`：{out}");
    assert_eq!(code, 1);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn unreadable_profile_is_reported() {
    // 非法 UTF-8 ⇒ `read_to_string` 失败 ⇒ 「读取失败」这一档
    let root = tmp("badutf8");
    let p = root.join(".smlex/profiles/api-docs.smlex");
    std::fs::create_dir_all(p.parent().unwrap()).expect("建目录");
    std::fs::write(&p, [0xFFu8, 0xFE, 0x00, 0x01]).expect("写非法 UTF-8");
    let doc = root.join("doc.sml");
    write(
        &doc,
        "@version v1\n@feature enable type.weak\n@weakdoc api-docs\ntitle: t\n",
    );

    let (out, code) = lint(&doc);
    assert!(out.contains("E-LINT-005"), "应报 E-LINT-005：{out}");
    assert!(out.contains("读取失败"), "文案应点明「读取失败」：{out}");
    assert_eq!(code, 1);
    let _ = std::fs::remove_dir_all(&root);
}

/// profile **找不到** ⇒ 只是 warning、**不阻断**（退出码 0）。
///
/// ⚠️ 现状怪点（故意钉住）：这条 warning 复用了 **`E-LINT-005`（E 级码）**。
/// 严重度是 warning 却挂 error 级码，会让「按码统计错误数」的 CI 误判；
/// 若要修，应给它单独一个 `W-LINT-00x`，或让码表支持「同码不同严重度」。
/// 这条用例只锁行为，不锁码 —— 将来换码时改这一处断言即可。
#[test]
fn missing_profile_is_only_a_warning() {
    let root = tmp("missing");
    // 不建 `.smlex/profiles/`（三处命名空间都命中不了）
    let doc = root.join("doc.sml");
    write(
        &doc,
        "@version v1\n@feature enable type.weak\n@weakdoc api-docs\ntitle: t\n",
    );

    let (out, code) = lint(&doc);
    assert!(out.contains("未找到 profile"), "应提示未找到：{out}");
    assert!(out.contains("warning"), "应是 warning：{out}");
    assert_eq!(code, 0, "找不到 profile 不阻断（退出码 0）：{out}");
    let _ = std::fs::remove_dir_all(&root);
}
