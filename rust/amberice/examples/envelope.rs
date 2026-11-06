// SPDX-License-Identifier: MulanPSL-2.0
//! 通用二进制信封：把**任意字节**装进 `.ambr`，再原样取出。
//! A general-purpose envelope: wrap arbitrary bytes, then unwrap them.
//!
//! 这才是 `.ambr` 重新定位后的主路径 —— SML 只是内置载荷之一。
//! `cargo run -p amberice --example envelope`
//! 想看压缩对照再加 `--features compress`。本例**不需要**任何 feature。

fn main() -> Result<(), Box<dyn std::error::Error>> {
    use amberice::{pack, payload, payloads};
    #[cfg(feature = "compress")]
    use amberice::{pack_with, DumpOptions};

    let text = b"hello amberice".as_slice();
    let binary = [0x00u8, 0x01, 0xff, 0xfe].as_slice();

    // 1) 两个载荷，名字就是将来取出来用的键
    let f = pack(&[("readme.txt", text), ("blob.bin", binary)])?;
    println!("归档 {} 字节", f.len());

    // 2) 列出 / 取出（偏移不存，按 `payloads[].len` 走出来）
    for p in payloads(&f)? {
        println!("  {} : {} B", p.name, p.len);
    }
    assert_eq!(&*payload(&f, "blob.bin")?, binary);

    // 3) 段级压缩是写者的旋钮；只有真变小的段才压。
    //    写了压缩就得能读回来 ⇒ 这一段必须跟着 `compress` feature 走，
    //    否则零 feature 跑本例会在**读**的时候报 `CompressFeatureDisabled`。
    #[cfg(feature = "compress")]
    {
        let opts = DumpOptions {
            compress: Some(9),
            ..Default::default()
        };
        let big = vec![b'x'; 200_000];
        let packed = pack_with(&[("big.txt", &big)], &opts)?;
        println!("200 KB 重复文本 → {} 字节", packed.len());
        assert_eq!(&*payload(&packed, "big.txt")?, big.as_slice());
    }
    #[cfg(not(feature = "compress"))]
    println!("（跳过压缩对照：需要 `--features compress`）");

    // 4) 载荷名来自文件。`unpack` 一律拒绝含 `..` / 绝对路径 / 盘符的名字（tar-slip）。
    //    库侧 `payload` 只按名字取字节，不写盘 —— 写盘的安全责任在工具层。

    println!("OK");
    Ok(())
}
