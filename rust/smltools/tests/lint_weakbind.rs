// SPDX-License-Identifier: MulanPSL-2.0
//! 钉子：**弱声明不算「契约定义」**。
//!
//! `W-LINT-002`「契约定义从未被应用」专门抓「定义了契约却没人 `@is`」。若 `@weakbind`
//! 被当成契约定义，每一处弱绑定都会吃一条假告警 —— 功能一落地就是噪音制造机
//! （`docs/weakbind-spec-draft.md` §4 末尾那条「连带必须改」）。

use std::io::Write;
use std::process::{Command, Stdio};

fn lint(src: &str) -> (String, String, bool) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_smltools"))
        .args(["--lint", "--from", "sml"])
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

#[test]
fn weak_declaration_is_not_a_contract_definition() {
    let src = "@version v1\n@feature enable type.weak\n\
               @weakbind tls\nweb {\n  host: h\n}\n";
    let (out, err, ok) = lint(src);
    let all = format!("{out}{err}");
    assert!(ok, "这份文档 lint 应通过：{all}");
    assert!(
        !all.contains("W-LINT-002") && !all.contains("契约定义从未被应用"),
        "弱声明不该触发「契约定义从未被应用」：{all}"
    );
}
