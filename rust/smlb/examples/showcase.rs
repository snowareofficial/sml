// SPDX-License-Identifier: MulanPSL-2.0
//! 生成**确定性**的 `.smlb` 样例，供 010 Editor 逐字节查看。
//! Deterministic showcase files for byte-level inspection in 010 Editor.
//!
//! 跑法：
//! `cargo run -p smlb --features "preserve_order,sign,compress,crypto" --example showcase`
//!
//! 产物（写到本 examples/ 目录，二进制文件不入库）：
//!   - `showcase.smlb`       明文 + 尾部层链签名 —— 看段表 / manifest / Trailer
//!   - `showcase-zstd.smlb`  段级 zstd —— 看 `comp=0x01` 与体积差
//!   - `showcase-aes.smlb`   按块 AEAD —— 看 `enc=0x10`、强制 4 KiB 对齐、密文段 + 明文 manifest
//!
//! 配套 `showcase.bt`（010 Editor Binary Template）逐字段解析这三个文件。
//!
//! 为什么用**固定密钥 / 固定内容**：每次生成的字节完全一致，你在 010 Editor 里
//! 看到的偏移与下面打印的字节地图能对得上（熵只进一次性标识，D41）。

#[cfg(not(all(
    feature = "preserve_order",
    feature = "sign",
    feature = "compress",
    feature = "crypto"
)))]
fn main() {
    eprintln!("需要 `preserve_order,sign,compress,crypto`：");
    eprintln!("  cargo run -p smlb --features \"preserve_order,sign,compress,crypto\" --example showcase");
}

#[cfg(all(
    feature = "preserve_order",
    feature = "sign",
    feature = "compress",
    feature = "crypto"
))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use smlb::{
        append_layer, dump, dump_encrypted, dump_with, info, CanonicalValue, DumpOptions, HashAlgo, KeyPair,
        KeySource, Map, Value, FLAG_ALIGN_BLOCK,
    };

    // 1) 一份「看得懂」的小文档：文本进 STRTAB、节点树进 VALUE
    let mut server = Map::ordered();
    server.insert("host".to_string(), Value::Str("web.example".into()));
    server.insert("port".to_string(), Value::Int(8080));
    let mut root = Map::ordered();
    root.insert("server".to_string(), Value::Object(server));
    let doc = Value::Object(root);

    // 2) 明文基文件（8 B 对齐，段表紧跟 40 B 头 ⇒ 布局最紧凑，便于看）
    let cv = CanonicalValue::try_from(doc.clone())?;
    let base = dump(&cv)?;
    // 3) 尾部追加一层（固定密钥 ⇒ 每次字节相同）
    let kp = KeyPair::from_seed(&[7u8; 32]);
    let signed = append_layer(&base, &kp, None, None, HashAlgo::Sha256)?;

    // 4) 压缩对照：文本多的文档里 STRTAB 是大头，压缩才看得出效果
    let rows: Vec<Value> = (0..200)
        .map(|i| {
            let mut m = Map::ordered();
            m.insert("idx".to_string(), Value::Int(i));
            m.insert(
                "text".to_string(),
                Value::Str(format!("row-{i}-{}", "abcdefgh".repeat(24))),
            );
            Value::Object(m)
        })
        .collect();
    let mut wide_root = Map::ordered();
    wide_root.insert("rows".to_string(), Value::Array(rows));
    let wide = Value::Object(wide_root);
    let cv_wide = CanonicalValue::try_from(wide.clone())?;
    let wide_plain = dump(&cv_wide)?;
    let wide_z = dump_with(
        &cv_wide,
        &DumpOptions {
            compress: Some(9),
            ..Default::default()
        },
    )?;

    // 5) 加密对照：段体是密文，manifest 仍是明文（KDF 参数在里面）
    let aes = dump_encrypted(
        &cv,
        &DumpOptions::default(),
        &KeySource::Bytes(&[9u8; 32]),
    )?;

    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let main_path = dir.join("showcase.smlb");
    let z_path = dir.join("showcase-zstd.smlb");
    let a_path = dir.join("showcase-aes.smlb");
    std::fs::write(&main_path, &signed)?;
    std::fs::write(&z_path, &wide_z)?;
    std::fs::write(&a_path, &aes)?;

    // ---- 字节地图（与 010 Editor 里的偏移一一对应） ----
    println!("=== {} ===", main_path.display());
    println!("size {} B", signed.len());
    let meta = info(&signed)?;
    let table_end = meta.table_off as usize + meta.segments.len() * smlb::SEG_HEADER_LEN;
    println!("[0,40)      文件头 magic=SWSB / edition / fmt / hdr_len=40");
    println!(
        "            seg_table_off={} seg_count={} flags=0x{:04x} (对齐 {} B)",
        meta.table_off,
        meta.segments.len(),
        meta.flags,
        meta.align
    );
    println!(
        "[{}, {})  段表 {} × {} B",
        meta.table_off,
        table_end,
        meta.segments.len(),
        smlb::SEG_HEADER_LEN
    );
    for (i, s) in meta.segments.iter().enumerate() {
        let at = meta.table_off as usize + i * smlb::SEG_HEADER_LEN;
        println!(
            "  @{at:<4} id=0x{:02x} comp={} enc={} len={:<5} sha256={:02x}{:02x}{:02x}{:02x}…",
            s.header.seg_id,
            s.header.comp,
            s.header.enc,
            s.header.len,
            s.header.hash[0],
            s.header.hash[1],
            s.header.hash[2],
            s.header.hash[3]
        );
    }
    for s in &meta.segments {
        println!(
            "[{:>5}, {:>5})  段 0x{:02x} 段体（{} B）",
            s.offset,
            s.offset + s.header.len as u64,
            s.header.seg_id,
            s.header.len
        );
    }
    println!(
        "[{:>5}, {:>5})  层记录（len:u32 ‖ prev_off:u64 ‖ reserved:u32 ‖ Value 字节）",
        base.len(),
        signed.len() - smlb::TRAILER_LEN
    );
    println!(
        "[{:>5}, {:>5})  Trailer 64 B，魔数 SMLTRLR1",
        signed.len() - smlb::TRAILER_LEN,
        signed.len()
    );
    println!("  不变量 3：signed[0..{}] == base（追加不改旧字节）", base.len());

    println!("\n=== {}（压缩对照）===", z_path.display());
    let z = info(&wide_z)?;
    println!(
        "{} B → {} B（{:.1}×；同一份数据明文 {} B）",
        wide_plain.len(),
        wide_z.len(),
        wide_plain.len() as f64 / wide_z.len() as f64,
        wide_plain.len()
    );
    for s in &z.segments {
        println!(
            "  段 0x{:02x} comp={} len={} B",
            s.header.seg_id, s.header.comp, s.header.len
        );
    }

    println!("\n=== {}（加密对照）===", a_path.display());
    let a = info(&aes)?;
    println!(
        "size {} B，flags=0x{:04x} ⇒ 对齐 {} B（位 0 = {}，加密强制）",
        aes.len(),
        a.flags,
        a.align,
        a.flags & FLAG_ALIGN_BLOCK != 0
    );
    for s in &a.segments {
        println!(
            "  段 0x{:02x} enc=0x{:02x} len={} B  ← {}",
            s.header.seg_id,
            s.header.enc,
            s.header.len,
            if s.header.enc == 0 { "明文（KDF 参数在里面）" } else { "密文" }
        );
    }
    println!("  注意：0x04 MANIFEST 的 enc=0 —— 它保持明文，KDF 参数在里面");
    println!("\n把 showcase.bt 拖进 010 Editor（Templates → Open）即可逐字段展开。");
    Ok(())
}
