// SPDX-License-Identifier: MulanPSL-2.0
//! L1：`Contracts` 闸门。每个用例都钉住一句「默认不变 / 关了确实不一样」。

use swml::easy::{Contracts, Doc};

const UNKNOWN_CONTRACT: &str = "@version v1\ndoc {\n  @is NoSuch\n  x: 1\n}\n";
const TYPE_MISMATCH: &str =
    "@version v1\n@contract P { n: str role: str default guest }\ndoc {\n  @is P\n  n: 42\n}\n";

#[test]
fn default_enforces_document_contracts() {
    let err = Doc::read(UNKNOWN_CONTRACT).unwrap_err();
    assert_eq!(err.code(), "E-CONTRACT-001");
    assert_eq!(Contracts::default(), Contracts::On, "默认档必须与 read 一致");
}

#[test]
fn off_matches_default_and_on_matches_read() {
    // On 与既有入口同码（不是「近似」）
    let a = Doc::read(UNKNOWN_CONTRACT).unwrap_err();
    let b = Doc::read_with(UNKNOWN_CONTRACT, Contracts::On).unwrap_err();
    assert_eq!(a.code(), b.code());

    // Off 放行，且值树与原文一致
    let doc = Doc::read_with(UNKNOWN_CONTRACT, Contracts::Off).unwrap();
    assert_eq!(doc.int("doc.x"), Some(1));
}

#[test]
fn off_skips_type_check_and_default_fill() {
    assert!(Doc::read(TYPE_MISMATCH).is_err(), "开启时应因类型不符报错");

    let doc = Doc::read_with(TYPE_MISMATCH, Contracts::Off).unwrap();
    assert_eq!(doc.int("doc.n"), Some(42));
    assert!(!doc.has("doc.role"), "关闭后不应回填默认值");
}

/// `read_file_with` 是本层唯一同时做到「展开 include」+「关掉契约」的入口。
#[test]
fn read_file_with_expands_include_and_can_disable_contracts() {
    let dir = std::env::temp_dir().join(format!("smlkit_easy_contracts_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("sub.sml"), "sub: 2\n").unwrap();
    let main = dir.join("main.sml");
    std::fs::write(
        &main,
        "@version v1\ninclude \"sub.sml\"\ndoc {\n  @is NoSuch\n  x: 1\n}\n",
    )
    .unwrap();

    // ① 旧入口关不掉契约
    let err = Doc::read_file(&main).unwrap_err();
    assert_eq!(err.code(), "E-CONTRACT-001");

    // ② 新入口：include 被展开，契约不报错
    let doc = Doc::read_file_with(&main, Contracts::Off).unwrap();
    assert_eq!(doc.int("sub"), Some(2), "include 必须真的展开");
    assert_eq!(doc.int("doc.x"), Some(1));

    let _ = std::fs::remove_dir_all(&dir);
}
