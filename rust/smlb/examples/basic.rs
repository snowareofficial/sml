// SPDX-License-Identifier: MulanPSL-2.0
//! 最小用例：一个 `Value` 写进 `.smlb`，再读回来、按路径取值、做完整性校验。
//! Minimal: dump a `Value` into `.smlb`, load it back, look up a path, check integrity.
//!
//! 跑法（canonical 字节要求源序，见 spec §18 C2）：
//! `cargo run -p smlb --features preserve_order --example basic`
//!
//! 没开 `preserve_order` 时本例只打印一行提示 —— 这是**明确拒绝**，不是偷偷排序。

#[cfg(not(feature = "preserve_order"))]
fn main() {
    eprintln!("本例需要 `preserve_order`：`cargo run -p smlb --features preserve_order --example basic`");
}

#[cfg(feature = "preserve_order")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use smlb::{check, dump, dump_with, get, info, load, DumpOptions, HashAlgo, Map, Value};

    // 1) 造一个值。键的顺序就是源序（开了 preserve_order 才成立）。
    let mut config = Map::ordered();
    config.insert("port".to_string(), Value::Int(8080));
    config.insert("name".to_string(), Value::Str("demo".into()));
    let mut root = Map::ordered();
    root.insert("config".to_string(), Value::Object(config));
    let doc = Value::Object(root);

    // 2) 写 → 读 → 往返一致
    let bytes = dump(&doc)?;
    let back = load(&bytes)?;
    assert!(back.eq_ordered(&doc), "值必须原样回来");

    // 3) 一眼看清格式信息；逐段校验摘要
    let meta = info(&bytes)?;
    println!("{} 段，{} 字节，对齐 {} B", meta.segments.len(), meta.file_len, meta.align);
    let report = check(&bytes)?;
    assert!(report.ok, "{:?}", report.lines);
    println!("{}", report.lines.join("\n"));

    // 4) 按路径取一个值（整树解码的慢版本；随机访问见 random_access.rs）
    assert_eq!(get(&bytes, "config.port")?, Value::Int(8080));

    // 5) 换摘要算法：读者永远以 manifest 的 hash_id 为准，不需要调用方记得住
    let sm3 = dump_with(
        &doc,
        &DumpOptions {
            hash: HashAlgo::Sm3,
            ..Default::default()
        },
    )?;
    assert!(check(&sm3)?.ok);
    assert_eq!(load(&sm3)?, back);

    println!("OK");
    Ok(())
}
