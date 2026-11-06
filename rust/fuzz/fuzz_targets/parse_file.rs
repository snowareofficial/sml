#![no_main]
//! L1 文件入口模糊：`parse_file*` 会**展开 `include` / `import`**，这是纯文本入口
//! 完全走不到的第三条路径（HANDOFF §4.4）。
//!
//! 做法：把同一份 fuzz 输入写进临时目录的两个文件（主文件 + 被包含文件），
//! 于是「include 的目标内容」也是 fuzz 控制的 —— 自包含 / 互相包含 / 深度递归
//! 全部由输入决定，靠库内的 `MAX_INCLUDE_DEPTH` / `MAX_INCLUDE_EXPANSIONS` 兜底。
//!
//! 目录按 pid 固定复用：每个迭代重写两个文件，避免反复 create/remove 把吞吐拖到几百 exec/s。
mod common;

use std::path::PathBuf;
use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;

fn scratch_dir() -> &'static PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!(
            "sml-fuzz-parse-file-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("建临时目录失败");
        dir
    })
}

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if text.len() > common::MAX_INPUT {
        return;
    }

    let dir = scratch_dir();
    let main = dir.join("main.sml");
    let inc = dir.join("inc.sml");
    let spare = dir.join("a.sml");
    if std::fs::write(&main, text).is_err() || std::fs::write(&inc, text).is_err() {
        return;
    }
    // 第三个文件让 `include *.sml` 这类 glob 目标也能命中；内容固定，结果可复现。
    let _ = std::fs::write(&spare, "spare: 1\n");

    let main_s = main.to_string_lossy().into_owned();

    let (_, _, bytes) = common::count_allocs(|| {
        // 默认文件入口（include 展开的唯一入口）
        match sml_parse::parse_file(&main_s) {
            Ok(v) => {
                let rendered = sml_value::to_sml(&v);
                match sml_parse::parse(&rendered) {
                    Ok(back) => assert!(
                        v.eq_ordered(&back),
                        "parse_file→to_sml→parse 值不相等\n原文：{text:?}"
                    ),
                    Err(e) => panic!("parse_file→to_sml 的产物重新 parse 失败：{e}"),
                }
            }
            Err(e) => common::assert_sml_error(&e),
        }

        // 保序文件入口：跨 include 的源序只有这条路径能保住
        if let Err(e) = sml_parse::parse_file_ordered(&main_s) {
            common::assert_sml_error(&e);
        }

        // 版本化入口：多了 `@version` 解析与代号映射
        if let Err(e) = sml_parse::parse_file_versioned(&main_s) {
            common::assert_sml_error(&e);
        }

        // 诊断入口：include 展开 + 非致命诊断同一条链
        if let Err(e) = sml_parse::parse_file_with_warnings(&main_s) {
            common::assert_sml_error(&e);
        }
    });

    common::assert_within_budget("parse_file 家族", text.len(), bytes);
});
