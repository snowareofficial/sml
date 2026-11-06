// SPDX-License-Identifier: MulanPSL-2.0
//! 最小用例：一个 `Value` 写进 `.ambr`，再读回来、按路径取值、做完整性校验。
//! Minimal: dump a `Value` into `.ambr`, load it back, look up a path, check integrity.
//!
//! 跑法（**不需要任何 feature**）：
//! `cargo run -p amberice --example basic`
//!
//! 键序是**值层事实**：`Map::ordered()` 在任何 feature 组合下都能用，
//! canonical 编码只认这一态（spec §18 C2）。所以本例不再需要 `preserve_order`。

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use amberice::{
        check, dump, dump_with, get, info, load, CanonicalValue, DumpOptions,
        HashAlgo, Map, Value,
    };

    // 1) 造一个值。`Map::ordered()` 显式声明「键序就是源序」。
    let mut config = Map::ordered();
    config.insert("port".to_string(), Value::Int(8080));
    config.insert("name".to_string(), Value::Str("demo".into()));
    let mut root = Map::ordered();
    root.insert("config".to_string(), Value::Object(config));
    let doc = Value::Object(root);

    // 2) 写 → 读 → 往返一致
    //    `CanonicalValue::try_from` 是**唯一**的键序证明点：`parse()` 的默认结果是无序的，
    //    传进来会在这里失败，而不是等到 dump 才在运行期报错。
    let cv = CanonicalValue::try_from(doc.clone())?;
    let bytes = dump(&cv)?;
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
        &cv,
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
