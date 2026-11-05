// SPDX-License-Identifier: MulanPSL-2.0
//! 随机访问：只读命中的子树，不把整个文档解成 `Value`。
//! Random access: decode only the subtree you ask for, not the whole tree.
//!
//! `cargo run -p smlb --example random_access`
//! （不带 `mmap` 也能跑，只是少了 `MappedFile` 那一段；本例**不需要** `preserve_order`）

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use smlb::{dump, load, Map, Store, Value};

    // 造一个「很多兄弟」的文档：取最后一个字段不必解码前面那 499 棵
    let rows: Vec<Value> = (0..500)
        .map(|i| {
            let mut m = Map::ordered();
            m.insert("idx".to_string(), Value::Int(i));
            m.insert("text".to_string(), Value::Str(format!("row-{i}")));
            Value::Object(m)
        })
        .collect();
    let mut root = Map::ordered();
    root.insert("rows".to_string(), Value::Array(rows));
    let doc = Value::Object(root);
    let bytes = dump(&doc)?;

    // 1) 从字节切片直接开（调用方也可以自己 mmap 后传进来）
    let store = Store::open(&bytes)?;
    let (v, visited) = store.get_with_stats("rows[499].idx")?;
    assert_eq!(v, Value::Int(499));
    println!("取 rows[499].idx = {v:?}，导航看过 {visited} 个节点");

    // 2) 零拷贝拿原始子树字节：不解码、不复制
    let raw = store.subtree_bytes("rows[0]")?;
    println!("rows[0] 原始字节 {} B（整份 VALUE 段更大）", raw.len());

    // 3) 与整树读取口径一致
    let whole: Value = load(&bytes)?;
    assert_eq!(v, whole.get("rows[499].idx").cloned().unwrap());

    // 4) 有 `mmap` feature 时：文件不进堆，只映射（100 MB 文件实测堆增 256 B）
    #[cfg(feature = "mmap")]
    {
        use smlb::MappedFile;
        let path = std::env::temp_dir().join(format!("smlb-example-{}.smlb", std::process::id()));
        std::fs::write(&path, &bytes)?;
        let mapped = MappedFile::open(&path)?;
        let s = mapped.store()?;
        assert_eq!(s.get("rows[0].idx")?, Value::Int(0));
        println!("MappedFile: {} 段", mapped.info()?.segments.len());
        let _ = std::fs::remove_file(&path);
    }

    println!("OK");
    Ok(())
}
