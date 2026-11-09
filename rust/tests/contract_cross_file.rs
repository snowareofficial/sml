//! L74：契约**跨文件复用**（配合 `include`）—— 把「能用但此前没测试」的行为钉住。
//!
//! 现状（2026-11-02）：`include` 在**解析前**把被包含文件的 token 拼进主流
//! （`sml-include/src/expand.rs` 的 `resolve_includes` → `sml_parse::parse_file`），
//! 因此被 include 文件里的 `@contract` 与主流里的 `@is` 走**同一张** `contracts` 表 ——
//! 只要 include 出现在使用之前，就能跨文件复用；`@is` 的应用是**块闭合时**做的
//! （`parser.rs` 的 `apply_contracts`），所以被 include 的契约必须先于该块被解析。
//!
//! 这不是新语法，也没有专门约定 —— 本文件只锁住「靠 token 合并天然成立」的现状，
//! 并明确其**顺序敏感**的边界。要做成正式特性（契约库、五端对齐）另开一轮。

fn tmpdir(tag: &str) -> std::path::PathBuf {
    let mut d = std::env::temp_dir();
    d.push(format!("sml_xfile_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("create tmpdir");
    d
}

/// 契约库在 include 的文件里定义、主文件 `@is` 使用 —— 应生效，并回填默认值。
#[test]
fn contract_from_included_file_is_visible() {
    let d = tmpdir("visible");
    std::fs::write(
        d.join("lib.sml"),
        "@contract User {\n  name: str\n  role: str default \"guest\"\n}\n",
    )
    .unwrap();
    std::fs::write(
        d.join("main.sml"),
        "@version v1\ninclude \"lib.sml\"\ndoc {\n  @is User\n  name: \"ada\"\n}\n",
    )
    .unwrap();

    let v = sml::parse_file(d.join("main.sml")).expect("跨文件契约应生效");
    assert_eq!(v.get("doc.name").and_then(|x| x.as_str()), Some("ada"));
    // 默认值回填，证明用的是 lib.sml 里那份契约，而不是别的同名定义。
    assert_eq!(
        v.get("doc.role").and_then(|x| x.as_str()),
        Some("guest"),
        "默认值未回填 ⇒ 契约没被跨文件识别"
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// 顺序敏感：同文件里「先用 `@is`、后 include 契约库」应报 `E-CONTRACT-001`。
///
/// 这正是「不能把 include 随手放文件末尾」的原因 —— 把它钉住，免得以后误以为跨文件
/// 契约是顺序无关的。
#[test]
fn contract_after_include_is_too_late() {
    let d = tmpdir("order");
    std::fs::write(d.join("lib.sml"), "@contract User {\n  name: str\n}\n").unwrap();
    std::fs::write(
        d.join("main.sml"),
        "@version v1\ndoc {\n  @is User\n  name: \"ada\"\n}\ninclude \"lib.sml\"\n",
    )
    .unwrap();

    let err = sml::parse_file(d.join("main.sml"))
        .expect_err("契约在使用之后才 include，应报未知契约");
    assert_eq!(err.code(), "E-CONTRACT-001", "{err}");
    let _ = std::fs::remove_dir_all(&d);
}

/// 负数对照：完全不 include 时同名契约不存在 —— 证明上一条不是「include 之外」的巧合。
#[test]
fn without_include_contract_is_unknown() {
    let d = tmpdir("absent");
    std::fs::write(
        d.join("main.sml"),
        "@version v1\ndoc {\n  @is User\n  name: \"ada\"\n}\n",
    )
    .unwrap();

    let err =
        sml::parse_file(d.join("main.sml")).expect_err("未定义 User，应报 E-CONTRACT-001");
    assert_eq!(err.code(), "E-CONTRACT-001", "{err}");
    let _ = std::fs::remove_dir_all(&d);
}
