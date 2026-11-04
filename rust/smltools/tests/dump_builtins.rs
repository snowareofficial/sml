// SPDX-License-Identifier: MulanPSL-2.0
//! `--dump-builtins` 的验收：把 `@contract` 定义导成编辑器可读的 JSON
//! （schema 见 `docs/weakbind-spec-draft.md` §6.2）。
//!
//! 走**真实二进制**（同 `error_codes.rs` 的理由）：用户拿到的是 stdout 里那份 JSON，
//! 而不是某个内部函数的返回值。

use std::io::Write;
use std::process::{Command, Stdio};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_smltools")
}

/// 用 stdin 喂文档，返回 `(stdout, stderr, 是否成功)`。
fn dump(src: &str) -> (String, String, bool) {
    let mut child = Command::new(bin())
        .args(["--dump-builtins", "--from", "sml"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn smltools");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(src.as_bytes())
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.success(),
    )
}

const SRC: &str = "@contract tls {\n    host: str required\n    port: int optional\n    sni: str default \"a.example.com\"\n}\n\
                   @contract cache loose {\n    ttl: int min 1 max 86400\n    mode: enum [lru fifo]\n}\n";

#[test]
fn dumps_contracts_as_json() {
    let (out, err, ok) = dump(SRC);
    assert!(ok, "应成功，stderr={err}");
    assert!(out.contains("\"schema\": 1"), "缺 schema 版本：{out}");
    assert!(out.contains("\"name\": \"tls\""), "缺契约 tls：{out}");
    assert!(out.contains("\"name\": \"cache\""), "缺契约 cache：{out}");
    // 严格性三态要按名字导出，不是 Debug 形态
    assert!(out.contains("\"strictness\": \"loose\""), "strictness 应导成名字：{out}");
    assert!(out.contains("\"strictness\": \"strict\""), "默认严格性应是 strict：{out}");
}

#[test]
fn field_specs_are_exported() {
    let (out, _, ok) = dump(SRC);
    assert!(ok);
    // required / optional 的默认档：不写 required 就是必填
    assert!(out.contains("\"name\": \"port\""), "缺 port：{out}");
    assert!(out.contains("\"required\": false"), "`optional` 应导成 required=false：{out}");
    // 区间与默认值
    assert!(out.contains("\"min\": 1") && out.contains("\"max\": 86400"), "区间缺失：{out}");
    assert!(
        out.contains("\"default\": \"a.example.com\""),
        "默认值应以 JSON 字符串导出：{out}"
    );
    // 枚举值数组
    assert!(
        out.contains("\"kind\":\"enum\",\"values\":[\"lru\", \"fifo\"]"),
        "枚举应导成数组：{out}"
    );
}

#[test]
fn no_contracts_yields_empty_array() {
    let (out, _, ok) = dump("a: 1\n");
    assert!(ok);
    assert!(out.contains("\"contracts\": [\n\n  ]"), "无契约时应是空数组：{out}");
}

/// 输出必须是确定的：同输入两次逐字节相同（可作 golden 基线）。
#[test]
fn output_is_deterministic() {
    let (a, _, _) = dump(SRC);
    let (b, _, _) = dump(SRC);
    assert_eq!(a, b, "同一输入两次导出必须逐字节相同");
}

/// 仓内那份 `profile.sml`（`.smlex` 的形状）必须能被导出 ——
/// 它是编辑器补全的形状来源（`docs/smlex-format-draft.md` §9）。
#[test]
fn profile_shape_dumps() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/profile.sml");
    let (out, err, ok) = run_file(path);
    assert!(ok, "profile.sml 应能导出：{err}");
    for name in ["profile", "applies_to", "checks", "hints", "emit"] {
        assert!(out.contains(&format!("\"name\": \"{name}\"")), "缺契约 {name}：{out}");
    }
    // 子块的可选字段必须导成 required=false（否则编辑器会提示"必填"，误导）
    assert!(
        out.contains("\"name\": \"glob\", \"ty\": {\"kind\":\"array\",\"of\":{\"kind\":\"str\"}}, \"required\": false"),
        "可选字段应导成 required=false：{out}"
    );
}

/// 跑一次 CLI（给定输入文件），返回 `(stdout, stderr, 是否成功)`。
fn run_file(path: &str) -> (String, String, bool) {
    let out = Command::new(bin())
        .args(["--dump-builtins", "-i", path])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run smltools");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.success(),
    )
}
